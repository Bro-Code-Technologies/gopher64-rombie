# Builds the N64 engine's two release files: the download, a zip with the engine, the Visual C++ runtime, its license,
# the licenses of everything compiled into it and a README; and the complete corresponding source, a zip of this
# commit's tree with every submodule and the crates vendored for an offline build. Run from the fork's root, from a
# clean tree, in the build's environment (docs\rombie\notes.md, "Building (Windows)"), after the engine build:
#   powershell -File rombie\package.ps1 [-Out dist] [-SourceStage <short folder>]
# The source is staged in a short folder, since SDL's sources in the vendored crates run deep.
param(
	[string]$Out = 'dist',
	[string]$SourceStage = (Join-Path $env:TEMP 'rn64-source')
)
$ErrorActionPreference = 'Stop'

if (git status --porcelain) { throw 'The tree has changes. Commit them first, so the zips are this commit.' }
$version = (Get-Content rombie\VERSION -TotalCount 1).Trim()
$target = 'x86_64-pc-windows-msvc'
$exe = "target\$target\release\gopher64.exe"
$name = "rombie-n64-engine-$version-windows-x64"
$sourceName = "rombie-n64-engine-$version-source"
$stage = Join-Path $Out $name
$zip = "$stage.zip"
$sourceZip = Join-Path $Out "$sourceName.zip"
foreach ($path in $stage, $zip, $sourceZip, $SourceStage) { if (Test-Path $path) { Remove-Item -Recurse -Force $path } }
New-Item -ItemType Directory -Force "$stage\licenses" | Out-Null

# The engine build prints the engine's version; gopher64's own build has no --version of that kind.
$built = (& $exe --version | Out-String).Trim()
if ($built -ne $version) { throw "$exe says '$built', not $version. Build it with --no-default-features first." }
Copy-Item $exe "$stage\rombie-n64-engine.exe"
Copy-Item LICENSE "$stage\LICENSE.txt"

# The Visual C++ runtime the exe needs, app-local, so nobody has to install the Visual C++ Redistributable first:
# msvcp140.dll and vcruntime140.dll (dumpbin /dependents; the api-ms-win-crt-* ones are Windows' own), and
# vcruntime140_1.dll, which msvcp140.dll imports. Visual Studio's own redist for its toolset, as the PS2 engine's
# package.ps1 takes it.
$vs = & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -version '[17,18)' -latest -property installationPath
$redistVersion = (Get-Content "$vs\VC\Auxiliary\Build\Microsoft.VCRedistVersion.default.txt").Trim()
$crt = "$vs\VC\Redist\MSVC\$redistVersion\x64\Microsoft.VC143.CRT"
foreach ($dll in 'msvcp140', 'vcruntime140', 'vcruntime140_1') { Copy-Item "$crt\$dll.dll" $stage }

$utf8 = New-Object System.Text.UTF8Encoding $false
$rule = '=' * 78
$notices = New-Object System.Text.StringBuilder
function Add-Notice([string]$title, [string[]]$files) {
	[void]$notices.Append("$rule`n$title`n$rule`n`n")
	foreach ($file in $files) {
		[void]$notices.Append(([IO.File]::ReadAllText((Resolve-Path $file))).Replace("`r`n", "`n").TrimEnd() + "`n`n")
	}
}

# What the fork compiles in from its own tree.
Add-Notice 'parallel-rdp (parallel-rdp/parallel-rdp-standalone)' @('parallel-rdp\parallel-rdp-standalone\LICENSE')
Add-Notice 'volk (parallel-rdp/parallel-rdp-standalone/volk)' @('rombie\licenses\volk.txt')
Add-Notice 'The Vulkan headers (parallel-rdp/parallel-rdp-standalone/vulkan-headers)' @('rombie\licenses\vulkan-headers.txt')
Add-Notice "X-Scale's CIC-NUS-6105 algorithm (src/device/pif.rs)" @('rombie\licenses\x-scale-cic-nus-6105.txt')
Add-Notice 'Roboto Mono (data/ui/RobotoMono-Regular.ttf)' @('rombie\licenses\roboto-mono.txt')

$metadata = (cargo metadata --format-version 1 --no-default-features --filter-platform $target) | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed.' }
$packages = @{}
foreach ($package in $metadata.packages) { $packages["$($package.name) v$($package.version)"] = $package }

