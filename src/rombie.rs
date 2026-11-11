//! Rombie's engine mode (docs/rombie/notes.md): gopher64 driven by one Rombie page over a localhost WebSocket, with no
//! window and no sound device. The engine build's launch modes, as pcsx2-rombie's and xemu-rombie's:
//!   (no arguments)                            register rombie-n64:// for this Windows user (HKCU), then exit
//!   --link <url>                              started by the link: serve the page with the URL's key, exit when gone
//!   --unregister                              remove the registration
//!   --key <key> [--port <p>] [--origin <o>]   development: serve pages presenting <key>; --origin also admits <o>
//!   --version                                 print the version
//! Any other command line is gopher64's own.
//!
//! Every multi-byte value is little-endian. The layouts are pcsx2-rombie's (Session.h) but for boot and pad.
//! Page to engine (binary, first byte is the type):
//!   0x01 boot       [u8 type][3 pad][u32 0][u64 ROM size][ROM name, UTF-8]   (the N64 has no BIOS to send)
//!   0x02 disc data  [u8 type][3 pad][u32 request id][bytes]                  (empty = the read failed)
//!   0x03 pad        [u8 type][u8 port][2 pad][u32 pad word]                  (port 0-3. The word as the N64 reads it:
//!                   buttons in bits 0-13 in the N64's order, d-pad right, left, down, up, Start, Z, B, A, C right,
//!                   left, down, up, R, L; then the stick's x and y as signed bytes, +y up, at most 85 either way)
//!   0x04 control    [u8 type][u8 op]                                         (1 pause, 2 resume, 3 exit)
//!   0x05 ping       [u8 type][7 pad][f64 page time, ms]
//!   0x06 save state [u8 type]                                                (answered with 0x84)
//!   0x07 load state [u8 type][7 pad][state bytes]                            (answered with state-loaded, error, or
//!                                                                             state-version: made on another version)
//!   0x08 game data  [u8 type][7 pad][blob]                                   (before boot; empty = a new game)
//!   0x09 speed      [u8 type][3 pad][f32 ratio]                              (1 = normal; more = fast-forward, to 10)
//!   0x0B lockstep   [u8 type][u8 flags][2 pad][u32 hash every][u16 year][u8 month][u8 day][u8 hour][u8 minute]
//!                   [u8 second][u8 pad]
//!                   (before boot. flags: 1 on, 2 unthrottled. On, VI N runs only once the page has sent VI N's input
//!                   with 0x0C, pad messages are ignored, and what could differ between two PCs is pinned: the cart's
//!                   clock starts at the time given and reads as UTC, and the RNG gopher64 times its PI and SI
//!                   interrupts with starts from a fixed seed. Fast-forward is ignored. Unthrottled runs as fast as the
//!                   input comes, for tests; 2 alone keeps it for a later join. Every <hash every> VIs (0 = never) the
//!                   engine sends 0x86. VI 0 is the first after the boot.)
//!   0x0C inputs     [u8 type][3 pad][u32 first VI][u32 count][count x 4 ports x u32 pad word, as 0x03's]
//!   0x0D netplay    [u8 type][u8 op][2 pad][u32 VI][u32 hash every][state bytes, op 2 only]
//!                   (a running game, done at its next VI and never while paused, so every PC does it at the same
//!                   point, between two instructions. Op 1 host: send the game data if the game changed it (0x85) and
//!                   the state (0x84), then load that state back, as the guests do. Op 2 join: load the state that
//!                   follows, one this engine version made (state-version otherwise). Both then go into lockstep from
//!                   <VI> as 0x0B describes, the clock as UTC, hashing every <hash every> VIs. Op 3 end: lockstep off,
//!                   and 0x03 pads play again. Each answers with the event "netplay", detail host, join or end.)
//! Engine to page (binary):
//!   0x81 disc read  [u8 type][3 pad][u32 request id][u64 offset][u32 length]  (the ROM, read whole before the boot)
//!   0x82 frame      [u8 type][3 pad][u32 frame no][f64 engine time, ms][u16 w][u16 h][u32 vi no][RGBA]
//!   0x83 audio      [u8 type][3 pad][u32 frames][f32 stereo, interleaved, 48 kHz]
//!   0x84 state      [u8 type][7 pad][state bytes]
//!   0x85 game data  [u8 type][7 pad][blob]   (a second after the game last wrote its saves, before a pause, after a
//!                                            state loads, and on exit ahead of "stopped")
//!   0x86 hashes     [u8 type][3 pad][u32 VI][u32 count][count x u64]  (lockstep: FxHash of the emulated state at the
//!                   start of that VI, before its input: RDRAM, CPU, RSP, RCP registers, PIF, cart, saves, caches, RNG)
//!   0x87 VI         [u8 type][3 pad][u32 VI]                          (lockstep: that VI is next, and runs once its
//!                                                                      input is in)
//! Engine to page (text, JSON): hello {"t":"hello","engine","version","protocol","gopher64"}, event
//! {"t":"event","what","detail"} (booted, paused, resumed, speed, state-loaded, state-version, netplay, stopped, error),
//! stats (once a second; "lw" is the ms it waited for lockstep input, "fs" for the GPU's full syncs in lockstep, "idle"
//! the ms the speed limiter slept, "hash" the ms spent hashing), and pong {"t":"pong","c":<page time>,"e":<engine
//! time>}.
//!
//! The game data blob is the game's one stored object: "RN64", u32 version 1, then u32 lengths of the EEPROM, SRAM,
//! FlashRAM and the four Controller Paks (each 0 or its full size: 2 KB, 32 KB, 128 KB, 4 x 32 KB), then those bytes.
//! The cart's save is in it as the game's save type has it. A state is "RN64STAT", u32 version 1, the engine version
//! and gopher64's build as u8-length strings, then gopher64's own state (its zip of the device, the saves with the
//! Controller Paks, and parallel-rdp's state).
//!
//! One game per process: it boots when the page's boot message comes, and the process ends when the game stops.
use crate::{device, ui};
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

pub const VERSION: &str = env!("ROMBIE_N64_ENGINE_VERSION");
const PROTOCOL: u32 = 1;
const ENGINE_PORT: u16 = 47654;
const LINK_SCHEME: &str = "rombie-n64";
const ALLOWED_ORIGINS: [&str; 4] = [
    "https://rombie.app",
    "https://staged.rombie.app",
    "http://localhost:5173",
    "http://127.0.0.1:5173",
];
/// How long a link-started engine waits for its page, and how long a running game outlives its page so a reload can
/// come back to it.
const STARTUP_WAIT_S: f64 = 60.0;
const RECONNECT_GRACE_S: f64 = 10.0;
/// A page that stops reading this long is frozen or gone, even with its connection open.
const STALLED_SEND_S: u64 = 10;
const DISC_CHUNK: u64 = 1 << 20;
const DISC_WAIT_S: u64 = 30;
const MIN_ROM_SIZE: u64 = 0x1000;
const MAX_ROM_SIZE: u64 = 64 << 20;
const MAX_SPEED: f32 = 10.0;
/// While fast-forwarding, about one frame per display refresh: the page only shows the newest.
const FAST_FRAME_INTERVAL_MS: f64 = 1000.0 / 60.0 - 2.0;

