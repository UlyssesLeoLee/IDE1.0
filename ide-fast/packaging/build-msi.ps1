param(
  [string]$Target,
  [string]$Out
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot\..\..").Path

if (-not $Target) {
    if ($env:CARGO_TARGET_DIR) {
        $Target = $env:CARGO_TARGET_DIR
    } else {
        $Target = Join-Path $root "target"
    }
}
if (-not $Out) { $Out = Join-Path $root "dist-fast" }
$Target = [System.IO.Path]::GetFullPath($Target)
$Out = [System.IO.Path]::GetFullPath($Out)

$wixCandidates = @(
    if ($env:WIX_BIN) { $env:WIX_BIN },
    "$env:LOCALAPPDATA\tauri\WixTools314",
    "C:\Program Files (x86)\WiX Toolset v3.14\bin",
    "C:\Program Files (x86)\WiX Toolset v3.11\bin"
)
$wix = $null
foreach ($c in $wixCandidates) {
    if ($c -and (Test-Path (Join-Path $c "candle.exe"))) {
        $wix = $c
        break
    }
}
if (-not $wix) {
    $cmd = Get-Command candle.exe -ErrorAction SilentlyContinue
    if ($cmd) {
        $wix = Split-Path $cmd.Source
    }
}
if (-not $wix) { throw "WiX v3 not found (candle.exe / light.exe); please set WIX_BIN" }
$verMatch = Select-String -Path "$root\Cargo.toml" -Pattern 'version\s*=\s*"([0-9.]+)"' | Select-Object -First 1
if (-not $verMatch) { throw "Cannot parse version from Cargo.toml" }
$ver = $verMatch.Matches[0].Groups[1].Value
$wixVer = "$ver.0"
$env:CARGO_TARGET_DIR = $Target
Push-Location $root
$cargoCmd = Get-Command cargo.exe -ErrorAction SilentlyContinue
$cargoBin = if ($cargoCmd) { $cargoCmd.Source } elseif (Test-Path "C:\Users\leo19\.codex\toolchains\star-rustup\cargo-home\bin\cargo.exe") { "C:\Users\leo19\.codex\toolchains\star-rustup\cargo-home\bin\cargo.exe" } else { "cargo" }
& $cargoBin build --release -j 2 -p ide-fast
if ($LASTEXITCODE) { Pop-Location; throw "cargo build failed" }
Pop-Location
$exe = Join-Path $Target "release\ide-fast.exe"
New-Item -ItemType Directory -Force $Out | Out-Null
$rtf = Join-Path $Out "license.rtf"
'{\rtf1\ansi IDE1.0 Fast is licensed under the GNU AGPL-3.0. See the LICENSE file in the source repository: https://github.com/UlyssesLeoLee/IDE1.0}' | Set-Content -Encoding ascii $rtf
$wixobj = Join-Path $Out "ide-fast.wixobj"
$icon = Join-Path $PSScriptRoot "ide-fast.ico"
$banner = Join-Path $PSScriptRoot "banner.bmp"
$dialog = Join-Path $PSScriptRoot "dialog.bmp"
if (-not (Test-Path $banner) -or -not (Test-Path $dialog)) {
    & python (Join-Path $PSScriptRoot "generate_comic_art.py")
}
& "$wix\candle.exe" -nologo -arch x64 "-dVersion=$wixVer" "-dExePath=$exe" "-dIconPath=$icon" "-dLicenseRtf=$rtf" "-dBannerBmp=$banner" "-dDialogBmp=$dialog" -out $wixobj "$PSScriptRoot\ide-fast.wxs"
if ($LASTEXITCODE) { throw "candle failed" }
$msi = Join-Path $Out "IDE1.0-Fast_${ver}_x64.msi"
& "$wix\light.exe" -nologo -ext WixUIExtension -cultures:en-us -sice:ICE91 -sice:ICE60 -out $msi $wixobj
if ($LASTEXITCODE) { throw "light failed" }
$portableExe = Join-Path $Out "ide-fast-$ver-windows-x64.exe"
Copy-Item $exe $portableExe -Force

$channel = if ($ver -match "-") { "dev" } else { "stable" }
$msiName = [System.IO.Path]::GetFileName($msi)
$exeName = [System.IO.Path]::GetFileName($portableExe)
$msiHash = (Get-FileHash -Algorithm SHA256 $msi).Hash.ToLowerInvariant()
$exeHash = (Get-FileHash -Algorithm SHA256 $portableExe).Hash.ToLowerInvariant()
$sumsContent = "version: $ver channel: $channel`n$msiHash  $msiName`n$exeHash  $exeName`n"
$sumsPath = Join-Path $Out "SHA256SUMS.txt"
[System.IO.File]::WriteAllText($sumsPath, $sumsContent, [System.Text.Encoding]::ASCII)

Write-Host "MSI: $msi"
Write-Host "Portable: $portableExe"
Write-Host "Checksums: $sumsPath"