# C libraries crates build from source into the exe, each with its own license. SDL's comes through sdl3-src and
# SDL_ttf's through sdl3-ttf-src, build dependencies of sdl3-sys and sdl3-ttf-sys. HIDAPI's BSD option is taken, and
# FreeType's FreeType License.
$libraries = @(
	@{ title = 'SDL'; crate = 'sdl3-src'; files = @('SDL\LICENSE.txt', 'SDL\src\hidapi\LICENSE-bsd.txt', 'SDL\src\video\yuv2rgb\LICENSE') },
	@{ title = 'SDL_ttf and FreeType'; crate = 'sdl3-ttf-src'; files = @('SDL_ttf\LICENSE.txt', 'SDL_ttf\external\freetype\LICENSE.TXT', 'SDL_ttf\external\freetype\docs\FTL.TXT') },
	@{ title = 'mimalloc'; crate = 'libmimalloc-sys'; files = @('c_src\mimalloc\v3\LICENSE') },
	@{ title = 'zstd'; crate = 'zstd-sys'; files = @('zstd\LICENSE') }
)
foreach ($library in $libraries) {
	$package = @($metadata.packages | Where-Object { $_.name -eq $library.crate }) | Select-Object -First 1
	if (-not $package) { throw "cargo metadata has no $($library.crate)." }
	$dir = Split-Path $package.manifest_path
	Add-Notice "$($library.title) (built from source by $($package.name) v$($package.version))" ($library.files | ForEach-Object { Join-Path $dir $_ })
}

# Every crate the engine build compiles, as the notes' crate table lists them, with each one's license files.
$tree = cargo tree --target $target --no-default-features -e normal --prefix none --format '{p}|{l}'
if ($LASTEXITCODE -ne 0) { throw 'cargo tree failed.' }
$crates = $tree | ForEach-Object { ($_ -replace ' \(\*\)$', '') } | Where-Object { $_ -and $_ -notmatch '^gopher64 ' } | Sort-Object -Unique
$missing = @()
foreach ($line in $crates) {
	$id, $license = $line -split '\|', 2
	$id = ($id -replace ' \(.*\)$', '').Trim()
	$package = $packages[$id]
	if (-not $package) { throw "cargo metadata has no $id." }
	$dir = Split-Path $package.manifest_path
	$files = @(Get-ChildItem $dir -File | Where-Object { $_.Name -match '^(LICEN[CS]E|COPYING|COPYRIGHT|NOTICE|UNLICENSE)' } | Sort-Object Name | ForEach-Object { $_.FullName })
	$title = "$id ($(if ($license) { $license } else { 'no license field' }))"
	if ($files.Count -eq 0) {
		$missing += $id
		[void]$notices.Append("$rule`n$title`n$rule`n`nThe crate carries no license file. Its authors: $($package.authors -join ', '). Source: $($package.repository)`n`n")
	} else {
		Add-Notice $title $files
	}
}
[IO.File]::WriteAllText((Join-Path (Resolve-Path $stage) 'licenses\THIRD-PARTY.txt'), $notices.ToString(), $utf8)

$release = "https://github.com/Bro-Code-Technologies/gopher64-rombie/releases/tag/rombie-n64-engine-$version"
$readme = @"
Rombie N64 Engine $version

