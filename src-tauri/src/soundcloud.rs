use crate::models;
use crate::stream_cache::Stream;
use reqwest::Client;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use url::Url;

#[derive(Debug)]
pub enum Error {
    Http(u16),
    Network,
    Discovery,
    Invalid,
    Large,
    Unavailable,
    Data,
    Stream,
}
impl Error {
    pub fn confirms_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable | Self::Http(404 | 410))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::Http(429) => "SoundCloud просит подождать. Повтори запрос чуть позже.".into(),
            Self::Http(401 | 403) => "SoundCloud отклонил запрос. Попробуй ещё раз позже.".into(),
            Self::Http(404 | 410) => "Эта музыка удалена или недоступна.".into(),
            Self::Http(code) => format!("SoundCloud временно не отвечает ({code})."),
            Self::Network => "Не удалось связаться с SoundCloud. Проверь подключение.".into(),
            Self::Discovery => {
                "Не удалось подключиться к SoundCloud. Повтори попытку через полминуты.".into()
            }
            Self::Invalid => "SoundCloud вернул неподдерживаемую ссылку.".into(),
            Self::Large => "Ответ SoundCloud превышает допустимый размер.".into(),
            Self::Unavailable => "У трека нет доступного полного потока.".into(),
            Self::Data => "Не удалось прочитать ответ SoundCloud.".into(),
            Self::Stream => "SoundCloud не выдал рабочую ссылку на поток. Повтори попытку.".into(),
        })
    }
}
struct Identity {
    cached: Option<String>,
    last_attempt: u64,
}
pub struct SoundCloud {
    client: Client,
    identity: Mutex<Identity>,
    retry_after: AtomicU64,
    directory: PathBuf,
    artwork_writes: AtomicU64,
    artwork_slots: tokio::sync::Semaphore,
    stream_cache: tokio::sync::OnceCell<crate::stream_cache::StreamCache>,
    resolved_streams: Mutex<HashMap<u64, std::sync::Arc<Mutex<Option<(Instant, Vec<Stream>)>>>>>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl SoundCloud {
    pub fn new(directory: PathBuf) -> Self {
        let cache = directory.join("artwork");
        std::thread::spawn(move || crate::persistence::prune_artwork(&cache));
        let cached = std::fs::read_to_string(directory.join("client-id"))
            .ok()
            .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_alphanumeric()));
        let client = Client::builder()
            .user_agent("Mozilla/5.0")
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(12))
            .redirect(reqwest::redirect::Policy::limited(4))
            .build()
            .expect("HTTP client");
        Self {
            stream_cache: tokio::sync::OnceCell::new(),
            resolved_streams: Mutex::new(HashMap::new()),
            artwork_writes: AtomicU64::new(0),
            artwork_slots: tokio::sync::Semaphore::new(6),
            client,
            identity: Mutex::new(Identity {
                cached,
                last_attempt: 0,
            }),
            retry_after: AtomicU64::new(0),
            directory,
        }
    }
    async fn bytes(&self, url: Url, limit: usize) -> Result<Vec<u8>, Error> {
        if url.scheme() != "https" {
            return Err(Error::Invalid);
        }
        if now() < self.retry_after.load(Ordering::Relaxed) {
            return Err(Error::Http(429));
        }
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| Error::Network)?;
        if response.status().as_u16() == 429 {
            let value = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("30");
            let until = value
                .parse::<u64>()
                .ok()
                .map(|v| now() + v.clamp(5, 3600))
                .or_else(|| {
                    httpdate::parse_http_date(value)
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|t| t.as_secs())
                })
                .unwrap_or(now() + 30);
            self.retry_after.store(until, Ordering::Relaxed);
        }
        if !response.status().is_success() {
            return Err(Error::Http(response.status().as_u16()));
        }
        if response.content_length().unwrap_or(0) > limit as u64 {
            return Err(Error::Large);
        }
        let mut result = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
            if result.len() + chunk.len() > limit {
                return Err(Error::Large);
            }
            result.extend_from_slice(&chunk);
        }
        Ok(result)
    }
    async fn validate(&self, id: &str) -> Result<bool, Error> {
        let mut url = Url::parse("https://api-v2.soundcloud.com/search/tracks").unwrap();
        url.query_pairs_mut()
            .extend_pairs([("q", "music"), ("limit", "1"), ("client_id", id)]);
        match self.bytes(url, 1_000_000).await {
            Ok(data) => Ok(serde_json::from_slice::<Value>(&data)
                .ok()
                .and_then(|v| v["collection"].as_array().map(|_| ()))
                .is_some()),
            Err(Error::Http(401 | 403)) => Ok(false),
            Err(e) => Err(e),
        }
    }
    async fn id(&self, rejected: Option<&str>) -> Result<String, Error> {
        // One shared lock coalesces concurrent discovery and authentication recovery.
        let mut identity = self.identity.lock().await;
        if let Some(cached) = identity.cached.clone() {
            if rejected != Some(cached.as_str()) {
                return Ok(cached);
            }
            if self.validate(&cached).await? {
                return Ok(cached);
            }
            identity.cached = None;
            let _ = tokio::fs::remove_file(self.directory.join("client-id")).await;
        }
        if now().saturating_sub(identity.last_attempt) < 30 {
            return Err(Error::Discovery);
        }
        identity.last_attempt = now();
        let html = self
            .bytes(Url::parse("https://soundcloud.com").unwrap(), 2_000_000)
            .await?;
        let script_re = regex::Regex::new(
            r#"<script[^>]+src=["'](https://a-v2\.sndcdn\.com/assets/[^"']+)["']"#,
        )
        .unwrap();
        let id_re = regex::Regex::new(r#"client_id\s*:\s*["']([a-zA-Z0-9]{32})["']"#).unwrap();
        let text = String::from_utf8_lossy(&html);
        let scripts: Vec<_> = script_re
            .captures_iter(&text)
            .map(|c| c[1].to_string())
            .collect();
        let mut tested = HashSet::new();
        for script in scripts.iter().rev().take(16) {
            let data = match self
                .bytes(Url::parse(script).map_err(|_| Error::Invalid)?, 8_000_000)
                .await
            {
                Ok(d) => d,
                Err(Error::Http(404)) => continue,
                Err(e) => return Err(e),
            };
            for candidate in id_re.captures_iter(&String::from_utf8_lossy(&data)) {
                let id = candidate[1].to_string();
                if tested.insert(id.clone()) && self.validate(&id).await? {
                    identity.cached = Some(id.clone());
                    let _ = tokio::fs::write(self.directory.join("client-id"), &id).await;
                    return Ok(id);
                }
            }
        }
        Err(Error::Discovery)
    }
    pub async fn request(&self, path: &str, query: &[(&str, String)]) -> Result<Value, Error> {
        let base = if path.starts_with("https://") {
            path.to_string()
        } else {
            format!(
                "https://api-v2.soundcloud.com/{}",
                path.trim_start_matches('/')
            )
        };
        let original = Url::parse(&base).map_err(|_| Error::Invalid)?;
        validate_api_url(&original)?;
        let mut id = self.id(None).await.map_err(discovery_error)?;
        for attempt in 0..2 {
            let mut url = original.clone();
            let pairs: Vec<(String, String)> = url
                .query_pairs()
                .filter(|(k, _)| k != "client_id" && !query.iter().any(|(key, _)| key == k))
                .map(|(a, b)| (a.into(), b.into()))
                .collect();
            url.set_query(None);
            url.query_pairs_mut()
                .extend_pairs(pairs)
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())))
                .append_pair("client_id", &id);
            match self.bytes(url, 8_000_000).await {
                Ok(data) => return serde_json::from_slice(&data).map_err(|_| Error::Data),
                Err(Error::Http(status @ (401 | 403))) if attempt == 0 => {
                    let recovered = self.id(Some(&id)).await.map_err(discovery_error)?;
                    if recovered == id {
                        return Err(Error::Http(status));
                    }
                    id = recovered;
                }
                Err(e) => return Err(e),
            }
        }
        Err(Error::Discovery)
    }
    pub async fn search(&self, query: &str) -> Result<Value, Error> {
        let params = [
            ("q", query.chars().take(200).collect()),
            ("limit", "30".into()),
            ("linked_partitioning", "1".into()),
        ];
        let (users, lists, tracks) = tokio::join!(
            self.request("search/users", &params),
            self.request("search/playlists", &params),
            self.request("search/tracks", &params)
        );
        let mut result = json!({"artists":[],"playlists":[],"tracks":[],"next":{"artists":null,"playlists":null,"tracks":null},"errors":[]});
        let mut errors = vec![];
        for (kind, page) in [("artists", users), ("playlists", lists), ("tracks", tracks)] {
            match page {
                Ok(page) => {
                    result[kind] = map_collection(kind, &page);
                    result["next"][kind] = clean_next(&page);
                }
                Err(e) => errors.push(e.to_string()),
            }
        }
        errors.sort();
        errors.dedup();
        result["errors"] = json!(errors);
        Ok(result)
    }
    pub async fn search_more(&self, kind: &str, url: &str) -> Result<Value, Error> {
        if !["artists", "playlists", "tracks"].contains(&kind) {
            return Err(Error::Invalid);
        }
        let page = self.request(url, &[]).await?;
        let mut result = json!({"artists":[],"playlists":[],"tracks":[],"next":{"artists":null,"playlists":null,"tracks":null},"errors":[]});
        result[kind] = map_collection(kind, &page);
        result["next"][kind] = clean_next(&page);
        Ok(result)
    }
    pub async fn artist_profile(&self, id: u64) -> Result<models::Artist, Error> {
        models::artist(&self.request(&format!("users/{id}"), &[]).await?).ok_or(Error::Data)
    }
    pub async fn artist_tracks(&self, id: u64, next: Option<String>) -> Result<Value, Error> {
        if let Some(ref next) = next {
            let u = Url::parse(next).map_err(|_| Error::Invalid)?;
            if u.path() != format!("/users/{id}/tracks") {
                return Err(Error::Invalid);
            }
        }
        let page = self
            .request(
                &next.unwrap_or_else(|| format!("users/{id}/tracks")),
                &[("limit", "200".into()), ("linked_partitioning", "1".into())],
            )
            .await?;
        Ok(
            json!({"tracks":map_collection("tracks",&page),"next":clean_next(&page),"total":page["total_results"].as_u64().unwrap_or(0)}),
        )
    }
    pub async fn playlist_tracks(&self, id: u64) -> Result<Value, Error> {
        let list = self
            .request(
                &format!("playlists/{id}"),
                &[("representation", "full".into())],
            )
            .await?;
        let raw = list["tracks"].as_array().ok_or(Error::Data)?;
        let total = list["track_count"].as_u64().unwrap_or(raw.len() as u64) as usize;
        // A playlist response contains sparse {id} entries after the first complete tracks.
        // Hydrate IDs in bounded batches, then restore the original order (including duplicates).
        let ids: Vec<u64> = raw.iter().filter_map(|v| v["id"].as_u64()).collect();
        let mut known: HashMap<u64, models::Track> = raw
            .iter()
            .filter_map(models::track)
            .map(|t| (t.id, t))
            .collect();
        let missing: Vec<u64> = ids
            .iter()
            .copied()
            .filter(|id| !known.contains_key(id))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        for batch in missing.chunks(50) {
            let joined = batch
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let page = self.request("tracks", &[("ids", joined)]).await?;
            let values = page
                .as_array()
                .or_else(|| page["collection"].as_array())
                .ok_or(Error::Data)?;
            for track in values.iter().filter_map(models::track) {
                known.insert(track.id, track);
            }
        }
        if ids.len() < total {
            return Err(Error::Data);
        }
        let tracks: Vec<_> = ids
            .iter()
            .map(|id| {
                known.get(id).cloned().unwrap_or(models::Track {
                    id: *id,
                    title: "Недоступный трек".into(),
                    artist: String::new(),
                    artist_id: 0,
                    artwork: None,
                    duration: 0.0,
                    permalink: String::new(),
                    available: false,
                })
            })
            .collect();
        Ok(json!({"tracks":tracks,"next":null,"total":total}))
    }
    pub async fn streams(&self, id: u64) -> Result<Vec<Stream>, Error> {
        let slot = {
            let mut slots = self.resolved_streams.lock().await;
            if slots.len() >= 16 && !slots.contains_key(&id) {
                slots.retain(|_, v| std::sync::Arc::strong_count(v) > 1);
            }
            slots
                .entry(id)
                .or_insert_with(|| std::sync::Arc::new(Mutex::new(None)))
                .clone()
        };
        let mut cached = slot.lock().await;
        if let Some((time, urls)) = &*cached {
            if time.elapsed() < Duration::from_secs(120) {
                return Ok(urls.clone());
            }
        }
        let urls = self.resolve_streams(id, false).await?;
        *cached = Some((Instant::now(), urls.clone()));
        Ok(urls)
    }
    pub async fn refresh_streams(&self, id: u64) -> Result<Vec<Stream>, Error> {
        self.resolved_streams.lock().await.remove(&id);
        self.resolve_streams(id, true).await
    }
    async fn resolve_streams(&self, id: u64, alternatives: bool) -> Result<Vec<Stream>, Error> {
        let track = self.request(&format!("tracks/{id}"), &[]).await?;
        if !models::track(&track).ok_or(Error::Data)?.available {
            return Err(Error::Unavailable);
        }
        let raw_variants = track["media"]["transcodings"]
            .as_array()
            .ok_or(Error::Data)?;
        if raw_variants.is_empty() {
            return Err(Error::Stream);
        }
        let mut variants: Vec<_> = raw_variants
            .iter()
            .filter(|v| {
                v["snipped"].as_bool() != Some(true)
                    && matches!(
                        v["format"]["protocol"].as_str(),
                        Some("hls" | "progressive")
                    )
            })
            .collect();
        if variants.is_empty() {
            return Err(Error::Unavailable);
        }
        variants.sort_by_key(|v| {
            std::cmp::Reverse(
                (if v["format"]["protocol"] == "progressive" {
                    100
                } else {
                    0
                }) + (if v["format"]["mime_type"]
                    .as_str()
                    .unwrap_or("")
                    .contains("mp4")
                {
                    20
                } else {
                    0
                }) + (if v["format"]["protocol"] == "hls" {
                    10
                } else {
                    0
                }),
            )
        });
        let mut streams = Vec::new();
        for v in variants.into_iter().take(5) {
            let Some(url) = v["url"].as_str() else {
                continue;
            };
            match self
                .request(
                    url,
                    &[(
                        "track_authorization",
                        models::text(&track, "track_authorization"),
                    )],
                )
                .await
            {
                Ok(value) => {
                    if let Some(url) = value["url"].as_str() {
                        let parsed = Url::parse(url).map_err(|_| Error::Invalid)?;
                        if parsed.scheme() == "https"
                            && is_media_host(parsed.host_str().unwrap_or(""))
                        {
                            let mut source = Stream {
                                url: url.to_string(),
                                segments: Vec::new(),
                            };
                            if let Ok(cache) = self
                                .stream_cache
                                .get_or_try_init(|| {
                                    crate::stream_cache::StreamCache::new(self.client.clone())
                                })
                                .await
                            {
                                if v["format"]["protocol"] == "progressive" {
                                    if let Ok(buffered) = cache.prepare(url).await {
                                        source.url = buffered;
                                    }
                                } else if let Ok(buffered) = cache.prepare_hls(url).await {
                                    source = buffered;
                                }
                            }
                            streams.push(source);
                            if !alternatives {
                                break;
                            }
                        }
                    }
                }
                Err(Error::Http(403 | 404 | 410)) => continue,
                Err(e) => return Err(e),
            }
        }
        if streams.is_empty() {
            Err(Error::Stream)
        } else {
            Ok(streams)
        }
    }
    pub async fn resolve(&self, url: &str) -> Result<Value, Error> {
        let parsed = Url::parse(url).map_err(|_| Error::Invalid)?;
        if parsed.scheme() != "https"
            || !matches!(
                parsed.host_str(),
                Some("soundcloud.com" | "www.soundcloud.com" | "on.soundcloud.com")
            )
        {
            return Err(Error::Invalid);
        }
        let value = self.request("resolve", &[("url", url.into())]).await?;
        let kind = value["kind"].as_str().ok_or(Error::Data)?;
        let entity = match kind {
            "user" => json!(models::artist(&value).ok_or(Error::Data)?),
            "playlist" => json!(models::playlist(&value).ok_or(Error::Data)?),
            "track" => json!(models::track(&value).ok_or(Error::Data)?),
            _ => return Err(Error::Data),
        };
        Ok(json!({"kind":kind,"entity":entity}))
    }
    pub async fn artwork(&self, url: &str) -> Result<String, Error> {
        let _permit = self
            .artwork_slots
            .acquire()
            .await
            .map_err(|_| Error::Network)?;
        use base64::Engine;
        use sha2::{Digest, Sha256};
        let parsed = Url::parse(url).map_err(|_| Error::Invalid)?;
        if parsed.scheme() != "https"
            || !parsed
                .host_str()
                .map(|h| h.ends_with(".sndcdn.com"))
                .unwrap_or(false)
        {
            return Err(Error::Invalid);
        }
        let directory = self.directory.join("artwork");
        let _ = tokio::fs::create_dir_all(&directory).await;
        let path = directory.join(format!("{:x}.jpg", Sha256::digest(url.as_bytes())));
        let bytes = if let Ok(data) = tokio::fs::read(&path).await {
            data
        } else {
            let raw = self.bytes(parsed, 3_000_000).await?;
            let data = tokio::task::spawn_blocking(move || {
                let mut reader = image::ImageReader::new(std::io::Cursor::new(&raw))
                    .with_guessed_format()
                    .map_err(|_| Error::Data)?;
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(4096);
                limits.max_image_height = Some(4096);
                limits.max_alloc = Some(64 * 1024 * 1024);
                reader.limits(limits);
                let image = reader
                    .decode()
                    .map_err(|_| Error::Data)?
                    .thumbnail(500, 500)
                    .into_rgb8();
                let mut bytes = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 86)
                    .encode_image(&image)
                    .map_err(|_| Error::Data)?;
                Ok::<_, Error>(bytes)
            })
            .await
            .map_err(|_| Error::Data)??;
            let _ = tokio::fs::write(&path, &data).await;
            if self.artwork_writes.fetch_add(1, Ordering::Relaxed) % 32 == 31 {
                let cache = directory.clone();
                tokio::task::spawn_blocking(move || crate::persistence::prune_artwork(&cache));
            }
            data
        };
        Ok(format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }
}
fn discovery_error(error: Error) -> Error {
    match error {
        Error::Http(404 | 410) => Error::Discovery,
        other => other,
    }
}
fn validate_api_url(url: &Url) -> Result<(), Error> {
    if url.scheme() == "https"
        && url.host_str() == Some("api-v2.soundcloud.com")
        && url.username().is_empty()
        && url.port().unwrap_or(443) == 443
    {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}
fn is_media_host(host: &str) -> bool {
    host.ends_with(".sndcdn.com")
        || host.ends_with(".soundcloud.cloud")
        || host.ends_with(".soundcloud.com")
}
fn clean_next(page: &Value) -> Value {
    page["next_href"]
        .as_str()
        .and_then(|s| Url::parse(s).ok())
        .filter(|u| validate_api_url(u).is_ok())
        .map(|mut u| {
            let pairs: Vec<_> = u
                .query_pairs()
                .filter(|(k, _)| k != "client_id")
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            u.set_query(None);
            u.query_pairs_mut().extend_pairs(pairs);
            json!(u.as_str())
        })
        .unwrap_or(Value::Null)
}
fn map_collection(kind: &str, page: &Value) -> Value {
    json!(page["collection"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| match kind {
            "artists" => models::artist(v).map(|v| json!(v)),
            "playlists" => models::playlist(v).map(|v| json!(v)),
            _ => models::track(v).map(|v| json!(v)),
        })
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "Live SoundCloud/FFmpeg check; requires YUKICLOUD_TEST_PROFILE"]
    async fn real_seek_buffers_match_decoded_position() {
        let directory =
            PathBuf::from(std::env::var("YUKICLOUD_TEST_PROFILE").expect("test profile"));
        let library: models::Library =
            serde_json::from_slice(&std::fs::read(directory.join("library.json")).unwrap())
                .unwrap();
        let api = SoundCloud::new(directory);
        let streams = api.refresh_streams(library.tracks[0].id).await.unwrap();
        for source in streams
            .into_iter()
            .filter(|s| s.url.starts_with("http://127.0.0.1"))
            .take(2)
        {
            let label = if source.segments.is_empty() {
                "MP3"
            } else {
                "HLS"
            };
            let copy = source.clone();
            let (reference, seek) = tokio::task::spawn_blocking(move || {
                let decode = |position: f64, duration: &str| {
                    let (url, offset, segmented) = copy.at(position);
                    let mut cmd = std::process::Command::new(
                        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/ffmpeg.exe"),
                    );
                    cmd.args([
                        "-nostdin",
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-probesize",
                        "32768",
                        "-analyzeduration",
                        "100000",
                    ]);
                    if offset > 0. && !segmented {
                        cmd.args(["-ss", &offset.to_string()]);
                    }
                    if segmented { cmd.args(["-extension_picky", "0"]); }
                    cmd.args(["-i", &url]);
                    if offset > 0. && segmented {
                        cmd.args(["-ss", &offset.to_string()]);
                    }
                    let result = cmd
                        .args([
                            "-t", duration, "-f", "f32le", "-ar", "48000", "-ac", "2", "pipe:1",
                        ])
                        .output()
                        .unwrap();
                    assert!(
                        result.status.success(),
                        "{}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    result
                        .stdout
                        .chunks_exact(4)
                        .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
                        .collect::<Vec<_>>()
                };
                (decode(0., "32"), decode(30., "1"))
            })
            .await
            .unwrap();
            assert!(seek.len() > 90000, "{label} produced no audio after seek");
            let mut best = (f64::MAX, 0i64);
            for lag in (-4800i64..=4800).step_by(2) {
                let mut error = 0.;
                let mut n = 0;
                for i in (2048..seek.len().min(80000)).step_by(64) {
                    let at = (30 * 96000) as i64 + i as i64 + lag;
                    if at >= 0 && (at as usize) < reference.len() {
                        let delta = seek[i] - reference[at as usize];
                        error += (delta * delta) as f64;
                        n += 1;
                    }
                }
                if n > 0 && error / (n as f64) < best.0 {
                    best = (error / n as f64, lag);
                }
            }
            println!(
                "{label}: seek bytes {}, alignment {} samples, mean squared error {:.8}",
                seek.len() * 4,
                best.1,
                best.0
            );
            assert!(
                best.1.abs() < 960,
                "{label} seeks to the wrong audio position"
            );
            assert!(best.0 < 0.002, "{label} audio differs from reference");
        }
    }
    #[test]
    fn only_confirmed_resource_failures_allow_skipping() {
        for error in [
            Error::Network,
            Error::Discovery,
            Error::Stream,
            Error::Data,
            Error::Http(429),
            Error::Http(500),
            Error::Http(401),
            Error::Http(403),
        ] {
            assert!(!error.confirms_unavailable());
        }
        for error in [Error::Unavailable, Error::Http(404), Error::Http(410)] {
            assert!(error.confirms_unavailable());
        }
        assert!(!discovery_error(Error::Http(404)).confirms_unavailable());
    }
    #[test]
    fn rejects_foreign_pagination_hosts() {
        for u in [
            "https://evil.test/a",
            "http://api-v2.soundcloud.com/a",
            "https://api-v2.soundcloud.com.evil.test/a",
        ] {
            assert!(validate_api_url(&Url::parse(u).unwrap()).is_err());
        }
    }
    #[test]
    fn next_page_never_exposes_client_id() {
        let value = clean_next(
            &json!({"next_href":"https://api-v2.soundcloud.com/users/1/tracks?offset=200&client_id=secret"}),
        );
        assert!(!value.as_str().unwrap().contains("secret"));
        assert!(value.as_str().unwrap().contains("offset=200"));
    }
}
