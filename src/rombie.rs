//! Rombie's engine mode, at this point the spike of plan Phase 3 (docs/rombie/notes.md). With
//! GOPHER64_ROMBIE_SPIKE_PORT set, the game opens no window and no sound device: it waits for one page on that
//! 127.0.0.1 port, sends it the frames and the sound, and plays the pads it sends. It ends when the page leaves.
//!
//! Every multi-byte value is little-endian, and the frame, audio and ping messages are pcsx2-rombie's (Session.h).
//! Page to engine (binary, first byte is the type):
//!   0x03 pad    [u8 type][u8 port][2 pad][u32 pad word]   (port 0-3. The word as the N64 reads it: buttons in bits
//!                                                        0-13 in the N64's order, then the stick's x and y as signed
//!                                                        bytes, +y up, at most 85 either way)
//!   0x05 ping   [u8 type][7 pad][f64 page time, ms]
//! Engine to page (binary):
//!   0x82 frame  [u8 type][3 pad][u32 frame no][f64 engine time, ms][u16 w][u16 h][u32 vi no][RGBA]
//!   0x83 audio  [u8 type][3 pad][u32 frames][f32 stereo, interleaved, 48 kHz]
//! Engine to page (text, JSON): hello, stats (once a second), pong.
use crate::ui;
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use tokio_tungstenite::tungstenite::Message;

const MSG_PAD: u8 = 0x03;
const MSG_PING: u8 = 0x05;
const MSG_FRAME: u8 = 0x82;
const MSG_AUDIO: u8 = 0x83;
const FRAME_HEADER_SIZE: usize = 24;
const AUDIO_HEADER_SIZE: usize = 8;
const AUDIO_RATE: f64 = 48000.0;
const AUDIO_BLOCK_FRAMES: u64 = 256;
const AUDIO_MAX_BLOCK_FRAMES: u64 = 2048;
const MAX_STICK: i8 = 85;
const PAD_BUTTONS: u32 = 0x3FFF;
/// Messages waiting for the page. A frame that finds the queue full is dropped, so a page that falls behind gets
/// the newer frames rather than a growing delay.
const SEND_QUEUE: usize = 16;

static PORT: std::sync::LazyLock<Option<u16>> = std::sync::LazyLock::new(|| {
    std::env::var("GOPHER64_ROMBIE_SPIKE_PORT")
        .ok()?
        .parse()
        .ok()
});
static START: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);
static CONNECTED: AtomicBool = AtomicBool::new(false);
static OUT: std::sync::Mutex<Option<tokio::sync::mpsc::Sender<Message>>> =
    std::sync::Mutex::new(None);
static PADS: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
static FRAME_NO: AtomicU32 = AtomicU32::new(0);
static VI_NO: AtomicU32 = AtomicU32::new(0);
static AUDIO_PAUSED: AtomicBool = AtomicBool::new(true);
static AUDIO_STOP: AtomicBool = AtomicBool::new(false);
static AUDIO_THREAD: std::sync::Mutex<Option<std::thread::JoinHandle<()>>> =
    std::sync::Mutex::new(None);

/// Counters for the once-a-second stats line, taken and reset as it is sent.
struct Stats {
    vis: AtomicU32,
    grabbed: AtomicU32,
    sent: AtomicU32,
    dropped: AtomicU32,
    width: AtomicU32,
    height: AtomicU32,
    hz_milli: AtomicU32,
    readback_us: AtomicU64,
    readback_max_us: AtomicU64,
    send_us: AtomicU64,
    send_max_us: AtomicU64,
    audio_frames: AtomicU64,
    audio_underruns: AtomicU32,
}

static STATS: Stats = Stats {
    vis: AtomicU32::new(0),
    grabbed: AtomicU32::new(0),
    sent: AtomicU32::new(0),
    dropped: AtomicU32::new(0),
    width: AtomicU32::new(0),
    height: AtomicU32::new(0),
    hz_milli: AtomicU32::new(0),
    readback_us: AtomicU64::new(0),
    readback_max_us: AtomicU64::new(0),
    send_us: AtomicU64::new(0),
    send_max_us: AtomicU64::new(0),
    audio_frames: AtomicU64::new(0),
    audio_underruns: AtomicU32::new(0),
};

pub fn active() -> bool {
    PORT.is_some()
}

pub fn connected() -> bool {
    CONNECTED.load(Relaxed)
}

fn now_ms() -> f64 {
    START.elapsed().as_secs_f64() * 1000.0
}

fn add_timing(sum: &AtomicU64, max: &AtomicU64, started: std::time::Instant) {
    let us = started.elapsed().as_micros() as u64;
    sum.fetch_add(us, Relaxed);
    max.fetch_max(us, Relaxed);
}

fn send(message: Message) -> bool {
    match OUT.lock().unwrap().as_ref() {
        Some(out) => out.try_send(message).is_ok(),
        None => false,
    }
}

