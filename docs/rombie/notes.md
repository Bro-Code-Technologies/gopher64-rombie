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

From upstream's `.github/workflows/build.yml` (`build-windows`), to be confirmed by the first build (plan Phase 2):

- Rust as `rust-toolchain.toml` pins it (1.99.0), target `x86_64-pc-windows-msvc`.
- LLVM and Clang at the LLVM version `rustc --version --verbose` reports, from the LLVM release's Windows installer.
  `bindgen` needs its libclang.
- `CC` and `CXX` set to `clang-cl`: `build.rs` passes Clang flags (`-flto=thin`, `-march=x86-64-v3`).
- `.cargo/config.toml` links with `rust-lld` and builds for `x86-64-v3`, so the engine needs a CPU with AVX2.
- The engine build: `cargo build --release --no-default-features` (no `gui` feature).

## License scan (plan Phase 1)

What `cargo build --release --no-default-features` compiles for `x86_64-pc-windows-msvc`:

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
- **Crates**: 382 packages, in the table below. Most are MIT or Apache-2.0. The rest are BSD-2-Clause, BSD-3-Clause,
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
  `mia/Firmware/Nintendo 64/pif.ntsc.rom` and `pif.pal.rom`, and begin with the PIF's IPL1. Plan Phase 2 removes them
  and boots by HLE, as mupen64plus-core's `src/device/pif/bootrom_hle.c` does (the user's call, 2026-11-10).
- **X-Scale's CIC-NUS-6105 algorithm** (`n64_cic_nus_6105` in `src/device/pif.rs`, "X-Scale's algorithm") is a port of
  mupen64plus-core's `n64_cic_nus_6105.c`. That file is "Copyright 2011 X-Scale. All rights reserved." under a
  two-clause BSD licence, which asks that binary redistributions reproduce the notice. gopher64 carries no notice, so
  the engine's release zip must.
- No non-commercial clause anywhere, and nothing GPL-2.0-only.
- **Engine mode doesn't need these**, which include everything that makes internet connections. Plan Phase 2 puts
  them behind a cargo feature, on by default and off for the engine:
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

Each crate's `license` field as `cargo tree --target x86_64-pc-windows-msvc --no-default-features -e normal` reports it.

