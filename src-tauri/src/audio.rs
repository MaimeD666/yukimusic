use crate::{
    models::{Playback, QueueEntry, Track},
    persistence::atomic_write,
    soundcloud::SoundCloud,
    stream_cache::Stream,
};
use rodio::{OutputStream, Sink, Source};
use rustfft::{num_complex::Complex, FftPlanner};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, SyncSender, TryRecvError},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const RATE: u32 = 48000;
const FREQUENCIES: [f32; 10] = [
    32., 64., 125., 250., 500., 1000., 2000., 4000., 8000., 16000.,
];
#[derive(Clone)]
struct DspSettings {
    bands: [f32; 10],
    enabled: bool,
    revision: u64,
}
impl Default for DspSettings {
    fn default() -> Self {
        Self {
            bands: [0.; 10],
            enabled: false,
            revision: 0,
        }
    }
}
#[derive(Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}
impl Default for Biquad {
    fn default() -> Self {
        Self {
            b0: 1.,
            b1: 0.,
            b2: 0.,
            a1: 0.,
            a2: 0.,
            z1: 0.,
            z2: 0.,
        }
    }
}
impl Biquad {
    fn peak(frequency: f32, gain: f32) -> Self {
        let a = 10f32.powf(gain / 40.);
        let w = 2. * std::f32::consts::PI * frequency / RATE as f32;
        let alpha = w.sin() / 2.;
        let a0 = 1. + alpha / a;
        Self {
            b0: (1. + alpha * a) / a0,
            b1: -2. * w.cos() / a0,
            b2: (1. - alpha * a) / a0,
            a1: -2. * w.cos() / a0,
            a2: (1. - alpha / a) / a0,
            ..Self::default()
        }
    }
    fn sample(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}
struct PcmSource {
    receiver: Receiver<Vec<f32>>,
    buffer: VecDeque<f32>,
    frames: Arc<AtomicU64>,
    buffering: Arc<AtomicBool>,
    dsp: Arc<Mutex<DspSettings>>,
    filters: [[Biquad; 10]; 2],
    revision: u64,
    preamp: f32,
    channel: usize,
    visualization: Arc<AtomicBool>,
    spectrum: SyncSender<Vec<f32>>,
    spectrum_buffer: Vec<f32>,
    pending_samples: u64,
}
impl Iterator for PcmSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.buffer.is_empty() {
            match self.receiver.try_recv() {
                Ok(samples) => {
                    self.buffer = samples.into();
                    self.buffering.store(false, Ordering::Relaxed);
                    let settings = self.dsp.lock().unwrap().clone();
                    if settings.revision != self.revision {
                        self.revision = settings.revision;
                        let peak = if settings.enabled {
                            settings.bands.iter().copied().fold(0f32, f32::max)
                        } else {
                            0.
                        };
                        self.preamp = 10f32.powf(-peak / 20.);
                        for ch in 0..2 {
                            for (i, freq) in FREQUENCIES.iter().enumerate() {
                                let mut filter = Biquad::peak(
                                    *freq,
                                    if settings.enabled {
                                        settings.bands[i]
                                    } else {
                                        0.
                                    },
                                );
                                filter.z1 = self.filters[ch][i].z1;
                                filter.z2 = self.filters[ch][i].z2;
                                self.filters[ch][i] = filter;
                            }
                        }
                        self.revision = settings.revision;
                    }
                }
                Err(TryRecvError::Empty) => {
                    self.buffering.store(true, Ordering::Relaxed);
                    return Some(0.);
                }
                Err(TryRecvError::Disconnected) => {
                    self.frames
                        .fetch_add(self.pending_samples / 2, Ordering::Relaxed);
                    self.pending_samples = 0;
                    return None;
                }
            }
        }
        let mut sample = self.buffer.pop_front()? * self.preamp;
        for filter in &mut self.filters[self.channel] {
            sample = filter.sample(sample);
        }
        sample = sample.clamp(-1., 1.);
        if self.channel == 0 && self.visualization.load(Ordering::Relaxed) {
            self.spectrum_buffer.push(sample);
            if self.spectrum_buffer.len() == 2048 {
                let _ = self.spectrum.try_send(self.spectrum_buffer.clone());
                self.spectrum_buffer.drain(..800);
            }
        } else if !self.visualization.load(Ordering::Relaxed) {
            self.spectrum_buffer.clear();
        }
        self.channel ^= 1;
        self.pending_samples += 1;
        if self.pending_samples >= 512 {
            self.frames
                .fetch_add(self.pending_samples / 2, Ordering::Relaxed);
            self.pending_samples = 0;
        }
        Some(sample)
    }
}
impl Source for PcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub enum Message {
    Action(String, Value),
    Resolved(u64, Result<Vec<Stream>, crate::soundcloud::Error>),
    DecoderFailed(u64),
    Buffered(u64),
    #[cfg(windows)]
    OutputChanged,
    Shutdown,
}
pub struct Player {
    pub state: Arc<Mutex<Playback>>,
    pub sender: Sender<Message>,
    pub visualization: Arc<AtomicBool>,
    pub visible: Arc<AtomicBool>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub media: Arc<Mutex<Option<crate::media::Controls>>>,
}
impl Player {
    pub fn new(app: AppHandle, api: Arc<SoundCloud>, directory: PathBuf, ffmpeg: PathBuf) -> Self {
        let mut restored = std::fs::read(directory.join("playback.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Playback>(&b).ok())
            .unwrap_or_default();
        restored.playing = false;
        restored.loading = false;
        restored.error = None;
        restored.volume = restored.volume.clamp(0., 1.);
        restored.eq.resize(10, 0.);
        restored.eq.truncate(10);
        if restored.original_order.is_empty() {
            restored.renumber();
        }
        if restored.index().is_none() {
            restored.current_key = restored.queue.first().map(|e| e.key.clone());
            restored.position = 0.;
        }
        let state = Arc::new(Mutex::new(restored));
        let visualization = Arc::new(AtomicBool::new(true));
        let visible = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel();
        let (spectrum_tx, spectrum_rx) = mpsc::sync_channel::<Vec<f32>>(2);
        let spectrum_app = app.clone();
        let spectrum_visible = visualization.clone();
        std::thread::Builder::new()
            .name("spectrum".into())
            .spawn(move || {
                let mut planner = FftPlanner::<f32>::new();
                let fft = planner.plan_fft_forward(2048);
                let mut buffer = vec![Complex::new(0., 0.); 2048];
                while let Ok(mut samples) = spectrum_rx.recv() {
                    while let Ok(latest) = spectrum_rx.try_recv() {
                        samples = latest;
                    }
                    if !spectrum_visible.load(Ordering::Relaxed) {
                        continue;
                    }
                    for (i, x) in samples.iter().enumerate() {
                        let window =
                            0.5 - 0.5 * (2. * std::f32::consts::PI * i as f32 / 2047.).cos();
                        buffer[i] = Complex::new(x * window, 0.);
                    }
                    fft.process(&mut buffer);
                    let mut bands = vec![0f32; 48];
                    for (i, value) in bands.iter_mut().enumerate() {
                        let lower = 35. * (16000f32 / 35.).powf(i as f32 / 48.);
                        let upper = 35. * (16000f32 / 35.).powf((i + 1) as f32 / 48.);
                        let a = ((lower / RATE as f32 * 2048.) as usize).max(1);
                        let b = ((upper / RATE as f32 * 2048.) as usize)
                            .max(a + 1)
                            .min(1024);
                        let power =
                            buffer[a..b].iter().map(|v| v.norm_sqr()).sum::<f32>() / (b - a) as f32;
                        let db = 10. * (power / (2048. * 2048.)).max(1e-10).log10();
                        *value = ((db + 70.) / 60.).clamp(0., 1.);
                    }
                    let _ = spectrum_app.emit("spectrum", bands);
                }
            })
            .expect("spectrum thread");
        let media = Arc::new(Mutex::new(None));
        let engine_media = media.clone();
        let output_state = state.clone();
        let output_tx = tx.clone();
        let output_viz = visualization.clone();
        let output_visible = visible.clone();
        let worker=std::thread::Builder::new().name("audio-engine".into()).spawn(move||{
            let output=OutputStream::try_default().ok();
            #[cfg(windows)]
            let output_id=crate::audio_devices::default_id();
            let (stream,output_handle)=match output { Some((stream,handle))=>(Some(stream),Some(handle)), None=>(None,None) };
            let sink=output_handle.as_ref().and_then(|handle|Sink::try_new(handle).ok()).unwrap_or_else(||Sink::new_idle().0);
            let dsp=Arc::new(Mutex::new(DspSettings::default()));
            let mut engine=Engine{media:engine_media,app,api,directory,ffmpeg,state:output_state,tx:output_tx,sink,stream,output_handle,
                #[cfg(windows)] output_id,
                #[cfg(windows)] output_retry:None,
                child:None,generation:0,frames:Arc::new(AtomicU64::new(0)),buffering:Arc::new(AtomicBool::new(false)),dsp,visualization:output_viz,visible:output_visible,spectrum:spectrum_tx,base_position:0.,active:false,desired:false,urls:VecDeque::new(),current_url:None,prefetch:None,retries:0,started:Instant::now()};
            if engine.stream.is_none() { engine.state.lock().unwrap().error=Some("Устройство вывода недоступно. Подключи наушники или колонки — плеер подхватит их автоматически.".into());engine.publish(); }
            engine.apply_settings();let mut tick=Instant::now();let mut saved=Instant::now();let mut dirty=false;
            loop{
                match rx.recv_timeout(Duration::from_millis(if engine.desired {100} else {500})){
                    Ok(Message::Shutdown)=>{engine.persist();engine.stop();break;},
                    Ok(message)=>{engine.handle(message);engine.publish();dirty=true;},
                    Err(mpsc::RecvTimeoutError::Disconnected)=>{engine.stop();break;},
                    Err(mpsc::RecvTimeoutError::Timeout)=>{},
                }
                if tick.elapsed()>=Duration::from_millis(400){engine.tick();tick=Instant::now();}
                if (dirty && saved.elapsed()>Duration::from_secs(1)) || (engine.desired && saved.elapsed()>Duration::from_secs(15)){engine.persist();saved=Instant::now();dirty=false;}
            }
        }).expect("audio thread");
        Self {
            state,
            sender: tx,
            visualization,
            visible,
            worker: Mutex::new(Some(worker)),
            media,
        }
    }
    pub fn shutdown(&self) {
        let _ = self.sender.send(Message::Shutdown);
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
    pub fn action(&self, action: String, value: Value) -> Result<(), String> {
        self.sender
            .send(Message::Action(action, value))
            .map_err(|_| "Аудиодвижок недоступен. Перезапусти приложение.".into())
    }
}
struct Engine {
    media: Arc<Mutex<Option<crate::media::Controls>>>,
    app: AppHandle,
    api: Arc<SoundCloud>,
    directory: PathBuf,
    ffmpeg: PathBuf,
    state: Arc<Mutex<Playback>>,
    tx: Sender<Message>,
    sink: Sink,
    stream: Option<OutputStream>,
    output_handle: Option<rodio::OutputStreamHandle>,
    #[cfg(windows)]
    output_id: Option<String>,
    #[cfg(windows)]
    output_retry: Option<(Instant, u8)>,
    child: Option<Arc<Mutex<Child>>>,
    generation: u64,
    frames: Arc<AtomicU64>,
    buffering: Arc<AtomicBool>,
    dsp: Arc<Mutex<DspSettings>>,
    visualization: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    spectrum: SyncSender<Vec<f32>>,
    base_position: f64,
    active: bool,
    desired: bool,
    urls: VecDeque<Stream>,
    current_url: Option<Stream>,
    prefetch: Option<(u64, Instant, tauri::async_runtime::JoinHandle<()>)>,
    retries: u8,
    started: Instant,
}
impl Engine {
    fn publish(&self) {
        #[cfg(windows)]
        crate::taskbar::update(&self.state.lock().unwrap());
        if let Some(media) = self.media.lock().unwrap().as_mut() {
            media.update(&self.state.lock().unwrap());
        }
        if self.visible.load(Ordering::Relaxed) {
            let _ = self
                .app
                .emit("playback", self.state.lock().unwrap().clone());
        }
    }
    fn persist(&self) {
        let state = self.state.lock().unwrap();
        let _ = atomic_write(&self.directory.join("playback.json"), &*state);
    }
    fn stop(&mut self) {
        // clear() waits for the device callback and can deadlock after unplugging it.
        self.sink.stop();
        self.sink = self
            .output_handle
            .as_ref()
            .and_then(|handle| Sink::try_new(handle).ok())
            .unwrap_or_else(|| Sink::new_idle().0);
        self.active = false;
        if let Some(child) = self.child.take() {
            let _ = child.lock().unwrap().kill();
        }
    }
    fn apply_settings(&self) {
        let state = self.state.lock().unwrap();
        self.sink
            .set_volume(if state.muted { 0. } else { state.volume });
        let mut dsp = self.dsp.lock().unwrap();
        for i in 0..10 {
            dsp.bands[i] = state.eq.get(i).copied().unwrap_or(0.).clamp(-12., 12.);
        }
        dsp.enabled = state.eq_enabled;
        dsp.revision += 1;
    }
    #[cfg(windows)]
    fn rebind_output(&mut self, retry: bool) {
        let id = crate::audio_devices::default_id();
        if !retry && id == self.output_id && self.stream.is_some() {
            return;
        }
        let attempt = if retry {
            self.output_retry.map(|(_, n)| n).unwrap_or(0)
        } else {
            0
        };
        self.sink.pause();
        let had_source = self.active;
        {
            let mut s = self.state.lock().unwrap();
            if had_source {
                s.position =
                    self.base_position + self.frames.load(Ordering::Relaxed) as f64 / RATE as f64;
            }
            self.base_position = s.position;
            s.playing = false;
            s.loading = self.desired;
        }
        // Invalidate pending decoder/network callbacks, but retain reusable URLs.
        self.generation += 1;
        self.output_handle.take();
        self.stop();
        self.stream.take();
        self.output_id = id.clone();
        match OutputStream::try_default()
            .ok()
            .and_then(|(stream, handle)| {
                Sink::try_new(&handle)
                    .ok()
                    .map(|sink| (stream, handle, sink))
            }) {
            Some((stream, handle, sink)) => {
                self.stream = Some(stream);
                self.output_handle = Some(handle);
                self.sink = sink;
                self.output_retry = None;
                self.state.lock().unwrap().error = None;
                self.apply_settings();
                if had_source || self.desired {
                    if let Some(url) = self.current_url.clone() {
                        self.urls.retain(|item| item.url != url.url);
                        self.urls.push_front(url);
                    }
                    if self.urls.is_empty() {
                        self.load(self.desired, true);
                    } else {
                        self.start_decoder();
                    }
                }
            }
            None => {
                // A newly selected driver can take a moment to become usable.
                // With no endpoint, wait for a native notification instead of polling.
                self.output_retry = if id.is_some() && attempt < 6 {
                    Some((
                        Instant::now() + Duration::from_millis(200 << attempt.min(4)),
                        attempt + 1,
                    ))
                } else {
                    None
                };
                self.state.lock().unwrap().error =
                    Some("Устройство вывода недоступно. Ожидаю наушники или колонки.".into());
            }
        }
        self.publish();
    }
    fn load(&mut self, autoplay: bool, reset_retries: bool) {
        self.desired = autoplay;
        self.urls.clear();
        self.current_url = None;
        if reset_retries {
            self.retries = 0;
        }
        let id = {
            let mut s = self.state.lock().unwrap();
            s.error = None;
            s.playing = false;
            s.loading = autoplay;
            self.base_position = s.position;
            s.track().map(|t| t.id)
        };
        self.stop();
        self.generation += 1;
        if let Some(id) = id {
            let api = self.api.clone();
            let tx = self.tx.clone();
            let generation = self.generation;
            tauri::async_runtime::spawn(async move {
                let result = if reset_retries {
                    api.streams(id).await
                } else {
                    api.refresh_streams(id).await
                };
                let _ = tx.send(Message::Resolved(generation, result));
            });
        } else {
            let mut s = self.state.lock().unwrap();
            s.playing = false;
            s.loading = false;
            self.desired = false;
        }
    }
    fn fail(&mut self, message: String) {
        self.stop();
        self.desired = false;
        let mut s = self.state.lock().unwrap();
        s.playing = false;
        s.loading = false;
        s.error = Some(message);
    }
    fn skip_unavailable(&mut self) {
        let mut state = self.state.lock().unwrap();
        let title = state.mark_current_unavailable().unwrap_or_default();
        let advanced = state.advance(false);
        drop(state);
        if self.visible.load(Ordering::Relaxed) {
            let _ = self
                .app
                .emit("notice", format!("{title} недоступен — пропущен"));
        }
        if advanced {
            self.load(self.desired, true);
        } else {
            self.fail("В очереди больше нет доступных треков.".into());
        }
    }
    fn start_decoder(&mut self) {
        if self.stream.is_none() {
            let mut s = self.state.lock().unwrap();
            s.playing = false;
            s.loading = self.desired;
            s.error = Some("Устройство вывода недоступно. Ожидаю наушники или колонки.".into());
            return;
        }
        {
            let mut s = self.state.lock().unwrap();
            s.playing = false;
            s.loading = self.desired;
        }
        let Some(url) = self.urls.pop_front() else {
            self.fail(
                "Не удалось воспроизвести этот трек. Попробуй ещё раз или выбери следующий.".into(),
            );
            return;
        };
        self.stop();
        self.current_url = Some(url.clone());
        let (url, offset, segmented) = url.at(self.base_position);
        self.frames.store(0, Ordering::Relaxed);
        self.buffering.store(true, Ordering::Relaxed);
        let mut command = Command::new(&self.ffmpeg);
        command.args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-rw_timeout",
            "15000000",
            "-user_agent",
            "Mozilla/5.0",
            "-probesize",
            "32768",
            "-analyzeduration",
            "100000",
        ]);
        let output_seek = segmented || url.contains(".m3u8");
        if offset > 0. && !output_seek {
            command.args(["-ss", &format!("{offset:.3}")]);
        }
        // Our validated HLS proxy replaces CDN filenames with numeric asset IDs.
        // FFmpeg 8's filename-extension check cannot infer their audio format.
        if segmented { command.args(["-extension_picky", "0"]); }
        command.args(["-i", &url]);
        if offset > 0. && output_seek {
            command.args(["-ss", &format!("{offset:.3}")]);
        }
        command
            .args([
                "-vn",
                "-sn",
                "-dn",
                "-threads",
                "1",
                "-f",
                "f32le",
                "-acodec",
                "pcm_f32le",
                "-ar",
                "48000",
                "-ac",
                "2",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(_) => {
                self.fail("Не удалось запустить аудиодвижок. Переустанови приложение.".into());
                return;
            }
        };
        let mut stdout = child.stdout.take().unwrap();
        let child = Arc::new(Mutex::new(child));
        self.child = Some(child.clone());
        let (data_tx, data_rx) = mpsc::sync_channel(16);
        let tx = self.tx.clone();
        let generation = self.generation;
        std::thread::spawn(move || {
            let mut block = [0u8; 32768];
            let mut carry = Vec::new();
            let mut total = 0;
            loop {
                match stdout.read(&mut block) {
                    Ok(0) => break,
                    Ok(n) => {
                        carry.extend_from_slice(&block[..n]);
                        let usable = carry.len() / 8 * 8;
                        let samples: Vec<f32> = carry[..usable]
                            .chunks_exact(4)
                            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                            .collect();
                        carry.drain(..usable);
                        let first = total == 0;
                        total += samples.len();
                        if !samples.is_empty() && data_tx.send(samples).is_err() {
                            return;
                        }
                        if first && total > 0 {
                            let _ = tx.send(Message::Buffered(generation));
                        }
                    }
                    Err(_) => break,
                }
            }
            let success = child
                .lock()
                .unwrap()
                .wait()
                .map(|s| s.success())
                .unwrap_or(false);
            if !success || total == 0 {
                let _ = tx.send(Message::DecoderFailed(generation));
            }
        });
        let source = PcmSource {
            receiver: data_rx,
            buffer: VecDeque::new(),
            frames: self.frames.clone(),
            buffering: self.buffering.clone(),
            dsp: self.dsp.clone(),
            filters: [[Biquad::default(); 10]; 2],
            revision: u64::MAX,
            preamp: 1.,
            channel: 0,
            visualization: self.visualization.clone(),
            spectrum: self.spectrum.clone(),
            spectrum_buffer: Vec::with_capacity(2048),
            pending_samples: 0,
        };
        self.sink.append(source);
        if self.desired {
            self.sink.play();
        } else {
            self.sink.pause();
        }
        self.active = true;
        self.started = Instant::now();
        self.apply_settings();
    }
    fn handle(&mut self, message: Message) {
        match message {
            #[cfg(windows)]
            Message::OutputChanged => self.rebind_output(false),
            Message::Buffered(generation) if generation == self.generation => {
                let mut s = self.state.lock().unwrap();
                s.loading = false;
                s.playing = self.desired;
                drop(s);
                self.prepare_next();
            }
            Message::Resolved(generation, result) if generation == self.generation => {
                match result {
                    Ok(urls) => {
                        self.urls = urls.into();
                        self.start_decoder();
                    }
                    Err(e) if e.confirms_unavailable() => self.skip_unavailable(),
                    Err(e) => self.fail(e.to_string()),
                }
            }
            Message::DecoderFailed(generation) if generation == self.generation => {
                self.base_position = self.state.lock().unwrap().position;
                if !self.urls.is_empty() {
                    self.start_decoder();
                } else if self.retries < 1 {
                    self.retries += 1;
                    self.load(self.desired, false);
                } else {
                    self.fail("Поток прервался или его формат недоступен. Очередь сохранена — можно повторить.".into());
                }
            }
            Message::Action(action, value) => self.action(&action, value),
            _ => {}
        }
    }
    fn prepare_next(&mut self) {
        if !self.desired {
            return;
        }
        let next = {
            let s = self.state.lock().unwrap();
            let mut next = s.clone();
            next.advance(false)
                .then(|| next.track().map(|t| t.id))
                .flatten()
        };
        if self
            .prefetch
            .as_ref()
            .map(|(id, time, _)| (Some(*id), time.elapsed() < Duration::from_secs(90)))
            == Some((next, true))
        {
            return;
        }
        if let Some((_, _, job)) = self.prefetch.take() {
            job.abort();
        }
        if let Some(id) = next {
            let api = self.api.clone();
            let job = tauri::async_runtime::spawn(async move {
                let _ = api.streams(id).await;
            });
            self.prefetch = Some((id, Instant::now(), job));
        }
    }
    fn action(&mut self, action: &str, value: Value) {
        match action {
            #[cfg(all(windows, debug_assertions))]
            "test_output_change" => self.rebind_output(true),
            "play" => {
                let tracks = serde_json::from_value::<Vec<Track>>(value["tracks"].clone())
                    .unwrap_or_default();
                let index = value["index"].as_u64().unwrap_or(0) as usize;
                if index >= tracks.len() || !tracks[index].available {
                    return;
                }
                let mut s = self.state.lock().unwrap();
                let index = tracks[..index].iter().filter(|t| t.available).count();
                let shuffled = s.shuffle;
                s.queue = tracks
                    .into_iter()
                    .filter(|t| t.available)
                    .map(QueueEntry::new)
                    .collect();
                s.current_key = Some(s.queue[index].key.clone());
                s.position = 0.;
                s.shuffle = false;
                s.renumber();
                if shuffled {
                    s.set_shuffle();
                }
                drop(s);
                self.load(true, true);
            }
            "toggle" => {
                if self.desired {
                    self.desired = false;
                    self.sink.pause();
                    let mut s = self.state.lock().unwrap();
                    s.position = self.base_position
                        + self.frames.load(Ordering::Relaxed) as f64 / RATE as f64;
                    s.playing = false;
                    s.loading = false;
                } else if self.active && !self.sink.empty() {
                    self.desired = true;
                    self.sink.play();
                    self.state.lock().unwrap().playing = true;
                } else {
                    let mut s = self.state.lock().unwrap();
                    if s.track()
                        .map(|t| s.position >= t.duration - 0.3)
                        .unwrap_or(false)
                    {
                        s.position = 0.;
                    }
                    drop(s);
                    self.load(true, true);
                }
            }
            "next" => {
                let changed = self.state.lock().unwrap().advance(false);
                if changed {
                    self.load(true, true);
                }
            }
            "previous" => {
                let mut s = self.state.lock().unwrap();
                if s.position <= 3. {
                    if let Some(i) = s.index() {
                        if let Some(previous) = s.queue[..i].iter().rfind(|e| e.track.available) {
                            s.current_key = Some(previous.key.clone());
                        }
                    }
                }
                s.position = 0.;
                drop(s);
                self.load(true, true);
            }
            "jump" => {
                let key = value.as_str().unwrap_or("");
                let mut s = self.state.lock().unwrap();
                if !s.queue.iter().any(|e| e.key == key && e.track.available) {
                    return;
                }
                s.current_key = Some(key.into());
                s.position = 0.;
                drop(s);
                self.load(true, true);
            }
            "seek" => {
                let mut s = self.state.lock().unwrap();
                if let Some(t) = s.track() {
                    let duration = t.duration.max(0.);
                    let target = value.as_f64().unwrap_or(0.);
                    if !target.is_finite() {
                        return;
                    }
                    s.position = target.clamp(0., duration);
                    if duration > 0. && s.position >= duration {
                        let advance = self.desired && s.advance(true);
                        s.playing = false;
                        s.loading = false;
                        s.error = None;
                        drop(s);
                        self.stop();
                        self.generation += 1;
                        if advance {
                            self.load(true, true);
                        } else {
                            self.desired = false;
                        }
                        return;
                    }
                    s.error = None;
                    s.playing = false;
                    drop(s);
                    if let Some(url) = self.current_url.clone() {
                        self.stop();
                        self.generation += 1;
                        self.retries = 0;
                        self.base_position = self.state.lock().unwrap().position;
                        self.state.lock().unwrap().loading = self.desired;
                        self.urls.retain(|candidate| candidate.url != url.url);
                        self.urls.push_front(url);
                        self.start_decoder();
                    } else {
                        self.load(self.desired, true);
                    }
                }
            }
            "enqueue" => {
                let tracks = serde_json::from_value::<Vec<Track>>(value["tracks"].clone())
                    .unwrap_or_default();
                self.state
                    .lock()
                    .unwrap()
                    .enqueue(tracks, value["next"].as_bool().unwrap_or(false));
            }
            "move" => {
                self.state.lock().unwrap().move_entry(
                    value["from"].as_str().unwrap_or(""),
                    value["to"].as_str().unwrap_or(""),
                );
            }
            "remove" => {
                let changed = self
                    .state
                    .lock()
                    .unwrap()
                    .remove(value.as_str().unwrap_or(""));
                if changed {
                    self.load(self.desired, true);
                }
            }
            "clear" => {
                let mut s = self.state.lock().unwrap();
                let current = s.current_key.clone();
                s.queue.retain(|e| Some(&e.key) == current.as_ref());
                s.renumber();
            }
            "volume" => {
                let mut s = self.state.lock().unwrap();
                let volume = value.as_f64().unwrap_or(0.) as f32;
                s.volume = if volume.is_finite() {
                    volume.clamp(0., 1.)
                } else {
                    0.
                };
                s.muted = false;
                drop(s);
                self.apply_settings();
            }
            "mute" => {
                let mut s = self.state.lock().unwrap();
                s.muted = !s.muted;
                drop(s);
                self.apply_settings();
            }
            "shuffle" => self.state.lock().unwrap().set_shuffle(),
            "repeat" => {
                let mode = value.as_str().unwrap_or("off");
                if ["off", "all", "one"].contains(&mode) {
                    self.state.lock().unwrap().repeat = mode.into();
                }
            }
            "eq" => {
                let mut s = self.state.lock().unwrap();
                s.eq = value["bands"]
                    .as_array()
                    .map(|bands| {
                        bands
                            .iter()
                            .take(10)
                            .map(|v| v.as_f64().unwrap_or(0.).clamp(-12., 12.) as f32)
                            .collect()
                    })
                    .unwrap_or_else(|| vec![0.; 10]);
                s.eq.resize(10, 0.);
                s.eq_enabled = value["enabled"].as_bool().unwrap_or(false);
                drop(s);
                self.apply_settings();
            }
            "dismiss_error" => self.state.lock().unwrap().error = None,
            _ => {}
        }
        if self.active && !self.buffering.load(Ordering::Relaxed) {
            self.prepare_next();
        }
    }
    fn tick(&mut self) {
        #[cfg(windows)]
        if self
            .output_retry
            .is_some_and(|(deadline, _)| Instant::now() >= deadline)
        {
            self.rebind_output(true);
        }
        if !self.active {
            return;
        }
        let frames = self.frames.load(Ordering::Relaxed);
        let mut s = self.state.lock().unwrap();
        s.position = self.base_position + frames as f64 / RATE as f64;
        s.loading = self.desired && self.buffering.load(Ordering::Relaxed);
        s.playing = self.desired && !s.loading;
        #[cfg(windows)]
        crate::taskbar::update(&s);
        if self.visible.load(Ordering::Relaxed) {
            let _ = self.app.emit(
                "timeline",
                serde_json::json!({"position":s.position,"playing":s.playing,"loading":s.loading}),
            );
        }
        if let Some(media) = self.media.lock().unwrap().as_mut() {
            media.update(&s);
        }
        let ended = self.desired
            && self.sink.empty()
            && self.started.elapsed() > Duration::from_millis(500);
        let approaching_end = s
            .track()
            .map(|t| t.duration - s.position < 15.)
            .unwrap_or(false);
        drop(s);
        if approaching_end {
            self.prepare_next();
        }
        if ended {
            // Only normal decoder completion advances; errors arrive as DecoderFailed.
            if frames == 0 {
                return;
            }
            let advance = self.state.lock().unwrap().advance(true);
            if advance {
                self.load(true, true);
            } else {
                self.stop();
                self.desired = false;
                let mut s = self.state.lock().unwrap();
                s.playing = false;
                s.loading = false;
            }
            self.publish();
            self.persist();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffering_is_not_treated_as_end_of_track() {
        let (tx, rx) = mpsc::sync_channel(1);
        let (spectrum, _) = mpsc::sync_channel(1);
        let mut source = PcmSource {
            receiver: rx,
            buffer: VecDeque::new(),
            frames: Arc::new(AtomicU64::new(0)),
            buffering: Arc::new(AtomicBool::new(false)),
            dsp: Arc::new(Mutex::new(DspSettings::default())),
            filters: [[Biquad::default(); 10]; 2],
            revision: u64::MAX,
            preamp: 1.0,
            channel: 0,
            visualization: Arc::new(AtomicBool::new(false)),
            spectrum,
            spectrum_buffer: Vec::new(),
            pending_samples: 0,
        };
        for _ in 0..10000 {
            assert_eq!(source.next(), Some(0.0));
        }
        assert_eq!(source.frames.load(Ordering::Relaxed), 0);
        tx.send(vec![0.25, 0.25]).unwrap();
        assert_eq!(source.next(), Some(0.25));
        assert_eq!(source.next(), Some(0.25));
        drop(tx);
        assert_eq!(source.next(), None);
    }
    #[test]
    fn zero_db_filter_is_identity() {
        let mut f = Biquad::peak(1000., 0.);
        for i in 0..10000 {
            let x = (i as f32 * 0.1).sin() * 0.2;
            assert!((f.sample(x) - x).abs() < 1e-5);
        }
    }
    #[test]
    fn boosted_filter_stays_finite() {
        let mut f = Biquad::peak(16000., 12.);
        for i in 0..48000 {
            assert!(f.sample((i as f32 * 0.8).sin() * 0.1).is_finite());
        }
    }
    #[test]
    fn peak_filter_produces_requested_gain_at_center_frequency() {
        let mut filter = Biquad::peak(1000.0, 6.0);
        let (mut input, mut output) = (0.0f64, 0.0f64);
        for i in 0..48000 {
            let x = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / RATE as f32).sin() * 0.1;
            let y = filter.sample(x);
            if i > 4800 {
                input += (x * x) as f64;
                output += (y * y) as f64;
            }
        }
        assert!((10.0 * (output / input).log10() - 6.0).abs() < 0.02);
    }
}
