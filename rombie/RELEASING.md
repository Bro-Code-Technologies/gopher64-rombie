# Releasing the Rombie N64 engine

How a new engine version gets from this fork to a download on GitHub. The engine is built and released from
`github.com/Bro-Code-Technologies/gopher64-rombie` (branch `rombie`, checked out at
`C:\Users\Michael\Projects\rombie-premium-engines\gopher64-rombie`). The build itself is in
[docs/rombie/notes.md](../docs/rombie/notes.md#building-windows). This file covers what comes after the code works:
the version, the zip, the tag, the GitHub release and promoting it.

A person publishes the release. Nothing here is automated.

## Names

| What | Format | 2026.0.0 |
| --- | --- | --- |
| Version | `YYYY.N.patch` | `2026.0.0` |
| Git tag | `rombie-n64-engine-<version>` | `rombie-n64-engine-2026.0.0` |
| Release name | `Rombie N64 Engine <version>` | `Rombie N64 Engine 2026.0.0` |
| Zip (from `package.ps1`) | `rombie-n64-engine-<version>-windows-x64.zip` | `rombie-n64-engine-2026.0.0-windows-x64.zip` |
| Source archive (from `package.ps1`) | `rombie-n64-engine-<version>-source.zip` | `rombie-n64-engine-2026.0.0-source.zip` |
| Commit message | `Rombie N64 engine <version>: <what changed>` | `Rombie N64 engine 2026.0.0: first release` |

Versions follow Rombie's other engines: `N` counts up through the year and goes back to 0 in January. Raise `N` when
the engine's behavior changes and `patch` for fixes. The engine releases on its own tag, never with Rombie's version
or gopher64's. The wire protocol has its own number (`PROTOCOL` in `src/rombie.rs`), and a protocol change is the
point to raise `N64_MIN_ENGINE_VERSION` in Rombie.

## Pre-release and official

Rombie's "Get the N64 engine" link points at
`https://github.com/Bro-Code-Technologies/gopher64-rombie/releases/latest`, and GitHub's "latest" skips pre-releases.
So:

- A **pre-release** is invisible to players. Staging testers get its direct link.
- An **official release** (the "latest release") is what every Rombie player is sent to. Promote one only after it has
  been checked on staging, and before the `apps.rombie.system.n64` flag is switched on in production.

A bug found after a release has gone out is fixed in a new version (2026.0.1), never by changing the files of the old
one.

## Steps

1. **Make the change** on the `rombie` branch.
   - `rombie/VERSION`: the version, one line. The build puts it in the exe (`--version`, the hello message, the version
     resource) and `package.ps1` in the zip's name.
   - `ALLOWED_ORIGINS` in `src/rombie.rs`, if the pages allowed to drive the engine change. Then update Rombie's
     `.docs/architecture.md` (N64, the Security bullet) to match.
2. **Commit and push the branch.** Build from a clean tree (`git status` shows nothing), so the exe really is built
   from the commit you are about to tag.
3. **Close any running engine.** A running `gopher64.exe` or `rombie-n64-engine.exe` holds the exe, and the build
   can't replace it. Check Task Manager.
4. **Build** the engine build, as the notes' "Building (Windows)" says, in its environment:
   ```powershell
   cargo build --release --no-default-features --target=x86_64-pc-windows-msvc
   .\target\x86_64-pc-windows-msvc\release\gopher64.exe --version | Out-String   # must print the new version
   ```
5. **Package** (from the fork's root, in the same environment, since it runs `cargo tree`, `cargo metadata` and
   `cargo vendor`):
   ```powershell
   powershell -File rombie\package.ps1
   ```
   It stops if the tree has changes, so both files are the commit you tag. It writes two files to `dist\`, which git
   doesn't track, and prints their SHA-256s:
   - `rombie-n64-engine-<version>-windows-x64.zip` (about 5 MB): the engine as `rombie-n64-engine.exe`,
     `msvcp140.dll`, `vcruntime140.dll` and `vcruntime140_1.dll` from Visual Studio's redist for its toolset, gopher64's `LICENSE` as
     `LICENSE.txt`, a `README.txt` (setup, the modification notice, the source archive, Microsoft's terms for the
     runtime) and `licenses\THIRD-PARTY.txt`. That holds the notices in `rombie\licenses\`, parallel-rdp's, the C
     libraries the crates build from source, and every crate the engine compiles with its license files. It stops if
     the exe isn't the engine build of this version, and names any crate with no license file.
   - `rombie-n64-engine-<version>-source.zip`: the complete corresponding source (GPL-3.0 section 6d). The commit's
     tree, every submodule at its pinned commit, and the crates from `cargo vendor`, with the `.cargo/config.toml`
     lines for an offline build. It's staged in a short folder under `%TEMP%`, since SDL's vendored sources run deep.

   Check that the source has no PIF bytes: `src\device\pif\rom.rs` must be missing from it, and the first 16 bytes of
   `NTSC_PIF_ROM` and of `PAL_PIF_ROM` (in upstream's `src/device/pif/rom.rs`, `git show
   f05daaf:src/device/pif/rom.rs`) must appear in none of its files.
6. **Try the zip the way a player would.** Unzip it into a new folder outside the build tree, then:
   - `.\rombie-n64-engine.exe --version | Out-String` prints the new version. The engine has no console window, so
     PowerShell only shows its output when it is piped, as here.
   - Run the exe once. It says it is set up. This also re-registers the `rombie-n64://` link to this folder, so a
     tester who had an older engine installed must do it once, or the browser keeps starting the old one.
   - Play an N64 game from Rombie's dev server (`http://localhost:5173`). The browser's prompts ("Open Rombie N64
     Engine?", tick Always allow, and access to other apps and services) should appear the first time.
7. **Tag the release commit and push the tag:**
   ```powershell
   git tag rombie-n64-engine-<version>
   git push origin rombie-n64-engine-<version>
   ```
8. **Publish on GitHub.** Repository, Releases, Draft a new release:
   1. Choose the tag you just pushed.
   2. Release title: `Rombie N64 Engine <version>`.
   3. Paste the description from the template below, with the two SHA-256s `package.ps1` printed.
   4. Attach both `rombie-n64-engine-<version>-windows-x64.zip` and `rombie-n64-engine-<version>-source.zip`. The
      download's `README.txt` names the source archive as its source, so the two always go out together.
   5. For a release that still needs staging: tick **Set as a pre-release**. Otherwise tick **Set as the latest
      release**.
   6. Publish release.
9. **Check the release page.** Both zips are attached with the right names, the tag points at the commit you meant,
   and the downloads work. For a pre-release, also open `.../releases/latest` and confirm it did **not** change.

## Promoting a pre-release to the official release

After staging has signed it off: open the release, Edit, untick **Set as a pre-release**, tick **Set as the latest
release**, Update release. Then open `.../releases/latest` and confirm it lands on this release, and follow the "Get
the N64 engine" link on rombie.app's Docs page. Do this **before** the N64 feature flag is turned on for everyone.

To roll back, open the previous good release and make it the latest.

## Description template

Everything between the lines goes into the GitHub release description. Replace `<version>`, and change the "New in"
line for each release: one plain sentence about what is different for the player.

```markdown
## Rombie N64 Engine <version>

The N64 engine is the small Windows program that runs Nintendo 64 games for [Rombie](https://rombie.app). You install it once. After that, Rombie starts it whenever you play an N64 game, and the game plays on Rombie's page like any other console.

### Install

1. Download `rombie-n64-engine-<version>-windows-x64.zip` below and unzip it into a folder you'll keep.
2. Run `rombie-n64-engine.exe` once. It isn't code-signed, so if Windows says it protected your PC, choose **More info**, then **Run anyway**. The engine tells you when it's set up.
3. Play an N64 game in Rombie. The first time, your browser asks to open Rombie N64 Engine: tick **Always allow** and open it. It may also ask whether the site can access other apps and services on your device: allow that too.

If you move the folder, run the engine once more from its new place. To remove it, run `rombie-n64-engine.exe --unregister`, then delete the folder.

### What you need

- Windows 10 (22H2) or Windows 11, 64-bit
- A processor with AVX2 and a graphics card with Vulkan 1.1
- Chrome or Edge on the same computer
- Your own N64 games (`.z64`, `.n64` or `.v64`). None are included, and none leave your computer.

Setup details are in Rombie's Docs page.

### About this release

- New in <version>: <one plain sentence about what is different for the player>
- Built from gopher64, with Rombie's engine mode, all on the `rombie` branch of this repository.
- Runs without a window and talks to Rombie only over your own computer.
- Boots games without Nintendo's PIF boot ROM, which isn't included.
- Rombie shows an "update the N64 engine" message if your version is too old for it. There is no auto-updater.

### License and source

The N64 engine is free software under the GNU GPL v3, as gopher64 is. It's a modified version of gopher64, changed by Bro Code Technologies for Rombie since 2026-11-10; `docs/rombie/notes.md` lists the changes. The complete source for this release is `rombie-n64-engine-<version>-source.zip` below, which builds the engine offline with every submodule and crate it needs. It's the commit tagged `rombie-n64-engine-<version>` in this repository.

`msvcp140.dll`, `vcruntime140.dll` and `vcruntime140_1.dll` in the zip are Microsoft's Visual C++ runtime, distributed under the [Visual Studio 2022 license terms](https://visualstudio.microsoft.com/license-terms/vs2022-ga-community/).

SHA-256:
- `rombie-n64-engine-<version>-windows-x64.zip`: `<sha-256>`
- `rombie-n64-engine-<version>-source.zip`: `<sha-256>`
```
