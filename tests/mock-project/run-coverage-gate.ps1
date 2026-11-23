param(
    [double]$MinNodeAllBranchPct = 90.0,
    [double]$MinRustRegionPct = 90.0,
    [string]$ReportPath = ""
)

$ErrorActionPreference = "Stop"
$mockDir = $PSScriptRoot
$root = (Resolve-Path (Join-Path $mockDir "..\..")).Path
$qaDir = Join-Path $root "qa"
if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $qaDir "coverage-gate-report.json"
}
if (-not (Test-Path $qaDir)) {
    New-Item -ItemType Directory -Force -Path $qaDir | Out-Null
}

function Parse-NodeCoverage {
    param([string]$ScriptRel)
    Push-Location $root
    try {
        $ErrorActionPreference = "Continue"
        $lines = @(& node --test --experimental-test-coverage $ScriptRel 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        $ErrorActionPreference = "Stop"
        if ($code -ne 0) {
            throw "node --test --experimental-test-coverage $ScriptRel 失败 (exit=$code)"
        }
    } finally {
        $ErrorActionPreference = "Stop"
        Pop-Location
    }

    $files = @()
    $allFiles = $null
    foreach ($line in $lines) {
        if ($line -match '^#\s+([^\s|]+)\s*\|\s*([0-9.]+)\s*\|\s*([0-9.]+)\s*\|\s*([0-9.]+)\s*\|(.*)$') {
            $name = $matches[1].Trim()
            $obj = [PSCustomObject]@{
                File      = $name
                LinePct   = [double]$matches[2]
                BranchPct = [double]$matches[3]
                FuncPct   = [double]$matches[4]
            }
            $files += $obj
        } elseif ($line -match '^#\s+all files\s*\|\s*([0-9.]+)\s*\|\s*([0-9.]+)\s*\|\s*([0-9.]+)\s*\|') {
            $allFiles = [PSCustomObject]@{
                LinePct   = [double]$matches[1]
                BranchPct = [double]$matches[2]
                FuncPct   = [double]$matches[3]
            }
        }
    }
    if ($null -eq $allFiles) {
        throw "未能解析 Node coverage 的 all files 汇总行"
    }
    return [PSCustomObject]@{
        AllFiles = $allFiles
        Files    = $files
    }
}

function Parse-RustLlvmCov {
    param([string]$Package)
    Push-Location $root
    try {
        $ErrorActionPreference = "Continue"
        $lines = @(& cargo llvm-cov -j 1 -p $Package --summary-only --ignore-filename-regex 'rustc|registry|\\tests\\' -- --test-threads=1 2>&1 | ForEach-Object { "$_" })
        $code = $LASTEXITCODE
        $ErrorActionPreference = "Stop"
        if ($code -ne 0) {
            $tail = ($lines | Select-Object -Last 30) -join "`n"
            throw "cargo llvm-cov -p $Package 失败 (exit=$code):`n$tail"
        }
    } finally {
        $ErrorActionPreference = "Stop"
        Pop-Location
    }

    foreach ($line in $lines) {
        if ($line -match '^TOTAL\s+(\d+)\s+(\d+)\s+([0-9.]+)%\s+(\d+)\s+(\d+)\s+([0-9.]+)%\s+(\d+)\s+(\d+)\s+([0-9.]+)%') {
            return [PSCustomObject]@{
                Package      = $Package
                RegionsTotal = [int]$matches[1]
                RegionsMiss  = [int]$matches[2]
                RegionPct    = [double]$matches[3]
                FuncTotal    = [int]$matches[4]
                FuncMiss     = [int]$matches[5]
                FuncPct      = [double]$matches[6]
                LinesTotal   = [int]$matches[7]
                LinesMiss    = [int]$matches[8]
                LinePct      = [double]$matches[9]
                Note         = "Stable rustc llvm-cov tracks Region/Function/Line coverage (Branch column is '-' without nightly MC/DC)"
            }
        }
    }
    throw "未能解析 cargo llvm-cov ($Package) 的 TOTAL 汇总行"
}

Write-Host "[Coverage-Gate] 1/3 正在采集 Node (api/v1) 分支覆盖率..." -ForegroundColor Cyan
$nodeCov = Parse-NodeCoverage -ScriptRel "tests/mcp-vercel-unit.test.mjs"
Write-Host ("  Node all files => Line={0}%, Branch={1}%, Func={2}%" -f $nodeCov.AllFiles.LinePct, $nodeCov.AllFiles.BranchPct, $nodeCov.AllFiles.FuncPct) -ForegroundColor Green

Write-Host "[Coverage-Gate] 2/3 正在采集 Rust (nema-codec) llvm-cov 覆盖率..." -ForegroundColor Cyan
$nemaCov = Parse-RustLlvmCov -Package "nema-codec"
Write-Host ("  nema-codec TOTAL => Region={0}%, Line={1}%, Func={2}%" -f $nemaCov.RegionPct, $nemaCov.LinePct, $nemaCov.FuncPct) -ForegroundColor Green

Write-Host "[Coverage-Gate] 3/3 正在采集 Rust (ide-mcp) llvm-cov 覆盖率..." -ForegroundColor Cyan
$mcpCov = Parse-RustLlvmCov -Package "ide-mcp"
Write-Host ("  ide-mcp TOTAL    => Region={0}%, Line={1}%, Func={2}%" -f $mcpCov.RegionPct, $mcpCov.LinePct, $mcpCov.FuncPct) -ForegroundColor Green

$failures = @()
if ($nodeCov.AllFiles.BranchPct -lt $MinNodeAllBranchPct) {
    $failures += "Node all files branch % ($($nodeCov.AllFiles.BranchPct)%) < gate ($MinNodeAllBranchPct%)"
}
if ($nemaCov.RegionPct -lt $MinRustRegionPct) {
    $failures += "nema-codec Region % ($($nemaCov.RegionPct)%) < gate ($MinRustRegionPct%)"
}
if ($mcpCov.RegionPct -lt $MinRustRegionPct) {
    $failures += "ide-mcp Region % ($($mcpCov.RegionPct)%) < gate ($MinRustRegionPct%)"
}

$report = [ordered]@{
    GeneratedAt          = (Get-Date).ToString("yyyy-MM-ddTHH:mm:sszzz")
    Suite                = "IDE1.0 Core Modules Coverage Gate (Node Branch + Rust llvm-cov)"
    Thresholds           = @{
        MinNodeAllBranchPct = $MinNodeAllBranchPct
        MinRustRegionPct    = $MinRustRegionPct
    }
    NodeServerlessMcp    = $nodeCov
    RustCrates           = @($nemaCov, $mcpCov)
    GatePassed           = ($failures.Count -eq 0)
    GateFailures         = $failures
}
$report | ConvertTo-Json -Depth 6 | Set-Content -Path $ReportPath -Encoding UTF8

if ($failures.Count -gt 0) {
    throw ("覆盖率门禁未通过: " + ($failures -join "; "))
}
Write-Host "[Coverage-Gate] 覆盖率门禁检查通过 -> $ReportPath" -ForegroundColor Green
exit 0
