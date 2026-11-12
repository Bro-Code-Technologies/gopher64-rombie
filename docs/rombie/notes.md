# Rombie N64 engine: notes

The `rombie` branch of this fork builds the Rombie N64 Engine, a windowless Windows program that Rombie's web page
drives over a localhost WebSocket, as `pcsx2-rombie` and `xemu-rombie` do for PS2 and Xbox. The plan it follows is
Rombie's `.docs/plans/n64-local-engine.md` (BRAIN-37). Nothing here is sent upstream.

## Pinned source

Pinned on 2026-11-10 (plan Phase 1).

| What | Commit | Date |
|---|---|---|
| gopher64 `v1.1.42` (the newest release tag; the `rombie` branch starts here) | `f05daaf9a8009399b0b5fd891e017db18539d0f4` | 2026-10-15 |
| `parallel-rdp/parallel-rdp-standalone` (gopher64's fork of parallel-rdp) | `5d0ab119bdd69e745396e8659a0f141f622bcb92` | 2026-10-13 |
| `retroachievements/rcheevos` | `1433173220a7eaede6a9ed7a18e94117be1821e0` | 2026-09-13 |
| `src/compat/sse2neon` (aarch64 only) | `b26afbb44df55ac05320f7003a8a0eef30e465c5` | 2026-03-29 |
| `android-project/libadrenotools` (Android only) | `1c49a786bb0442f5192d983eb30138aad878c74b` | 2026-10-04 |

`upstream` is `https://github.com/gopher64/gopher64.git`.

## Changes

Bro Code Technologies' changes to gopher64 for Rombie, by date, as GPL-3.0 section 5a asks of a modified version. The
README's notice at the top points here. Each change is in its own section below.

| Date | Commit | Change |
|---|---|---|
| 2026-11-10 | `77ed00e` | These notes: the pinned source, the Windows build and the license scan (plan Phase 1) |
| 2026-11-10 | `f14a41f` | The PIF boot ROM removed for an HLE boot; netplay, RetroAchievements, Discord, web requests, TLS and the cheat list left out of the engine build behind the `upstream-extras` feature; the speed log (Phase 2) |
| 2026-11-10 | `d0e81f7` | Engine mode with no window: parallel-rdp on a Vulkan device with no surface, frames read back and sent with 48 kHz sound to a page over a localhost WebSocket, the page's pads on all four ports (Phase 3) |
| 2026-11-10 | `eb3eda0` | The engine: launch modes and the `rombie-n64://` link, guarded connections, the boot from the page's ROM, pause, resume, exit and fast-forward, its own folder and log, no saves written, the version resource, and the release zip (Phase 4) |
| 2026-11-10 | `10d1b24` and the commit after it | The Visual C++ runtime in the zip, the full source archive, the README's notice, and `build.rs` reading `rombie/GIT_DESCRIBE` in the archive (Phase 4.1) |
| 2026-11-10 | `e74ef10` and the commit after it | Game data and save states to and from the page, states only on the engine version that made them, and a separate queue for frames and sound (Phase 5) |
| 2026-11-10 | the commit after `249c7b8` | With no page, the frame context flushes the RDP first, so a game whose page left stops instead of hanging (Phase 5) |
| 2026-11-11 | the commit after `2f54a66` | Lockstep and netplay for Rombie's page: the page's input for each VI, the RNG and the cart clock pinned, full syncs waited for, the state's hashes, and a new RDP before a netplay load (Phase 6) |
| 2026-11-11 | the commit after `a301767` | The log names the GPU in use (Phase 6) |
| 2026-11-11 | the commit after `38ab61f` | The full-sync wait, the speed limiter's sleep and the hashing timed in the stats, and RDRAM dumps for the lockstep checks (Phase 6) |
| 2026-11-11 | the commit after `7c2d8b3` | Frames read back on their own thread while the game runs on, the frame context's wait in the stats, and no subgroups on Intel GPUs (Phase 6) |

## Building (Windows)

From upstream's `.github/workflows/build.yml` (`build-windows`), confirmed by the first builds on 2026-11-10 (plan
Phase 2):

- Rust as `rust-toolchain.toml` pins it (1.99.0), target `x86_64-pc-windows-msvc`, through rustup.
- LLVM and Clang at the LLVM version `rustc --version --verbose` reports (23.1.1 for Rust 1.99.0), from the LLVM
  release's `LLVM-<version>-win64.msi`. An administrative extract (`msiexec /a <msi> /qn TARGETDIR=<folder>`) is enough;
  nothing needs installing. Its `bin` goes first on `PATH`.
- Visual Studio 2022 with the C++ workload, for the MSVC libraries, the Windows SDK and CMake: SDL is built with CMake's
  Visual Studio generator. Visual Studio's own CMake (`Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin`) does.
- Environment:
  - `CC` and `CXX` set to `clang-cl`: `build.rs` passes Clang flags (`-flto=thin`, `-march=x86-64-v3`).
  - `LIBCLANG_PATH` set to LLVM's `bin`, for `bindgen`. Upstream's CI gets it from its LLVM install action.
  - Only where Visual Studio's default v143 toolset (`VC\Auxiliary\Build\Microsoft.VCToolsVersion.v143.default.txt`)
    names an MSVC without its x64 libraries, as on the main PC (14.38 without them, 14.43 complete): `VCToolsVersion`
    set to the complete one, which MSBuild reads. Otherwise SDL's CMake step stops at "No CMAKE_C_COMPILER could be
    found".
