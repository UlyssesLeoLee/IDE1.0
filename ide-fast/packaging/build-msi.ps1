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
$rawExe = Join-Path $Target "release\ide-fast.exe"
$exe = Join-Path $Target "release\IDE1.0.exe"
if (Test-Path $rawExe) {
    Copy-Item $rawExe $exe -Force
}

# 验证二进制可启动性门禁 (Launch Smoke Test)
Write-Host "=== Running Launch Smoke Test ==="
& $exe --smoke-test
if ($LASTEXITCODE -ne 0) {
    throw "Built IDE1.0.exe failed smoke test with exit code $LASTEXITCODE"
}
Write-Host "=== Launch Smoke Test PASSED ==="

New-Item -ItemType Directory -Force $Out | Out-Null
$wixobj = Join-Path $Out "ide-fast.wixobj"
$icon = Join-Path $PSScriptRoot "ide-fast.ico"
& "$wix\candle.exe" -nologo -arch x64 "-dVersion=$wixVer" "-dExePath=$exe" "-dIconPath=$icon" -out $wixobj "$PSScriptRoot\ide-fast.wxs"
if ($LASTEXITCODE) { throw "candle failed" }
$msi = Join-Path $Out "IDE1.0_${ver}_x64.msi"
& "$wix\light.exe" -nologo -cultures:en-us -sice:ICE91 -sice:ICE60 -out $msi $wixobj
if ($LASTEXITCODE) { throw "light failed" }

# 验证 MSI 解包产物完整性与可启动性门禁
Write-Host "=== Running MSI Extraction & Smoke Test ==="
$msiExtractDir = Join-Path $env:TEMP "ide10_msi_extract_$PID"
if (Test-Path $msiExtractDir) { Remove-Item $msiExtractDir -Recurse -Force -ErrorAction SilentlyContinue }
$msiProc = Start-Process -FilePath "msiexec.exe" -ArgumentList @("/a", "`"$msi`"", "/qn", "TARGETDIR=`"$msiExtractDir`"") -Wait -PassThru
if ($msiProc.ExitCode -ne 0) {
    Remove-Item $msiExtractDir -Recurse -Force -ErrorAction SilentlyContinue
    throw "msiexec /a extraction failed with exit code $($msiProc.ExitCode)"
}
$extractedExe = Get-ChildItem -Path $msiExtractDir -Filter "IDE1.0.exe" -Recurse | Select-Object -First 1
if (-not $extractedExe) {
    Remove-Item $msiExtractDir -Recurse -Force -ErrorAction SilentlyContinue
    throw "MSI extraction check failed: IDE1.0.exe not found inside $msi"
}
& $extractedExe.FullName --smoke-test
$extractSmokeCode = $LASTEXITCODE
Remove-Item $msiExtractDir -Recurse -Force -ErrorAction SilentlyContinue
if ($extractSmokeCode -ne 0) {
    throw "MSI-extracted IDE1.0.exe failed --smoke-test with exit code $extractSmokeCode"
}
Write-Host "=== MSI Extraction & Smoke Test PASSED ==="

$portableExe = Join-Path $Out "IDE1.0-$ver-windows-x64.exe"
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
