<#
.SYNOPSIS
  IDE1.0 专属 Mock 工程全功能点逐次自动化测试总控入口
.DESCRIPTION
  读取 tests/mock-project/feature-matrix.json，按 F-01 ~ F-16 顺序逐次验证 IDE1.0 全部核心功能点，
  每完成一项实时增量写入 qa/mock-project-feature-report.json。
.EXAMPLE
  .\tests\mock-project\run-all-features.ps1 -List
  .\tests\mock-project\run-all-features.ps1 -StopOnFailure
  .\tests\mock-project\run-all-features.ps1 -From F-08 -StopOnFailure
  .\tests\mock-project\run-all-features.ps1 -StepByStep
#>
param(
    [string[]]$Feature = @(),
    [string]$From = "",
    [switch]$Fast = $false,
    [switch]$StopOnFailure = $false,
    [switch]$StepByStep = $false,
    [switch]$List = $false,
    [switch]$SkipBinaryLaunch = $false,
    [string]$TargetDir = "",
    [string]$ReportPath = ""
)

$ErrorActionPreference = "Continue"
$mockDir = $PSScriptRoot
$root = (Resolve-Path (Join-Path $mockDir "..\..")).Path
$matrixPath = Join-Path $mockDir "feature-matrix.json"
$qaDir = Join-Path $root "qa"

if (-not (Test-Path $matrixPath)) {
    Write-Host "[ERR] 未找到功能矩阵清单: $matrixPath" -ForegroundColor Red
    exit 1
}

if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    if (-not [string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        $TargetDir = $env:CARGO_TARGET_DIR
    }
}
if (-not [string]::IsNullOrWhiteSpace($TargetDir)) {
    $TargetDir = [System.IO.Path]::GetFullPath($TargetDir)
}

if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $qaDir "mock-project-feature-report.json"
}
if (-not (Test-Path $qaDir)) {
    New-Item -ItemType Directory -Force -Path $qaDir | Out-Null
}

$matrix = Get-Content -Path $matrixPath -Raw -Encoding UTF8 | ConvertFrom-Json

if ($List) {
    Write-Host "`nIDE1.0 Mock 工程功能点清单 ($($matrix.features.Count) 项):" -ForegroundColor Cyan
    Write-Host "--------------------------------------------------------------------------------" -ForegroundColor DarkGray
    foreach ($f in $matrix.features) {
        $tag = if ($f.fast -eq $true) { "FAST" } else { "FULL" }
        Write-Host ("  [{0}] [{1}] ({2}) {3}" -f $f.id, $tag, $f.category, $f.name)
    }
    Write-Host "--------------------------------------------------------------------------------`n" -ForegroundColor DarkGray
    exit 0
}

$candidateFeatures = @($matrix.features)
if (-not [string]::IsNullOrWhiteSpace($From)) {
    $fromId = $From.Trim().ToUpperInvariant()
    $startIdx = -1
    for ($i = 0; $i -lt $candidateFeatures.Count; $i++) {
        if ($candidateFeatures[$i].id.ToUpperInvariant() -eq $fromId) {
            $startIdx = $i
            break
        }
    }
    if ($startIdx -lt 0) {
        Write-Host "[ERR] -From 指定的起始功能点不存在: $fromId" -ForegroundColor Red
        exit 1
    }
    $candidateFeatures = @($candidateFeatures[$startIdx..($candidateFeatures.Count - 1)])
}

$selectedIds = @(
    $Feature |
        ForEach-Object { $_ -split ',' } |
        ForEach-Object { $_.Trim().ToUpperInvariant() } |
        Where-Object { $_ -ne "" }
)

$featuresToRun = @($candidateFeatures | Where-Object {
    $idMatch = ($selectedIds.Count -eq 0) -or ($selectedIds -contains $_.id.ToUpperInvariant())
    $fastMatch = (-not $Fast) -or ($_.fast -eq $true)
    $idMatch -and $fastMatch
})

if ($featuresToRun.Count -eq 0) {
    Write-Host "[ERR] 过滤条件下没有可执行的功能点（From=$From, Feature=$($Feature -join ','), Fast=$Fast）" -ForegroundColor Red
    exit 1
}