/// Before the game starts: listens on the port and returns once a page has connected.
pub fn wait_for_page() {
    let port = PORT.expect("engine mode is off");
    let _ = *START;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    tokio::spawn(serve(port, ready_tx));
    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => panic!("Could not listen on 127.0.0.1:{port}: {err}"),
        Err(_) => panic!("The page server stopped"),
    }
}

async fn serve(port: u16, ready: std::sync::mpsc::Sender<Result<(), String>>) {
    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
        Ok(listener) => listener,
        Err(err) => {
            let _ = ready.send(Err(err.to_string()));
            return;
        }
    };
    eprintln!("rombie: waiting for a page on 127.0.0.1:{port}");
    let ws = loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let _ = stream.set_nodelay(true);
        if let Ok(ws) = tokio_tungstenite::accept_async(stream).await {
            break ws;
        }
    };
    // One page a run: no other page can connect while the game plays.
    drop(listener);

    let (mut sink, mut source) = ws.split();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<Message>(SEND_QUEUE);
    let _ = out_tx.try_send(Message::Text(
        format!(
            r#"{{"t":"hello","engine":"gopher64-rombie","version":"{}","spike":true}}"#,
            env!("GIT_DESCRIBE")
        )
        .into(),
    ));
    *OUT.lock().unwrap() = Some(out_tx.clone());
    CONNECTED.store(true, Relaxed);
    let _ = ready.send(Ok(()));

    let writer = tokio::spawn(async move {
        while let Some(message) = out_rx.recv().await {
            let frame = matches!(&message, Message::Binary(b) if b.first() == Some(&MSG_FRAME));
            let started = std::time::Instant::now();
            if sink.send(message).await.is_err() {
                break;
            }
            if frame {
                STATS.sent.fetch_add(1, Relaxed);
                add_timing(&STATS.send_us, &STATS.send_max_us, started);
            }
        }
    });
    let stats = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        interval.tick().await;
        let mut last = std::time::Instant::now();
        loop {
            interval.tick().await;
            let now = std::time::Instant::now();
            let _ = out_tx.try_send(Message::Text(take_stats_json(now - last).into()));
            last = now;
        }
    });

    while let Some(Ok(message)) = source.next().await {
        match message {
            Message::Binary(data) => handle_message(&data),
            Message::Close(_) => break,
            _ => {}
        }
    }

    CONNECTED.store(false, Relaxed);
    OUT.lock().unwrap().take();
    stats.abort();
    writer.abort();
    eprintln!("rombie: the page left");
}

