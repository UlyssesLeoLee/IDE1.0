<#
.SYNOPSIS
  ide-fast 性能回归基准: 冷启动各阶段 / 空闲资源 (对一个 exe), 以及文本引擎的无头基准 (cargo test --ignored)。

.DESCRIPTION
  1. 启动: 对 -Exe 连跑 -Runs 次冷启动 (IDE_FAST_SETTINGS=off, IDE_FAST_TRACE=<文件>, --new),
     取 main 里各追踪点 (main enter / settings loaded / before run_native / event loop builder /
     window builder / window + GL context ready / fonts installed / App::new done / first frame) 的中位数和 p90;
     首帧后再空闲 -Idle 秒, 读常驻内存、线程数, 并在随后 2 秒里量 CPU。
  2. 引擎: cargo test -p ide-fast --release perf_bench -- --ignored (打开 5MB / 30MB / 20 万行 / 1MB 单行,
     滚动一帧, 全部查找, 撤销 ...), 解析 `PERF <名字> <毫秒>` 行。
  发行版 exe 是 GUI 子系统没有控制台, 所以追踪写到文件 (IDE_FAST_TRACE 的值是路径时追加写文件)。

.EXAMPLE
  powershell -File ide-fast/tests/perf.ps1 -Exe E:\IDE1.0-wt\fast-target\release\ide-fast.exe -Json before.json
  powershell -File ide-fast/tests/perf.ps1 -Exe ... -Compare before.json      # 与之前保存的结果对比
  powershell -File ide-fast/tests/perf.ps1 -SkipStartup                       # 只跑引擎基准
#>
param(
    [string]$Exe = "E:\IDE1.0-wt\fast-target\release\ide-fast.exe",
    [int]$Runs = 10,
    [int]$Idle = 3,
    [switch]$SkipStartup,
    [switch]$SkipEngine,
    [string]$Json = "",
    [string]$Compare = "",
    [string]$CargoTargetDir = "E:\IDE1.0-wt\fast-target"
)
$ErrorActionPreference = "Stop"
$results = [ordered]@{}

function Get-Median($v) { $s = @($v | Sort-Object); if ($s.Count -eq 0) { return [double]::NaN }; $s[[int][math]::Floor(($s.Count - 1) / 2)] }
function Get-P90($v) { $s = @($v | Sort-Object); if ($s.Count -eq 0) { return [double]::NaN }; $s[[int][math]::Ceiling(($s.Count - 1) * 0.9)] }

# ---------------- 1. 冷启动 ----------------
if (-not $SkipStartup) {
    if (-not (Test-Path $Exe)) { throw "找不到 exe: $Exe" }
    $dir = Join-Path $env:TEMP ("idefast-perf-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
    New-Item -ItemType Directory -Force $dir | Out-Null
    $points = @{}   # 追踪点 -> 每次的毫秒
    $wall = @(); $rss = @(); $priv = @(); $thr = @(); $cpu = @()
    $oldS = $env:IDE_FAST_SETTINGS; $oldT = $env:IDE_FAST_TRACE
    try {
        for ($i = 1; $i -le $Runs; $i++) {
            $trace = Join-Path $dir "trace$i.txt"
            $env:IDE_FAST_SETTINGS = "off"; $env:IDE_FAST_TRACE = $trace
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
    } finally { $env:IDE_FAST_SETTINGS = $oldS; $env:IDE_FAST_TRACE = $oldT }
    $order = @("main enter", "settings loaded", "before run_native", "event loop builder", "window builder (window + GL creation starts)", "window + GL context ready", "fonts installed", "App::new done", "first frame")
    foreach ($k in $order) {
        if ($points.ContainsKey($k)) {
            $results["trace: $k (median ms)"] = [math]::Round((Get-Median $points[$k]), 1)
            $results["trace: $k (p90 ms)"] = [math]::Round((Get-P90 $points[$k]), 1)
        }
    }
    $results["wall: Start-Process -> first frame seen (median ms)"] = [math]::Round((Get-Median $wall), 1)
    $results["wall: Start-Process -> first frame seen (p90 ms)"] = [math]::Round((Get-P90 $wall), 1)
    $results["idle ${Idle}s: working set (median MB)"] = [math]::Round((Get-Median $rss), 1)
    $results["idle ${Idle}s: private bytes (median MB)"] = [math]::Round((Get-Median $priv), 1)
    $results["idle ${Idle}s: threads (median)"] = Get-Median $thr
    $results["idle ${Idle}s+2s: CPU (median % of one core)"] = [math]::Round((Get-Median $cpu), 2)
    $results["runs completed"] = $wall.Count
    Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
}

# ---------------- 2. 文本引擎 (无头) ----------------
if (-not $SkipEngine) {
    $repo = Resolve-Path (Join-Path $PSScriptRoot "..\..")
    $env:CARGO_BUILD_JOBS = "2"; $env:CARGO_TARGET_DIR = $CargoTargetDir
    Push-Location $repo
    try {
        $out = & cargo test -p ide-fast --release perf_bench -- --ignored --nocapture --test-threads=1 2>&1 | Out-String
    } finally { Pop-Location }
    $n = 0
    foreach ($l in ($out -split "`r?`n")) {
        if ($l -match 'PERF (\S+) ([0-9.]+)') { $results["engine: $($Matches[1])"] = [double]$Matches[2]; $n++ }
    }
    if ($n -eq 0) { Write-Warning "没有解析到 PERF 行:`n$out" }
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