Plays Nintendo 64 games in Rombie (https://rombie.app). It runs gopher64 on this computer, without a window of its
own, and shows the game inside Rombie.

Setting it up
  1. Unzip this folder somewhere it can stay, for example Documents\Rombie N64 Engine.
  2. Run rombie-n64-engine.exe once. It says it's set up, and that's all.
  From then on Rombie starts the engine whenever you play an N64 game. The first time, your browser asks whether to
  open "Rombie N64 Engine": allow it, and tick "always allow" so it doesn't ask again.

Moving or removing it
  Moved the folder? Run rombie-n64-engine.exe once more from its new place.
  To remove it, run "rombie-n64-engine.exe --unregister", then delete the folder.

Privacy
  The engine only accepts connections from this computer, and only from Rombie. Your games stay on this computer:
  Rombie reads them from your ROM folder and hands them to the engine, which keeps them in memory only.

Requirements
  64-bit Windows 10 or 11, a processor with AVX2, and a graphics card with Vulkan 1.1.

License
  The engine is free software under the GNU General Public License v3 (LICENSE.txt). It's built from gopher64
  (https://github.com/gopher64/gopher64). licenses\THIRD-PARTY.txt has the licenses of the libraries, crates, font
  and code it includes.

Modified version
  This is a modified version of gopher64. Bro Code Technologies has changed it for Rombie since 2026-11-10: it adds an
  engine mode with no window that Rombie's page drives over a local connection, boots games without Nintendo's PIF
  boot ROM, and leaves gopher64's netplay, RetroAchievements, web requests and cheats out. docs/rombie/notes.md in the
  source lists the changes with their dates.

Source code
  The complete source of this release is $sourceName.zip, on the same release page as this download:
  $release
  It builds this engine offline, with every submodule and crate it needs. The same commit is the tag
  rombie-n64-engine-$version of https://github.com/Bro-Code-Technologies/gopher64-rombie.

Microsoft files
  msvcp140.dll, vcruntime140.dll and vcruntime140_1.dll (the Visual C++ runtime) are Microsoft's, distributed with the
  engine under the Visual Studio 2022 license terms at
  https://visualstudio.microsoft.com/license-terms/vs2022-ga-community/. You may use them only as part of the engine,
  and you may not modify, reverse engineer or distribute them separately.
  Microsoft provides them as is, without any warranty, and is not liable for any damages arising from them.
"@
[IO.File]::WriteAllText((Join-Path (Resolve-Path $stage) 'README.txt'), ($readme -replace "`r?`n", "`r`n"), $utf8)

Compress-Archive -Path $stage -DestinationPath $zip
"{0} ({1:N1} MB), {2} crates" -f $zip, ((Get-Item $zip).Length / 1MB), $crates.Count
if ($missing.Count) { "Crates with no license file (named with their authors instead): $($missing -join ', ')" }

# The source: this commit's tree (it has no PIF arrays), every submodule at its pinned commit, the crates from
# cargo vendor for Cargo.lock, and the .cargo/config.toml lines that build from them offline. The tree has no .git,
# so rombie/GIT_DESCRIBE stands in for git describe (build.rs).
$src = Join-Path $SourceStage $sourceName
New-Item -ItemType Directory -Force $src | Out-Null
$tar = Join-Path $SourceStage 'part.tar'
git archive --format=tar -o $tar HEAD
tar -xf $tar -C $src
if ($LASTEXITCODE -ne 0) { throw 'Extracting the tree failed.' }
foreach ($line in git submodule status --recursive) {
	$path = ($line.Trim() -split '\s+')[1]
	New-Item -ItemType Directory -Force (Join-Path $src $path) | Out-Null
	git -C $path archive --format=tar -o $tar HEAD
	tar -xf $tar -C (Join-Path $src $path)
	if ($LASTEXITCODE -ne 0) { throw "Extracting the submodule $path failed." }
}
Remove-Item $tar
[IO.File]::WriteAllText((Join-Path $src 'rombie\GIT_DESCRIBE'), (git describe --dirty).Trim() + "`n", $utf8)
cargo vendor --locked (Join-Path $src 'vendor') | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'cargo vendor failed.' }
$config = Join-Path $src '.cargo\config.toml'
$offline = "`n# The source archive's crates (rombie/package.ps1), for an offline build.`n[source.crates-io]`nreplace-with = `"vendored-sources`"`n`n[source.vendored-sources]`ndirectory = `"vendor`"`n"
[IO.File]::WriteAllText($config, ([IO.File]::ReadAllText($config)).TrimEnd() + "`n" + $offline, $utf8)
$fullSourceZip = Join-Path (Resolve-Path $Out) "$sourceName.zip"
tar -a -cf $fullSourceZip -C $SourceStage $sourceName
if ($LASTEXITCODE -ne 0) { throw 'Zipping the source failed.' }
Remove-Item -Recurse -Force $SourceStage
"{0} ({1:N1} MB)" -f $sourceZip, ((Get-Item $sourceZip).Length / 1MB)
foreach ($file in $zip, $sourceZip) { "SHA-256 {0} {1}" -f (Get-FileHash $file -Algorithm SHA256).Hash, (Split-Path $file -Leaf) }