fn handle_message(data: &[u8]) {
    match data.first() {
        Some(&MSG_PAD) if data.len() >= 8 && data[1] < 4 => {
            let word = u32::from_le_bytes(data[4..8].try_into().unwrap());
            let stick = |v: u32| (v as u8 as i8).clamp(-MAX_STICK, MAX_STICK) as u8 as u32;
            PADS[data[1] as usize].store(
                (word & PAD_BUTTONS) | stick(word >> 16) << 16 | stick(word >> 24) << 24,
                Relaxed,
            );
        }
        Some(&MSG_PING) if data.len() >= 16 => {
            let page_ms = f64::from_le_bytes(data[8..16].try_into().unwrap());
            send(Message::Text(
                format!(r#"{{"t":"pong","c":{page_ms:.3},"e":{:.3}}}"#, now_ms()).into(),
            ));
        }
        _ => {}
    }
}

fn take_stats_json(window: std::time::Duration) -> String {
    let grabbed = STATS.grabbed.swap(0, Relaxed);
    let sent = STATS.sent.swap(0, Relaxed);
    let readback_us = STATS.readback_us.swap(0, Relaxed);
    let send_us = STATS.send_us.swap(0, Relaxed);
    format!(
        r#"{{"t":"stats","ms":{:.0},"vi":{},"grab":{},"sent":{},"drop":{},"w":{},"h":{},"hz":{:.3},"rb":{:.2},"rbMax":{:.2},"send":{:.2},"sendMax":{:.2},"af":{},"au":{}}}"#,
        window.as_secs_f64() * 1000.0,
        STATS.vis.swap(0, Relaxed),
        grabbed,
        sent,
        STATS.dropped.swap(0, Relaxed),
        STATS.width.load(Relaxed),
        STATS.height.load(Relaxed),
        STATS.hz_milli.load(Relaxed) as f64 / 1000.0,
        if grabbed > 0 {
            readback_us as f64 / 1000.0 / grabbed as f64
        } else {
            0.0
        },
        STATS.readback_max_us.swap(0, Relaxed) as f64 / 1000.0,
        if sent > 0 {
            send_us as f64 / 1000.0 / sent as f64
        } else {
            0.0
        },
        STATS.send_max_us.swap(0, Relaxed) as f64 / 1000.0,
        STATS.audio_frames.swap(0, Relaxed),
        STATS.audio_underruns.swap(0, Relaxed),
    )
}

/// The port's pad word, as the page last sent it.
pub fn pad(port: usize) -> u32 {
    PADS[port].load(Relaxed)
}

/// Emulation thread, at each VI: the moment the windowed build would draw the frame. Reads it back and queues it.
pub fn send_frame(frame_time: f64) {
    STATS.vis.fetch_add(1, Relaxed);
    STATS.hz_milli.store((1000.0 / frame_time) as u32, Relaxed);
    let vi_no = VI_NO.fetch_add(1, Relaxed) + 1;
    if !connected() {
        return;
    }
    let ready_ms = now_ms();
    let started = std::time::Instant::now();
    let mut message = Vec::new();
    let Some((width, height)) = ui::video::read_frame(&mut message, FRAME_HEADER_SIZE) else {
        return;
    };
    // The VI's alpha isn't display opacity; a canvas would show these pixels dimmed or see-through.
    for alpha in message[FRAME_HEADER_SIZE + 3..].iter_mut().step_by(4) {
        *alpha = 0xFF;
    }
    let frame_no = FRAME_NO.fetch_add(1, Relaxed) + 1;
    message[0] = MSG_FRAME;
    message[4..8].copy_from_slice(&frame_no.to_le_bytes());
    message[8..16].copy_from_slice(&ready_ms.to_le_bytes());
    message[16..18].copy_from_slice(&(width as u16).to_le_bytes());
    message[18..20].copy_from_slice(&(height as u16).to_le_bytes());
    message[20..24].copy_from_slice(&vi_no.to_le_bytes());
    add_timing(&STATS.readback_us, &STATS.readback_max_us, started);
    STATS.grabbed.fetch_add(1, Relaxed);
    STATS.width.store(width, Relaxed);
    STATS.height.store(height, Relaxed);
    if !send(Message::Binary(message.into())) {
        STATS.dropped.fetch_add(1, Relaxed);
    }
}

pub fn set_audio_paused(paused: bool) {
    AUDIO_PAUSED.store(paused, Relaxed);
}

/// Pulls the stream at 48 kHz, as a sound card would, and sends it to the page instead of playing it. The stream's
/// get callback (ui::audio) pads a shortfall with silence and nudges the rate, as it does for a sound card.
pub fn start_audio(stream: *mut sdl3_sys::audio::SDL_AudioStream) {
    let stream = stream as usize;
    AUDIO_STOP.store(false, Relaxed);
    *AUDIO_THREAD.lock().unwrap() = Some(std::thread::spawn(move || {
        let stream = stream as *mut sdl3_sys::audio::SDL_AudioStream;
        let mut message = vec![0u8; AUDIO_HEADER_SIZE + AUDIO_MAX_BLOCK_FRAMES as usize * 2 * 4];
        message[0] = MSG_AUDIO;
        let mut start = std::time::Instant::now();
        let mut frames_done = 0u64;
        while !AUDIO_STOP.load(Relaxed) {
            if !connected() {
                std::thread::sleep(std::time::Duration::from_millis(5));
                start = std::time::Instant::now();
                frames_done = 0;
                continue;
            }
            let due = (start.elapsed().as_secs_f64() * AUDIO_RATE) as u64;
            if due < frames_done + AUDIO_BLOCK_FRAMES {
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            // After a stall, don't burst to catch up; start again from now.
            if due - frames_done > AUDIO_MAX_BLOCK_FRAMES * 4 {
                start = std::time::Instant::now();
                frames_done = 0;
                continue;
            }
            let frames = (due - frames_done).min(AUDIO_MAX_BLOCK_FRAMES);
            let bytes = frames as usize * 2 * 4;
            let samples = &mut message[AUDIO_HEADER_SIZE..AUDIO_HEADER_SIZE + bytes];
            if AUDIO_PAUSED.load(Relaxed) {
                // While the game's AI is idle, a paused sound card still plays silence. Sending nothing instead
                // would drain the page's buffer by every pause.
                samples.fill(0);
            } else {
                if unsafe { sdl3_sys::audio::SDL_GetAudioStreamAvailable(stream) } < bytes as i32 {
                    STATS.audio_underruns.fetch_add(1, Relaxed);
                }
                let got = unsafe {
                    sdl3_sys::audio::SDL_GetAudioStreamData(
                        stream,
                        samples.as_mut_ptr() as *mut std::ffi::c_void,
                        bytes as i32,
                    )
                }
                .max(0) as usize;
                samples[got.min(bytes)..].fill(0);
            }
            message[4..8].copy_from_slice(&(frames as u32).to_le_bytes());
            send(Message::Binary(
                message[..AUDIO_HEADER_SIZE + bytes].to_vec().into(),
            ));
            frames_done += frames;
            STATS.audio_frames.fetch_add(frames, Relaxed);
        }
    }));
}

/// Before the stream is destroyed.
pub fn stop_audio() {
    AUDIO_STOP.store(true, Relaxed);
    if let Some(thread) = AUDIO_THREAD.lock().unwrap().take() {
        let _ = thread.join();
    }
}
