param(
    [string]$TargetDir = ""
)

$ErrorActionPreference = "Stop"
$mockDir = $PSScriptRoot
$root = (Resolve-Path (Join-Path $mockDir "..\..")).Path
$planPath = Join-Path $mockDir "live-gui-plan.json"
$reportPath = Join-Path $root "qa\live-gui-e2e-report.json"

if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    if (-not [string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        $TargetDir = $env:CARGO_TARGET_DIR
    } else {
        $TargetDir = Join-Path $root "target"
    }
}
$TargetDir = [System.IO.Path]::GetFullPath($TargetDir)

$binPath = Join-Path $TargetDir "debug\ide-fast.exe"
Push-Location $root
try {
    Write-Host "[Live-GUI] 正在构建最新版 ide-fast 桌面可执行程序..." -ForegroundColor Cyan
    & cargo build -j 1 --target-dir $TargetDir -p ide-fast | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build -p ide-fast 失败 (exit=$LASTEXITCODE)"
    }
    if (-not (Test-Path $binPath)) {
        throw "未找到可执行文件: $binPath"
    }

    if (Test-Path $reportPath) {
        Remove-Item -Path $reportPath -Force
    }

    Write-Host "[Live-GUI] 正在真实启动 $binPath --new --e2e-script $planPath ..." -ForegroundColor Cyan
    $proc = Start-Process -FilePath $binPath `
        -ArgumentList @("--new", "--e2e-script", $planPath) `
        -WorkingDirectory $root `
        -PassThru

    $finished = $proc.WaitForExit(60000)
    if (-not $finished) {
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        throw "真实 GUI 进程在 60 秒超时内未完成退出"
    }
    if ($proc.ExitCode -ne 0) {
        throw "真实 GUI 进程非正常退出 (exit=$($proc.ExitCode))"
    }
    if (-not (Test-Path $reportPath)) {
        throw "未生成实测报告文件: $reportPath"
    }

    $rep = Get-Content -Path $reportPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($rep.Finished -ne $true -or $rep.TotalTests -le 0 -or $rep.FailedTests -gt 0) {
        throw "真实 GUI E2E 校验未通过: Total=$($rep.TotalTests), Passed=$($rep.PassedTests), Failed=$($rep.FailedTests), Finished=$($rep.Finished)"
    }

    foreach ($r in $rep.Results) {
        $c = if ($r.Status -eq "Passed") { "Green" } else { "Red" }
        Write-Host ("  [{0}] {1} ({2}): {3}" -f $r.Id, $r.Name, $r.Status, $r.Detail) -ForegroundColor $c
    }
    Write-Host "[Live-GUI] 真实桌面窗口自动实测全部通过: $($rep.PassedTests)/$($rep.TotalTests)" -ForegroundColor Green
} finally {
    Pop-Location
}
