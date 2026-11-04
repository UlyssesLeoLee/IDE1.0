# 构建 IDE1.0 Fast 的独立安装包 (.msi) 和便携版 (.exe)。
# 用法 (仓库根目录):  powershell -File ide-fast/packaging/build-msi.ps1 [-Target <cargo 产物目录>] [-Out <输出目录>]
# 前提: 已有 WiX v3 (Tauri 打包时会下载到 %LOCALAPPDATA%\tauri\WixTools314, 也可设 WIX_BIN 指向别处) 和 cargo。
param(
  [string]$Target = "$PSScriptRoot\..\..\target",
  [string]$Out = "$PSScriptRoot\..\..\dist-fast"
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot\..\..").Path
$wix = if ($env:WIX_BIN) { $env:WIX_BIN } else { "$env:LOCALAPPDATA\tauri\WixTools314" }
if (-not (Test-Path "$wix\candle.exe")) { throw "找不到 WiX v3 ($wix\candle.exe); 设 WIX_BIN 指向含 candle.exe / light.exe 的目录" }
$ver = (Select-String -Path "$root\Cargo.toml" -Pattern '^version = "([0-9.]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$wixVer = "$ver.0"     # MSI 版本要四段
$env:CARGO_TARGET_DIR = $Target
Push-Location $root
cargo build --release -p ide-fast
if ($LASTEXITCODE) { Pop-Location; throw "cargo 构建失败" }
Pop-Location
$exe = Join-Path $Target "release\ide-fast.exe"
New-Item -ItemType Directory -Force $Out | Out-Null
# 许可证对话框用的 RTF (AGPL-3.0 全文太长, 这里放一句说明 + 指向 LICENSE)
$rtf = Join-Path $Out "license.rtf"
'{\rtf1\ansi IDE1.0 Fast is licensed under the GNU AGPL-3.0. See the LICENSE file in the source repository: https://github.com/UlyssesLeoLee/IDE1.0}' | Set-Content -Encoding ascii $rtf
$wixobj = Join-Path $Out "ide-fast.wixobj"
$icon = Join-Path $PSScriptRoot "ide-fast.ico"
& "$wix\candle.exe" -nologo -arch x64 "-dVersion=$wixVer" "-dExePath=$exe" "-dIconPath=$icon" "-dLicenseRtf=$rtf" -out $wixobj "$PSScriptRoot\ide-fast.wxs"
if ($LASTEXITCODE) { throw "candle 失败" }
$msi = Join-Path $Out "IDE1.0-Fast_${ver}_x64.msi"
& "$wix\light.exe" -nologo -ext WixUIExtension -cultures:en-us -sice:ICE91 -out $msi $wixobj
if ($LASTEXITCODE) { throw "light 失败" }
Copy-Item $exe (Join-Path $Out "ide-fast-$ver-windows-x64.exe") -Force
Write-Host "完成: $msi"
