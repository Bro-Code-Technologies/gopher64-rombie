# Builds the N64 engine's download: a zip with the engine, its license, the licenses of everything compiled into it
# and a README. Run from the fork's root, in the build's environment (docs\rombie\notes.md, "Building (Windows)"),
# after the engine build:
#   powershell -File rombie\package.ps1 [-Out dist]
param(
	[string]$Out = 'dist'
)
$ErrorActionPreference = 'Stop'

$version = (Get-Content rombie\VERSION -TotalCount 1).Trim()
$target = 'x86_64-pc-windows-msvc'
$exe = "target\$target\release\gopher64.exe"
$name = "rombie-n64-engine-$version-windows-x64"
$stage = Join-Path $Out $name
$zip = "$stage.zip"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
if (Test-Path $zip) { Remove-Item -Force $zip }
New-Item -ItemType Directory -Force "$stage\licenses" | Out-Null

# The engine build prints the engine's version; gopher64's own build has no --version of that kind.
$built = (& $exe --version | Out-String).Trim()
if ($built -ne $version) { throw "$exe says '$built', not $version. Build it with --no-default-features first." }
Copy-Item $exe "$stage\rombie-n64-engine.exe"
Copy-Item LICENSE "$stage\LICENSE.txt"

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
  Source code: https://github.com/Bro-Code-Technologies/gopher64-rombie/tree/rombie-n64-engine-$version
"@
[IO.File]::WriteAllText((Join-Path (Resolve-Path $stage) 'README.txt'), ($readme -replace "`r?`n", "`r`n"), $utf8)

Compress-Archive -Path $stage -DestinationPath $zip
"{0} ({1:N1} MB), {2} crates" -f $zip, ((Get-Item $zip).Length / 1MB), $crates.Count
if ($missing.Count) { "Crates with no license file (named with their authors instead): $($missing -join ', ')" }
