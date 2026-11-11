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
The left-out crates used to turn them on for it. SDL_ttf with FreeType and Roboto Mono stay for now, since gopher64's
window draws its on-screen text with them. Phase 3's windowless engine draws no text, so they can go once the engine
build has no window.

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

## Windowless spike (plan Phase 3)

With `GOPHER64_ROMBIE_SPIKE_PORT` set, `gopher64.exe <rom> [--load-state <slot>]` runs in engine mode, the core of
what Phase 4 makes the engine. Unset, nothing changes. `src/rombie.rs` holds it, and its header lists the messages:

- **No window.** `rdp_init` in `parallel-rdp/interface.cpp` takes a null window to mean engine mode: it creates
  Granite's Vulkan context and device with no surface and no WSI. With no surface to match, Granite takes the discrete
  GPU over an integrated one, so a two-GPU laptop runs on its GeForce. `GRANITE_VULKAN_DEVICE_INDEX=<n>` (Granite's
  own) picks the nth GPU in the order the log lists them, for the Intel UHD run of Phase 6. SDL's video and TTF aren't
  started, and on-screen messages are dropped.
- **Frames.** At each VI, where the windowed build draws, `rdp_read_frame` reads parallel-rdp's scanout back at the
  VI's own size (`scanout_sync`, the same scanout options as the window) and `rombie::send_frame` sends it as RGBA,
  alpha forced opaque. The engine time in the message is taken before the readback, so the page's delay includes it.
  `rdp_update_screen` moves Granite to its next frame context, as the swapchain's frame did. 1x, with no supersampling,
  widescreen or CRT, whatever the config says.
- **Sound.** The game's audio stream converts to 48 kHz stereo float with no device behind it. A thread pulls it at
  48 kHz by the clock, as a sound card would, and sends blocks of 256 to 2048 frames, as pcsx2-rombie's audio does.
  gopher64's own pacing stays: the stream's get callback pads a shortfall with silence and nudges the rate. While the
  game's AI is idle (where gopher64 pauses its sound device), the thread keeps the clock and sends silence, as a paused
  device plays it. Stopping instead drained the page's buffer by each pause: GoldenEye's emptied in 33 seconds.
- **Pads.** The page sends each port's pad word as the N64 reads it. All four ports are on, and gopher64's own keyboard
  and pad input isn't opened, so nothing else reaches the game.
- **The page.** One page, on `127.0.0.1` only, with no key or origin check yet (Phase 4). The game starts once it
  connects and ends when it leaves. Rombie's `engine/n64/engine-test.html` is that page.

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
  - `data/ui/RobotoMono-Regular.ttf`: Roboto Mono, Apache-2.0. The repo has no copy of its licence.
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
