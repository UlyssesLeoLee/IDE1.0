<#
.SYNOPSIS
  IDE1.0 数学化性能基准测试脚本与复杂度回归验收框架
.DESCRIPTION
  1. 构建 IDE1.0 Release 二进制 (`target\release\IDE1.0.exe`)。
  2. 运行 Rust 无头数理基准 (`perf_bench_open`, `perf_bench_find_undo`, `perf_bench_frames`,
     `perf_bench_startup_and_ops`, `perf_bench_mathematical_complexity`):
     - 统计分布: Warmup, Sample N, P50 (Median), Mean (mu), StdDev (sigma), CV = sigma/mu,
       Student's t 95% 置信区间 (+/- t_0.975 * sigma / sqrt(N), 下界>=0), Tukey 1.5*IQR 离群值与
       去极值均值, P90/P95/P99 (N>=5), IQR, Throughput, SLA Gate.
     - OLS 对数-对数复杂度拟合: ln T(N) = alpha * ln N + beta (4尺度 5k/20k/80k/200k).
  3. 对 `IDE1.0.exe` 执行真实进程冷启动追踪 (1 次预热 + N 次测量, IDE10_TRACE + 工作集统计).
  4. 支持 `-Compare <baseline.json>` 历史基线对比与回归拦截 (>+30% 且 >+1ms -> REG).
#>
param(
    [string]$TargetDir = "",
    [int]$StartupRuns = 5,
    [switch]$SkipStartup = $false,
    [switch]$SkipBuild = $false,
    [string]$ReportPath = "",
    [string]$Compare = ""
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot\..").Path
if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    $TargetDir = Join-Path $root "target"
}
$TargetDir = [System.IO.Path]::GetFullPath($TargetDir)
if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $root "qa\perf-math-report.json"
}
$qaDir = Split-Path -Parent $ReportPath
if (-not (Test-Path $qaDir)) {
    New-Item -ItemType Directory -Force -Path $qaDir | Out-Null
}

function Get-Percentile([double[]]$Sorted, [double]$P) {
    $n = $Sorted.Count
    if ($n -eq 0) { return 0.0 }
    if ($n -eq 1) { return $Sorted[0] }
    $pos = [Math]::Max(0.0, [Math]::Min(1.0, $P)) * ($n - 1)
    $lo = [int][Math]::Floor($pos)
    $hi = [int][Math]::Ceiling($pos)
    $frac = $pos - $lo
    return $Sorted[$lo] * (1.0 - $frac) + $Sorted[$hi] * $frac
}

function Get-TCrit95([int]$Df) {
    $table = @(
        12.706204736432095, 4.302652729911275, 3.182446305284263, 2.7764451051977987, 2.5705818366147395,
        2.4469118487916806, 2.3646242510102993, 2.3060041350333704, 2.2621571627409915, 2.2281388519649385,
        2.200985160082949,  2.1788128296634177, 2.1603686564610127, 2.1447866879169273, 2.131449545559323,
        2.1199052992210112, 2.1098155778331806, 2.10092204024096,   2.093024054408263,  2.0859634472658364,
        2.079613844727662,  2.0738730679040147, 2.0686576104190406, 2.0638985616280205, 2.059538552753294,
        2.055529438642871,  2.0518305164802833, 2.048407141795244,  2.045229642132703,  2.0422724563012373
    )
    if ($Df -le 0) { return 0.0 }
    if ($Df -le $table.Count) { return $table[$Df - 1] }
    $z = 1.959963984540054
    $d = [double]$Df
    return $z + ([Math]::Pow($z, 3) + $z) / (4.0 * $d) + (5.0 * [Math]::Pow($z, 5) + 16.0 * [Math]::Pow($z, 3) + 3.0 * $z) / (96.0 * $d * $d)
}

