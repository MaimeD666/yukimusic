//! Bounded compressed audio buffer. The decoder reads ranges over loopback;
//! reqwest keeps CDN connections alive across seeks and tracks.
use reqwest::Client;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex as AsyncMutex,
};

const BLOCK: u64 = 128 * 1024;
const BLOCKS: usize = 128; // 16 MiB, shared by all tracks.
type Bytes = Arc<Vec<u8>>;
#[derive(Default)]
struct Memory {
    blocks: HashMap<(u64, u64), Bytes>,
    order: VecDeque<(u64, u64)>,
}
impl Memory {
    fn get(&mut self, key: (u64, u64)) -> Option<Bytes> {
        let bytes = self.blocks.get(&key)?.clone();
        self.order.retain(|k| *k != key);
        self.order.push_back(key);
        Some(bytes)
    }
    fn insert(&mut self, key: (u64, u64), bytes: Bytes) {
        self.order.retain(|k| *k != key);
        self.blocks.insert(key, bytes);
        self.order.push_back(key);
        while self.blocks.len() > BLOCKS {
            if let Some(old) = self.order.pop_front() {
                self.blocks.remove(&old);
            }
        }
    }
}
struct Asset {
    id: u64,
    url: String,
    total: Mutex<u64>,
    fetch: AsyncMutex<()>,
}
struct Inner {
    client: Client,
    memory: Mutex<Memory>,
    assets: Mutex<VecDeque<Arc<Asset>>>,
    token: String,
    port: u16,
    playlists: Mutex<VecDeque<(u64, Playlist)>>,
}
struct Playlist {
    header: String,
    segments: Vec<(f64, String)>,
}
#[derive(Clone)]
pub struct Stream {
    pub url: String,
    pub segments: Vec<f64>,
}
impl Stream {
    pub fn at(&self, position: f64) -> (String, f64, bool) {
        if self.segments.is_empty() {
            return (self.url.clone(), position, false);
        }
        let index = self
            .segments
            .partition_point(|start| *start <= position)
            .saturating_sub(1);
        (
            format!("{}?segment={index}", self.url),
            (position - self.segments[index]).max(0.),
            true,
        )
    }
}
#[derive(Clone)]
pub struct StreamCache(Arc<Inner>);
impl StreamCache {
    pub async fn new(client: Client) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        let inner = Arc::new(Inner {
            client,
            memory: Mutex::new(Memory::default()),
            assets: Mutex::new(VecDeque::new()),
            token: uuid::Uuid::new_v4().simple().to_string(),
            port: listener.local_addr().map_err(|e| e.to_string())?.port(),
            playlists: Mutex::new(VecDeque::new()),
        });
        let server = inner.clone();
        tauri::async_runtime::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let server = server.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = serve(socket, server).await;
                });
            }
        });
        Ok(Self(inner))
    }
    pub async fn prepare(&self, url: &str) -> Result<String, String> {
        let asset = self.register(url);
        block(&self.0, &asset, 0).await?;
        Ok(self.address(asset.id))
    }
    fn address(&self, id: u64) -> String {
        format!("http://127.0.0.1:{}/{}/{}", self.0.port, self.0.token, id)
    }
    fn register(&self, url: &str) -> Arc<Asset> {
        let mut assets = self.0.assets.lock().unwrap();
        if let Some(asset) = assets.iter().find(|a| a.url == url) {
            asset.clone()
        } else {
            let id = assets.back().map(|a| a.id + 1).unwrap_or(1);
            let asset = Arc::new(Asset {
                id,
                url: url.into(),
                total: Mutex::new(0),
                fetch: AsyncMutex::new(()),
            });
            assets.push_back(asset.clone());
            while assets.len() > 2048 {
                assets.pop_front();
            }
            asset
        }
    }
    pub async fn prepare_hls(&self, url: &str) -> Result<Stream, String> {
        let mut response = self
            .0
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if data.len() + chunk.len() > 256 * 1024 {
                return Err("large playlist".into());
            }
            data.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(data).map_err(|_| "invalid playlist")?;
        let base = url::Url::parse(url).map_err(|_| "invalid URL")?;
        let local = |value: &str| -> Result<String, String> {
            let target = base.join(value).map_err(|_| "invalid segment")?;
            let host = target.host_str().unwrap_or("");
            if target.scheme() != "https"
                || !(host.ends_with(".sndcdn.com")
                    || host.ends_with(".soundcloud.cloud")
                    || host.ends_with(".soundcloud.com"))
            {
                return Err("invalid segment host".into());
            }
            Ok(self.address(self.register(target.as_str()).id))
        };
        let mut header = String::from("#EXTM3U\n#EXT-X-PLAYLIST-TYPE:VOD\n");
        let mut segments = Vec::new();
        let mut pending = String::new();
        let mut starts = Vec::new();
        let mut duration = 0.;
        let mut next_duration = 0.;
        let uri = regex::Regex::new(r#"URI="([^"]+)""#).unwrap();
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if line.starts_with("#EXT-X-KEY") || line.starts_with("#EXT-X-STREAM-INF") {
                return Err("unsupported playlist".into());
            }
            if let Some(value) = line.strip_prefix("#EXTINF:") {
                next_duration = value
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .parse::<f64>()
                    .map_err(|_| "invalid duration")?;
                if !next_duration.is_finite() || next_duration <= 0. {
                    return Err("invalid duration".into());
                }
                pending.push_str(line);
                pending.push('\n');
            } else if line.starts_with("#EXT-X-MAP:") {
                let capture = uri.captures(line).ok_or("invalid map")?;
                let target = local(&capture[1])?;
                header.push_str(&line.replace(&capture[1], &target));
                header.push('\n');
            } else if line.starts_with("#EXT-X-VERSION:")
                || line.starts_with("#EXT-X-TARGETDURATION:")
            {
                header.push_str(line);
                header.push('\n');
            } else if line.starts_with("#EXT-X-BYTERANGE:") || line == "#EXT-X-DISCONTINUITY" {
                pending.push_str(line);
                pending.push('\n');
            } else if !line.starts_with('#') {
                if next_duration <= 0. || segments.len() >= 1000 {
                    return Err("invalid segment".into());
                }
                starts.push(duration);
                duration += next_duration;
                next_duration = 0.;
                pending.push_str(&local(line)?);
                pending.push('\n');
                segments.push((duration, std::mem::take(&mut pending)));
            }
        }
        if segments.is_empty() {
            return Err("empty playlist".into());
        }
        let mut playlists = self.0.playlists.lock().unwrap();
        let id = playlists.back().map(|p| p.0 + 1).unwrap_or(1);
        playlists.push_back((id, Playlist { header, segments }));
        while playlists.len() > 16 {
            playlists.pop_front();
        }
        Ok(Stream {
            url: format!("http://127.0.0.1:{}/{}/hls/{id}", self.0.port, self.0.token),
            segments: starts,
        })
    }
}
async fn block(inner: &Inner, asset: &Asset, index: u64) -> Result<Bytes, String> {
    let key = (asset.id, index);
    if let Some(bytes) = inner.memory.lock().unwrap().get(key) {
        return Ok(bytes);
    }
    let _fetch = asset.fetch.lock().await;
    if let Some(bytes) = inner.memory.lock().unwrap().get(key) {
        return Ok(bytes);
    }
    let start = index.checked_mul(BLOCK).ok_or("invalid range")?;
    let mut response = inner
        .client
        .get(&asset.url)
        .header("Range", format!("bytes={}-{}", start, start + BLOCK - 1))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err("CDN does not support ranges".into());
    }
    let range = response
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .ok_or("missing range")?;
    let (actual_start, total) = content_range(range).ok_or("invalid range")?;
    if actual_start != start {
        return Err("mismatched range".into());
    }
    *asset.total.lock().unwrap() = total;
    let mut bytes = Vec::with_capacity(BLOCK as usize);
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > BLOCK as usize {
            return Err("oversized block".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() as u64 != BLOCK.min(total - start) {
        return Err("incomplete block".into());
    }
    let bytes = Arc::new(bytes);
    inner.memory.lock().unwrap().insert(key, bytes.clone());
    Ok(bytes)
}
fn content_range(value: &str) -> Option<(u64, u64)> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let (start, end, total) = (
        start.parse::<u64>().ok()?,
        end.parse::<u64>().ok()?,
        total.parse::<u64>().ok()?,
    );
    (start <= end && end < total).then_some((start, total))
}
fn requested_range(value: Option<&str>, total: u64) -> Option<(u64, u64)> {
    if total == 0 {
        return None;
    }
    let Some(value) = value else {
        return Some((0, total - 1));
    };
    let (start, end) = value.trim().strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let length = end.parse::<u64>().ok()?;
        return (length > 0).then_some((total.saturating_sub(length), total - 1));
    }
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().ok()?.min(total - 1)
    };
    (start <= end && start < total).then_some((start, end))
}
async fn serve(mut socket: TcpStream, inner: Arc<Inner>) -> Result<(), String> {
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        let byte = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read_u8())
            .await
            .map_err(|_| "timeout")?
            .map_err(|e| e.to_string())?;
        request.push(byte);
        if request.len() > 8192 {
            return Err("large request".into());
        }
    }
    let request = String::from_utf8(request).map_err(|_| "invalid request")?;
    let mut lines = request.lines();
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("");
    let path = first.next().unwrap_or("");
    if method != "GET" && method != "HEAD" {
        return Err("method".into());
    }
    if let Some(value) = path.strip_prefix(&format!("/{}/hls/", inner.token)) {
        let (id, query) = value.split_once('?').unwrap_or((value, ""));
        let id = id.parse::<u64>().map_err(|_| "id")?;
        let index = query
            .strip_prefix("segment=")
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| "segment")?;
        let body = {
            let playlists = inner.playlists.lock().unwrap();
            let (_, playlist) = playlists
                .iter()
                .find(|p| p.0 == id)
                .ok_or("missing playlist")?;
            if index >= playlist.segments.len() {
                return Err("segment".into());
            }
            let mut body = format!("{}#EXT-X-MEDIA-SEQUENCE:{index}\n", playlist.header);
            for (_, segment) in &playlist.segments[index..] {
                body.push_str(segment);
            }
            body.push_str("#EXT-X-ENDLIST\n");
            body
        };
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.map_err(|e|e.to_string())?;
        if method == "GET" {
            socket
                .write_all(body.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let id = path
        .strip_prefix(&format!("/{}/", inner.token))
        .and_then(|id| id.parse::<u64>().ok());
    let asset = inner
        .assets
        .lock()
        .unwrap()
        .iter()
        .find(|a| Some(a.id) == id)
        .cloned();
    let Some(asset) = asset else {
        socket
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    };
    if *asset.total.lock().unwrap() == 0 {
        block(&inner, &asset, 0).await?;
    }
    let total = *asset.total.lock().unwrap();
    let range = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("range"))
        .map(|(_, value)| value.trim());
    let Some((start, end)) = requested_range(range, total) else {
        socket.write_all(format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await.map_err(|e|e.to_string())?;
        return Ok(());
    };
    let status = if range.is_some() {
        "206 Partial Content"
    } else {
        "200 OK"
    };
    let range_header = if range.is_some() {
        format!("Content-Range: bytes {start}-{end}/{total}\r\n")
    } else {
        String::new()
    };
    socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: audio/mpeg\r\nAccept-Ranges: bytes\r\nContent-Length: {}\r\n{range_header}Connection: close\r\n\r\n",end-start+1).as_bytes()).await.map_err(|e|e.to_string())?;
    if method == "HEAD" {
        return Ok(());
    }
    let mut position = start;
    while position <= end {
        let bytes = block(&inner, &asset, position / BLOCK).await?;
        let offset = (position % BLOCK) as usize;
        let length = (bytes.len() - offset).min((end - position + 1) as usize);
        socket
            .write_all(&bytes[offset..offset + length])
            .await
            .map_err(|e| e.to_string())?;
        position += length as u64;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_are_bounded() {
        assert_eq!(requested_range(Some("bytes=40-"), 100), Some((40, 99)));
        assert_eq!(requested_range(Some("bytes=-10"), 100), Some((90, 99)));
        assert_eq!(requested_range(Some("bytes=40-200"), 100), Some((40, 99)));
        for value in ["bytes=100-", "bytes=8-2", "bytes=0-1,3-4"] {
            assert_eq!(requested_range(Some(value), 100), None);
        }
        assert_eq!(content_range("bytes 0-131071/900000"), Some((0, 900000)));
        assert_eq!(content_range("bytes 0-200/100"), None);
    }
    #[test]
    fn cache_has_a_hard_memory_bound_and_preserves_recent_blocks() {
        let mut memory = Memory::default();
        for i in 0..BLOCKS as u64 {
            memory.insert((1, i), Arc::new(vec![0; BLOCK as usize]));
        }
        assert!(memory.get((1, 0)).is_some());
        memory.insert((2, 0), Arc::new(vec![0; BLOCK as usize]));
        assert_eq!(memory.blocks.len(), BLOCKS);
        assert!(memory.get((1, 1)).is_none());
        assert!(memory.get((1, 0)).is_some());
    }
}
