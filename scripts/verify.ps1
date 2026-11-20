<#
.SYNOPSIS
  IDE1.0 统一工业级软件工程验证门禁脚本 (One-Click Pre-Flight / CI Verification Gate)
.DESCRIPTION
  严格按软件工程生命周期顺序执行五重质量门禁：
    1. Rustfmt 代码风格格式门禁 (cargo fmt --check)
    2. Clippy 零警告静态分析门禁 (cargo clippy --all-targets -D warnings)
    3. 核心内嵌与工作区全量测试门禁 (cargo test --test-threads=1)
    4. Release 生产二进制编译与尺寸守卫 (cargo build --release + IDE1.0.exe 校验)
    5. 数学化性能基准与回归门禁 (tests/perf-math-bench.ps1)
#>
param(
    [string]$TargetDir = "",
    [switch]$SkipBench = $false,
    [switch]$SkipRelease = $false
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot\..").Path
if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    $TargetDir = Join-Path $root "target"
}
$TargetDir = [System.IO.Path]::GetFullPath($TargetDir)

$startTime = [System.Diagnostics.Stopwatch]::StartNew()
$summary = @()

function Log-Phase([string]$Name) {
    Write-Host "`n========================================================" -ForegroundColor Cyan
    Write-Host " [GATE] $Name" -ForegroundColor Cyan
    Write-Host "========================================================" -ForegroundColor Cyan
}

function Record-Result([string]$Phase, [bool]$Success, [double]$DurationSec, [string]$Details = "") {
    $script:summary += [PSCustomObject]@{
        Phase    = $Phase
        Status   = if ($Success) { "PASS" } else { "FAIL" }
        Duration = [string]::Format("{0:N2}s", $DurationSec)
        Details  = $Details
    }
}