const MSG_BOOT: u8 = 0x01;
const MSG_DISC_DATA: u8 = 0x02;
const MSG_PAD: u8 = 0x03;
const MSG_CONTROL: u8 = 0x04;
const MSG_PING: u8 = 0x05;
const MSG_SAVE_STATE: u8 = 0x06;
const MSG_LOAD_STATE: u8 = 0x07;
const MSG_GAME_DATA: u8 = 0x08;
const MSG_SPEED: u8 = 0x09;
const MSG_LOCKSTEP: u8 = 0x0B;
const MSG_INPUTS: u8 = 0x0C;
const MSG_NETPLAY: u8 = 0x0D;
const MSG_DISC_READ: u8 = 0x81;
const MSG_FRAME: u8 = 0x82;
const MSG_AUDIO: u8 = 0x83;
const MSG_STATE: u8 = 0x84;
const MSG_GAME_DATA_SAVED: u8 = 0x85;
const MSG_HASHES: u8 = 0x86;
const MSG_VI: u8 = 0x87;
const LOCKSTEP_ON: u8 = 1;
const LOCKSTEP_UNTHROTTLED: u8 = 2;
const NETPLAY_HOST: u8 = 1;
const NETPLAY_JOIN: u8 = 2;
const NETPLAY_END: u8 = 3;
/// The RNG's seed in lockstep from boot: any fixed value does, the same on every PC.
const LOCKSTEP_SEED: u64 = 0x524F_4D42_4945_3634;
const GAME_DATA_MAGIC: &[u8; 4] = b"RN64";
const GAME_DATA_VERSION: u32 = 1;
const GAME_DATA_HEADER_SIZE: usize = 24;
const STATE_MAGIC: &[u8; 8] = b"RN64STAT";
const STATE_VERSION: u32 = 1;
const CONTROL_PAUSE: u8 = 1;
const CONTROL_RESUME: u8 = 2;
const CONTROL_EXIT: u8 = 3;
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

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Link,
    Dev,
}

struct Engine {
    dev_origin: Option<String>,
    dir: std::path::PathBuf,
}

static ENGINE: std::sync::OnceLock<Engine> = std::sync::OnceLock::new();
static START: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);
static KEY: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
/// When the engine last lost its page (or started), and how long it may stay without one.
static IDLE: std::sync::Mutex<Option<(std::time::Instant, f64)>> = std::sync::Mutex::new(None);
static CONNECTED: AtomicBool = AtomicBool::new(false);
static BOOTED: AtomicBool = AtomicBool::new(false);
static OUT: std::sync::Mutex<Option<Outbox>> = std::sync::Mutex::new(None);
static BOOT: std::sync::Mutex<Option<(u64, String)>> = std::sync::Mutex::new(None);
static GAME_NAME: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DISC_REPLY: std::sync::Mutex<Option<(u32, std::sync::mpsc::Sender<Vec<u8>>)>> =
    std::sync::Mutex::new(None);
static PADS: [AtomicU32; 4] = [const { AtomicU32::new(0) }; 4];
static SPEED_BITS: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0
static PAUSE_WANTED: AtomicBool = AtomicBool::new(false);
static EXIT_WANTED: AtomicBool = AtomicBool::new(false);
static EMU_PAUSED: AtomicBool = AtomicBool::new(false);
static FRAME_NO: AtomicU32 = AtomicU32::new(0);
static VI_NO: AtomicU32 = AtomicU32::new(0);
static LAST_FRAME_MS: AtomicU64 = AtomicU64::new(0);
static AUDIO_PAUSED: AtomicBool = AtomicBool::new(true);
static AUDIO_STOP: AtomicBool = AtomicBool::new(false);
static AUDIO_THREAD: std::sync::Mutex<Option<std::thread::JoinHandle<()>>> =
    std::sync::Mutex::new(None);
/// The page's game data for the boot (None: a new game), or that what it sent didn't parse.
static GAME_DATA: std::sync::Mutex<Option<GameData>> = std::sync::Mutex::new(None);
static GAME_DATA_BAD: AtomicBool = AtomicBool::new(false);
static SAVE_STATE_WANTED: AtomicBool = AtomicBool::new(false);
static STATE_TO_LOAD: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
/// The state being loaded, handed to gopher64's loader, and the engine version that made it.
static LOADING_STATE: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
static LOADING_STATE_VERSION: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static STATE_STARTED: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
/// Lockstep: each VI waits for the page's input for it. LOCKSTEP_VI is the next VI's number.
static LOCKSTEP: AtomicBool = AtomicBool::new(false);
static UNTHROTTLED: AtomicBool = AtomicBool::new(false);
static LOCKSTEP_EVERY: AtomicU32 = AtomicU32::new(0);
static LOCKSTEP_VI: AtomicU32 = AtomicU32::new(0);
/// From 0x0B before the boot: lockstep from the first VI, with the cart clock starting here (Unix seconds, as UTC).
static LOCKSTEP_BOOT: std::sync::Mutex<Option<i64>> = std::sync::Mutex::new(None);
static UTC_CLOCK: AtomicBool = AtomicBool::new(false);
static INPUTS: std::sync::Mutex<std::collections::BTreeMap<u32, [u32; 4]>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());
static INPUTS_IN: std::sync::Condvar = std::sync::Condvar::new();
static NETPLAY_OP: std::sync::Mutex<Option<NetplayOp>> = std::sync::Mutex::new(None);
/// While hosting, the state the host saved, kept to load it back.
static HOSTING: AtomicBool = AtomicBool::new(false);
static HOST_STATE: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);
/// A netplay load answers with "netplay", not "state-loaded", and sends no game data: the saves are the host's.
static NETPLAY_LOAD: AtomicBool = AtomicBool::new(false);
static LOAD_OK: AtomicBool = AtomicBool::new(false);

struct NetplayOp {
    op: u8,
    vi: u32,
    every: u32,
    state: Vec<u8>,
}

/// The page's two queues: frames and sound, which may be dropped, and everything else, which the writer sends first.
struct Outbox {
    media: tokio::sync::mpsc::Sender<Message>,
    control: tokio::sync::mpsc::UnboundedSender<Message>,
}

struct GameData {
    eeprom: Vec<u8>,
    sram: Vec<u8>,
    flash: Vec<u8>,
    mempak: Vec<u8>,
}

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
    lockstep_wait_us: AtomicU64,
    full_sync_us: AtomicU64,
    idle_us: AtomicU64,
    hash_us: AtomicU64,
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
    lockstep_wait_us: AtomicU64::new(0),
    full_sync_us: AtomicU64::new(0),
    idle_us: AtomicU64::new(0),
    hash_us: AtomicU64::new(0),
};

pub fn active() -> bool {
    ENGINE.get().is_some()
}

fn connected() -> bool {
    CONNECTED.load(Relaxed)
}

/// The engine's own folders, so it never shares settings or saves with a gopher64 the player installed themselves.
pub fn dirs() -> Option<ui::Dirs> {
    let dir = &ENGINE.get()?.dir;
    Some(ui::Dirs {
        config_dir: dir.join("config"),
        data_dir: dir.join("data"),
        cache_dir: dir.join("cache"),
    })
}

fn now_ms() -> f64 {
    START.elapsed().as_secs_f64() * 1000.0
}

fn add_timing(sum: &AtomicU64, max: &AtomicU64, started: std::time::Instant) {
    let us = started.elapsed().as_micros() as u64;
    sum.fetch_add(us, Relaxed);
    max.fetch_max(us, Relaxed);
}

/// Frames and sound: dropped when the page falls behind, so it gets the newer ones rather than a growing delay.
fn send_media(message: Message) -> bool {
    match OUT.lock().unwrap().as_ref() {
        Some(out) => out.media.try_send(message).is_ok(),
        None => false,
    }
}

/// Everything else, never dropped and in order: events, states, game data, disc reads.
fn send(message: Message) {
    if let Some(out) = OUT.lock().unwrap().as_ref() {
        let _ = out.control.send(message);
    }
}