function Get-MathStats([string]$Name, [double[]]$Raw, [double]$SlaLimitMs) {
    $n = $Raw.Count
    $sorted = @($Raw | Sort-Object)
    $sum = ($sorted | Measure-Object -Sum).Sum
    $mean = $sum / $n
    $std = 0.0
    if ($n -gt 1) {
        $ss = 0.0
        foreach ($x in $sorted) { $d = $x - $mean; $ss += $d * $d }
        $std = [Math]::Sqrt($ss / ($n - 1))
    }
    $cv = if ($mean -gt 1e-12) { $std / $mean } else { 0.0 }
    $tCrit = Get-TCrit95 ([Math]::Max(0, $n - 1))
    $ci95 = $tCrit * $std / [Math]::Sqrt($n)
    $ciLow = [Math]::Max(0.0, $mean - $ci95)
    $ciHigh = $mean + $ci95
    $p25 = Get-Percentile $sorted 0.25
    $p50 = Get-Percentile $sorted 0.50
    $p75 = Get-Percentile $sorted 0.75
    $iqr = $p75 - $p25
    $p90 = if ($n -ge 5) { [Math]::Round((Get-Percentile $sorted 0.90), 3) } else { $null }
    $p95 = if ($n -ge 5) { [Math]::Round((Get-Percentile $sorted 0.95), 3) } else { $null }
    $p99 = if ($n -ge 5) { [Math]::Round((Get-Percentile $sorted 0.99), 3) } else { $null }

    $outliers = 0
    $tMean = $mean
    if ($n -ge 4) {
        $loFence = $p25 - 1.5 * $iqr
        $hiFence = $p75 + 1.5 * $iqr
        $inlierSum = 0.0
        $inlierCnt = 0
        foreach ($x in $sorted) {
            if ($x -lt $loFence -or $x -gt $hiFence) {
                $outliers++
            } else {
                $inlierSum += $x
                $inlierCnt++
            }
        }
        if ($inlierCnt -gt 0) { $tMean = $inlierSum / $inlierCnt }
    }

    return [ordered]@{
        name = $Name
        n = $n
        median_ms = [Math]::Round($p50, 3)
        mean_ms = [Math]::Round($mean, 3)
        std_dev_ms = [Math]::Round($std, 3)
        cv = [Math]::Round($cv, 4)
        ci95_ms = [Math]::Round($ci95, 3)
        ci95_low_ms = [Math]::Round($ciLow, 3)
        ci95_high_ms = [Math]::Round($ciHigh, 3)
        min_ms = [Math]::Round($sorted[0], 3)
        max_ms = [Math]::Round($sorted[$n - 1], 3)
        p90_ms = $p90
        p95_ms = $p95
        p99_ms = $p99
        iqr_ms = [Math]::Round($iqr, 3)
        outliers = $outliers
        trimmed_mean_ms = [Math]::Round($tMean, 3)
        sla_limit_ms = $SlaLimitMs
        sla_pass = ($SlaLimitMs -le 0 -or $p50 -le $SlaLimitMs)
    }
}

Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "           IDE1.0 数学化性能评估与渐近复杂度证明框架 (Math Perf Suite)          " -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "工作区根目录 : $root" -ForegroundColor Gray
Write-Host "目标产物目录 : $TargetDir" -ForegroundColor Gray

# 1. 构建并同步 IDE1.0.exe
$releaseDir = Join-Path $TargetDir "release"
$rawExe = Join-Path $releaseDir "ide-fast.exe"
$ideExe = Join-Path $releaseDir "IDE1.0.exe"