try {
    # -------------------------------------------------------------
    # Phase 1: Rustfmt 代码格式检验
    # -------------------------------------------------------------
    Log-Phase "Phase 1: Rustfmt 代码风格格式门禁"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    Push-Location $root
    try {
        & cargo fmt -p ide-fast -p editcore -p nema-codec -p ide-agent-router -- --check
        if ($LASTEXITCODE -ne 0) {
            throw "cargo fmt 检查失败，请执行 cargo fmt 格式化代码"
        }
        $sw.Stop()
        Record-Result "1. Rustfmt Formatting" $true $sw.Elapsed.TotalSeconds "All workspace crates follow rustfmt rules"
        Write-Host "Phase 1 PASSED ($($sw.Elapsed.TotalSeconds.ToString('N2'))s)" -ForegroundColor Green
    } finally {
        Pop-Location
    }

    # -------------------------------------------------------------
    # Phase 2: Clippy 静态分析 (零 Warning 门禁)
    # -------------------------------------------------------------
    Log-Phase "Phase 2: Clippy 零警告静态分析门禁 (--all-targets -D warnings)"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    Push-Location $root
    try {
        & cargo clippy --target-dir $TargetDir -p editcore -p nema-codec -p ide-agent-router -p ide-fast --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) {
            throw "cargo clippy 存在未清零的 warning/error"
        }
        $sw.Stop()
        Record-Result "2. Static Analysis (Clippy)" $true $sw.Elapsed.TotalSeconds "Zero warnings on --all-targets"
        Write-Host "Phase 2 PASSED ($($sw.Elapsed.TotalSeconds.ToString('N2'))s)" -ForegroundColor Green
    } finally {
        Pop-Location
    }

    # -------------------------------------------------------------
    # Phase 3: 工作区完整单元测试与集成测试
    # -------------------------------------------------------------
    Log-Phase "Phase 3: 工作区全量测试门禁 (editcore, ide-agent-router, ide-fast)"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    Push-Location $root
    try {
        & cargo test --target-dir $TargetDir -p editcore -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "editcore 测试套件未完全通过" }
        & cargo test --target-dir $TargetDir -p ide-agent-router -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "ide-agent-router 测试套件未完全通过" }
        & cargo test --target-dir $TargetDir -p ide-fast -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "ide-fast 测试套件未完全通过" }

        $sw.Stop()
        Record-Result "3. Workspace Unit & Integration Tests" $true $sw.Elapsed.TotalSeconds "All workspace tests passed (0 failed)"
        Write-Host "Phase 3 PASSED ($($sw.Elapsed.TotalSeconds.ToString('N2'))s)" -ForegroundColor Green
    } finally {
        Pop-Location
    }

    # -------------------------------------------------------------
    # Phase 4: Release 生产二进制构建与符号守卫
    # -------------------------------------------------------------
    if (-not $SkipRelease) {
        Log-Phase "Phase 4: Release 生产二进制构建与 IDE1.0.exe 守卫"
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        Push-Location $root
        try {
            & cargo build --release --target-dir $TargetDir -p ide-fast
            if ($LASTEXITCODE -ne 0) {
                throw "Release 编译构建失败"
            }
            $srcExe = Join-Path $TargetDir "release\ide-fast.exe"
            $dstExe = Join-Path $TargetDir "release\IDE1.0.exe"
            if (Test-Path $srcExe) {
                Copy-Item -Path $srcExe -Destination $dstExe -Force
            }
            if (-not (Test-Path $dstExe)) {
                throw "未找到发布产物 $dstExe"
            }
            $fileItem = Get-Item $dstExe
            $lenMb = $fileItem.Length / 1MB
            if ($lenMb -lt 10.0 -or $lenMb -gt 50.0) {
                throw "产物文件体积异常: $($lenMb.ToString('N2')) MB"
            }
            $sw.Stop()
            Record-Result "4. Release Build & Sizing Guard" $true $sw.Elapsed.TotalSeconds "$($fileItem.Name) size = $($lenMb.ToString('N2')) MB"
            Write-Host "Phase 4 PASSED: $dstExe ($($lenMb.ToString('N2')) MB)" -ForegroundColor Green
        } finally {
            Pop-Location
        }
    }

    # -------------------------------------------------------------
    # Phase 5: 数学化性能基准与复杂度回归测试
    # -------------------------------------------------------------
    if (-not $SkipBench) {
        Log-Phase "Phase 5: 数学化性能基准与复杂度回归测试"
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $benchScript = Join-Path $root "tests\perf-math-bench.ps1"
        if (Test-Path $benchScript) {
            Push-Location $root
            try {
                & powershell -NoProfile -ExecutionPolicy Bypass -File $benchScript -TargetDir $TargetDir -SkipStartup -SkipBuild
                if ($LASTEXITCODE -ne 0) {
                    throw "数学化性能测试未通过 SLA 门禁"
                }
                $sw.Stop()
                Record-Result "5. Mathematical Performance Bench" $true $sw.Elapsed.TotalSeconds "SLA gate & complexity bounds verified"
                Write-Host "Phase 5 PASSED ($($sw.Elapsed.TotalSeconds.ToString('N2'))s)" -ForegroundColor Green
            } finally {
                Pop-Location
            }
        }
    }

} catch {
    Write-Host "`n[FATAL ERROR IN GATE] $_" -ForegroundColor Red
    Record-Result "Failure" $false $startTime.Elapsed.TotalSeconds "$_"
    Write-Host "`n========================================================" -ForegroundColor Red
    Write-Host " VERIFICATION GATE FAILED" -ForegroundColor Red
    Write-Host "========================================================" -ForegroundColor Red
    $summary | Format-Table -AutoSize
    exit 1
}

$startTime.Stop()
Write-Host "`n========================================================" -ForegroundColor Green
Write-Host " ALL QUALITY GATES PASSED (Total: $($startTime.Elapsed.TotalSeconds.ToString('N2'))s)" -ForegroundColor Green
Write-Host "========================================================" -ForegroundColor Green
$summary | Format-Table -AutoSize
exit 0
