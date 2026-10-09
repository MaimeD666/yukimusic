use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: u64,
    pub name: String,
    pub artwork: Option<String>,
    pub track_count: usize,
    pub permalink: String,
    #[serde(default)]
    pub artwork_verified: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub artist_id: u64,
    pub artwork: Option<String>,
    pub duration: f64,
    pub permalink: String,
    pub available: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub artwork: Option<String>,
    pub track_count: usize,
    pub permalink: String,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Library {
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
    pub tracks: Vec<Track>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueEntry {
    pub key: String,
    pub track: Track,
}
impl QueueEntry {
    pub fn new(track: Track) -> Self {
        Self {
            key: uuid::Uuid::new_v4().to_string(),
            track,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Playback {
    pub queue: Vec<QueueEntry>,
    pub current_key: Option<String>,
    pub playing: bool,
    pub loading: bool,
    pub position: f64,
    pub volume: f32,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: String,
    pub eq: Vec<f32>,
    pub eq_enabled: bool,
    pub error: Option<String>,
    pub original_order: Vec<String>,
}
impl Default for Playback {
    fn default() -> Self {
        Self {
            queue: vec![],
            current_key: None,
            playing: false,
            loading: false,
            position: 0.0,
            volume: 0.72,
            muted: false,
            shuffle: false,
            repeat: "off".into(),
            eq: vec![0.0; 10],
            eq_enabled: false,
            error: None,
            original_order: vec![],
        }
    }
}
impl Playback {
    pub fn index(&self) -> Option<usize> {
        self.current_key
            .as_ref()
            .and_then(|k| self.queue.iter().position(|e| &e.key == k))
    }
    pub fn track(&self) -> Option<&Track> {
        self.index().map(|i| &self.queue[i].track)
    }
    pub fn renumber(&mut self) {
        self.original_order = self.queue.iter().map(|e| e.key.clone()).collect();
    }
    pub fn enqueue(&mut self, tracks: Vec<Track>, next: bool) {
        let at = if next {
            self.index().map(|i| i + 1).unwrap_or(self.queue.len())
        } else {
            self.queue.len()
        };
        let added: Vec<_> = tracks
            .into_iter()
            .filter(|t| t.available)
            .map(QueueEntry::new)
            .collect();
        let original_at = if next {
            self.current_key
                .as_ref()
                .and_then(|k| self.original_order.iter().position(|v| v == k))
                .map(|i| i + 1)
                .unwrap_or(self.original_order.len())
        } else {
            self.original_order.len()
        };
        self.original_order.splice(
            original_at..original_at,
            added.iter().map(|e| e.key.clone()),
        );
        self.queue.splice(at..at, added);
        if self.current_key.is_none() {
            self.current_key = self.queue.first().map(|e| e.key.clone());
        }
    }
    pub fn move_entry(&mut self, from: &str, to: &str) {
        if let (Some(a), Some(b)) = (
            self.queue.iter().position(|e| e.key == from),
            self.queue.iter().position(|e| e.key == to),
        ) {
            let item = self.queue.remove(a);
            self.queue.insert(b, item);
            self.renumber();
        }
    }
    pub fn remove(&mut self, key: &str) -> bool {
        let Some(index) = self.queue.iter().position(|e| e.key == key) else {
            return false;
        };
        let current = self.current_key.as_deref() == Some(key);
        self.queue.remove(index);
        if current {
            self.current_key = self
                .queue
                .get(index.min(self.queue.len().saturating_sub(1)))
                .map(|e| e.key.clone());
            self.position = 0.0;
        }
        self.original_order.retain(|k| k != key);
        current
    }
    pub fn advance(&mut self, automatic: bool) -> bool {
        let Some(index) = self.index() else {
            return false;
        };
        if automatic && self.repeat == "one" && self.queue[index].track.available {
            self.position = 0.0;
            return true;
        }
        let next = (1..=self.queue.len()).find_map(|step| {
            let next = index + step;
            if next >= self.queue.len() && self.repeat != "all" {
                return None;
            }
            let next = next % self.queue.len();
            self.queue[next].track.available.then_some(next)
        });
        let Some(next) = next else {
            return false;
        };
        self.current_key = Some(self.queue[next].key.clone());
        self.position = 0.0;
        true
    }
    pub fn mark_current_unavailable(&mut self) -> Option<String> {
        let track = self.track()?;
        let id = track.id;
        let title = track.title.clone();
        for entry in &mut self.queue {
            if entry.track.id == id {
                entry.track.available = false;
            }
        }
        Some(title)
    }
    pub fn set_shuffle(&mut self) {
        use rand::seq::SliceRandom;
        let start = self.index().map(|i| i + 1).unwrap_or(0);
        if !self.shuffle {
            self.renumber();
            self.queue[start..].shuffle(&mut rand::thread_rng());
        } else {
            let order: std::collections::HashMap<_, _> = self
                .original_order
                .iter()
                .enumerate()
                .map(|(i, k)| (k, i))
                .collect();
            self.queue[start..].sort_by_key(|e| order.get(&e.key).copied().unwrap_or(usize::MAX));
        }
        self.shuffle = !self.shuffle;
    }
}

pub fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}
pub fn cover(v: &Value) -> Option<String> {
    v.as_str().filter(|s| s.starts_with("https://")).map(|s| {
        let re = regex::Regex::new(r"-(large|t\d+x\d+)(\.[a-z]+)(\?.*)?$").unwrap();
        re.replace(s, "-t500x500$2$3").into_owned()
    })
}
pub fn track(v: &Value) -> Option<Track> {
    Some(Track {
        id: v["id"].as_u64()?,
        title: v["title"].as_str()?.into(),
        artist: text(&v["user"], "username"),
        artist_id: v["user"]["id"].as_u64().unwrap_or(0),
        artwork: cover(&v["artwork_url"]).or_else(|| cover(&v["user"]["avatar_url"])),
        duration: v["duration"].as_f64().unwrap_or(0.0) / 1000.0,
        permalink: text(v, "permalink_url"),
        available: v["streamable"].as_bool() != Some(false)
            && !matches!(v["policy"].as_str(), Some("BLOCK" | "SNIP")),
    })
}
pub fn artist(v: &Value) -> Option<Artist> {
    Some(Artist {
        id: v["id"].as_u64()?,
        name: text(v, "username"),
        artwork: cover(&v["avatar_url"]).filter(|url| !url.contains("default_avatar")),
        artwork_verified: true,
        track_count: v["track_count"].as_u64().unwrap_or(0) as usize,
        permalink: text(v, "permalink_url"),
    })
}
pub fn playlist(v: &Value) -> Option<Playlist> {
    Some(Playlist {
        id: v["id"].as_u64()?,
        title: text(v, "title"),
        artist: text(&v["user"], "username"),
        artwork: cover(&v["artwork_url"]).or_else(|| cover(&v["tracks"][0]["artwork_url"])),
        track_count: v["track_count"].as_u64().unwrap_or(0) as usize,
        permalink: text(v, "permalink_url"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_duplicates_are_skipped_even_under_repeat_one() {
        let mut p = Playback::default();
        p.enqueue(vec![t(1), t(1), t(2)], false);
        p.repeat = "one".into();
        p.mark_current_unavailable();
        assert!(p.advance(false));
        assert_eq!(p.track().unwrap().id, 2);
        assert!(!p.queue[0].track.available && !p.queue[1].track.available);
    }
    #[test]
    fn all_unavailable_queue_stops_even_with_repeat_all() {
        let mut p = Playback::default();
        p.enqueue(vec![t(1), t(2)], false);
        p.repeat = "all".into();
        p.mark_current_unavailable();
        assert!(p.advance(false));
        p.mark_current_unavailable();
        assert!(!p.advance(false));
        assert!(!p.advance(true));
        assert_eq!(p.queue.len(), 2);
    }
    fn t(id: u64) -> Track {
        Track {
            id,
            title: id.to_string(),
            artist: "a".into(),
            artist_id: 1,
            artwork: None,
            duration: 60.0,
            permalink: String::new(),
            available: true,
        }
    }
    #[test]
    fn duplicate_tracks_have_independent_occurrences() {
        let mut p = Playback::default();
        p.enqueue(vec![t(1), t(1), t(2)], false);
        assert_ne!(p.queue[0].key, p.queue[1].key);
        let k = p.queue[1].key.clone();
        p.remove(&k);
        assert_eq!(p.queue.len(), 2);
        assert_eq!(p.track().unwrap().id, 1);
    }
    #[test]
    fn repeat_one_does_not_trap_manual_next() {
        let mut p = Playback::default();
        p.enqueue(vec![t(1), t(2)], false);
        p.repeat = "one".into();
        assert!(p.advance(true));
        assert_eq!(p.track().unwrap().id, 1);
        assert!(p.advance(false));
        assert_eq!(p.track().unwrap().id, 2);
    }
    #[test]
    fn reorder_preserves_playing_occurrence() {
        let mut p = Playback::default();
        p.enqueue(vec![t(1), t(2), t(3)], false);
        let playing = p.current_key.clone();
        let a = p.queue[2].key.clone();
        let b = p.queue[0].key.clone();
        p.move_entry(&a, &b);
        assert_eq!(p.current_key, playing);
        assert_eq!(p.index(), Some(1));
    }
    #[test]
    fn shuffle_restores_upcoming_order() {
        let mut p = Playback::default();
        p.enqueue((1..10).map(t).collect(), false);
        let original = p.queue.iter().map(|e| e.key.clone()).collect::<Vec<_>>();
        p.set_shuffle();
        p.set_shuffle();
        assert_eq!(
            original,
            p.queue.iter().map(|e| e.key.clone()).collect::<Vec<_>>()
        );
    }
    #[test]
    fn delete_last_current_keeps_valid_selection() {
        let mut p = Playback::default();
        p.enqueue(vec![t(1)], false);
        let key = p.current_key.clone().unwrap();
        assert!(p.remove(&key));
        assert!(p.current_key.is_none());
        assert!(p.queue.is_empty());
    }
    #[test]
    fn shuffle_order_survives_restart_and_new_tracks() {
        let mut p = Playback::default();
        p.enqueue((1..20).map(t).collect(), false);
        p.set_shuffle();
        p.enqueue(vec![t(20)], false);
        let mut restored: Playback =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        restored.set_shuffle();
        assert_eq!(
            restored
                .queue
                .iter()
                .map(|e| e.track.id)
                .collect::<Vec<_>>(),
            (1..=20).collect::<Vec<_>>()
        );
    }
    #[test]
    fn repeat_all_cycles_without_creating_occurrences() {
        let mut p = Playback::default();
        p.enqueue(vec![t(1), t(2)], false);
        p.repeat = "all".into();
        let keys = p.queue.iter().map(|e| e.key.clone()).collect::<Vec<_>>();
        for _ in 0..100 {
            assert!(p.advance(true));
        }
        assert_eq!(
            p.queue.iter().map(|e| e.key.clone()).collect::<Vec<_>>(),
            keys
        );
        assert_eq!(p.current_key.as_ref(), Some(&keys[0]));
    }
}