function Invoke-CargoTestStep {
    param(
        [string]$Package,
        [string]$Filter
    )
    $cargoArgs = @("test", "-j", "1")
    if (-not [string]::IsNullOrWhiteSpace($TargetDir)) {
        $cargoArgs += @("--target-dir", $TargetDir)
    }
    $cargoArgs += @("-p", $Package)
    if (-not [string]::IsNullOrWhiteSpace($Filter)) {
        $cargoArgs += @($Filter, "--", "--test-threads=1")
    } else {
        $cargoArgs += @("--", "--test-threads=1")
    }

    Push-Location $root
    try {
        $rawOut = @(& cargo @cargoArgs 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
    } finally {
        Pop-Location
    }

    $passed = 0
    $failed = 0
    $suites = 0
    foreach ($line in $rawOut) {
        if ($line -match 'test result:\s+(?:ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed') {
            $passed += [int]$matches[1]
            $failed += [int]$matches[2]
            $suites++
        }
    }
    if ($code -ne 0 -or $failed -gt 0 -or $suites -eq 0) {
        $tail = ($rawOut | Select-Object -Last 35) -join "`n"
        throw "cargo $($cargoArgs -join ' ') 失败 (exit=$code, suites=$suites, passed=$passed, failed=$failed)`n$tail"
    }
    return "$Package`: $passed passed ($suites suite(s))"
}

function Invoke-QualityGateStep {
    Push-Location $root
    try {
        $fmtOut = @(& cargo fmt -p editcore -p nema-codec -p ide-agent-router -p ide-mcp -p ide-fast -- --check 2>&1 | ForEach-Object { "$_" })
        if ($LASTEXITCODE -ne 0) {
            throw "cargo fmt 检查失败 (exit=$LASTEXITCODE):`n$($fmtOut -join "`n")"
        }
        $clippyArgs = @("clippy", "-j", "1")
        if (-not [string]::IsNullOrWhiteSpace($TargetDir)) {
            $clippyArgs += @("--target-dir", $TargetDir)
        }
        $clippyArgs += @("-p", "editcore", "-p", "nema-codec", "-p", "ide-agent-router", "-p", "ide-mcp", "-p", "ide-fast", "--all-targets", "--", "-D", "warnings")
        $clippyOut = @(& cargo @clippyArgs 2>&1 | ForEach-Object { "$_" })
        if ($LASTEXITCODE -ne 0) {
            $tail = ($clippyOut | Select-Object -Last 40) -join "`n"
            throw "cargo clippy 检查失败 (exit=$LASTEXITCODE):`n$tail"
        }
        return "cargo fmt + clippy (5 crates --all-targets -D warnings) OK"
    } finally {
        Pop-Location
    }
}

function Test-MockAssets {
    param([object]$Feat)
    foreach ($rel in $Feat.fixtures) {
        $full = Join-Path $mockDir $rel
        if (-not (Test-Path $full)) {
            throw "缺少 Mock 工程资产文件: $rel"
        }
        $item = Get-Item $full
        if ($item.Length -le 0) {
            throw "Mock 工程资产文件为空: $rel"
        }
        if ($rel.EndsWith(".json", [System.StringComparison]::OrdinalIgnoreCase)) {
            try {
                $parsed = Get-Content -Path $full -Raw -Encoding UTF8 | ConvertFrom-Json
                if ($rel -eq ".mcp.json" -and ($null -eq $parsed.mcpServers)) {
                    throw ".mcp.json 缺少 mcpServers 字段"
                }
                if ($rel -eq "fixtures/sticky-note-sample.json" -and ($null -eq $parsed.notes -or $parsed.notes.Count -eq 0)) {
                    throw "sticky-note-sample.json 缺少有效的 notes 列表"
                }
            } catch {
                throw "JSON 资产解析失败 ($rel): $_"
            }
        }
    }
    $nemaPath = Join-Path $mockDir "knowledge\architecture.nema"
    $nemaText = [System.IO.File]::ReadAllText($nemaPath, [System.Text.Encoding]::UTF8)
    $nemaNorm = $nemaText -replace "`r`n", "`n"
    if ($nemaNorm -notmatch "^ \\1 1") {
        throw "knowledge/architecture.nema 未以合法 5W1H 域头起始"
    }
    if (-not $nemaNorm.EndsWith("`n`n`n")) {
        throw "knowledge/architecture.nema 必须以标准 3 LF 终止"
    }
    return "$($Feat.fixtures.Count) fixtures verified"
}

function Invoke-BinarySmokeTest {
    if ($SkipBinaryLaunch) {
        return @{ Status = "Skipped"; Detail = "Skipped by -SkipBinaryLaunch" }
    }
    $candidates = @()
    $distExe = Get-ChildItem -Path (Join-Path $root "dist-fast") -Filter "IDE1.0*-windows-x64.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if ($distExe) { $candidates += $distExe.FullName }
    if (-not [string]::IsNullOrWhiteSpace($TargetDir)) {
        $candidates += (Join-Path $TargetDir "release\IDE1.0.exe")
        $candidates += (Join-Path $TargetDir "release\ide-fast.exe")
    }
    $candidates += (Join-Path $root "target\release\IDE1.0.exe")
    $candidates += (Join-Path $root "IDE1.0.exe")

    $bin = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if (-not $bin) {
        return @{ Status = "Skipped"; Detail = "No release binary found; build or pass without skip after packaging" }
    }

    $out = @(& $bin --smoke-test 2>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
    $joined = $out -join "`n"
    if ($code -ne 0 -or ($joined -and $joined -notmatch "IDE1\.0_SMOKE_TEST_OK")) {
        throw "Smoke test 失败 ($bin, exit=$code): $joined"
    }
    return @{ Status = "Passed"; Detail = "$bin --smoke-test OK" }
}

function Write-IncrementalReport {
    param(
        [array]$Records,
        [double]$ElapsedSec
    )
    $passedCount = @($Records | Where-Object { $_.Status -eq "Passed" }).Count
    $skippedCount = @($Records | Where-Object { $_.Status -eq "Skipped" }).Count
    $failedCount = @($Records | Where-Object { $_.Status -eq "Failed" }).Count
    $report = [ordered]@{
        GeneratedAt  = (Get-Date).ToString("yyyy-MM-ddTHH:mm:sszzz")
        Suite        = $matrix.suite
        MockProject  = $mockDir
        FastMode     = [bool]$Fast
        TotalTests   = $Records.Count
        PassedTests  = $passedCount
        SkippedTests = $skippedCount
        FailedTests  = $failedCount
        ElapsedSec   = [math]::Round($ElapsedSec, 2)
        Results      = $Records
    }
    $report | ConvertTo-Json -Depth 6 | Set-Content -Path $ReportPath -Encoding UTF8
}

$totalSw = [System.Diagnostics.Stopwatch]::StartNew()
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "        IDE1.0 专属 Mock 工程逐次全功能点自动化测试系统 (F-01 ~ F-16)           " -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "工作区根目录 : $root" -ForegroundColor Gray
Write-Host "Mock 工程    : $mockDir" -ForegroundColor Gray
Write-Host "执行范围     : $($featuresToRun.Count) / $($matrix.features.Count) 项 (Fast=$Fast, StopOnFailure=$StopOnFailure, StepByStep=$StepByStep)" -ForegroundColor Gray
Write-Host "报告输出     : $ReportPath`n" -ForegroundColor Gray

$records = @()
$stepNo = 0
$abortedEarly = $false

foreach ($f in $featuresToRun) {
    $stepNo++
    Write-Host "--------------------------------------------------------------------------------" -ForegroundColor DarkGray
    Write-Host "[$stepNo/$($featuresToRun.Count)] [$($f.id)] $($f.name) ($($f.category))" -ForegroundColor Yellow

    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $status = "Passed"
    $detail = ""
    $errMsg = ""
    try {
        switch ($f.kind) {
            "asset_check" {
                $detail = Test-MockAssets -Feat $f
            }
            "cargo_test" {
                $detail = Invoke-CargoTestStep -Package $f.package -Filter $f.filter
            }
            "cargo_test_multi" {
                $parts = @()
                foreach ($s in $f.steps) {
                    $parts += Invoke-CargoTestStep -Package $s.package -Filter $s.filter
                }
                $detail = $parts -join "; "
            }
            "quality_gate" {
                $detail = Invoke-QualityGateStep
            }
            "node_test" {
                $scriptPath = Join-Path $root $f.script
                Push-Location $root
                try {
                    $out = @(& node --test $scriptPath 2>&1 | ForEach-Object { "$_" })
                    if ($LASTEXITCODE -ne 0) {
                        throw "node --test 失败 (exit=$LASTEXITCODE):`n$($out -join "`n")"
                    }
                    $detail = "node --test $($f.script) OK"
                } finally {
                    Pop-Location
                }
            }
            "node_script" {
                $scriptPath = Join-Path $root $f.script
                Push-Location $root
                try {
                    $out = @(& node $scriptPath 2>&1 | ForEach-Object { "$_" })
                    if ($LASTEXITCODE -ne 0) {
                        throw "node 脚本失败 (exit=$LASTEXITCODE):`n$($out -join "`n")"
                    }
                    $detail = "node $($f.script) OK"
                } finally {
                    Pop-Location
                }
            }
            "ps_script" {
                $scriptPath = Join-Path $root $f.script
                $psArgs = @("-ExecutionPolicy", "Bypass", "-File", $scriptPath)
                if ($null -ne $f.args) {
                    foreach ($a in $f.args) {
                        $psArgs += [string]$a
                    }
                }
                if (-not [string]::IsNullOrWhiteSpace($TargetDir)) {
                    $psArgs += @("-TargetDir", $TargetDir)
                }
                if ($SkipBinaryLaunch -and ($f.script -like "*e2e-*")) {
                    $psArgs += "-SkipBinaryLaunch"
                }
                $out = @(& powershell @psArgs 2>&1 | ForEach-Object { "$_" })
                if ($LASTEXITCODE -ne 0) {
                    throw "PowerShell 套件失败 (exit=$LASTEXITCODE):`n$(($out | Select-Object -Last 30) -join "`n")"
                }
                $detail = "$($f.script) OK"
            }
            "binary_smoke" {
                $smoke = Invoke-BinarySmokeTest
                $status = $smoke.Status
                $detail = $smoke.Detail
            }
            default {
                throw "未知功能测试类型: $($f.kind)"
            }
        }
    } catch {
        $status = "Failed"
        $errMsg = $_.ToString()
    }
    $sw.Stop()
    $dur = [math]::Round($sw.Elapsed.TotalSeconds, 2)

    $color = switch ($status) {
        "Passed" { "Green" }
        "Skipped" { "DarkYellow" }
        default { "Red" }
    }
    $msg = if ($status -eq "Failed") { $errMsg } else { $detail }
    Write-Host "  [$($status.ToUpper())] ($($dur)s) $msg" -ForegroundColor $color

    $records += [PSCustomObject]@{
        Order       = $stepNo
        Id          = $f.id
        Name        = $f.name
        Category    = $f.category
        Kind        = $f.kind
        Status      = $status
        DurationSec = $dur
        Detail      = $detail
        Error       = $errMsg
    }
    Write-IncrementalReport -Records $records -ElapsedSec $totalSw.Elapsed.TotalSeconds

    if ($status -eq "Failed" -and $StopOnFailure) {
        Write-Host "`n[STOP] -StopOnFailure 已在 [$($f.id)] 中止；修复后可使用以下命令续测:" -ForegroundColor Red
        Write-Host "  powershell -ExecutionPolicy Bypass -File tests/mock-project/run-all-features.ps1 -From $($f.id) -StopOnFailure" -ForegroundColor Yellow
        $abortedEarly = $true
        break
    }

    if ($StepByStep -and $stepNo -lt $featuresToRun.Count) {
        $ans = Read-Host "按 Enter 继续下一项（输入 q 并回车中止）"
        if ($ans -match '^[qQ]') {
            Write-Host "[STOP] 用户主动中止后续逐项验证。" -ForegroundColor DarkYellow
            $abortedEarly = $true
            break
        }
    }
}

$totalSw.Stop()
$totalSec = [math]::Round($totalSw.Elapsed.TotalSeconds, 2)
Write-IncrementalReport -Records $records -ElapsedSec $totalSec

$passedCount = @($records | Where-Object { $_.Status -eq "Passed" }).Count
$skippedCount = @($records | Where-Object { $_.Status -eq "Skipped" }).Count
$failedCount = @($records | Where-Object { $_.Status -eq "Failed" }).Count

Write-Host "`n================================================================================" -ForegroundColor Cyan
Write-Host "                IDE1.0 Mock 工程逐次功能点验证汇总                              " -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  执行总数 : $($records.Count)   通过 : $passedCount   跳过 : $skippedCount   失败 : $failedCount   耗时 : $totalSec 秒" -ForegroundColor White
Write-Host "  评测报告 : $ReportPath" -ForegroundColor Gray
Write-Host "================================================================================`n" -ForegroundColor Cyan

if ($failedCount -gt 0 -or $abortedEarly) {
    exit 1
}
exit 0
