use crate::{audio::Message, models::Playback};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::{
    sync::{mpsc::Sender, Arc, Mutex},
    time::Duration,
};

pub struct Controls {
    controls: MediaControls,
    track: Option<u64>,
    status: Option<bool>,
    second: u64,
}
impl Controls {
    pub fn new(
        hwnd: *mut std::ffi::c_void,
        sender: Sender<Message>,
        state: Arc<Mutex<Playback>>,
    ) -> Option<Self> {
        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: "yukicloud",
            display_name: "yukicloud",
            hwnd: Some(hwnd),
        })
        .ok()?;
        controls
            .attach(move |event| {
                let s = state.lock().unwrap();
                let (action, value) = match event {
                    MediaControlEvent::Play if !s.playing && !s.loading => {
                        ("toggle", serde_json::Value::Null)
                    }
                    MediaControlEvent::Pause | MediaControlEvent::Stop
                        if s.playing || s.loading =>
                    {
                        ("toggle", serde_json::Value::Null)
                    }
                    MediaControlEvent::Toggle => ("toggle", serde_json::Value::Null),
                    MediaControlEvent::Next => ("next", serde_json::Value::Null),
                    MediaControlEvent::Previous => ("previous", serde_json::Value::Null),
                    MediaControlEvent::SetPosition(position) => {
                        ("seek", serde_json::json!(position.0.as_secs_f64()))
                    }
                    MediaControlEvent::Seek(direction) => (
                        "seek",
                        serde_json::json!((s.position
                            + if direction == SeekDirection::Forward {
                                5.0
                            } else {
                                -5.0
                            })
                        .max(0.0)),
                    ),
                    _ => return,
                };
                drop(s);
                let _ = sender.send(Message::Action(action.into(), value));
            })
            .ok()?;
        Some(Self {
            controls,
            track: None,
            status: None,
            second: u64::MAX,
        })
    }
    pub fn update(&mut self, state: &Playback) {
        let track = state.track();
        if self.track != track.map(|t| t.id) {
            self.track = track.map(|t| t.id);
            if let Some(t) = track {
                let _ = self.controls.set_metadata(MediaMetadata {
                    title: Some(&t.title),
                    artist: Some(&t.artist),
                    cover_url: t.artwork.as_deref(),
                    duration: Some(Duration::from_secs_f64(t.duration.max(0.0))),
                    ..Default::default()
                });
            }
        }
        let playing = state.playing || state.loading;
        let second = state.position.max(0.0) as u64;
        if self.status != Some(playing) || second != self.second {
            let progress = Some(MediaPosition(Duration::from_secs(second)));
            let _ = self.controls.set_playback(if track.is_none() {
                MediaPlayback::Stopped
            } else if playing {
                MediaPlayback::Playing { progress }
            } else {
                MediaPlayback::Paused { progress }
            });
            self.status = Some(playing);
            self.second = second;
        }
    }
}