| Crate | Version | License (Cargo.toml) |
|---|---|---|
| aead | v0.5.2 | MIT OR Apache-2.0 |
| aes | v0.8.4 | MIT OR Apache-2.0 |
| aes | v0.9.3 | MIT OR Apache-2.0 |
| aes-gcm | v0.10.3 | Apache-2.0 OR MIT |
| aho-corasick | v1.1.5 | Unlicense OR MIT |
| anstream | v1.0.0 | MIT OR Apache-2.0 |
| anstyle | v1.0.14 | MIT OR Apache-2.0 |
| anstyle-parse | v1.0.0 | MIT OR Apache-2.0 |
| anstyle-query | v1.1.5 | MIT OR Apache-2.0 |
| anstyle-wincon | v3.0.11 | MIT OR Apache-2.0 |
| anyhow | v1.0.104 | MIT OR Apache-2.0 |
| arc-swap | v1.9.2 | MIT OR Apache-2.0 |
| asn1-rs | v0.6.2 | MIT OR Apache-2.0 |
| asn1-rs-derive | v0.5.1 (proc-macro) | MIT OR Apache-2.0 |
| asn1-rs-impl | v0.2.0 (proc-macro) | MIT/Apache-2.0 |
| async-channel | v1.9.0 | Apache-2.0 OR MIT |
| async-channel | v2.5.0 | Apache-2.0 OR MIT |
| async-compat | v0.2.6 | Apache-2.0 OR MIT |
| async-executor | v1.14.0 | Apache-2.0 OR MIT |
| async-global-executor | v2.4.1 | Apache-2.0 OR MIT |
| async-io | v2.6.0 | Apache-2.0 OR MIT |
| async-lock | v3.4.2 | Apache-2.0 OR MIT |
| async-std | v1.13.2 | Apache-2.0 OR MIT |
| async-task | v4.7.1 | Apache-2.0 OR MIT |
| async-tls | v0.13.0 | MIT/Apache-2.0 |
| async-trait | v0.1.92 (proc-macro) | MIT OR Apache-2.0 |
| async-tungstenite | v0.32.1 | MIT |
| atomic-waker | v1.1.2 | Apache-2.0 OR MIT |
| base16ct | v0.2.0 | Apache-2.0 OR MIT |
| base64 | v0.21.7 | MIT OR Apache-2.0 |
| base64 | v0.22.1 | MIT OR Apache-2.0 |
| base64 | v0.23.1 | MIT OR Apache-2.0 |
| base64ct | v1.8.3 | Apache-2.0 OR MIT |
| bincode | v1.3.3 | MIT |
| bitfield-rle | v0.2.1 | MIT OR Apache-2.0 |
| bitflags | v1.3.2 | MIT/Apache-2.0 |
| bitflags | v2.13.2 | MIT OR Apache-2.0 |
| block-buffer | v0.10.4 | MIT OR Apache-2.0 |
| block-buffer | v0.12.1 | MIT OR Apache-2.0 |
| block-padding | v0.3.3 | MIT OR Apache-2.0 |
| block-padding | v0.4.2 | MIT OR Apache-2.0 |
| blocking | v1.7.0 | Apache-2.0 OR MIT |
| bumpalo | v3.20.3 | MIT OR Apache-2.0 |
| bytecheck | v0.8.3 | MIT |
| bytecheck_derive | v0.8.3 (proc-macro) | MIT |
| byteorder | v1.5.0 | Unlicense OR MIT |
| bytes | v1.12.1 | MIT |
| bzip2 | v0.6.1 | MIT OR Apache-2.0 |
| cbc | v0.1.2 | MIT OR Apache-2.0 |
| cbc | v0.2.1 | MIT OR Apache-2.0 |
| ccm | v0.5.0 | Apache-2.0 OR MIT |
| cfg-if | v1.0.5 | MIT OR Apache-2.0 |
| chacha20 | v0.10.2 | MIT OR Apache-2.0 |
| chacha20 | v0.9.1 | Apache-2.0 OR MIT |
| chacha20poly1305 | v0.10.1 | Apache-2.0 OR MIT |
| chrono | v0.4.45 | MIT OR Apache-2.0 |
| cipher | v0.4.4 | MIT OR Apache-2.0 |
| cipher | v0.5.2 | MIT OR Apache-2.0 |
| clap | v4.6.7 | MIT OR Apache-2.0 |
| clap_builder | v4.6.7 | MIT OR Apache-2.0 |
| clap_derive | v4.6.7 (proc-macro) | MIT OR Apache-2.0 |
| clap_lex | v1.1.1 | MIT OR Apache-2.0 |
| cmov | v0.5.4 | Apache-2.0 OR MIT |
| cobs | v0.3.0 | MIT OR Apache-2.0 |
| colorchoice | v1.0.5 | MIT OR Apache-2.0 |
| concurrent-queue | v2.5.0 | Apache-2.0 OR MIT |
| const-oid | v0.10.2 | Apache-2.0 OR MIT |
| const-oid | v0.9.6 | Apache-2.0 OR MIT |
| constant_time_eq | v0.6.1 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| convert_case | v0.10.0 | MIT |
| core_detect | v1.0.0 | MIT/Apache-2.0 |
| cpubits | v0.1.1 | MIT OR Apache-2.0 |
| cpufeatures | v0.2.17 | MIT OR Apache-2.0 |
| cpufeatures | v0.3.1 | MIT OR Apache-2.0 |
| crc | v3.4.0 | MIT OR Apache-2.0 |
| crc-catalog | v2.5.0 | MIT OR Apache-2.0 |
| crc32fast | v1.5.2 | MIT OR Apache-2.0 |
| crossbeam-utils | v0.8.23 | MIT OR Apache-2.0 |
| crypto-bigint | v0.5.5 | Apache-2.0 OR MIT |
| crypto-common | v0.1.7 | MIT OR Apache-2.0 |
| crypto-common | v0.2.2 | MIT OR Apache-2.0 |
| ctr | v0.9.2 | MIT OR Apache-2.0 |
| ctutils | v0.4.3 | Apache-2.0 OR MIT |
| curve25519-dalek | v4.1.3 | BSD-3-Clause |
| curve25519-dalek-derive | v0.1.1 (proc-macro) | MIT/Apache-2.0 |
| data-encoding | v2.11.1 | MIT |
| deflate64 | v0.1.12 | MIT |
| der | v0.7.10 | Apache-2.0 OR MIT |
| der-parser | v9.0.0 | MIT/Apache-2.0 |
| deranged | v0.5.9 | MIT OR Apache-2.0 |
| derive_more | v2.1.1 | MIT |
| derive_more-impl | v2.1.1 (proc-macro) | MIT |
| digest | v0.10.7 | MIT OR Apache-2.0 |
| digest | v0.11.3 | MIT OR Apache-2.0 |
| dirs | v7.0.0 | MIT OR Apache-2.0 |
| dirs-sys | v0.5.0 | MIT OR Apache-2.0 |
| discord-rich-presence | v1.1.0 | MIT |
| displaydoc | v0.2.7 (proc-macro) | MIT OR Apache-2.0 |
| dtls | v0.17.2 | MIT/Apache-2.0 |
| ecdsa | v0.16.9 | Apache-2.0 OR MIT |
| elliptic-curve | v0.13.8 | Apache-2.0 OR MIT |
| encoding_rs | v0.8.42 | (Apache-2.0 OR MIT) AND BSD-3-Clause |
| equivalent | v1.0.3 | Apache-2.0 OR MIT |
| errno | v0.3.14 | MIT OR Apache-2.0 |
| event-listener | v2.5.3 | Apache-2.0 OR MIT |
| event-listener | v5.4.2 | Apache-2.0 OR MIT |
| event-listener-strategy | v0.5.4 | Apache-2.0 OR MIT |
| fastrand | v2.5.0 | Apache-2.0 OR MIT |
| fatfs | v0.3.6 | MIT |
| ff | v0.13.1 | MIT/Apache-2.0 |
| flate2 | v1.1.10 | MIT OR Apache-2.0 |
| fnv | v1.0.7 | Apache-2.0 / MIT |
| form_urlencoded | v1.2.2 | MIT OR Apache-2.0 |
| futures | v0.3.34 | MIT OR Apache-2.0 |
| futures-channel | v0.3.34 | MIT OR Apache-2.0 |
| futures-core | v0.3.34 | MIT OR Apache-2.0 |
| futures-executor | v0.3.34 | MIT OR Apache-2.0 |
| futures-io | v0.3.34 | MIT OR Apache-2.0 |
| futures-lite | v2.6.1 | Apache-2.0 OR MIT |
| futures-macro | v0.3.34 (proc-macro) | MIT OR Apache-2.0 |
| futures-sink | v0.3.34 | MIT OR Apache-2.0 |
| futures-task | v0.3.34 | MIT OR Apache-2.0 |
| futures-timer | v3.0.4 | MIT/Apache-2.0 |
| futures-util | v0.3.34 | MIT OR Apache-2.0 |
| generic-array | v0.14.7 | MIT |
| getrandom | v0.2.17 | MIT OR Apache-2.0 |
| getrandom | v0.3.4 | MIT OR Apache-2.0 |
| getrandom | v0.4.3 | MIT OR Apache-2.0 |
| ggrs | v0.13.0 | MIT OR Apache-2.0 |
| ghash | v0.5.1 | Apache-2.0 OR MIT |
| group | v0.13.0 | MIT/Apache-2.0 |
| h2 | v0.4.20 | MIT |
| hash32 | v0.2.1 | MIT OR Apache-2.0 |
| hashbrown | v0.17.1 | MIT OR Apache-2.0 |
| heapless | v0.7.17 | MIT OR Apache-2.0 |
| heck | v0.5.0 | MIT OR Apache-2.0 |
| hex | v0.4.3 | MIT OR Apache-2.0 |
| hkdf | v0.12.4 | MIT OR Apache-2.0 |
| hmac | v0.12.1 | MIT OR Apache-2.0 |
| hmac | v0.13.0 | MIT OR Apache-2.0 |
| http | v1.5.0 | MIT OR Apache-2.0 |
| http-body | v1.1.0 | MIT |
| http-body-util | v0.1.5 | MIT |
| httparse | v1.10.1 | MIT OR Apache-2.0 |
| hybrid-array | v0.4.15 | MIT OR Apache-2.0 |
| hyper | v1.12.0 | MIT |
| hyper-rustls | v0.27.10 | Apache-2.0 OR ISC OR MIT |
| hyper-util | v0.1.21 | MIT |
| icu_collections | v2.3.0 | Unicode-3.0 |
| icu_locale_core | v2.3.0 | Unicode-3.0 |
| icu_normalizer | v2.3.0 | Unicode-3.0 |
| icu_normalizer_data | v2.3.0 | Unicode-3.0 |
| icu_properties | v2.3.0 | Unicode-3.0 |
| icu_properties_data | v2.3.0 | Unicode-3.0 |
| icu_provider | v2.3.1 | Unicode-3.0 |
| idna | v1.1.0 | MIT OR Apache-2.0 |
| idna_adapter | v1.2.2 | Apache-2.0 OR MIT |
| indexmap | v2.14.2 | Apache-2.0 OR MIT |
| inout | v0.1.4 | MIT OR Apache-2.0 |
| inout | v0.2.3 | MIT OR Apache-2.0 |
| instant | v0.1.13 | BSD-3-Clause |
| interceptor | v0.17.2 | MIT/Apache-2.0 |
| ipnet | v2.12.2 | MIT OR Apache-2.0 |
| is_terminal_polyfill | v1.70.2 | MIT OR Apache-2.0 |
| itoa | v1.0.18 | MIT OR Apache-2.0 |
| kv-log-macro | v1.0.7 | MIT OR Apache-2.0 |
| lazy_static | v1.5.1 | MIT OR Apache-2.0 |
| libbz2-rs-sys | v0.2.5 | bzip2-1.0.6 |
| libc | v0.2.190 | MIT OR Apache-2.0 |
| libmimalloc-sys | v0.1.49 | MIT |
| litemap | v0.8.3 | Unicode-3.0 |
| lock_api | v0.4.14 | MIT OR Apache-2.0 |
| log | v0.4.34 | MIT OR Apache-2.0 |
| lzma-rust2 | v0.21.0 | Apache-2.0 |
| lzma-rust2 | v0.22.0 | Apache-2.0 |
| matchbox_protocol | v0.14.0 | MIT OR Apache-2.0 |
| matchbox_socket | v0.14.0 | MIT OR Apache-2.0 |
| md-5 | v0.10.6 | MIT OR Apache-2.0 |
| memchr | v2.8.3 | Unlicense OR MIT |
| mimalloc | v0.1.52 | MIT |
| mime | v0.3.17 | MIT OR Apache-2.0 |
| minimal-lexical | v0.2.1 | MIT/Apache-2.0 |
| mio | v1.2.4 | MIT |
| multiversion_no_op | v1.0.0 (proc-macro) | Apache-2.0 OR MIT |
| munge | v0.4.7 | MIT |
| munge_macro | v0.4.7 (proc-macro) | MIT |
| nom | v7.1.3 | MIT |
| num-bigint | v0.4.8 | MIT OR Apache-2.0 |
| num-conv | v0.2.2 | MIT OR Apache-2.0 |
| num-integer | v0.1.47 | MIT OR Apache-2.0 |
| num-traits | v0.2.19 | MIT OR Apache-2.0 |
| obfstr | v0.4.6 | MIT |
| oid-registry | v0.7.1 | MIT OR Apache-2.0 |
| once_cell | v1.21.4 | MIT OR Apache-2.0 |
| once_cell_polyfill | v1.70.2 | MIT OR Apache-2.0 |
| opaque-debug | v0.3.1 | MIT OR Apache-2.0 |
| option-ext | v0.2.0 | MPL-2.0 |
| p256 | v0.13.2 | Apache-2.0 OR MIT |
| p384 | v0.13.1 | Apache-2.0 OR MIT |
| parking | v2.2.1 | Apache-2.0 OR MIT |
| parking_lot | v0.12.5 | MIT OR Apache-2.0 |
| parking_lot_core | v0.9.12 | MIT OR Apache-2.0 |
| pbkdf2 | v0.13.0 | MIT OR Apache-2.0 |
| pem | v3.0.6 | MIT |
| pem-rfc7468 | v0.7.0 | Apache-2.0 OR MIT |
| percent-encoding | v2.3.2 | MIT OR Apache-2.0 |
| pin-project-lite | v0.2.17 | Apache-2.0 OR MIT |
| pin-utils | v0.1.1 | MIT OR Apache-2.0 |
| piper | v0.2.5 | MIT OR Apache-2.0 |
| pkcs8 | v0.10.2 | Apache-2.0 OR MIT |
| polling | v3.11.0 | Apache-2.0 OR MIT |
| poly1305 | v0.8.0 | Apache-2.0 OR MIT |
| polyval | v0.6.2 | Apache-2.0 OR MIT |
| portable-atomic | v1.15.0 | Apache-2.0 OR MIT |
| postcard | v1.1.3 | MIT OR Apache-2.0 |
| potential_utf | v0.1.6 | Unicode-3.0 |
| powerfmt | v0.2.1 | MIT OR Apache-2.0 |
| ppmd-rust | v1.5.0 | CC0-1.0 OR MIT-0 |
| ppv-lite86 | v0.2.21 | MIT OR Apache-2.0 |
| primeorder | v0.13.6 | Apache-2.0 OR MIT |
| proc-macro2 | v1.0.107 | MIT OR Apache-2.0 |
| ptr_meta | v0.3.2 | MIT |
| ptr_meta_derive | v0.3.2 (proc-macro) | MIT |
| quick-xml | v0.42.0 | MIT |
| quote | v1.0.47 | MIT OR Apache-2.0 |
| rancor | v0.1.3 | MIT |
| rand | v0.10.3 | MIT OR Apache-2.0 |
| rand | v0.8.8 | MIT OR Apache-2.0 |
| rand | v0.9.5 | MIT OR Apache-2.0 |
| rand_chacha | v0.3.1 | MIT OR Apache-2.0 |
| rand_chacha | v0.9.0 | MIT OR Apache-2.0 |
| rand_core | v0.10.1 | MIT OR Apache-2.0 |
| rand_core | v0.6.4 | MIT OR Apache-2.0 |
| rand_core | v0.9.5 | MIT OR Apache-2.0 |
| rcgen | v0.13.2 | MIT OR Apache-2.0 |
| regex | v1.13.1 | MIT OR Apache-2.0 |
| regex-automata | v0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | v0.8.11 | MIT OR Apache-2.0 |
| rend | v0.5.4 | MIT |
| reqwest | v0.13.5 | MIT OR Apache-2.0 |
| rfc6979 | v0.4.0 | Apache-2.0 OR MIT |
| ring | v0.17.14 | Apache-2.0 AND ISC |
| rkyv | v0.8.18 | MIT |
| rkyv_derive | v0.8.18 (proc-macro) | MIT |
| rtcp | v0.17.2 | MIT/Apache-2.0 |
| rtp | v0.17.2 | MIT/Apache-2.0 |
| rustc-hash | v2.1.3 | Apache-2.0 OR MIT |
| rusticata-macros | v4.1.0 | MIT/Apache-2.0 |
| rustix | v1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| rustls | v0.21.12 | Apache-2.0 OR ISC OR MIT |
| rustls | v0.23.46 | Apache-2.0 OR ISC OR MIT |
| rustls-pemfile | v1.0.4 | Apache-2.0 OR ISC OR MIT |
| rustls-pki-types | v1.15.1 | MIT OR Apache-2.0 |
| rustls-platform-verifier | v0.7.1 | MIT OR Apache-2.0 |
| rustls-webpki | v0.101.7 | ISC |
| rustls-webpki | v0.103.15 | ISC |
| rustversion | v1.0.23 (proc-macro) | MIT OR Apache-2.0 |
| ryu | v1.0.23 | Apache-2.0 OR BSL-1.0 |
| scopeguard | v1.2.0 | MIT OR Apache-2.0 |
| sct | v0.7.1 | Apache-2.0 OR ISC OR MIT |
| sdl3-sys | v0.7.2+SDL-3.4.18 | Zlib |
| sdl3-ttf-sys | v0.7.1+SDL-ttf-3.2.2 | Zlib |
| sdp | v0.17.2 | MIT/Apache-2.0 |
| sec1 | v0.7.3 | Apache-2.0 OR MIT |
| semver | v1.0.28 | MIT OR Apache-2.0 |
| serde | v1.0.229 | MIT OR Apache-2.0 |
| serde-big-array | v0.5.1 | MIT OR Apache-2.0 |
| serde_core | v1.0.229 | MIT OR Apache-2.0 |
| serde_derive | v1.0.229 (proc-macro) | MIT OR Apache-2.0 |
| serde_json | v1.0.154 | MIT OR Apache-2.0 |
| serde_repr | v0.1.21 (proc-macro) | MIT OR Apache-2.0 |
| serde_urlencoded | v0.7.1 | MIT/Apache-2.0 |
| sevenz-rust2 | v0.24.0 | Apache-2.0 |
| sha1 | v0.10.7 | MIT OR Apache-2.0 |
| sha1 | v0.11.0 | MIT OR Apache-2.0 |
| sha2 | v0.10.9 | MIT OR Apache-2.0 |
| sha2 | v0.11.1 | MIT OR Apache-2.0 |
| signature | v2.2.0 | Apache-2.0 OR MIT |
| simd-adler32 | v0.3.10 | MIT |
| simdutf8 | v0.1.5 | MIT OR Apache-2.0 |
| slab | v0.4.12 | MIT |
| smallvec | v1.16.3 | MIT OR Apache-2.0 |
| smol_str | v0.2.2 | MIT OR Apache-2.0 |
| socket2 | v0.5.10 | MIT OR Apache-2.0 |
| socket2 | v0.6.5 | MIT OR Apache-2.0 |
| spin | v0.9.9 | MIT |
| spin_sleep | v1.3.3 | Apache-2.0 |
| spki | v0.7.3 | Apache-2.0 OR MIT |
| stable_deref_trait | v1.2.1 | MIT OR Apache-2.0 |
| strsim | v0.11.1 | MIT |
| stun | v0.17.2 | MIT/Apache-2.0 |
| substring | v1.4.5 | MIT OR Apache-2.0 |
| subtle | v2.6.1 | BSD-3-Clause |
| syn | v2.0.119 | MIT OR Apache-2.0 |
| syn | v3.0.9 | MIT OR Apache-2.0 |
| sync_wrapper | v1.0.2 | Apache-2.0 |
| synstructure | v0.13.2 | MIT |
| synstructure | v0.14.0 | MIT |
| thiserror | v1.0.69 | MIT OR Apache-2.0 |
| thiserror | v2.0.21 | MIT OR Apache-2.0 |
| thiserror-impl | v1.0.69 (proc-macro) | MIT OR Apache-2.0 |
| thiserror-impl | v2.0.21 (proc-macro) | MIT OR Apache-2.0 |
| time | v0.3.55 | MIT OR Apache-2.0 |
| time-core | v0.1.9 | MIT OR Apache-2.0 |
| time-macros | v0.2.32 (proc-macro) | MIT OR Apache-2.0 |
| tinystr | v0.8.4 | Unicode-3.0 |
| tokio | v1.53.3 | MIT |
| tokio-macros | v2.7.2 (proc-macro) | MIT |
| tokio-rustls | v0.26.6 | MIT OR Apache-2.0 |
| tokio-tungstenite | v0.30.0 | MIT |
| tokio-util | v0.7.20 | MIT |
| tower | v0.5.3 | MIT |
| tower-http | v0.6.11 | MIT |
| tower-layer | v0.3.3 | MIT |
| tower-service | v0.3.3 | MIT |
| tracing | v0.1.44 | MIT |
| tracing-attributes | v0.1.31 (proc-macro) | MIT |
| tracing-core | v0.1.36 | MIT |
| try-lock | v0.2.5 | MIT |
| tungstenite | v0.28.0 | MIT OR Apache-2.0 |
| tungstenite | v0.30.0 | MIT OR Apache-2.0 |
| turn | v0.17.2 | MIT/Apache-2.0 |
| typed-path | v0.12.3 | MIT OR Apache-2.0 |
| typenum | v1.20.1 | MIT OR Apache-2.0 |
| unicase | v2.10.0 | MIT OR Apache-2.0 |
| unicode-ident | v1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| unicode-segmentation | v1.13.3 | MIT OR Apache-2.0 |
| unicode-xid | v0.2.6 | MIT OR Apache-2.0 |
| universal-hash | v0.5.1 | MIT OR Apache-2.0 |
| untrusted | v0.9.0 | ISC |
| url | v2.5.8 | MIT OR Apache-2.0 |
| utf-8 | v0.7.6 | MIT OR Apache-2.0 |
| utf8_iter | v1.0.4 | Apache-2.0 OR MIT |
| utf8parse | v0.2.2 | Apache-2.0 OR MIT |
| uuid | v0.8.2 | Apache-2.0 OR MIT |
| uuid | v1.28.0 | Apache-2.0 OR MIT |
| value-bag | v1.14.1 | Apache-2.0 OR MIT |
| varinteger | v1.0.6 | MIT |
| waitgroup | v0.1.2 | Apache-2.0 |
| want | v0.3.2 | MIT |
| webpki | v0.22.4 | (none in Cargo.toml; see below) |
| webpki-roots | v0.22.6 | MPL-2.0 |
| webpki-roots | v0.26.11 | CDLA-Permissive-2.0 |
| webpki-roots | v1.0.9 | CDLA-Permissive-2.0 |
| webrtc | v0.17.2 | MIT/Apache-2.0 |
| webrtc-data | v0.17.2 | MIT/Apache-2.0 |
| webrtc-ice | v0.17.2 | MIT/Apache-2.0 |
| webrtc-mdns | v0.17.2 | MIT/Apache-2.0 |
| webrtc-media | v0.17.2 | MIT/Apache-2.0 |
| webrtc-sctp | v0.17.2 | MIT/Apache-2.0 |
| webrtc-srtp | v0.17.2 | MIT/Apache-2.0 |
| webrtc-util | v0.17.2 | MIT/Apache-2.0 |
| winapi | v0.3.9 | MIT/Apache-2.0 |
| windows-link | v0.2.1 | MIT OR Apache-2.0 |
| windows-registry | v0.6.1 | MIT OR Apache-2.0 |
| windows-result | v0.4.1 | MIT OR Apache-2.0 |
| windows-strings | v0.5.1 | MIT OR Apache-2.0 |
| windows-sys | v0.52.0 | MIT OR Apache-2.0 |
| windows-sys | v0.61.2 | MIT OR Apache-2.0 |
| windows-targets | v0.52.6 | MIT OR Apache-2.0 |
| windows_x86_64_msvc | v0.52.6 | MIT OR Apache-2.0 |
| writeable | v0.6.4 | Unicode-3.0 |
| x25519-dalek | v2.0.1 | BSD-3-Clause |
| x509-parser | v0.16.0 | MIT OR Apache-2.0 |
| yasna | v0.5.2 | MIT OR Apache-2.0 |
| yoke | v0.8.3 | Unicode-3.0 |
| yoke-derive | v0.8.4 (proc-macro) | Unicode-3.0 |
| zerocopy | v0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerofrom | v0.1.8 | Unicode-3.0 |
| zerofrom-derive | v0.1.8 (proc-macro) | Unicode-3.0 |
| zeroize | v1.9.1 | Apache-2.0 OR MIT |
| zeroize_derive | v1.5.0 (proc-macro) | Apache-2.0 OR MIT |
| zerotrie | v0.2.5 | Unicode-3.0 |
| zerovec | v0.11.8 | Unicode-3.0 |
| zerovec-derive | v0.11.6 (proc-macro) | Unicode-3.0 |
| zip | v9.0.3 | MIT |
| zlib-rs | v0.6.8 | Zlib |
| zmij | v1.0.23 | MIT |
| zopfli | v0.8.4 | Apache-2.0 |
| zstd | v0.14.2 | BSD-3-Clause |
| zstd-safe | v8.1.0 | BSD-3-Clause |
| zstd-sys | v2.1.1+zstd.1.5.7 | BSD-3-Clause |
