<#
.SYNOPSIS
  IDE1.0 性能回归基准: 冷启动各阶段 / 空闲资源 (对 IDE1.0.exe), 以及文本引擎的数理统计无头基准 (cargo test --ignored)。

.DESCRIPTION
  1. 启动: 对 -Exe 连跑 -Runs 次冷启动 (IDE_FAST_SETTINGS=off, IDE_FAST_TRACE=<文件>, --new),
     取 main 里各追踪点的 Median / Mean / StdDev / CV / P90 / P95 / P99;
     首帧后再空闲 -Idle 秒, 读常驻内存、线程数, 并在随后 2 秒里量 CPU。
  2. 引擎: cargo test -p ide-fast --release perf_bench -- --ignored (打开 5MB / 30MB / 20 万行 / 1MB 单行,
     滚动一帧, 全部查找, 撤销, 数学复杂度), 解析 `PERF` 与 `PERF_MATH` 行。

.EXAMPLE
  powershell -File ide-fast/tests/perf.ps1 -Json before.json
  powershell -File ide-fast/tests/perf.ps1 -Compare before.json      # 与之前保存的结果对比
  powershell -File ide-fast/tests/perf.ps1 -SkipStartup              # 只跑引擎基准
#>
param(
    [string]$Exe = "",
    [int]$Runs = 10,
    [int]$Idle = 3,
    [switch]$SkipStartup,
    [switch]$SkipEngine,
    [string]$Json = "",
    [string]$Compare = "",
    [string]$CargoTargetDir = ""
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if ([string]::IsNullOrWhiteSpace($CargoTargetDir)) {
    $CargoTargetDir = Join-Path $repo "target"
}
if ([string]::IsNullOrWhiteSpace($Exe)) {
    $Exe = Join-Path $CargoTargetDir "release\IDE1.0.exe"
}
$rawExe = Join-Path $CargoTargetDir "release\ide-fast.exe"
if ((Test-Path $rawExe) -and ((-not (Test-Path $Exe)) -or ((Get-Item $rawExe).LastWriteTimeUtc -gt (Get-Item $Exe).LastWriteTimeUtc))) {
    Copy-Item $rawExe $Exe -Force
}

$results = [ordered]@{}

function Get-Pct([double[]]$v, [double]$p) {
    $s = @($v | Sort-Object)
    $n = $s.Count
    if ($n -eq 0) { return [double]::NaN }
    if ($n -eq 1) { return $s[0] }
    $pos = [Math]::Max(0.0, [Math]::Min(1.0, $p)) * ($n - 1)
    $lo = [int][Math]::Floor($pos)
    $hi = [int][Math]::Ceiling($pos)
    $frac = $pos - $lo
    $s[$lo] * (1.0 - $frac) + $s[$hi] * $frac
}
function Get-Median($v) { Get-Pct $v 0.50 }
function Get-P90($v) { Get-Pct $v 0.90 }
function Get-P95($v) { Get-Pct $v 0.95 }
function Get-P99($v) { Get-Pct $v 0.99 }
function Get-Mean($v) { if ($v.Count -eq 0) { return [double]::NaN }; ($v | Measure-Object -Sum).Sum / $v.Count }
function Get-Std([double[]]$v) {
    if ($v.Count -le 1) { return 0.0 }
    $m = Get-Mean $v
    $ss = 0.0
    foreach ($x in $v) { $d = $x - $m; $ss += $d * $d }
    [Math]::Sqrt($ss / ($v.Count - 1))
}

# ---------------- 1. 冷启动 ----------------
if (-not $SkipStartup) {
    if (-not (Test-Path $Exe)) { throw "找不到 exe: $Exe" }
    $dir = Join-Path $env:TEMP ("IDE1.0-perf-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
    New-Item -ItemType Directory -Force $dir | Out-Null
    $points = @{}   # 追踪点 -> 每次的毫秒
    $wall = @(); $rss = @(); $priv = @(); $thr = @(); $cpu = @()
    $oldS10 = $env:IDE10_SETTINGS; $oldT10 = $env:IDE10_TRACE
    $oldS = $env:IDE_FAST_SETTINGS; $oldT = $env:IDE_FAST_TRACE
    try {
        Remove-Item Env:IDE_FAST_SETTINGS -ErrorAction SilentlyContinue
        Remove-Item Env:IDE_FAST_TRACE -ErrorAction SilentlyContinue
        for ($i = 1; $i -le $Runs; $i++) {
            $trace = Join-Path $dir "trace$i.txt"
            $env:IDE10_SETTINGS = "off"; $env:IDE10_TRACE = $trace
            $sw = [Diagnostics.Stopwatch]::StartNew()
            $p = Start-Process $Exe -ArgumentList "--new" -PassThru
            try {
                $seen = $false
                while ($sw.ElapsedMilliseconds -lt 15000) {
                    if ((Test-Path $trace) -and ((Get-Content $trace -ErrorAction SilentlyContinue) -match "first frame")) { $seen = $true; break }
                    Start-Sleep -Milliseconds 5
                }
                if (-not $seen) { Write-Warning "第 $i 次: 15 秒内没有首帧"; continue }
                $wall += $sw.Elapsed.TotalMilliseconds
                Start-Sleep -Seconds $Idle
                $p.Refresh()
                $rss += $p.WorkingSet64 / 1MB; $priv += $p.PrivateMemorySize64 / 1MB; $thr += $p.Threads.Count
                $c0 = $p.TotalProcessorTime.TotalMilliseconds; $t0 = [Diagnostics.Stopwatch]::StartNew()
                Start-Sleep -Seconds 2
                $p.Refresh()
                $cpu += ($p.TotalProcessorTime.TotalMilliseconds - $c0) / $t0.Elapsed.TotalMilliseconds * 100
            } finally { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force } }
            foreach ($l in Get-Content $trace) {
                if ($l -match '^\[fast \+([0-9.]+)ms\] (.+)$') {
                    $k = $Matches[2]; if (-not $points.ContainsKey($k)) { $points[$k] = @() }; $points[$k] += [double]$Matches[1]
                }
            }
        }
    } finally {
        if ($null -eq $oldS10) { Remove-Item Env:IDE10_SETTINGS -ErrorAction SilentlyContinue } else { $env:IDE10_SETTINGS = $oldS10 }
        if ($null -eq $oldT10) { Remove-Item Env:IDE10_TRACE -ErrorAction SilentlyContinue } else { $env:IDE10_TRACE = $oldT10 }
        if ($null -eq $oldS) { Remove-Item Env:IDE_FAST_SETTINGS -ErrorAction SilentlyContinue } else { $env:IDE_FAST_SETTINGS = $oldS }
        if ($null -eq $oldT) { Remove-Item Env:IDE_FAST_TRACE -ErrorAction SilentlyContinue } else { $env:IDE_FAST_TRACE = $oldT }
    }
    $order = @("main enter", "settings loaded", "before run_native", "event loop builder", "window builder (window + GL creation starts)", "window + GL context ready", "fonts installed", "App::new done", "first frame")
    foreach ($k in $order) {
        if ($points.ContainsKey($k)) {
            $arr = [double[]]$points[$k]
            $mu = Get-Mean $arr
            $sig = Get-Std $arr
            $results["trace: $k (median ms)"] = [math]::Round((Get-Median $arr), 2)
            $results["trace: $k (mean ms)"] = [math]::Round($mu, 2)
            $results["trace: $k (std ms)"] = [math]::Round($sig, 2)
            $results["trace: $k (cv)"] = if ($mu -gt 1e-12) { [math]::Round(($sig / $mu), 4) } else { 0.0 }
            $results["trace: $k (p90 ms)"] = [math]::Round((Get-P90 $arr), 2)
            $results["trace: $k (p95 ms)"] = [math]::Round((Get-P95 $arr), 2)
            $results["trace: $k (p99 ms)"] = [math]::Round((Get-P99 $arr), 2)
        }
    }
    if ($wall.Count -gt 0) {
        $wallArr = [double[]]$wall
        $wMu = Get-Mean $wallArr
        $wSig = Get-Std $wallArr
        $results["wall: Start-Process -> first frame seen (median ms)"] = [math]::Round((Get-Median $wallArr), 2)
        $results["wall: Start-Process -> first frame seen (mean ms)"] = [math]::Round($wMu, 2)
        $results["wall: Start-Process -> first frame seen (std ms)"] = [math]::Round($wSig, 2)
        $results["wall: Start-Process -> first frame seen (cv)"] = if ($wMu -gt 1e-12) { [math]::Round(($wSig / $wMu), 4) } else { 0.0 }
        $results["wall: Start-Process -> first frame seen (p90 ms)"] = [math]::Round((Get-P90 $wallArr), 2)
        $results["wall: Start-Process -> first frame seen (p95 ms)"] = [math]::Round((Get-P95 $wallArr), 2)
        $results["wall: Start-Process -> first frame seen (p99 ms)"] = [math]::Round((Get-P99 $wallArr), 2)
        $results["idle ${Idle}s: working set (median MB)"] = [math]::Round((Get-Median ([double[]]$rss)), 1)
        $results["idle ${Idle}s: private bytes (median MB)"] = [math]::Round((Get-Median ([double[]]$priv)), 1)
        $results["idle ${Idle}s: threads (median)"] = Get-Median ([double[]]$thr)
        $results["idle ${Idle}s+2s: CPU (median % of one core)"] = [math]::Round((Get-Median ([double[]]$cpu)), 2)
    }
    $results["runs completed"] = $wall.Count
    Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
}

# ---------------- 2. 文本引擎 (无头数学基准) ----------------
if (-not $SkipEngine) {
    $env:CARGO_BUILD_JOBS = "2"; $env:CARGO_TARGET_DIR = $CargoTargetDir
    Push-Location $repo
    $prevEap = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $out = & cargo test -p ide-fast --release perf_bench -- --ignored --nocapture --test-threads=1 2>&1 | ForEach-Object { "$_" } | Out-String
        if ($LASTEXITCODE -ne 0) { Write-Warning "cargo test perf_bench exited with $LASTEXITCODE" }
    } finally {
        $ErrorActionPreference = $prevEap
        Pop-Location
    }
    $n = 0
    foreach ($l in ($out -split "`r?`n")) {
        if ($l -match '^PERF_MATH\s+(\{.*\})$') {
            $m = $Matches[1] | ConvertFrom-Json
            $results["engine: $($m.name) (median)"] = [double]$m.median_ms
            $results["engine: $($m.name) (mean)"] = [double]$m.mean_ms
            if ($null -ne $m.p95_ms) { $results["engine: $($m.name) (p95)"] = [double]$m.p95_ms }
            $results["engine: $($m.name) (cv)"] = [double]$m.cv
            $n++
        } elseif ($l -match '^PERF (\S+) ([0-9.]+)') {
            $key = "engine: $($Matches[1]) (median)"
            if (-not $results.Contains($key)) {
                $results[$key] = [double]$Matches[2]
                $n++
            }
        }
    }
    if ($n -eq 0) { Write-Warning "没有解析到 PERF/PERF_MATH 行:`n$out" }
}

# ---------------- 输出 ----------------
$base = $null
if ($Compare -and (Test-Path $Compare)) { $base = Get-Content $Compare -Raw | ConvertFrom-Json }
$cpuName = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
$ramGb = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
"Machine: $cpuName / ${ramGb} GB / $([Environment]::OSVersion.VersionString)"
"| item | now |" + $(if ($base) { " before | change |" } else { "" })
"|---|---|" + $(if ($base) { "---|---|" } else { "" })
foreach ($k in $results.Keys) {
    $v = $results[$k]
    if ($base -and $base.PSObject.Properties.Name -contains $k) {
        $b = [double]$base.$k
        $d = if ($b -ne 0) { [math]::Round(($v - $b) / $b * 100, 1) } else { 0 }
        "| $k | $v | $b | $d% |"
    } else { "| $k | $v |" + $(if ($base) { " | |" } else { "" }) }
}
if ($Json) { $results | ConvertTo-Json | Set-Content -Encoding UTF8 $Json }