- A short path for the checkout. SDL's build folders under `target\` run deep, and past Windows' 260-character limit
  MSBuild's FileTracker can't write its logs, which also ends in "No CMAKE_C_COMPILER could be found".
- `.cargo/config.toml` links with `rust-lld` and builds for `x86-64-v3`, so the engine needs a CPU with AVX2.
- The engine build: `cargo build --release --no-default-features --target=x86_64-pc-windows-msvc`, which gives
  `target\x86_64-pc-windows-msvc\release\gopher64.exe` (14.7 MB). From a fresh clone of `f14a41f` with its submodules,
  it took 2 minutes 6 seconds on the main PC (Ryzen 9 5950X).
- From a release's source archive (`rombie-n64-engine-<version>-source.zip`): unzip it, set `CARGO_TARGET_DIR` to a
  short folder (the archive's own folder name makes the paths under `target\` too long for MSBuild even in a short
  place), then the same build with `--offline`. Its `.cargo/config.toml` points cargo at the archive's `vendor`
  folder, and `rombie/GIT_DESCRIBE` stands in for `git describe`, as the archive has no `.git`. From
  `rombie-n64-engine-2026.0.0-source.zip` of `10d1b24`, unzipped into an empty folder, it built in 1 minute 53 seconds
  with nothing downloaded, and that engine booted Ocarina of Time on the test page.

## What the engine build leaves out (plan Phase 2)

`Cargo.toml`'s `upstream-extras` feature holds what Phase 1 listed for leaving out. `gui` turns it on, so upstream's
default build is unchanged. The engine build (`--no-default-features`) has none of it:

- gopher64's netplay, `ggrs` and `matchbox_socket` with their WebRTC and TLS crates. `src/netplay_off.rs` stands in for
  `src/netplay.rs`: its `Netplay` can't be constructed, so `device.netplay` is always `None`.
- RetroAchievements (rcheevos, which `build.rs` compiles only with the feature), with Discord presence and their web
  requests. `src/retroachievements_off.rs` stands in for `src/retroachievements.rs`, with achievements never enabled.
  parallel-rdp's F8 key (in-progress achievements) is behind `GOPHER64_UPSTREAM_EXTRAS`.
- `reqwest`, `rustls`, and the TLS in `tokio-tungstenite`, which itself stays for the engine's own server.
- The cheat list, `data/cheats.json`. The engine build finds no cheats.

gopher64's `tokio` dependency now names the features its own code uses (`sync`, `macros`, `time`, `net`, `io-util`).
The left-out crates used to turn them on for it. SDL_ttf with FreeType and Roboto Mono stay, since gopher64's window
draws its on-screen text with them, and the engine build still opens that window for development runs
(`gopher64.exe <rom>`). Engine mode draws no text.

The engine build compiles 139 crates (the table below), down from Phase 1's 382, and none of the left-out ones. Its exe
holds no cheat text and no URL but error-message links (clap, libbzip2, PIX).

## Boot without the PIF ROM (plan Phase 2)

`src/device/pif/rom.rs`, Nintendo's PIF boot ROM (see the findings below), is gone. `pif::boot_hle` in
`src/device/pif.rs` leaves the CPU and the RCP as the boot ROM would and starts the cart's own IPL3 in DMEM. It follows
mupen64plus-core's `src/device/pif/bootrom_hle.c` ("Copyright (C) 2016 Bobby Smiles", GPL-2.0-or-later) at
`a9ca8ec0b7decd17390c76c6341cf423d5d79321` (master, 2026-11-06; the file last changed on 2023-04-07). It runs at power-on
(`device::run_game`, after `cpu::init`) and after a soft reset (`exceptions::reset_event`). Reads of the boot ROM's
addresses return 0, as a locked-out ROM does.