fn send_event(what: &str, detail: &str) {
    let detail: String = detail
        .chars()
        .flat_map(|c| match c {
            '"' | '\\' => vec!['\\', c],
            c if (c as u32) < 0x20 => vec![' '],
            c => vec![c],
        })
        .collect();
    send(Message::Text(
        format!(r#"{{"t":"event","what":"{what}","detail":"{detail}"}}"#).into(),
    ));
}

fn mark_idle(limit_s: f64) {
    *IDLE.lock().unwrap() = Some((std::time::Instant::now(), limit_s));
}

fn idle_overrun() -> bool {
    match *IDLE.lock().unwrap() {
        Some((since, limit_s)) => since.elapsed().as_secs_f64() > limit_s,
        None => false,
    }
}

fn speed() -> f32 {
    if LOCKSTEP.load(Relaxed) {
        return 1.0;
    }
    f32::from_bits(SPEED_BITS.load(Relaxed))
}

/// The VI pace's divisor: more than 1 while fast-forwarding.
pub fn speed_ratio() -> f64 {
    speed() as f64
}

/// Whether the game should go on: not once the page asked it to stop, or its page stayed away past the grace.
pub fn keep_running() -> bool {
    !EXIT_WANTED.load(Relaxed) && (connected() || !idle_overrun())
}

// ---------------------------------------------------------------------------------------------------------------------
// Launch modes

/// The engine build's own launch modes. None leaves the command line to gopher64; Some is the exit code.
pub fn main() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        return Some(match register() {
            Ok(()) => {
                report_info(
                    "The N64 engine is set up.\n\nYou don't need to start it yourself: Rombie starts it whenever you play \
                     an N64 game. If you move this folder, run the engine once more from its new place.",
                );
                0
            }
            Err(err) => {
                report_error(
                    &format!("The N64 engine couldn't set itself up: {err}"),
                    false,
                );
                1
            }
        });
    }

    let mut mode = None;
    let mut key = String::new();
    let mut port = ENGINE_PORT;
    let mut dev_origin = None;
    let mut i = 1;
    while i < args.len() {
        let value = args.get(i + 1).cloned();
        match (args[i].as_str(), value) {
            ("--version", _) => {
                println!("{VERSION}");
                return Some(0);
            }
            ("--unregister", _) => {
                return Some(if unregister() {
                    report_info("The N64 engine's link is removed. You can delete its folder now.");
                    0
                } else {
                    report_error("The N64 engine's link couldn't be removed.", false);
                    1
                });
            }
            ("--link", Some(url)) => {
                mode = Some(Mode::Link);
                key = query_param(&url, "key");
                if !valid_key(&key) {
                    report_error(
                        "Rombie sent the N64 engine a start link it doesn't understand. Update the N64 engine, or \
                         start the game again.",
                        false,
                    );
                    return Some(1);
                }
                i += 1;
            }
            ("--key", Some(value)) => {
                mode = Some(Mode::Dev);
                key = value;
                i += 1;
            }
            ("--port", Some(value)) => {
                let Some(value) = value.parse().ok().filter(|p| *p > 0) else {
                    report_error("Invalid port.", true);
                    return Some(1);
                };
                port = value;
                i += 1;
            }
            ("--origin", Some(value)) => {
                dev_origin = Some(value);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    let mode = mode?;
    if mode == Mode::Dev && key.len() < 8 {
        report_error("A --key of at least 8 characters is required.", true);
        return Some(1);
    }
    // The link always uses the one port Rombie knows, and only Rombie's own origins.
    if mode == Mode::Link {
        port = ENGINE_PORT;
        dev_origin = None;
    }

    let Some(local) = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from) else {
        report_error(
            "Can't find this user's local app data folder.",
            mode == Mode::Dev,
        );
        return Some(1);
    };
    let dir = if mode == Mode::Dev && port != ENGINE_PORT {
        local.join(format!("RombieN64-{port}"))
    } else {
        local.join("RombieN64")
    };
    for sub in ["config", "data", "cache"] {
        if std::fs::create_dir_all(dir.join(sub)).is_err() {
            report_error(
                &format!("Can't create {}.", dir.display()),
                mode == Mode::Dev,
            );
            return Some(1);
        }
    }
    *KEY.lock().unwrap() = key;
    let _ = ENGINE.set(Engine { dev_origin, dir });
    let _ = *START;
    Some(run(mode, port))
}

fn valid_key(key: &str) -> bool {
    (16..=128).contains(&key.len())
        && key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn query_param(url: &str, name: &str) -> String {
    let Some((_, query)) = url.split_once('?') else {
        return String::new();
    };
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        .unwrap_or_default()
        .to_string()
}

fn run(mode: Mode, port: u16) -> i32 {
    let listener = match listen(port) {
        Ok(listener) => listener,
        Err(err) => {
            // An engine of ours already holds the port: an idle one takes this link's key and serves the page; a busy
            // one tells the page so. Either way this process is done.
            if mode == Mode::Link && hand_off_key(port) {
                return 0;
            }
            eprintln!("rombie: can't listen on 127.0.0.1:{port}: {err}");
            report_error(
                &format!(
                    "The N64 engine needs port {port} on this computer, but another program is using it. Close that \
                     program, then start the game in Rombie again."
                ),
                mode == Mode::Dev,
            );
            return 2;
        }
    };
    if mode == Mode::Link {
        // One log per run, beside the engine's other files: the first place to look when a player reports a problem.
        // Only the engine that holds the port writes it, so a start that hands off its key leaves it alone.
        redirect_stderr(&ENGINE.get().unwrap().dir.join("engine.log"));
    }
    eprintln!(
        "Rombie N64 engine {VERSION} (gopher64 {})",
        env!("GIT_DESCRIBE")
    );
    eprintln!("rombie: listening on 127.0.0.1:{port} (protocol {PROTOCOL})");
    mark_idle(STARTUP_WAIT_S);
    tokio::spawn(serve(listener));

    let game = loop {
        if EXIT_WANTED.load(Relaxed) {
            break None;
        }
        if mode == Mode::Link && !connected() && idle_overrun() {
            eprintln!("rombie: no page; exiting");
            return 0;
        }
        let boot = BOOT.lock().unwrap().take();
        let Some((size, name)) = boot else {
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        };
        match read_rom(size) {
            Ok(rom) => break Some((rom, name)),
            Err(err) => send_event("error", &err),
        }
    };

    if let Some((rom, name)) = game {
        eprintln!("rombie: booting '{name}' ({} bytes)", rom.len());
        *GAME_NAME.lock().unwrap() = name.clone();
        BOOTED.store(true, Relaxed);
        send_event("booted", &name);
        play(&rom);
        eprintln!("rombie: the game stopped");
    }
    send_event("stopped", "");
    send(Message::Close(Some(
        tokio_tungstenite::tungstenite::protocol::CloseFrame {
            code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
            reason: "stopped".into(),
        },
    )));
    // Let the writer hand the last messages to the page, whose answer to the close ends the session, before the process
    // goes.
    let started = std::time::Instant::now();
    while connected() && started.elapsed() < std::time::Duration::from_secs(2) {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    0
}

/// Reads the ROM from the page whole, in chunks, and puts its byte order right.
fn read_rom(size: u64) -> Result<Vec<u8>, String> {
    let mut rom = Vec::with_capacity(size as usize);
    let mut id = 0u32;
    while (rom.len() as u64) < size {
        if !connected() {
            return Err("The page left while the engine read the game.".into());
        }
        let offset = rom.len() as u64;
        let length = DISC_CHUNK.min(size - offset) as u32;
        id += 1;
        let (tx, rx) = std::sync::mpsc::channel();
        *DISC_REPLY.lock().unwrap() = Some((id, tx));
        let mut request = vec![0u8; 20];
        request[0] = MSG_DISC_READ;
        request[4..8].copy_from_slice(&id.to_le_bytes());
        request[8..16].copy_from_slice(&offset.to_le_bytes());
        request[16..20].copy_from_slice(&length.to_le_bytes());
        send(Message::Binary(request.into()));
        match rx.recv_timeout(std::time::Duration::from_secs(DISC_WAIT_S)) {
            Ok(bytes) if bytes.len() == length as usize => rom.extend_from_slice(&bytes),
            _ => return Err("Reading the game from the page failed.".into()),
        }
    }
    DISC_REPLY.lock().unwrap().take();
    device::swap_rom(rom).ok_or_else(|| "That file isn't an N64 game (.z64, .n64 or .v64).".into())
}

fn play(rom: &[u8]) {
    let mut device = device::Device::new(true);
    // What the engine always is, whatever the config says: all four ports with a Controller Pak each, and the
    // Expansion Pak. ui::storage writes no saves in engine mode.
    let config = &mut device.ui.config;
    config.input.controller_enabled = [true; 4];
    config.input.transfer_pak = [false; 4];
    config.input.emulate_vru = false;
    config.emulation.usb = false;
    config.emulation.rewind = false;
    device.ui.video.fullscreen = false;
    device.ui.video.widescreen = false;
    device::run_game(
        &mut device,
        rom,
        ui::GameSettings {
            overclock: false,
            disable_expansion_pak: false,
            cheats: Default::default(),
            load_savestate_slot: None,
        },
        crate::retroachievements::RAConfig::default(),
        None,
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Connections

fn listen(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    let socket = tokio::net::TcpSocket::new_v4()?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        // Nobody else may share the port, not even a socket that asked to.
        win::exclusive_address(socket.as_raw_socket());
    }
    socket.bind(std::net::SocketAddr::from(([127, 0, 0, 1], port)))?;
    socket.listen(4)
}

/// Another engine holds the port. If it has no page, it takes this link's key over a plain HTTP request a browser
/// can't forge: the custom header would need a CORS preflight, which the engine never answers.
fn hand_off_key(port: u16) -> bool {
    use std::io::{Read, Write};
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(3))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
    let request = format!(
        "GET /handoff?key={} HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Rombie-Handoff: 1\r\nConnection: close\r\n\r\n",
        KEY.lock().unwrap()
    );
    let mut reply = [0u8; 64];
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let n = stream.read(&mut reply).unwrap_or(0);
    // 204: an idle engine took the key. 409: it's busy, and the page hears so from it.
    reply[..n].starts_with(b"HTTP/1.1 204") || reply[..n].starts_with(b"HTTP/1.1 409")
}

async fn serve(listener: tokio::net::TcpListener) {
    loop {
        if let Ok((stream, _)) = listener.accept().await {
            let _ = stream.set_nodelay(true);
            tokio::spawn(connection(stream));
        }
    }
}

struct Request {
    path: String,
    origin: String,
    ws_key: String,
    upgrade: bool,
    handoff: bool,
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<Request> {
    let mut raw = Vec::new();
    while !raw.ends_with(b"\r\n\r\n") {
        if raw.len() >= 8192 {
            return None;
        }
        raw.push(stream.read_u8().await.ok()?);
    }
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let mut lines = raw.split("\r\n");
    let path = lines
        .next()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?
        .to_string();
    let mut request = Request {
        path,
        origin: String::new(),
        ws_key: String::new(),
        upgrade: false,
        handoff: false,
    };
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().to_string();
        match name.trim().to_ascii_lowercase().as_str() {
            "origin" => request.origin = value,
            "sec-websocket-key" => request.ws_key = value,
            "upgrade" => request.upgrade = value.eq_ignore_ascii_case("websocket"),
            "x-rombie-handoff" => request.handoff = true,
            _ => {}
        }
    }
    Some(request)
}

#[derive(Clone, Copy)]
enum Decision {
    Status(u16),
    Refuse(u16, &'static str),
    Accept,
}

/// Who may connect: only Rombie's own pages, only with the current key, and only one at a time. Refusals a page should
/// understand complete the handshake and close with a code, since a failed handshake tells page script nothing: 4001
/// the key isn't this engine's, 4002 another page is playing, 4003 the game is stopping (try again in a moment).
fn decide(request: &Request) -> Decision {
    let engine = ENGINE.get().unwrap();
    if request.handoff {
        if !request.origin.is_empty() {
            return Decision::Status(403);
        }
        if connected() {
            return Decision::Status(409);
        }
        let key = query_param(&request.path, "key");
        if !valid_key(&key) {
            return Decision::Status(400);
        }
        *KEY.lock().unwrap() = key;
        mark_idle(STARTUP_WAIT_S);
        eprintln!("rombie: took the key from a newer link");
        return Decision::Status(204);
    }
    let allowed = ALLOWED_ORIGINS.contains(&request.origin.as_str())
        || engine.dev_origin.as_deref() == Some(request.origin.as_str());
    if !request.upgrade || request.ws_key.is_empty() || !allowed {
        eprintln!("rombie: refused origin '{}'", request.origin);
        return Decision::Status(403);
    }
    if BOOTED.load(Relaxed) && !keep_running() {
        return Decision::Refuse(4003, "stopping");
    }
    if query_param(&request.path, "key") != *KEY.lock().unwrap() {
        eprintln!("rombie: refused a stale key from '{}'", request.origin);
        return Decision::Refuse(4001, "key");
    }
    if CONNECTED
        .compare_exchange(false, true, Relaxed, Relaxed)
        .is_err()
    {
        return Decision::Refuse(4002, "busy");
    }
    Decision::Accept
}

async fn connection(mut stream: tokio::net::TcpStream) {
    let Ok(Some(request)) =
        tokio::time::timeout(std::time::Duration::from_secs(5), read_request(&mut stream)).await
    else {
        return;
    };
    let decision = decide(&request);
    if let Decision::Status(status) = decision {
        let _ = stream
            .write_all(
                format!("HTTP/1.1 {status} \r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
                    .as_bytes(),
            )
            .await;
        return;
    }
    let accept =
        tokio_tungstenite::tungstenite::handshake::derive_accept_key(request.ws_key.as_bytes());
    let upgrade = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: \
         {accept}\r\n\r\n"
    );
    let upgraded = stream.write_all(upgrade.as_bytes()).await.is_ok();
    if upgraded {
        let mut ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
            stream,
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        )
        .await;
        match decision {
            Decision::Refuse(code, reason) => {
                let _ = ws
                    .close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: code.into(),
                        reason: reason.into(),
                    }))
                    .await;
                return;
            }
            Decision::Accept => {
                eprintln!("rombie: page connected from '{}'", request.origin);
                session(ws).await;
                eprintln!("rombie: page disconnected");
            }
            Decision::Status(_) => unreachable!(),
        }
    }
    if matches!(decision, Decision::Accept) {
        CONNECTED.store(false, Relaxed);
    }
}