if (-not $SkipBuild) {
    Write-Host "`n[1/3] 构建 Release 产物 (cargo build --release -p ide-fast)..." -ForegroundColor Yellow
    Push-Location $root
    try {
        & cargo build --release --target-dir $TargetDir -p ide-fast
        if ($LASTEXITCODE -ne 0) { throw "Release build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
}
if (Test-Path $rawExe) {
    Copy-Item $rawExe $ideExe -Force
    Write-Host "      已同步发行二进制: $ideExe" -ForegroundColor Green
}

# 2. 运行 Rust 无头数学基准与渐近复杂度回归检验
Write-Host "`n[2/3] 运行 Rust 无头统计分布与 OLS 复杂度检验 (perf_bench)..." -ForegroundColor Yellow
Push-Location $root
$prevEap = $ErrorActionPreference
try {
    $ErrorActionPreference = "Continue"
    $benchOut = & cargo test --target-dir $TargetDir -p ide-fast --release perf_bench -- --ignored --nocapture --test-threads=1 2>&1 | ForEach-Object { "$_" } | Out-String
    $benchExit = $LASTEXITCODE
} finally {
    $ErrorActionPreference = $prevEap
    Pop-Location
}

$mathRecords = @()
$complexityRecords = @()
foreach ($line in ($benchOut -split "`r?`n")) {
    if ($line -match '^PERF_MATH\s+(\{.*\})$') {
        $mathRecords += ($Matches[1] | ConvertFrom-Json)
    } elseif ($line -match '^PERF_COMPLEXITY\s+(\{.*\})$') {
        $complexityRecords += ($Matches[1] | ConvertFrom-Json)
    }
}

if ($benchExit -ne 0 -or $mathRecords.Count -eq 0 -or $complexityRecords.Count -eq 0) {
    Write-Host $benchOut -ForegroundColor Red
    throw "perf_bench execution failed (exit=$benchExit, math=$($mathRecords.Count), complexity=$($complexityRecords.Count))"
}

# 3. 冷启动真实进程追踪 (可选: 1 次未计时预热 + N 次测量)
$startupStats = @()
if (-not $SkipStartup) {
    if (-not (Test-Path $ideExe)) {
        throw "找不到二进制文件: $ideExe"
    }
    Write-Host "`n[3/3] 测量 $ideExe 冷启动统计分布 (Warmup=1, N=$StartupRuns)..." -ForegroundColor Yellow
    $tmpDir = Join-Path $env:TEMP ("IDE1.0-math-perf-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
    New-Item -ItemType Directory -Force $tmpDir | Out-Null
    $points = @{}
    $wall = @()
    $rss = @()
    $oldS10 = $env:IDE10_SETTINGS; $oldT10 = $env:IDE10_TRACE
    $oldS = $env:IDE_FAST_SETTINGS; $oldT = $env:IDE_FAST_TRACE
    try {
        Remove-Item Env:IDE_FAST_SETTINGS -ErrorAction SilentlyContinue
        Remove-Item Env:IDE_FAST_TRACE -ErrorAction SilentlyContinue
        for ($i = 0; $i -le $StartupRuns; $i++) {
            $isWarmup = ($i -eq 0)
            $trace = if ($isWarmup) { Join-Path $tmpDir "trace_warmup.txt" } else { Join-Path $tmpDir "trace_$i.txt" }
            $env:IDE10_SETTINGS = "off"
            $env:IDE10_TRACE = $trace
            $sw = [Diagnostics.Stopwatch]::StartNew()
            $p = Start-Process $ideExe -ArgumentList "--new" -PassThru
            try {
                $seen = $false
                while ($sw.ElapsedMilliseconds -lt 15000) {
                    if ((Test-Path $trace) -and ((Get-Content $trace -ErrorAction SilentlyContinue) -match "first frame")) {
                        $seen = $true
                        break
                    }
                    Start-Sleep -Milliseconds 5
                }
                if ($seen -and -not $isWarmup) {
                    $wall += $sw.Elapsed.TotalMilliseconds
                    $p.Refresh()
                    $rss += ($p.WorkingSet64 / 1MB)
                    foreach ($l in Get-Content $trace) {
                        if ($l -match '^\[fast \+([0-9.]+)ms\] (.+)$') {
                            $k = $Matches[2]
                            if (-not $points.ContainsKey($k)) { $points[$k] = @() }
                            $points[$k] += [double]$Matches[1]
                        }
                    }
                }
            } finally {
                if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
            }
        }
    } finally {
        if ($null -eq $oldS10) { Remove-Item Env:IDE10_SETTINGS -ErrorAction SilentlyContinue } else { $env:IDE10_SETTINGS = $oldS10 }
        if ($null -eq $oldT10) { Remove-Item Env:IDE10_TRACE -ErrorAction SilentlyContinue } else { $env:IDE10_TRACE = $oldT10 }
        if ($null -eq $oldS) { Remove-Item Env:IDE_FAST_SETTINGS -ErrorAction SilentlyContinue } else { $env:IDE_FAST_SETTINGS = $oldS }
        if ($null -eq $oldT) { Remove-Item Env:IDE_FAST_TRACE -ErrorAction SilentlyContinue } else { $env:IDE_FAST_TRACE = $oldT }
        Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
    }
    if ($wall.Count -gt 0) {
        $startupStats += (Get-MathStats "startup_wall_first_frame_ms" $wall 800.0)
    }
    if ($points.ContainsKey("first frame")) {
        $startupStats += (Get-MathStats "startup_trace_first_frame_ms" $points["first frame"] 600.0)
    }
    if ($rss.Count -gt 0) {
        $startupStats += (Get-MathStats "startup_working_set_mb" $rss 250.0)
    }
} else {
    Write-Host "`n[3/3] 已跳过真实窗口冷启动探针 (-SkipStartup)" -ForegroundColor Gray
}

function Format-OptMs($v) {
    if ($null -eq $v) { "-" } else { "{0:F3}" -f [double]$v }
}

# 打印数学统计报表
Write-Host "`n---------------- 1. 数理统计分布 (Latency & Throughput) ----------------" -ForegroundColor Cyan
$header = "{0,-34} {1,4} {2,9} {3,16} {4,7} {5,9} {6,9} {7,12} {8,6}" -f "Metric", "N", "P50(ms)", "Mean +/- CI95", "CV", "P95(ms)", "P99(ms)", "Throughput", "SLA"
Write-Host $header -ForegroundColor White
Write-Host ("-" * 116) -ForegroundColor DarkGray

$allPassed = $true
$baseJson = $null
if ($Compare -and (Test-Path $Compare)) {
    $baseJson = Get-Content $Compare -Raw | ConvertFrom-Json
}

foreach ($m in $mathRecords) {
    $slaTxt = if ($null -eq $m.sla_limit_ms) { "N/A" } elseif ($m.sla_pass) { "PASS" } else { "FAIL" }
    if (-not $m.sla_pass) { $allPassed = $false }
    $meanCi = ("{0:F2}+/-{1:F2}" -f $m.mean_ms, $m.ci95_ms)
    $tpTxt = if ($m.throughput -gt 0) { ("{0:N0} {1}" -f $m.throughput, $m.unit) } else { "-" }
    $p95Txt = Format-OptMs $m.p95_ms
    $p99Txt = Format-OptMs $m.p99_ms
    $rowColor = if ($m.sla_pass) { "Gray" } else { "Red" }
    $line = "{0,-34} {1,4} {2,9:F3} {3,16} {4,7:F3} {5,9} {6,9} {7,12} {8,6}" -f $m.name, $m.n, $m.median_ms, $meanCi, $m.cv, $p95Txt, $p99Txt, $tpTxt, $slaTxt
    Write-Host $line -ForegroundColor $rowColor
}

foreach ($s in $startupStats) {
    $slaTxt = if ($s.sla_pass) { "PASS" } else { "FAIL" }
    if (-not $s.sla_pass) { $allPassed = $false }
    $meanCi = ("{0:F2}+/-{1:F2}" -f $s.mean_ms, $s.ci95_ms)
    $p95Txt = Format-OptMs $s.p95_ms
    $p99Txt = Format-OptMs $s.p99_ms
    $rowColor = if ($s.sla_pass) { "Gray" } else { "Red" }
    $line = "{0,-34} {1,4} {2,9:F3} {3,16} {4,7:F3} {5,9} {6,9} {7,12} {8,6}" -f $s.name, $s.n, $s.median_ms, $meanCi, $s.cv, $p95Txt, $p99Txt, "-", $slaTxt
    Write-Host $line -ForegroundColor $rowColor
}

Write-Host "`n---------------- 2. OLS 对数-对数复杂度拟合 (Asymptotic Scaling) ----------------" -ForegroundColor Cyan
$cHeader = "{0,-38} {1,8} {2,10} {3,10} {4,10} {5,28} {6,6}" -f "Benchmark", "Expect", "Alpha", "MaxAlpha", "R^2", "Medians(5k/20k/80k/200k)", "Gate"
Write-Host $cHeader -ForegroundColor White
Write-Host ("-" * 118) -ForegroundColor DarkGray
foreach ($c in $complexityRecords) {
    $gate = if ($c.pass) { "PASS" } else { "FAIL" }
    if (-not $c.pass) { $allPassed = $false }
    $meds = ($c.medians_ms | ForEach-Object { "{0:F2}" -f $_ }) -join " / "
    $rowColor = if ($c.pass) { "Green" } else { "Red" }
    $line = "{0,-38} {1,8} {2,10:F4} {3,10:F2} {4,10:F4} {5,28} {6,6}" -f $c.name, $c.expected, $c.alpha, $c.max_alpha, $c.r_squared, $meds, $gate
    Write-Host $line -ForegroundColor $rowColor
}

$baseMap = @{}
if ($baseJson) {
    foreach ($item in @($baseJson.Metrics) + @($baseJson.Startup)) {
        if ($null -ne $item -and $item.name) {
            $baseMap[$item.name] = [double]$item.median_ms
        }
    }
}
$compareRows = @()
if ($baseMap.Count -gt 0) {
    Write-Host "`n---------------- 3. 基线对比回归分析 (Baseline Comparison) ----------------" -ForegroundColor Cyan
    Write-Host ("{0,-34} {1,10} {2,10} {3,10} {4,9} {5,6}" -f "Metric", "Base(ms)", "Curr(ms)", "Delta(ms)", "Delta(%)", "Gate") -ForegroundColor White
    Write-Host ("-" * 86) -ForegroundColor DarkGray
    foreach ($cur in @($mathRecords) + @($startupStats)) {
        if ($baseMap.ContainsKey($cur.name)) {
            $baseMs = $baseMap[$cur.name]
            $currMs = [double]$cur.median_ms
            $deltaMs = [Math]::Round($currMs - $baseMs, 3)
            $deltaPct = if ($baseMs -gt 1e-9) { [Math]::Round((($currMs - $baseMs) / $baseMs) * 100.0, 2) } else { 0.0 }
            $regressed = ($deltaPct -gt 30.0 -and $deltaMs -gt 1.0)
            $status = if ($regressed) { "REG" } else { "OK" }
            if ($regressed) { $allPassed = $false }
            $color = if ($regressed) { "Red" } elseif ($deltaPct -lt -5.0) { "Green" } else { "Gray" }
            Write-Host ("{0,-34} {1,10:F3} {2,10:F3} {3,10:F3} {4,8:F2}% {5,6}" -f $cur.name, $baseMs, $currMs, $deltaMs, $deltaPct, $status) -ForegroundColor $color
            $compareRows += [ordered]@{
                name = $cur.name
                baseline_ms = $baseMs
                current_ms = $currMs
                delta_ms = $deltaMs
                delta_pct = $deltaPct
                status = $status
            }
        }
    }
}

$cpuName = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
$ramGb = [Math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
$report = [ordered]@{
    Product = "IDE1.0"
    Timestamp = (Get-Date).ToString("o")
    BinaryPath = $ideExe
    Machine = [ordered]@{
        Cpu = $cpuName
        RamGb = $ramGb
        Os = [Environment]::OSVersion.VersionString
    }
    Metrics = $mathRecords
    Complexity = $complexityRecords
    Startup = $startupStats
    Comparison = $compareRows
    Summary = [ordered]@{
        MetricsCount = $mathRecords.Count
        ComplexityCount = $complexityRecords.Count
        StartupCount = $startupStats.Count
        AllGatesPassed = $allPassed
    }
}

$report | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $ReportPath
Write-Host "`n数学性能测试报告已写入: $ReportPath" -ForegroundColor Cyan

if (-not $allPassed) {
    Write-Host "性能数学验收门禁未通过!" -ForegroundColor Red
    exit 1
}
Write-Host "所有 SLA 与渐近复杂度数学验算门禁 100% 通过!" -ForegroundColor Green
exit 0