It keeps one piece of Nintendo's code, as `bootrom_hle.c` does: the 8 instruction words (32 bytes) that IPL2 leaves at
the start of IMEM. CIC-6105 carts (Ocarina of Time, Majora's Mask, Donkey Kong 64 and Perfect Dark among the test games)
don't boot without them, and the check is on the bytes themselves. With one of them replaced by an equivalent
instruction (`addiu` by `ori`), Ocarina of Time stays on a black screen. The user decided on 2026-11-10 to keep them
(Rombie's `.docs/licenses.md`, legal question 14).

## Speed log

With `GOPHER64_ROMBIE_SPEED_LOG` naming a file, the engine writes a line to it for each finished frame
(`f <microseconds> <frame time in ns>`) and each audio underrun (`u <microseconds> <bytes>`: SDL asked for more sound
than was queued, and got silence). Unset, it does nothing. Phase 2's speed table is worked out from it.

## Engine mode (plan Phases 3 and 4)

`src/rombie.rs` holds it, and its header lists every message. Phase 3 built it as a spike started by an environment
variable; Phase 4 made it the engine below, and the variable is gone. Upstream's default build never enters it.

### Launch modes

The engine build (`--no-default-features`) reads its command line before gopher64 does (`main.rs`, then
`rombie::main`), as pcsx2-rombie's and xemu-rombie's engines do:

```
(no arguments)                            first run: register rombie-n64:// for this Windows user (HKCU, no admin), then exit
--link <url>                              started by the link: serve the page with that URL's key, exit when it's gone
--unregister                              remove the registration
--key <key> [--port <p>] [--origin <o>]   development: serve pages presenting <key>; --origin also admits pages from <o>
--version                                 print the version (rombie/VERSION)
```

Any other command line is gopher64's own, so `gopher64.exe <rom>` still opens gopher64's window for development runs
like Phase 2's speed test.

- **The registration** is `HKCU\Software\Classes\rombie-n64`, with `shell\open\command` = `"<exe>" --link "%1"`.
  Running the exe again from a new folder points it there. Browsers show the exe's version resource name, "Rombie N64
  Engine" (`build.rs`, the engine build only), in their prompt.
- **Who may connect.** The engine listens on 127.0.0.1:47654 only, with `SO_EXCLUSIVEADDRUSE`. A page must come from
  `https://rombie.app`, `https://staged.rombie.app` or the dev server (`http://localhost:5173`,
  `http://127.0.0.1:5173`), or from `--origin` in development; anything else, or no `Origin`, gets a 403. It must
  present the current key. One page at a time. Refusals a page should understand complete the handshake and close at
  once with a code: 4001 the key isn't this engine's, 4002 another page is playing, 4003 the game is stopping.
- **Started on demand.** Rombie opens `rombie-n64://start?key=<random key>`, and Windows runs the engine with
  `--link <url>`. If an engine already holds the port, the new one hands it the key (`GET /handoff` with an
  `X-Rombie-Handoff` header, which a web page can't send without a CORS preflight the engine never answers) and exits.
  An idle engine takes it (204); a busy one says 409, and the page hears 4002 from it.
- **Lifetime.** A link-started engine exits 60 seconds after starting if no page connects, and a running game ends 10
  seconds after its page goes, so a reload can come back to it. A page that stops reading for 10 seconds is dropped.
  One game per process: the process ends when the game stops.
- **Its own folders.** `%LOCALAPPDATA%\RombieN64` (`RombieN64-<port>` for a development engine on another port), with
  gopher64's `config` there, so it never shares settings with a gopher64 the player installed. A link start writes
  `engine.log` there, both Rust's and the C runtime's stderr, so parallel-rdp's line naming the GPU is in it. Only the
  engine holding the port writes it.
- **No windows.** The exe is a Windows-subsystem app, so a link start shows nothing. Setup and errors use message
  boxes; a development run prints errors instead.

### Where the protocol differs from pcsx2-rombie's

- **Boot** (`0x01`) has no BIOS: `[u8 type][3 pad][u32 0][u64 ROM size][ROM name]`. The ROM must be 4 KB to 64 MB.
- **The ROM** is read whole before the boot, with the disc messages (`0x81`, `0x02`) in 1 MB chunks, and kept in memory
  only. `device::swap_rom` puts a `.v64`'s or an `.n64`'s byte order right, and a file it doesn't recognize gets the
  event "error" and no boot.
- **Pad** (`0x03`) is the port's pad word as the N64 reads it: buttons in bits 0-13 in the N64's order, then the
  stick's x and y as signed bytes, +y up, clamped to 85 either way.
- **Pong** is pcsx2-rombie's: `{"t":"pong","c":<page time>,"e":<engine time>}`.
- **Events**: booted (with the ROM's name, sent again to a page that comes back), paused, resumed, speed,
  state-loaded, state-version, netplay, stopped and error. After "stopped" the engine closes the connection normally
  (1000, "stopped") and exits.
- **Queues**: frames and sound go in a small queue that drops them when the page falls behind; everything else
  (events, states, game data, disc reads) in an unbounded one the writer serves first, so nothing of it is lost.
- **Lockstep** (below) counts VIs where pcsx2-rombie counts vsyncs. `0x0C` carries four pad words a VI, as `0x03`
  has them, not PS2's buttons and four axes. `0x0D` has no multitap byte; the byte is padding. `0x86` has nine FxHash
  parts, not ten XXH3 ones.

### Game data and save states (plan Phase 5)

The message numbers are pcsx2-rombie's: `0x08` game data before boot and `0x85` from the engine, `0x06` save state
answered with `0x84`, `0x07` load state answered with the event "state-loaded" (pcsx2-rombie's), "error" or
"state-version".

- **Game data** is the game's one stored object, which the page keeps gzipped as its in-game save: `RN64`, u32
  version 1, the u32 lengths of the EEPROM, SRAM, FlashRAM and the four Controller Paks (each 0 or its full size: 2 KB,
  32 KB, 128 KB, 4 x 32 KB), then those bytes. The cart's save goes in as gopher64's save type for the game has it
  (`get_save_type`, which gives unlisted games both EEPROM and SRAM), and the Controller Paks always: 162 KB for Super
  Mario 64 or Ocarina of Time. The page sends it before boot, `ui::storage::load_saves` puts it in place of the save
  files, and gopher64's formatting fills only what's missing. The engine sends it back where gopher64 would write its
  files: a second after the game's last write (`EVENT_TYPE_SAVE`), before a pause, after a state loads, and when the
  game stops, ahead of "stopped". A blob that doesn't parse gets an "error", and so does the boot after it: booting
  with fresh saves would send those back as the game's. No blob is a new game.
- **Save states** are gopher64's own (its zip of the device, the saves with the Controller Paks, and parallel-rdp's
  state), behind a header: `RN64STAT`, u32 version 1, then the engine version and gopher64's build as u8-length
  strings. They're served at a VI, and while paused, rather than where gopher64's hotkeys run them (a VI origin write),
  so a paused game can save and load. A loaded state replaces the saves too, so the page gets the game data after it.
- **Only the engine version that made a state loads it.** gopher64's state isn't self-describing (postcard), and a
  build with one more `u32` in `Vi` loaded a state from this one with no error. Ocarina of Time even played on, but
  the fields after it were read from the wrong bytes. So a state from another engine version gets "state-version", with
  the version that made it, and isn't loaded, as the Xbox engine refuses one. Rombie shows "Made on" for such a slot.
- **Sizes and times** on the main PC, measured at the page: Super Mario 64's state 1.07 MB and Ocarina of Time's 1.72
  MB, saved in 52 to 87 ms and loaded in 30 to 54 ms, the engine's own part 25 to 49 ms.

### Lockstep and netplay (plan Phase 6)

pcsx2-rombie's messages, counted in VIs: `0x0B` before the boot puts a game in lockstep from its first VI, and `0x0D`
hosts, joins or ends netplay in a running game. `src/rombie.rs`'s header has their layouts.

- **The lockstep point** is the start of `vertical_interrupt_event` (`src/device/vi.rs`): every VI, between two
  instructions, before the game sees the VI. There `rombie::at_vi_start` runs a netplay op waiting for it, then in
  lockstep hashes the state (every Nth VI), sends `0x87` with the VI's number, and waits for that VI's input. Input for
  VIs already run is dropped. The time spent waiting goes into the stats as `"lw"`.
- **What's pinned.** gopher64's own netplay shares the RNG's seed and the cart clock's start between its PCs
  (`init_rng_rtc`); lockstep pins them instead. The RNG, which times the PI and SI interrupts, starts from a fixed seed;
  the cart clock starts at `0x0B`'s time and reads as UTC (`time2data` in `src/device/cart.rs`), so a time zone makes no
  difference. Fast-forward is ignored, and so are `0x03` pads: the only input is `0x0C`'s.
- **Unthrottled** (`0x0B`'s flag 2) skips gopher64's speed limiter, so a test replays a recording as fast as the PC
  runs it.
- **Hashes.** Nine FxHash parts, each of the device as a save state serializes it: RDRAM, CPU, RSP, RCP registers (RDP,
  MI, PI, VI, AI, SI, RI and RDRAM's), PIF, cart, saves, caches and RNG. The CPU's leaves out the TLB's lookup maps:
  they follow from the TLB, and their order depends on how they grew, which differs after a state loads. parallel-rdp
  writes RDRAM from its own thread, so the RDP is idled (`ui::video::idle`) before hashing. Without that, RDRAM hashed
  at the same VI differed between two runs.
- **Full syncs waited for.** gopher64 lets the GPU draw while the game runs on, and waits for it only when the game
  touches RDRAM its tracking marks as the RDP's (`rdp_check_framebuffers`). In the first ten-minute runs, one Mario
  Kart 64 run of three differed from the other two in RDRAM alone, at VIs 2136 to 2141, then matched again: one frame
  drawn differently, until its buffer was drawn over. The CPU and the rest matched throughout, so the game never read
  it, but a game that reads its frame buffer would have gone its own way. In lockstep, `rdp_finish_commands` waits for
  a full sync as soon as its batch of commands is in, as parallel-rdp's synchronous mode does in other emulators.
  Waiting after every batch instead ran Mario Kart 64 at 8 VIs a second flat out, 12 times slower.
- **Netplay ops** run at the VI they name, never while paused. The host sends its game data, saves a state at that VI
  and loads it again, so both start from the same bytes; the joiner loads the host's state, which must be from this engine version ("state-version"
  otherwise). Both then run in lockstep from that VI, with the clock as UTC. A netplay load sends neither
  "state-loaded" nor game data: the saves are the host's.
- **A new RDP before every netplay load.** A save state holds parallel-rdp's own state but not all of its RDP's: an
  engine that had just booted, joining a host's state, differed from the host in RDRAM from the next VI (6 of 57 VIs
  in the first try). So both sides drop parallel-rdp's command processor and make a new one
  (`rdp_reset_processor` in `parallel-rdp/interface.cpp`) before loading. After that, a replay from the state matched
  its recording at every VI. pcsx2-rombie reboots at a handoff instead; the N64 needs only the new RDP.
- **No subgroups on Intel GPUs.** On a laptop's Intel UHD Graphics 630 (Vulkan 1.2, driver 0.402.1472), RDRAM
  differed from the main PC's from a replay's first VI, in frame-buffer-sized ranges (110 KB in Mario Kart 64, 175 KB
  in Super Smash Bros., which reads its frame buffer back, so its whole state drifted). The same laptop's GTX 1060
  matched byte for byte. parallel-rdp's `PARALLEL_RDP_SUBGROUP=0` (no subgroup tile binning) made the Intel GPU match
  at every VI, and so did its ubershader at a third of the speed or less. So on an Intel GPU, `init_headless` in
  `parallel-rdp/interface.cpp` sets `PARALLEL_RDP_SUBGROUP=0` before the processor is made, unless it's set already;
  the log shows "Allow subgroups = 0". The picture there is right, not only the same as other PCs'.
- **For the checks.** The stats line also carries `"fs"`, the ms the game waited for full syncs, `"idle"`, the ms the
  speed limiter slept (how far ahead of the clock the PC is), `"fc"`, the ms it waited moving to the next frame
  context (see Frames), and `"hash"`, the ms spent hashing. With
  `ROMBIE_DUMP_RDRAM` naming a folder, the RDRAM of the first three hashed VIs goes there as `<game>-<VI>.bin`, to find
  where two PCs' RDRAM differs.
- **On the main PC** (Ryzen 9 5950X, RTX 5070 Ti), with Rombie's `engine/n64/lockstep-test.html`: a ten-minute
  recording (35,896 VIs) each of four-player Mario Kart 64 and Super Smash Bros., four players of scripted input after
  an opening played from boot. Each recording hosts netplay where its input starts, so its replays join from the
  host's state, as a netplay guest does. Replayed flat out and in real time with the full-sync wait, all nine hashes
  matched the recording's at every VI: 0 of 35,897 differ, in all four replays. Flat out, Mario Kart 64 ran at 168% of
  the VIs due (127% in its slowest 30 seconds) and Super Smash Bros. at 156% (136%). In real time both held 100%
  (99.95% and 99.94% in the slowest 30 seconds). RDRAM's hash changed at every VI, so the games were playing, not
  stuck on a screen.

### What the engine is, whatever the config says

- **No window.** `rdp_init` in `parallel-rdp/interface.cpp` takes a null window to mean engine mode: it creates
  Granite's Vulkan context and device with no surface and no WSI. With no surface to match, Granite takes the discrete
  GPU over an integrated one, so a two-GPU laptop runs on its GeForce. `GRANITE_VULKAN_DEVICE_INDEX=<n>` (Granite's
  own) picks the nth GPU in the order the log lists them, for the Intel UHD run of Phase 6. Granite's "Found Vulkan
  GPU" lines list every GPU, so the engine adds "Using Vulkan GPU" with the one it took. SDL's video and TTF aren't
  started, and on-screen messages are dropped.
- **Frames.** At each VI, where the windowed build draws, `rombie::send_frame` calls `rdp_start_frame`, which records
  parallel-rdp's scanout at the VI's own size and its copy back (`scanout_async_buffer`, the same scanout options as
  the window). A readback thread then waits for the GPU (`rdp_finish_frame`) and sends the frame as RGBA, alpha forced
  opaque, so the game runs on meanwhile. Reading it back on the game's thread (`scanout_sync`) cost a GTX 1060 laptop
  2.4 to 3.2 ms a VI, a sixth of each second, and kept it under full speed. At most three frames wait for the GPU;
  past that a frame is dropped. The engine time in the message is taken before the readback, so the page's delay
  includes it. `rdp_update_screen` moves Granite to its next frame context, as the swapchain's frame did; with two
  contexts, that waits for the VI before's GPU work, so a frame is read back by then at the latest. Before the RDP
  closes, `ui::video::close` waits for the readback thread. 1x, with no supersampling, widescreen or CRT, whatever
  the config says.
- **The frame context with no page.** `rdp_update_screen` calls parallel-rdp's `begin_frame_context`, which flushes the
  RDP's command buffer and drains its ring before Granite's `next_frame_context`. That call waits for every command
  buffer to be submitted. The readback flushes too, but with no page there is none. Then a few primitives the game sent
  since its last full sync stayed open: the RDP thread flushes on its own only past 32 primitives or 2 render passes,
  and the game was waiting at the VI. Majora's Mask hung that way when its page left, the game never stopped, and the
  engine refused every page with 4003 until it was killed.
- **Sound.** The game's audio stream converts to 48 kHz stereo float with no device behind it. A thread pulls it at
  48 kHz by the clock, as a sound card would, and sends blocks of 256 to 2048 frames, as pcsx2-rombie's audio does.
  gopher64's own pacing stays: the stream's get callback pads a shortfall with silence and nudges the rate. While the
  game's AI is idle (where gopher64 pauses its sound device), the thread keeps the clock and sends silence, as a paused
  device plays it. Stopping instead drained the page's buffer by each pause: GoldenEye's emptied in 33 seconds.
- **Pads.** All four ports are on, each with a Controller Pak, and the Expansion Pak is in. gopher64's own keyboard and
  pad input isn't opened, so nothing but the page reaches the game. gopher64 gives Chameleon Twist (`NCT`) a Rumble Pak
  instead, as it doesn't work with a Controller Pak; the engine leaves that be, and nothing vibrates.
- **Controls.** Pause holds the game at its next VI, until resume, exit or the page's grace runs out. Fast-forward
  divides the VI pace by the page's ratio (at most 10). While fast, the engine sends about one frame per display
  refresh, and gopher64's own audio queue drops what the 48 kHz pull can't take.
- **Saves.** `ui::storage::load_saves` and `write_saves` read and write no files in engine mode, so no game's save
  reaches the engine's folder: they come from the page and go back to it as the game data (below).
- **Settings** gopher64 reads from its config are set for the run: no Transfer Pak, VRU, USB, rewind or overclock,
  not fullscreen, and the picture as above.

### Packaging

`rombie/package.ps1` makes a release's two files, from a clean tree, and `rombie/RELEASING.md` says how a release goes
out. The download, `rombie-n64-engine-<version>-windows-x64.zip`, holds:

- `rombie-n64-engine.exe`;
- `msvcp140.dll`, `vcruntime140.dll` and `vcruntime140_1.dll`, the Visual C++ runtime, from Visual Studio's redist
  for its toolset (14.42.34438 now), as the PS2 engine ships them (the user's choice, 2026-11-10). The exe imports the
  first two (`dumpbin /dependents`; its `api-ms-win-crt-*` ones are Windows' own), and `msvcp140.dll` imports the
  third: without it in the zip, the engine loaded it from System32 on a PC that has the redistributable;
- gopher64's `LICENSE` as `LICENSE.txt`;
- a `README.txt` with the modification notice, the source archive named as the source, and Microsoft's terms for the
  runtime;
- `licenses\THIRD-PARTY.txt`, below.

The source, `rombie-n64-engine-<version>-source.zip` (GPL-3.0 section 6d), holds the commit's tree (`git archive`,
so no PIF arrays), every submodule at its pinned commit, the crates from `cargo vendor` for `Cargo.lock`, the
`.cargo/config.toml` lines that build from them offline, and `rombie/GIT_DESCRIBE`.

`licenses\THIRD-PARTY.txt` holds:

- the notices in `rombie/licenses/`: volk's and the Vulkan headers' (both from the submodule's headers, which carry no
  license file), X-Scale's (from mupen64plus-core's `n64_cic_nus_6105.c` at `a9ca8ec`), and Roboto Mono's, which is
  under the SIL Open Font License 1.1 by its own name table, not Apache-2.0 as the Phase 1 scan below says (its
  `OFL.txt` from github.com/googlefonts/RobotoMono);
- parallel-rdp's `LICENSE`;
- SDL (with HIDAPI's BSD licence and yuv2rgb's), SDL_ttf with FreeType's licence and the FreeType License, mimalloc and
  zstd, from the crates that build them;
- every crate `cargo tree -e normal` lists, with its own license files. All 138 have them.

## License scan (plan Phase 1)

What `cargo build --release --no-default-features` compiled for `x86_64-pc-windows-msvc` before Phase 2 left the extras
out:

- **gopher64's own code**: 59 `.rs` files under `src/`, the GUI's among them (behind the `gui` feature, so not
  compiled), plus `parallel-rdp/interface.cpp`, `parallel-rdp/wsi_platform.cpp` and the headers beside them, and
  `retroachievements/retroachievements.c`. No file carries a header of its own, so `LICENSE` covers them:
  GPL-3.0, with no "or any later version". The README says many portions are adapted from mupen64plus and ares.
  The mupen64plus-core files say "either version 2 ... or (at your option) any later version", and ares is ISC.
- **C and C++ from `build.rs`**: 73 files and the 125 in-repo headers they include.
  - parallel-rdp-standalone: MIT, "Copyright (c) 2020 Themaister".
  - volk: MIT.
  - The Vulkan headers: Apache-2.0.
  - rcheevos (29 files): MIT, "Copyright (c) 2018 RetroAchievements.org".
- **Crates**: 382 packages. The table below lists the 139 the engine build kept after Phase 2. Most are MIT or Apache-2.0. The rest are BSD-2-Clause, BSD-3-Clause,
  ISC, Zlib, Unicode-3.0, MPL-2.0, CC0-1.0, MIT-0, BSL-1.0, CDLA-Permissive-2.0 and the bzip2 licence.
  - The two with no license field are gopher64 itself and `webpki` 0.22.4 (ISC-style, its `LICENSE`).
  - These compile C into the exe:
    - SDL 3.4.18 (zlib). Its hidapi is GPL-3.0, BSD or HIDAPI's own; the BSD option is taken.
    - SDL_ttf 3.2.2 (zlib), with FreeType (the FreeType License or GPL-2.0-or-later). HarfBuzz and plutosvg are off.
    - mimalloc (MIT).
    - zstd 1.5.7 (BSD-3-Clause or GPL-2.0; BSD is taken).
    - ring 0.17.14 (ISC, and Apache-2.0 for its BoringSSL code; no OpenSSL-licensed code).
- **Embedded data**:
  - `data/ui/RobotoMono-Regular.ttf`: Roboto Mono, Apache-2.0. The repo has no copy of its licence. (Corrected in
    Phase 4: version 3.001, which this is, is under the SIL Open Font License 1.1, as its name table says.)
  - `data/cheats.json`: a cheat list with no stated source.
  - gopher64's shaders and icon.
  - The No-Intro DAT under `data/ui/` is read by the GUI only.

Findings:

- **Nintendo's PIF boot ROM.** `src/device/pif/rom.rs` holds `NTSC_PIF_ROM` and `PAL_PIF_ROM`, 1,984 bytes each, from
  gopher64's first commit (2023-10-04) with no attribution. They are byte-identical to ares'
  `mia/Firmware/Nintendo 64/pif.ntsc.rom` and `pif.pal.rom`, and begin with the PIF's IPL1. Plan Phase 2 removed them
  and boots by HLE, as mupen64plus-core's `src/device/pif/bootrom_hle.c` does (the user's call, 2026-11-10), keeping
  the 32 bytes of IPL2 that 6105 carts check (see "Boot without the PIF ROM").
- **X-Scale's CIC-NUS-6105 algorithm** (`n64_cic_nus_6105` in `src/device/pif.rs`, "X-Scale's algorithm") is a port of
  mupen64plus-core's `n64_cic_nus_6105.c`. That file is "Copyright 2011 X-Scale. All rights reserved." under a
  two-clause BSD licence, which asks that binary redistributions reproduce the notice. gopher64 carries no notice, so
  the engine's release zip must.
- No non-commercial clause anywhere, and nothing GPL-2.0-only.
- **Engine mode doesn't need these**, which include everything that makes internet connections. Plan Phase 2 put
  them behind a cargo feature, on by default and off for the engine (see "What the engine build leaves out"):
  - gopher64's netplay (`ggrs`, `matchbox_socket` and the WebRTC and TLS crates it brings: `rustls` 0.21, `webpki`,
    the dalek curves);
  - `reqwest`;
  - RetroAchievements (`rcheevos`);
  - Discord presence;
  - TLS through `tokio-tungstenite` and `rustls` 0.23 with `ring`. `tokio-tungstenite` itself stays, without TLS, for
    the engine's own server.
  - the cheats;
  - SDL_ttf with FreeType and Roboto Mono, if engine mode draws no on-screen text.

### Crates

Each crate the engine build compiles, with its `license` field as
`cargo tree --target x86_64-pc-windows-msvc --no-default-features -e normal` reports it after Phase 2. Phase 1
read all 382 that compiled before the extras were left out.

| Crate | Version | License (Cargo.toml) |
|---|---|---|
| aes | v0.9.3 | MIT OR Apache-2.0 |
| aho-corasick | v1.1.5 | Unlicense OR MIT |
| anstream | v1.0.0 | MIT OR Apache-2.0 |
| anstyle | v1.0.14 | MIT OR Apache-2.0 |
| anstyle-parse | v1.0.0 | MIT OR Apache-2.0 |
| anstyle-query | v1.1.5 | MIT OR Apache-2.0 |
| anstyle-wincon | v3.0.11 | MIT OR Apache-2.0 |
| bitflags | v1.3.2 | MIT/Apache-2.0 |
| block-buffer | v0.12.1 | MIT OR Apache-2.0 |
| block-padding | v0.4.2 | MIT OR Apache-2.0 |
| bumpalo | v3.20.3 | MIT OR Apache-2.0 |
| byteorder | v1.5.0 | Unlicense OR MIT |
| bytes | v1.12.1 | MIT |
| bzip2 | v0.6.1 | MIT OR Apache-2.0 |
| cbc | v0.2.1 | MIT OR Apache-2.0 |
| cfg-if | v1.0.5 | MIT OR Apache-2.0 |
| chacha20 | v0.10.2 | MIT OR Apache-2.0 |
| chrono | v0.4.45 | MIT OR Apache-2.0 |
| cipher | v0.5.2 | MIT OR Apache-2.0 |
| clap | v4.6.7 | MIT OR Apache-2.0 |
| clap_builder | v4.6.7 | MIT OR Apache-2.0 |
| clap_derive v4.6.7 | (proc-macro) | MIT OR Apache-2.0 |
| clap_lex | v1.1.1 | MIT OR Apache-2.0 |
| cmov | v0.5.4 | Apache-2.0 OR MIT |
| cobs | v0.3.0 | MIT OR Apache-2.0 |
| colorchoice | v1.0.5 | MIT OR Apache-2.0 |
| constant_time_eq | v0.6.1 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| const-oid | v0.10.2 | Apache-2.0 OR MIT |
| core_detect | v1.0.0 | MIT/Apache-2.0 |
| cpubits | v0.1.1 | MIT OR Apache-2.0 |
| cpufeatures | v0.3.1 | MIT OR Apache-2.0 |
| crc32fast | v1.5.2 | MIT OR Apache-2.0 |
| crypto-common | v0.2.2 | MIT OR Apache-2.0 |
| ctutils | v0.4.3 | Apache-2.0 OR MIT |
| data-encoding | v2.11.1 | MIT |
| deflate64 | v0.1.12 | MIT |
| deranged | v0.5.9 | MIT OR Apache-2.0 |
| digest | v0.11.3 | MIT OR Apache-2.0 |
| dirs | v7.0.0 | MIT OR Apache-2.0 |
| dirs-sys | v0.5.0 | MIT OR Apache-2.0 |
| encoding_rs | v0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| equivalent | v1.0.3 | Apache-2.0 OR MIT |
| fatfs | v0.3.6 | MIT |
| flate2 | v1.1.10 | MIT OR Apache-2.0 |
| futures | v0.3.34 | MIT OR Apache-2.0 |
| futures-channel | v0.3.34 | MIT OR Apache-2.0 |
| futures-core | v0.3.34 | MIT OR Apache-2.0 |
| futures-executor | v0.3.34 | MIT OR Apache-2.0 |
| futures-io | v0.3.34 | MIT OR Apache-2.0 |
| futures-macro v0.3.34 | (proc-macro) | MIT OR Apache-2.0 |
| futures-sink | v0.3.34 | MIT OR Apache-2.0 |
| futures-task | v0.3.34 | MIT OR Apache-2.0 |
| futures-util | v0.3.34 | MIT OR Apache-2.0 |
| getrandom | v0.4.3 | MIT OR Apache-2.0 |
| gopher64 | v1.1.42 | GPL-3.0 (its `LICENSE`) |
| hash32 | v0.2.1 | MIT OR Apache-2.0 |
| hashbrown | v0.17.1 | MIT OR Apache-2.0 |
| heapless | v0.7.17 | MIT OR Apache-2.0 |
| heck | v0.5.0 | MIT OR Apache-2.0 |
| hmac | v0.13.0 | MIT OR Apache-2.0 |
| http | v1.5.0 | MIT OR Apache-2.0 |
| httparse | v1.10.1 | MIT OR Apache-2.0 |
| hybrid-array | v0.4.15 | MIT OR Apache-2.0 |
| indexmap | v2.14.2 | Apache-2.0 OR MIT |
| inout | v0.2.3 | MIT OR Apache-2.0 |
| is_terminal_polyfill | v1.70.2 | MIT OR Apache-2.0 |
| itoa | v1.0.18 | MIT OR Apache-2.0 |
| libbz2-rs-sys | v0.2.5 | bzip2-1.0.6 |
| libmimalloc-sys | v0.1.49 | MIT |
| lock_api | v0.4.14 | MIT OR Apache-2.0 |
| log | v0.4.34 | MIT OR Apache-2.0 |
| lzma-rust2 | v0.21.0 | Apache-2.0 |
| lzma-rust2 | v0.22.0 | Apache-2.0 |
| memchr | v2.8.3 | Unlicense OR MIT |
| mimalloc | v0.1.52 | MIT |
| mio | v1.2.4 | MIT |
| multiversion_no_op v1.0.0 | (proc-macro) | Apache-2.0 OR MIT |
| num-conv | v0.2.2 | MIT OR Apache-2.0 |
| num-traits | v0.2.19 | MIT OR Apache-2.0 |
| obfstr | v0.4.6 | MIT |
| once_cell_polyfill | v1.70.2 | MIT OR Apache-2.0 |
| option-ext | v0.2.0 | MPL-2.0 |
| pbkdf2 | v0.13.0 | MIT OR Apache-2.0 |
| pin-project-lite | v0.2.17 | Apache-2.0 OR MIT |
| postcard | v1.1.3 | MIT OR Apache-2.0 |
| powerfmt | v0.2.1 | MIT OR Apache-2.0 |
| ppmd-rust | v1.5.0 | CC0-1.0 OR MIT-0 |
| proc-macro2 | v1.0.107 | MIT OR Apache-2.0 |
| quick-xml | v0.42.0 | MIT |
| quote | v1.0.47 | MIT OR Apache-2.0 |
| rand | v0.10.3 | MIT OR Apache-2.0 |
| rand_core | v0.10.1 | MIT OR Apache-2.0 |
| regex | v1.13.1 | MIT OR Apache-2.0 |
| regex-automata | v0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | v0.8.11 | MIT OR Apache-2.0 |
| rustc-hash | v2.1.3 | Apache-2.0 OR MIT |
| scopeguard | v1.2.0 | MIT OR Apache-2.0 |
| sdl3-sys | v0.7.2+SDL-3.4.18 | Zlib |
| sdl3-ttf-sys | v0.7.1+SDL-ttf-3.2.2 | Zlib |
| semver | v1.0.28 | MIT OR Apache-2.0 |
| serde | v1.0.229 | MIT OR Apache-2.0 |
| serde_core | v1.0.229 | MIT OR Apache-2.0 |
| serde_derive v1.0.229 | (proc-macro) | MIT OR Apache-2.0 |
| serde_json | v1.0.154 | MIT OR Apache-2.0 |
| serde-big-array | v0.5.1 | MIT OR Apache-2.0 |
| sevenz-rust2 | v0.24.0 | Apache-2.0 |
| sha1 | v0.11.0 | MIT OR Apache-2.0 |
| sha2 | v0.11.1 | MIT OR Apache-2.0 |
| simd-adler32 | v0.3.10 | MIT |
| simdutf8 | v0.1.5 | MIT OR Apache-2.0 |
| slab | v0.4.12 | MIT |
| socket2 | v0.6.5 | MIT OR Apache-2.0 |
| spin | v0.9.9 | MIT |
| spin_sleep | v1.3.3 | Apache-2.0 |
| stable_deref_trait | v1.2.1 | MIT OR Apache-2.0 |
| strsim | v0.11.1 | MIT |
| syn | v3.0.9 | MIT OR Apache-2.0 |
| thiserror | v2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl v2.0.21 | (proc-macro) | MIT OR Apache-2.0 |
| time | v0.3.55 | MIT OR Apache-2.0 |
| time-core | v0.1.9 | MIT OR Apache-2.0 |
| tokio | v1.53.3 | MIT |
| tokio-macros v2.7.2 | (proc-macro) | MIT |
| tokio-tungstenite | v0.30.0 | MIT |
| tungstenite | v0.30.0 | MIT OR Apache-2.0 |
| typed-path | v0.12.3 | MIT OR Apache-2.0 |
| typenum | v1.20.1 | MIT OR Apache-2.0 |
| unicode-ident | v1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| utf8parse | v0.2.2 | Apache-2.0 OR MIT |
| windows-link | v0.2.1 | MIT OR Apache-2.0 |
| windows-sys | v0.61.2 | MIT OR Apache-2.0 |
| zeroize | v1.9.1 | Apache-2.0 OR MIT |
| zip | v9.0.3 | MIT |
| zlib-rs | v0.6.8 | Zlib |
| zmij | v1.0.23 | MIT |
| zopfli | v0.8.4 | Apache-2.0 |
| zstd | v0.14.2 | BSD-3-Clause |
| zstd-safe | v8.1.0 | BSD-3-Clause |
| zstd-sys | v2.1.1+zstd.1.5.7 | BSD-3-Clause |