/// One page, from its first message to its last.
async fn session(ws: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>) {
    let (mut sink, mut source) = ws.split();
    let (media_tx, mut media_rx) = tokio::sync::mpsc::channel::<Message>(SEND_QUEUE);
    let (control_tx, mut control_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let _ = control_tx.send(Message::Text(
        format!(
            r#"{{"t":"hello","engine":"gopher64-rombie","version":"{VERSION}","protocol":{PROTOCOL},"gopher64":"{}"}}"#,
            env!("GIT_DESCRIBE")
        )
        .into(),
    ));
    let stats_tx = control_tx.clone();
    *OUT.lock().unwrap() = Some(Outbox {
        media: media_tx,
        control: control_tx,
    });
    if BOOTED.load(Relaxed) {
        send_event("booted", &GAME_NAME.lock().unwrap().clone());
    }

    let writer = async move {
        loop {
            let message = tokio::select! {
                biased;
                message = control_rx.recv() => message,
                message = media_rx.recv() => message,
            };
            let Some(message) = message else {
                break;
            };
            let frame = matches!(&message, Message::Binary(b) if b.first() == Some(&MSG_FRAME));
            let started = std::time::Instant::now();
            match tokio::time::timeout(
                std::time::Duration::from_secs(STALLED_SEND_S),
                sink.send(message),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(_)) => break,
                Err(_) => {
                    eprintln!("rombie: the page stopped reading; dropping it");
                    break;
                }
            }
            if frame {
                STATS.sent.fetch_add(1, Relaxed);
                add_timing(&STATS.send_us, &STATS.send_max_us, started);
            }
        }
    };
    let stats = async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        interval.tick().await;
        let mut last = std::time::Instant::now();
        loop {
            interval.tick().await;
            let now = std::time::Instant::now();
            let _ = stats_tx.send(Message::Text(take_stats_json(now - last).into()));
            last = now;
        }
    };
    let reader = async move {
        while let Some(Ok(message)) = source.next().await {
            match message {
                Message::Binary(data) => handle_message(&data),
                Message::Close(_) => break,
                _ => {}
            }
        }
    };
    tokio::select! {
        _ = writer => {}
        _ = stats => {}
        _ = reader => {}
    }

    OUT.lock().unwrap().take();
    DISC_REPLY.lock().unwrap().take();
    for pad in &PADS {
        pad.store(0, Relaxed);
    }
    // A page that comes back (a reload) starts at normal speed.
    SPEED_BITS.store(1.0f32.to_bits(), Relaxed);
    mark_idle(if BOOTED.load(Relaxed) {
        RECONNECT_GRACE_S
    } else {
        STARTUP_WAIT_S
    });
}

fn handle_message(data: &[u8]) {
    match data.first() {
        Some(&MSG_BOOT) if data.len() >= 16 => {
            let size = u64::from_le_bytes(data[8..16].try_into().unwrap());
            let name = String::from_utf8_lossy(&data[16..]).into_owned();
            if BOOTED.load(Relaxed) || BOOT.lock().unwrap().is_some() {
                send_event("error", "A game is already running in this engine.");
            } else if GAME_DATA_BAD.load(Relaxed) {
                // Booting with fresh saves instead would send those back as the game's saves.
                send_event(
                    "error",
                    "Boot failed: the game's stored data isn't N64 engine game data.",
                );
            } else if !(MIN_ROM_SIZE..=MAX_ROM_SIZE).contains(&size) {
                send_event("error", "That file is the wrong size for an N64 game.");
            } else {
                *BOOT.lock().unwrap() = Some((size, name));
            }
        }
        Some(&MSG_DISC_DATA) if data.len() >= 8 => {
            let id = u32::from_le_bytes(data[4..8].try_into().unwrap());
            let mut reply = DISC_REPLY.lock().unwrap();
            if reply.as_ref().is_some_and(|(want, _)| *want == id) {
                let (_, tx) = reply.take().unwrap();
                let _ = tx.send(data[8..].to_vec());
            }
        }
        Some(&MSG_PAD) if data.len() >= 8 && data[1] < 4 && !LOCKSTEP.load(Relaxed) => {
            let word = u32::from_le_bytes(data[4..8].try_into().unwrap());
            PADS[data[1] as usize].store(pad_word(word), Relaxed);
        }
        Some(&MSG_LOCKSTEP) if data.len() >= 16 => {
            if BOOTED.load(Relaxed) || BOOT.lock().unwrap().is_some() {
                send_event("error", "Lockstep comes before the boot.");
                return;
            }
            let flags = data[1];
            let year = u16::from_le_bytes(data[8..10].try_into().unwrap());
            let clock =
                chrono::NaiveDate::from_ymd_opt(year as i32, data[10] as u32, data[11] as u32)
                    .and_then(|date| {
                        date.and_hms_opt(data[12] as u32, data[13] as u32, data[14] as u32)
                    })
                    .map(|time| time.and_utc().timestamp());
            UNTHROTTLED.store(flags & LOCKSTEP_UNTHROTTLED != 0, Relaxed);
            *LOCKSTEP_BOOT.lock().unwrap() = None;
            if flags & LOCKSTEP_ON != 0 {
                match clock {
                    Some(clock) => {
                        *LOCKSTEP_BOOT.lock().unwrap() = Some(clock);
                        LOCKSTEP_EVERY
                            .store(u32::from_le_bytes(data[4..8].try_into().unwrap()), Relaxed);
                    }
                    None => send_event("error", "That lockstep clock isn't a date and time."),
                }
            }
        }
        Some(&MSG_INPUTS) if data.len() >= 12 => {
            let first = u32::from_le_bytes(data[4..8].try_into().unwrap());
            let count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
            if data.len() != 12 + count * 16 {
                return;
            }
            let mut inputs = INPUTS.lock().unwrap();
            for (i, frame) in data[12..].as_chunks::<16>().0.iter().enumerate() {
                let mut pads = [0u32; 4];
                for (pad, word) in pads.iter_mut().zip(frame.as_chunks::<4>().0) {
                    *pad = pad_word(u32::from_le_bytes(*word));
                }
                inputs.insert(first.wrapping_add(i as u32), pads);
            }
            INPUTS_IN.notify_all();
        }
        Some(&MSG_NETPLAY) if data.len() >= 12 => {
            let op = data[1];
            if !BOOTED.load(Relaxed) {
                send_event("error", "Netplay needs a running game.");
            } else if matches!(op, NETPLAY_HOST | NETPLAY_END)
                || (op == NETPLAY_JOIN && data.len() > 12)
            {
                *NETPLAY_OP.lock().unwrap() = Some(NetplayOp {
                    op,
                    vi: u32::from_le_bytes(data[4..8].try_into().unwrap()),
                    every: u32::from_le_bytes(data[8..12].try_into().unwrap()),
                    state: data[12..].to_vec(),
                });
            }
        }
        Some(&MSG_CONTROL) if data.len() >= 2 => match data[1] {
            CONTROL_PAUSE => PAUSE_WANTED.store(true, Relaxed),
            CONTROL_RESUME => PAUSE_WANTED.store(false, Relaxed),
            CONTROL_EXIT => EXIT_WANTED.store(true, Relaxed),
            _ => {}
        },
        Some(&MSG_PING) if data.len() >= 16 => {
            let page_ms = f64::from_le_bytes(data[8..16].try_into().unwrap());
            send(Message::Text(
                format!(r#"{{"t":"pong","c":{page_ms:.3},"e":{:.3}}}"#, now_ms()).into(),
            ));
        }
        Some(&MSG_SPEED) if data.len() >= 8 => {
            let ratio = f32::from_le_bytes(data[4..8].try_into().unwrap());
            // NaN and anything up to 1 are normal speed.
            let speed = if ratio > 1.0 {
                ratio.min(MAX_SPEED)
            } else {
                1.0
            };
            SPEED_BITS.store(speed.to_bits(), Relaxed);
            send_event("speed", &speed.to_string());
        }
        Some(&MSG_GAME_DATA) if data.len() >= 8 => {
            if BOOTED.load(Relaxed) {
                send_event("error", "The game data comes before the boot.");
            } else if data.len() == 8 {
                *GAME_DATA.lock().unwrap() = None;
                GAME_DATA_BAD.store(false, Relaxed);
            } else {
                let parsed = parse_game_data(&data[8..]);
                GAME_DATA_BAD.store(parsed.is_none(), Relaxed);
                if parsed.is_none() {
                    send_event(
                        "error",
                        "The game's stored data isn't N64 engine game data.",
                    );
                }
                *GAME_DATA.lock().unwrap() = parsed;
            }
        }
        Some(&MSG_SAVE_STATE) if BOOTED.load(Relaxed) => {
            *STATE_STARTED.lock().unwrap() = Some(std::time::Instant::now());
            SAVE_STATE_WANTED.store(true, Relaxed);
        }
        Some(&MSG_LOAD_STATE) if data.len() > 8 && BOOTED.load(Relaxed) => {
            *STATE_STARTED.lock().unwrap() = Some(std::time::Instant::now());
            *STATE_TO_LOAD.lock().unwrap() = Some(data[8..].to_vec());
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
        r#"{{"t":"stats","ms":{:.0},"vi":{},"grab":{},"sent":{},"drop":{},"w":{},"h":{},"hz":{:.3},"speed":{},"rb":{:.2},"rbMax":{:.2},"send":{:.2},"sendMax":{:.2},"af":{},"au":{},"lw":{:.1},"fs":{:.1},"idle":{:.1},"hash":{:.1}}}"#,
        window.as_secs_f64() * 1000.0,
        STATS.vis.swap(0, Relaxed),
        grabbed,
        sent,
        STATS.dropped.swap(0, Relaxed),
        STATS.width.load(Relaxed),
        STATS.height.load(Relaxed),
        STATS.hz_milli.load(Relaxed) as f64 / 1000.0,
        speed(),
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
        STATS.lockstep_wait_us.swap(0, Relaxed) as f64 / 1000.0,
        STATS.full_sync_us.swap(0, Relaxed) as f64 / 1000.0,
        STATS.idle_us.swap(0, Relaxed) as f64 / 1000.0,
        STATS.hash_us.swap(0, Relaxed) as f64 / 1000.0,
    )
}

/// The stats' `"fs"`: time the game waited in lockstep for the GPU to finish a full sync.
pub fn add_full_sync_time(started: std::time::Instant) {
    STATS
        .full_sync_us
        .fetch_add(started.elapsed().as_micros() as u64, Relaxed);
}

/// The stats' `"idle"`: time the speed limiter slept, which is how far ahead of the clock the PC is.
pub fn add_idle_time(slept: std::time::Duration) {
    STATS.idle_us.fetch_add(slept.as_micros() as u64, Relaxed);
}

// ---------------------------------------------------------------------------------------------------------------------
// The emulation thread's side

/// The port's pad word, as the page last sent it.
pub fn pad(port: usize) -> u32 {
    PADS[port].load(Relaxed)
}

/// A page's pad word with only the N64's buttons, and the stick no further than a real one goes.
fn pad_word(word: u32) -> u32 {
    let stick = |v: u32| (v as u8 as i8).clamp(-MAX_STICK, MAX_STICK) as u8 as u32;
    (word & PAD_BUTTONS) | stick(word >> 16) << 16 | stick(word >> 24) << 24
}

/// gopher64's own netplay shares these two between its PCs; lockstep from boot pins them instead.
pub fn pin_rng_rtc(rng: &mut rand::rngs::Xoshiro256PlusPlus, rtc_timestamp: &mut i64) {
    use rand::SeedableRng;
    if let Some(clock) = *LOCKSTEP_BOOT.lock().unwrap() {
        *rng = rand::rngs::Xoshiro256PlusPlus::seed_from_u64(LOCKSTEP_SEED);
        *rtc_timestamp = clock;
        LOCKSTEP_VI.store(0, Relaxed);
        UTC_CLOCK.store(true, Relaxed);
        LOCKSTEP.store(true, Relaxed);
    }
}

/// In lockstep the cart's clock reads as UTC, the same on every PC whatever its time zone.
pub fn utc_clock() -> bool {
    UTC_CLOCK.load(Relaxed)
}

pub fn lockstep() -> bool {
    LOCKSTEP.load(Relaxed)
}

/// Lockstep for tests: as fast as the input comes, with no pacing.
pub fn unthrottled() -> bool {
    LOCKSTEP.load(Relaxed) && UNTHROTTLED.load(Relaxed)
}

/// At the start of each VI, between two instructions, before the game sees it: a netplay op waiting for it, then in
/// lockstep the state's hashes, the VI announced, and its input, waited for.
pub fn at_vi_start(device: &mut device::Device) {
    let op = NETPLAY_OP.lock().unwrap().take();
    if let Some(op) = op {
        netplay(device, op);
    }
    if !LOCKSTEP.load(Relaxed) {
        return;
    }
    let vi = LOCKSTEP_VI.fetch_add(1, Relaxed);
    let every = LOCKSTEP_EVERY.load(Relaxed);
    if every != 0 && vi.is_multiple_of(every) {
        let started = std::time::Instant::now();
        // The RDP writes RDRAM on the GPU, behind the game; what it was given has to be in before RDRAM is hashed.
        ui::video::idle();
        let hashes = hash_state(device);
        STATS
            .hash_us
            .fetch_add(started.elapsed().as_micros() as u64, Relaxed);
        if vi / every < 3 {
            dump_rdram(device, vi);
        }
        let mut message = vec![0u8; 12];
        message[0] = MSG_HASHES;
        message[4..8].copy_from_slice(&vi.to_le_bytes());
        message[8..12].copy_from_slice(&(hashes.len() as u32).to_le_bytes());
        for hash in hashes {
            message.extend_from_slice(&hash.to_le_bytes());
        }
        send(Message::Binary(message.into()));
    }
    let mut message = vec![0u8; 8];
    message[0] = MSG_VI;
    message[4..8].copy_from_slice(&vi.to_le_bytes());
    send(Message::Binary(message.into()));

    let started = std::time::Instant::now();
    let mut inputs = INPUTS.lock().unwrap();
    let pads = loop {
        if let Some(pads) = inputs.remove(&vi) {
            break Some(pads);
        }
        if !keep_running() {
            break None;
        }
        inputs = INPUTS_IN
            .wait_timeout(inputs, std::time::Duration::from_millis(5))
            .unwrap()
            .0;
    };
    drop_inputs_before(&mut inputs, vi);
    drop(inputs);
    STATS
        .lockstep_wait_us
        .fetch_add(started.elapsed().as_micros() as u64, Relaxed);
    if let Some(pads) = pads {
        for (port, pad) in pads.into_iter().enumerate() {
            PADS[port].store(pad, Relaxed);
        }
    }
}

/// A development flag for finding where two PCs' RDRAM differs: with `ROMBIE_DUMP_RDRAM` naming a folder, the first
/// three hashed VIs' RDRAM goes there as `<game>-<VI>.bin`.
fn dump_rdram(device: &device::Device, vi: u32) {
    let Some(folder) = std::env::var_os("ROMBIE_DUMP_RDRAM") else {
        return;
    };
    let name = format!("{}-{vi}.bin", ui::storage::get_game_name(&device.cart.rom));
    let path = std::path::Path::new(&folder).join(name);
    if let Err(e) = std::fs::write(&path, &device.rdram.mem) {
        eprintln!("rombie: couldn't write {}: {e}", path.display());
    }
}

fn drop_inputs_before(inputs: &mut std::collections::BTreeMap<u32, [u32; 4]>, vi: u32) {
    while inputs
        .first_key_value()
        .is_some_and(|(&first, _)| first < vi)
    {
        inputs.pop_first();
    }
}

fn netplay(device: &mut device::Device, op: NetplayOp) {
    match op.op {
        NETPLAY_END => {
            LOCKSTEP.store(false, Relaxed);
            INPUTS.lock().unwrap().clear();
            send_event("netplay", "end");
            return;
        }
        NETPLAY_HOST => {
            send_game_data(&mut device.ui.storage, false);
            *STATE_STARTED.lock().unwrap() = Some(std::time::Instant::now());
            HOST_STATE.lock().unwrap().take();
            HOSTING.store(true, Relaxed);
            crate::savestates::create_savestate(device, false, None);
            // The state is compressed on another thread; the game waits here for it.
            let started = std::time::Instant::now();
            let state = loop {
                if let Some(state) = HOST_STATE.lock().unwrap().take() {
                    break state;
                }
                if started.elapsed() > std::time::Duration::from_secs(30) {
                    break Vec::new();
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            };
            HOSTING.store(false, Relaxed);
            if state.is_empty() || !load_netplay_state(device, state) {
                send_event(
                    "error",
                    "Hosting failed: the state didn't save and load back.",
                );
                return;
            }
        }
        NETPLAY_JOIN => match parse_state(&op.state) {
            Some((version, _)) if version != VERSION => {
                send_event("state-version", &version);
                return;
            }
            Some((_, state)) => {
                if !load_netplay_state(device, state) {
                    send_event("error", "Joining failed: the state didn't load.");
                    return;
                }
            }
            None => {
                send_event("error", "That isn't an N64 engine save state.");
                return;
            }
        },
        _ => return,
    }
    LOCKSTEP_VI.store(op.vi, Relaxed);
    LOCKSTEP_EVERY.store(op.every, Relaxed);
    drop_inputs_before(&mut INPUTS.lock().unwrap(), op.vi);
    UTC_CLOCK.store(true, Relaxed);
    LOCKSTEP.store(true, Relaxed);
    send_event(
        "netplay",
        if op.op == NETPLAY_HOST {
            "host"
        } else {
            "join"
        },
    );
}

fn load_netplay_state(device: &mut device::Device, state: Vec<u8>) -> bool {
    *LOADING_STATE_VERSION.lock().unwrap() = VERSION.to_string();
    *LOADING_STATE.lock().unwrap() = Some(state);
    LOAD_OK.store(false, Relaxed);
    NETPLAY_LOAD.store(true, Relaxed);
    ui::video::reset_rdp();
    crate::savestates::load_savestate(device, false, None);
    NETPLAY_LOAD.store(false, Relaxed);
    LOAD_OK.load(Relaxed)
}

/// The emulated state in parts, for comparing PCs. The TLB's lookup maps are left out: they follow from the TLB, and
/// their order depends on how they grew, which differs after a state loads.
fn hash_state(device: &mut device::Device) -> [u64; 9] {
    use std::hash::Hasher;
    fn hash(parts: &[&[u8]]) -> u64 {
        let mut hasher = rustc_hash::FxHasher::default();
        for part in parts {
            hasher.write(part);
        }
        hasher.finish()
    }
    fn serialized<T: serde::Serialize>(value: &T) -> u64 {
        hash(&[&postcard::to_stdvec(value).unwrap_or_default()])
    }
    let lut_r = std::mem::take(&mut device.cpu.cop0.tlb_lut_r);
    let lut_w = std::mem::take(&mut device.cpu.cop0.tlb_lut_w);
    let cpu = serialized(&device.cpu);
    device.cpu.cop0.tlb_lut_r = lut_r;
    device.cpu.cop0.tlb_lut_w = lut_w;
    let saves = &device.ui.storage.saves;
    [
        hash(&[&device.rdram.mem]),
        cpu,
        serialized(&device.rsp),
        serialized(&(
            &device.rdp,
            &device.mi,
            &device.pi,
            &device.vi,
            &device.ai,
            &device.si,
            &device.ri,
            &device.rdram.regs,
        )),
        serialized(&device.pif),
        serialized(&device.cart),
        hash(&[
            &saves.eeprom.data,
            &saves.sram.data,
            &saves.flash.data,
            &saves.mempak.data,
        ]),
        serialized(&device.memory),
        serialized(&device.rng),
    ]
}

/// At each VI, after the frame: serves the page's save state requests, and holds the game while the page has it paused,
/// serving them meanwhile. True if it paused, so the pace starts again from now.
pub fn at_vi(device: &mut device::Device) -> bool {
    serve_states(device);
    if !PAUSE_WANTED.load(Relaxed) || !keep_running() {
        return false;
    }
    // A save the game wrote just before the pause shouldn't wait for the game to resume.
    ui::storage::write_saves(device);
    EMU_PAUSED.store(true, Relaxed);
    send_event("paused", "");
    while PAUSE_WANTED.load(Relaxed) && keep_running() {
        serve_states(device);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    EMU_PAUSED.store(false, Relaxed);
    if keep_running() {
        send_event("resumed", "");
    }
    true
}

/// gopher64's own state code, at a VI rather than where its hotkeys run it (a VI origin write), so a paused game can
/// save and load too.
fn serve_states(device: &mut device::Device) {
    if SAVE_STATE_WANTED.swap(false, Relaxed) {
        crate::savestates::create_savestate(device, false, None);
    }
    let load = STATE_TO_LOAD.lock().unwrap().take();
    if let Some(bytes) = load {
        match parse_state(&bytes) {
            // gopher64's state isn't self-describing: one from a build whose device layout differs can load with
            // wrong values and no error. So only the engine version that made a state loads it, as on the Xbox.
            Some((version, _)) if version != VERSION => {
                state_ms();
                send_event("state-version", &version);
            }
            Some((version, state)) => {
                *LOADING_STATE_VERSION.lock().unwrap() = version;
                *LOADING_STATE.lock().unwrap() = Some(state);
                crate::savestates::load_savestate(device, false, None);
            }
            None => send_event("error", "That isn't an N64 engine save state."),
        }
    }
}

fn parse_state(bytes: &[u8]) -> Option<(String, Vec<u8>)> {
    let rest = bytes.strip_prefix(STATE_MAGIC)?;
    if u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) != STATE_VERSION {
        return None;
    }
    let mut at = 4;
    let mut texts = Vec::new();
    for _ in 0..2 {
        let len = *rest.get(at)? as usize;
        texts.push(String::from_utf8_lossy(rest.get(at + 1..at + 1 + len)?).into_owned());
        at += 1 + len;
    }
    Some((texts.swap_remove(0), rest[at..].to_vec()))
}

fn state_ms() -> u128 {
    STATE_STARTED
        .lock()
        .unwrap()
        .take()
        .map_or(0, |started| started.elapsed().as_millis())
}

/// From gopher64's state code: the state it made, sent to the page behind the engine's header.
pub fn state_saved(state: Option<Vec<u8>>) {
    let Some(state) = state else {
        if HOSTING.load(Relaxed) {
            *HOST_STATE.lock().unwrap() = Some(Vec::new());
        }
        send_event("error", "Saving the state failed.");
        return;
    };
    if HOSTING.load(Relaxed) {
        *HOST_STATE.lock().unwrap() = Some(state.clone());
    }
    let mut message = vec![0u8; 8];
    message[0] = MSG_STATE;
    message.extend_from_slice(STATE_MAGIC);
    message.extend_from_slice(&STATE_VERSION.to_le_bytes());
    for text in [VERSION, env!("GIT_DESCRIBE")] {
        message.push(text.len() as u8);
        message.extend_from_slice(text.as_bytes());
    }
    message.extend_from_slice(&state);
    eprintln!(
        "rombie: state saved, {} bytes in {} ms",
        message.len() - 8,
        state_ms()
    );
    send(Message::Binary(message.into()));
}

/// For gopher64's state loader, in place of its file: the state the page sent.
pub fn take_state() -> Option<Vec<u8>> {
    LOADING_STATE.lock().unwrap().take()
}

/// From gopher64's state loader. A state carries the saves, so the page gets the game data the engine now holds.
pub fn state_loaded(loaded: bool, storage: &mut ui::Storage) {
    LOAD_OK.store(loaded, Relaxed);
    if NETPLAY_LOAD.load(Relaxed) {
        eprintln!(
            "rombie: netplay state loaded ({loaded}) in {} ms",
            state_ms()
        );
        return;
    }
    let version = LOADING_STATE_VERSION.lock().unwrap().clone();
    if loaded {
        eprintln!("rombie: state loaded in {} ms", state_ms());
        send_event("state-loaded", &version);
        send_game_data(storage, true);
    } else {
        state_ms();
        send_event(
            "error",
            &format!(
                "The save state didn't load. It's from N64 engine {version}, and this is {VERSION}."
            ),
        );
    }
}

fn parse_game_data(blob: &[u8]) -> Option<GameData> {
    if blob.len() < GAME_DATA_HEADER_SIZE
        || &blob[..4] != GAME_DATA_MAGIC
        || u32::from_le_bytes(blob[4..8].try_into().unwrap()) != GAME_DATA_VERSION
    {
        return None;
    }
    let sizes = [
        device::cart::EEPROM_MAX_SIZE,
        device::cart::sram::SRAM_SIZE,
        device::cart::sram::FLASHRAM_SIZE,
        device::controller::mempak::MEMPAK_SIZE * 4,
    ];
    let mut parts = Vec::new();
    let mut at = GAME_DATA_HEADER_SIZE;
    for (i, size) in sizes.iter().enumerate() {
        let len = u32::from_le_bytes(blob[8 + i * 4..12 + i * 4].try_into().unwrap()) as usize;
        if len != 0 && len != *size {
            return None;
        }
        parts.push(blob.get(at..at + len)?.to_vec());
        at += len;
    }
    if at != blob.len() {
        return None;
    }
    let mut parts = parts.into_iter();
    Some(GameData {
        eeprom: parts.next()?,
        sram: parts.next()?,
        flash: parts.next()?,
        mempak: parts.next()?,
    })
}

/// In place of gopher64's save files: the page's game data, before gopher64 formats what's missing.
pub fn load_game_data(saves: &mut ui::storage::Saves) {
    if let Some(data) = GAME_DATA.lock().unwrap().take() {
        saves.eeprom.data = data.eeprom;
        saves.sram.data = data.sram;
        saves.flash.data = data.flash;
        saves.mempak.data = data.mempak;
    }
}

/// In place of writing gopher64's save files: the game data to the page, if the game wrote any (or always, when
/// forced). It holds the cart's save as the game's save type has it, and the four Controller Paks.
pub fn send_game_data(storage: &mut ui::Storage, force: bool) {
    let saves = &mut storage.saves;
    let pending = saves.eeprom.write_pending
        || saves.sram.write_pending
        || saves.flash.write_pending
        || saves.mempak.write_pending;
    if !pending && !force {
        return;
    }
    let types = &storage.save_type;
    let has = |kinds: &[ui::storage::SaveTypes]| kinds.iter().any(|kind| types.contains(kind));
    let parts: [&[u8]; 4] = [
        if has(&[
            ui::storage::SaveTypes::Eeprom4k,
            ui::storage::SaveTypes::Eeprom16k,
        ]) {
            &saves.eeprom.data
        } else {
            &[]
        },
        if has(&[ui::storage::SaveTypes::Sram]) {
            &saves.sram.data
        } else {
            &[]
        },
        if has(&[ui::storage::SaveTypes::Flash]) {
            &saves.flash.data
        } else {
            &[]
        },
        &saves.mempak.data,
    ];
    let mut message = vec![0u8; 8];
    message[0] = MSG_GAME_DATA_SAVED;
    message.extend_from_slice(GAME_DATA_MAGIC);
    message.extend_from_slice(&GAME_DATA_VERSION.to_le_bytes());
    for part in parts {
        message.extend_from_slice(&(part.len() as u32).to_le_bytes());
    }
    for part in parts {
        message.extend_from_slice(part);
    }
    saves.eeprom.write_pending = false;
    saves.sram.write_pending = false;
    saves.flash.write_pending = false;
    saves.mempak.write_pending = false;
    eprintln!("rombie: game data sent, {} bytes", message.len() - 8);
    send(Message::Binary(message.into()));
}

/// At each VI: the moment the windowed build would draw the frame. Reads it back and queues it.
pub fn send_frame(frame_time: f64) {
    STATS.vis.fetch_add(1, Relaxed);
    STATS.hz_milli.store((1000.0 / frame_time) as u32, Relaxed);
    let vi_no = VI_NO.fetch_add(1, Relaxed) + 1;
    if !connected() {
        return;
    }
    let ready_ms = now_ms();
    if speed() > 1.0
        && ready_ms - f64::from_bits(LAST_FRAME_MS.load(Relaxed)) < FAST_FRAME_INTERVAL_MS
    {
        return;
    }
    LAST_FRAME_MS.store(ready_ms.to_bits(), Relaxed);
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
    if !send_media(Message::Binary(message.into())) {
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
            // With no page, or the game paused by it, nothing plays; start again from then.
            if !connected() || EMU_PAUSED.load(Relaxed) {
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
            send_media(Message::Binary(
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

// ---------------------------------------------------------------------------------------------------------------------
// Windows: the link's registration, message boxes, the exclusive port and the log

/// Errors go to a message box, except for development runs, which have a console.
fn report_error(message: &str, dev: bool) {
    eprintln!("{message}");
    if !dev {
        win::message_box(message, true);
    }
}

fn report_info(message: &str) {
    win::message_box(message, false);
}

fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let exe = exe.display();
    win::register_link(
        LINK_SCHEME,
        "URL:Rombie N64 Engine",
        &format!("\"{exe}\",0"),
        &format!("\"{exe}\" --link \"%1\""),
    )
}

fn unregister() -> bool {
    win::unregister_link(LINK_SCHEME)
}

fn redirect_stderr(path: &std::path::Path) {
    win::redirect_stderr(path);
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const KEY_WRITE: u32 = 0x20006;
    const REG_SZ: u32 = 1;
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const MB_ICONERROR: u32 = 0x10;
    const MB_ICONINFORMATION: u32 = 0x40;
    const SOL_SOCKET: i32 = 0xFFFF;
    const SO_EXCLUSIVEADDRUSE: i32 = !4;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const IONBF: i32 = 0x0004;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegCreateKeyExW(
            key: isize,
            subkey: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            sam: u32,
            security: *const c_void,
            result: *mut isize,
            disposition: *mut u32,
        ) -> i32;
        fn RegSetKeyValueW(
            key: isize,
            subkey: *const u16,
            name: *const u16,
            kind: u32,
            data: *const c_void,
            size: u32,
        ) -> i32;
        fn RegCloseKey(key: isize) -> i32;
        fn RegDeleteTreeW(key: isize, subkey: *const u16) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(window: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    #[link(name = "ws2_32")]
    unsafe extern "system" {
        fn setsockopt(socket: usize, level: i32, name: i32, value: *const u8, size: i32) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetStdHandle(which: u32, handle: isize) -> i32;
    }
    unsafe extern "C" {
        fn __acrt_iob_func(index: u32) -> *mut c_void;
        fn _wfreopen(path: *const u16, mode: *const u16, stream: *mut c_void) -> *mut c_void;
        fn setvbuf(stream: *mut c_void, buffer: *mut u8, mode: i32, size: usize) -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn message_box(text: &str, error: bool) {
        let kind = if error {
            MB_ICONERROR
        } else {
            MB_ICONINFORMATION
        };
        unsafe {
            MessageBoxW(
                0,
                wide(text).as_ptr(),
                wide("Rombie N64 Engine").as_ptr(),
                kind,
            )
        };
    }

    pub fn register_link(
        scheme: &str,
        description: &str,
        icon: &str,
        command: &str,
    ) -> Result<(), String> {
        let mut key = 0isize;
        let path = wide(&format!("Software\\Classes\\{scheme}"));
        let mut status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_WRITE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if status == 0 {
            let set = |subkey: Option<&str>, name: Option<&str>, value: &str| {
                let subkey = subkey.map(wide);
                let name = name.map(wide);
                let value = wide(value);
                unsafe {
                    RegSetKeyValueW(
                        key,
                        subkey.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                        name.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                        REG_SZ,
                        value.as_ptr() as *const c_void,
                        (value.len() * 2) as u32,
                    )
                }
            };
            for (subkey, name, value) in [
                (None, None, description),
                (None, Some("URL Protocol"), ""),
                (Some("DefaultIcon"), None, icon),
                (Some("shell\\open\\command"), None, command),
            ] {
                if status == 0 {
                    status = set(subkey, name, value);
                }
            }
            unsafe { RegCloseKey(key) };
        }
        if status != 0 {
            return Err(format!("Windows refused the change (error {status})."));
        }
        Ok(())
    }

    pub fn unregister_link(scheme: &str) -> bool {
        let path = wide(&format!("Software\\Classes\\{scheme}"));
        let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, path.as_ptr()) };
        status == 0 || status == ERROR_FILE_NOT_FOUND
    }

    pub fn exclusive_address(socket: u64) {
        let on: u32 = 1;
        unsafe {
            setsockopt(
                socket as usize,
                SOL_SOCKET,
                SO_EXCLUSIVEADDRUSE,
                &on as *const u32 as *const u8,
                4,
            )
        };
    }

    /// The engine has no console. Rust's and the C runtime's stderr (parallel-rdp's log names the GPU) both go to the
    /// file, appended so neither overwrites the other.
    pub fn redirect_stderr(path: &std::path::Path) {
        use std::os::windows::io::IntoRawHandle;
        if std::fs::File::create(path).is_err() {
            return;
        }
        if let Ok(file) = std::fs::OpenOptions::new().append(true).open(path) {
            unsafe { SetStdHandle(STD_ERROR_HANDLE, file.into_raw_handle() as isize) };
        }
        let path: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let stderr = __acrt_iob_func(2);
            if !_wfreopen(path.as_ptr(), wide("a").as_ptr(), stderr).is_null() {
                setvbuf(stderr, std::ptr::null_mut(), IONBF, 0);
            }
        }
    }
}

#[cfg(not(windows))]
mod win {
    pub fn message_box(_text: &str, _error: bool) {}

    pub fn register_link(
        _scheme: &str,
        _description: &str,
        _icon: &str,
        _command: &str,
    ) -> Result<(), String> {
        Err("The Rombie N64 engine is Windows-only.".into())
    }

    pub fn unregister_link(_scheme: &str) -> bool {
        false
    }

    pub fn redirect_stderr(_path: &std::path::Path) {}
}
