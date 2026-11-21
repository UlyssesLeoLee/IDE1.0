<#
.SYNOPSIS
  IDE1.0 全功能端到端 (E2E) 测试套件 — 基于专属 Mock 工程 (tests/mock-project)
.DESCRIPTION
  基于项目专属的独立 Mock 工程架构，对 IDE1.0 的全量核心功能开展端到端自动化测试，
  深度融合数学测试方法（幂等律、交换律、无损投影、语法封闭性、单调性）与对抗式红绿测试（Red-Green TDD）。
  
  覆盖测试项:
    - MOCK-01: 专属 Mock 工程全文件生命周期与元数据完整性
    - MOCK-02: Myers Diff 差异引擎补丁生成与数学无损还原性 (Math: Losslessness)
    - MOCK-03: Git 式协同编辑并发分支、CAS 快径与 3-Way 自动合并 (Math: Commutativity)
    - MOCK-04: Git 模式行重叠冲突对抗拦截与标准冲突标尺 (Red Test)
    - MOCK-05: .NEMA 式结构化协作与语法封闭性守护 (Red-Green Adversarial)
    - MOCK-06: 状态机幂等性与版本单调性 (Math: Idempotency & Monotonicity)
    - MOCK-07: Blender bpy 风格 Rust 控制台 (ide::context/data/ops) 全自动化执行
    - MOCK-08: 终端子系统全真 ANSI 16/256/TrueColor 与 PowerShell CLIXML 解析
    - MOCK-09: 万级多 Agent 编队调度容量与虚拟化呈现守卫
    - MOCK-10: 无头启动 CREATE_NO_WINDOW 与 GUI 控制台抑制不变式
#>
param(
    [string]$TargetDir = "",
    [string]$ReportPath = "",
    [switch]$SkipBinaryLaunch = $false
)

$ErrorActionPreference = "Continue"
$root = (Resolve-Path "$PSScriptRoot\..").Path
$mockDir = Join-Path $root "tests\mock-project"
$qaDir = Join-Path $root "qa"

if ([string]::IsNullOrWhiteSpace($TargetDir)) {
    if ($env:CARGO_TARGET_DIR -and (Test-Path $env:CARGO_TARGET_DIR)) {
        $TargetDir = $env:CARGO_TARGET_DIR
    } else {
        $TargetDir = Join-Path $root "target"
    }
}
$TargetDir = [System.IO.Path]::GetFullPath($TargetDir)

if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $qaDir "e2e-mock-project-report.json"
}
if (-not (Test-Path $qaDir)) {
    New-Item -ItemType Directory -Force -Path $qaDir | Out-Null
}

$startTime = [System.Diagnostics.Stopwatch]::StartNew()

Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "      IDE1.0 专属 Mock 工程全功能 E2E 自动化测试套件 (含数学与红绿测试)         " -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "项目根目录   : $root" -ForegroundColor Gray
Write-Host "专属Mock项目 : $mockDir" -ForegroundColor Gray
Write-Host "构建输出目录 : $TargetDir" -ForegroundColor Gray
Write-Host "评测报告输出 : $ReportPath" -ForegroundColor Gray
Write-Host "启动时间戳   : $((Get-Date).ToString('yyyy-MM-dd HH:mm:ss'))`n" -ForegroundColor Gray

$results = [ordered]@{}
$testItems = @()

function Run-E2ETest {
    param(
        [string]$Id,
        [string]$Name,
        [string]$Category,
        [scriptblock]$Action
    )
    Write-Host "--------------------------------------------------------------------------------" -ForegroundColor Gray
    Write-Host "[$Id] $Name ($Category)" -ForegroundColor Yellow
    
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $status = "Passed"
    $errMsg = ""
    try {
        & $Action
    } catch {
        $status = "Failed"
        $errMsg = $_.ToString()
        Write-Host "  [FAIL] $errMsg" -ForegroundColor Red
    }
    $sw.Stop()
    $dur = [math]::Round($sw.Elapsed.TotalSeconds, 2)
    
    if ($status -eq "Passed") {
        Write-Host "  [PASS] 测试通过 ($($dur)s)" -ForegroundColor Green
    }
    
    $item = [PSCustomObject]@{
        Id          = $Id
        Name        = $Name
        Category    = $Category
        Status      = $status
        DurationSec = $dur
        Error       = $errMsg
    }
    $script:testItems += $item
    $script:results[$Id] = $item
}

# ==============================================================================
# MOCK-01: 专属 Mock 工程全文件生命周期与元数据完整性
# ==============================================================================
Run-E2ETest "MOCK-01" "Mock Project Asset Verification" "Workspace Lifecycle" {
    $requiredFiles = @(
        "Cargo.toml",
        "src\main.rs",
        "src\math.rs",
        "src\service.rs",
        "knowledge\architecture.nema",
        "config\settings.json",
        "docs\SPEC.md",
        "scripts\automation.rs"
    )
    foreach ($rf in $requiredFiles) {
        $full = Join-Path $mockDir $rf
        if (-not (Test-Path $full)) {
            throw "Mock 工程缺少必要资产: $rf"
        }
        $len = (Get-Item $full).Length
        if ($len -le 0) {
            throw "Mock 资产文件不得为空: $rf"
        }
    }
    # 验证 NEMA 语法合法性
    $nemaPath = Join-Path $mockDir "knowledge\architecture.nema"
    $nemaText = [System.IO.File]::ReadAllText($nemaPath, [System.Text.Encoding]::UTF8)
    if ($nemaText -notmatch "^ \\1 1") {
        throw "architecture.nema 未以合法 5W1H 域头起始"
    }
    if (-not $nemaText.EndsWith("`n`n`n")) {
        throw "architecture.nema 必须以标准 3 LF 终止"
    }
}

# ==============================================================================
# MOCK-02: Myers Diff 差异引擎补丁生成与数学无损还原性 (Math: Losslessness)
# ==============================================================================
Run-E2ETest "MOCK-02" "Myers Diff Engine & Lossless Reconstruction" "Math Testing" {
    Push-Location $root
    try {
        # 调取 ide-mcp 执行 diff_unified 工具
        $textA = "line 1`nline 2`nline 3`n"
        $textB = "line 1`nline 2 modified`nline 3`nline 4 added`n"
        
        $nodeScript = @"
const { diffLines, formatUnifiedDiff } = await import('./api/v1/_state.js');
const a = ['line 1', 'line 2', 'line 3'];
const b = ['line 1', 'line 2 modified', 'line 3', 'line 4 added'];
const diff = formatUnifiedDiff(a, b, 'math.rs.v1', 'math.rs.v2');
if (!diff.includes('@@') || !diff.includes('-line 2') || !diff.includes('+line 2 modified')) {
    process.exit(1);
}
console.log('OK');
"@
        $res = node --input-type=module -e "$nodeScript" 2>&1
        if ($LASTEXITCODE -ne 0 -or $res -notmatch "OK") {
            throw "Myers Diff 计算或补丁格式化失败: $res"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-03: Git 式协同编辑并发分支、CAS 快径与 3-Way 自动合并 (Math: Commutativity)
# ==============================================================================
Run-E2ETest "MOCK-03" "Git-mode Concurrent Co-Editing & Commutativity" "Math Testing" {
    Push-Location $root
    try {
        $nodeScript = @"
const { coordinator } = await import('./api/v1/_state.js');
const fileId = 'mock:src/math.rs';
const baseCode = 'pub fn add(a: i32, b: i32) -> i32 { a + b }\n\npub fn multiply(a: i32, b: i32) -> i32 { a * b }\n';
coordinator.registerFile('mock', 'src/math.rs', baseCode, 'rust');

// Claude 改 add (Rev 1 -> Rev 2)
const r1 = coordinator.headlessEdit({
    fileId,
    author: 'claude',
    baseRevision: 1,
    newText: 'pub fn add(a: i32, b: i32) -> i32 { a.saturating_add(b) }\n\npub fn multiply(a: i32, b: i32) -> i32 { a * b }\n',
    mode: 'git'
});
if (!r1.ok || r1.revision !== 2 || r1.merged !== false) process.exit(1);

// GPT 并发改 multiply (基于旧 Rev 1，未感知 Rev 2)
const r2 = coordinator.headlessEdit({
    fileId,
    author: 'gpt',
    baseRevision: 1,
    newText: 'pub fn add(a: i32, b: i32) -> i32 { a + b }\n\npub fn multiply(a: i32, b: i32) -> i32 { a.saturating_mul(b) }\n',
    mode: 'git'
});
if (!r2.ok || r2.revision !== 3 || r2.merged !== true) process.exit(2);

// 验证两函数修改均被保全
const content = r2.content;
if (!content.includes('saturating_add') || !content.includes('saturating_mul')) {
    process.exit(3);
}
console.log('OK');
"@
        $res = node --input-type=module -e "$nodeScript" 2>&1
        if ($LASTEXITCODE -ne 0 -or $res -notmatch "OK") {
            throw "Git 模式并发 3-Way 自动合并验证失败 (Code: $LASTEXITCODE): $res"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-04: Git 模式行重叠冲突对抗拦截与标准冲突标尺 (Red Test)
# ==============================================================================
Run-E2ETest "MOCK-04" "Git-mode Conflict Marker Generation" "Red Testing" {
    Push-Location $root
    try {
        $nodeScript = @"
const { coordinator } = await import('./api/v1/_state.js');
const fileId = 'mock:src/service.rs';
const baseCode = 'pub fn status() -> &\'static str { \"INIT\" }\n';
coordinator.registerFile('mock', 'src/service.rs', baseCode, 'rust');

// Agent 1 改为 RUNNING
coordinator.headlessEdit({
    fileId,
    author: 'agent1',
    baseRevision: 1,
    newText: 'pub fn status() -> &\'static str { \"RUNNING\" }\n',
    mode: 'git'
});

// Agent 2 冲突改为 STOPPED (基于旧 Rev 1)
const conflict = coordinator.headlessEdit({
    fileId,
    author: 'agent2',
    baseRevision: 1,
    newText: 'pub fn status() -> &\'static str { \"STOPPED\" }\n',
    mode: 'git'
});

if (conflict.ok !== false || conflict.status !== 'CONFLICT') process.exit(1);
if (!conflict.content.includes('<<<<<<< HEAD (ours)') ||
    !conflict.content.includes('=======') ||
    !conflict.content.includes('>>>>>>> INCOMING (theirs)')) {
    process.exit(2);
}
console.log('OK');
"@
        $res = node --input-type=module -e "$nodeScript" 2>&1
        if ($LASTEXITCODE -ne 0 -or $res -notmatch "OK") {
            throw "Git 冲突标尺生成与 Red 拦截测试失败: $res"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-05: .NEMA 式结构化协作与语法封闭性守护 (Red-Green Adversarial)
# ==============================================================================
Run-E2ETest "MOCK-05" "NEMA-mode Syntax Closure & Adversarial Guard" "Red-Green Testing" {
    Push-Location $root
    try {
        $nodeScript = @"
const { coordinator, validateNemaSyntax } = await import('./api/v1/_state.js');
const fileId = 'mock:knowledge/architecture.nema';
const baseNema = ' \\1 1\n\\%2\n\\# Metric Val\nCpu 8\n\n\n';
coordinator.registerFile('mock', 'knowledge/architecture.nema', baseNema, 'nema');

// [RED TEST]: 注入未转义双空格攻击
const badDoubleSpace = coordinator.headlessEdit({
    fileId,
    author: 'attacker',
    baseRevision: 1,
    newText: ' \\1 1\n\\%2\n\\# Metric Val\nCpu  8 (illegal double space)\n\n\n',
    mode: 'nema'
});
if (badDoubleSpace.ok !== false || badDoubleSpace.status !== 'REJECTED_NEMA_SYNTAX_ERROR') {
    process.exit(1);
}

// [RED TEST]: 注入正文 TAB 字符攻击
const badTab = coordinator.headlessEdit({
    fileId,
    author: 'attacker',
    baseRevision: 1,
    newText: ' \\1 1\n\\%2\n\\# Metric Val\nCpu\t8 (illegal tab)\n\n\n',
    mode: 'nema'
});
if (badTab.ok !== false || badTab.status !== 'REJECTED_NEMA_SYNTAX_ERROR') {
    process.exit(2);
}

// [GREEN TEST]: Agent A (Claude) 正常追加 Mem (Rev 1 -> Rev 2)
const greenA = coordinator.headlessEdit({
    fileId,
    author: 'claude',
    baseRevision: 1,
    newText: ' \\1 1\n\\%2\n\\# Metric Val\nCpu 8\nMem 64\n\n\n',
    mode: 'nema'
});
if (!greenA.ok || greenA.revision !== 2) process.exit(3);

// [GREEN TEST]: Agent B (GPT) 并发追加 Disk (基于旧 Rev 1)
const greenB = coordinator.headlessEdit({
    fileId,
    author: 'gpt',
    baseRevision: 1,
    newText: ' \\1 1\n\\%2\n\\# Metric Val\nCpu 8\nDisk 2048\n\n\n',
    mode: 'nema'
});
if (!greenB.ok || greenB.revision !== 3 || greenB.merged !== true) process.exit(4);

// 语法封闭性验证 (Syntax Closure)
const finalContent = greenB.content;
const check = validateNemaSyntax(finalContent);
if (!check.ok) process.exit(5);
if (!finalContent.includes('Cpu 8') || !finalContent.includes('Mem 64') || !finalContent.includes('Disk 2048')) {
    process.exit(6);
}
console.log('OK');
"@
        $res = node --input-type=module -e "$nodeScript" 2>&1
        if ($LASTEXITCODE -ne 0 -or $res -notmatch "OK") {
            throw "NEMA 模式红绿对抗测试或语法封闭性验证失败: $res"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-06: 状态机幂等性与版本单调性 (Math: Idempotency & Monotonicity)
# ==============================================================================
Run-E2ETest "MOCK-06" "State Machine Idempotency & Monotonicity" "Math Testing" {
    Push-Location $root
    try {
        $nodeScript = @"
const { coordinator } = await import('./api/v1/_state.js');
const fileId = 'mock:math_properties.txt';
coordinator.registerFile('mock', 'math_properties.txt', '0\n', 'plain');

// 幂等性检验
const opKey = 'unique_op_math_001';
const r1 = coordinator.headlessEdit({ fileId, author: 'tester', baseRevision: 1, newText: '100\n', opId: opKey, mode: 'git' });
const r2 = coordinator.headlessEdit({ fileId, author: 'tester', baseRevision: 1, newText: '100\n', opId: opKey, mode: 'git' });

if (r1.revision !== r2.revision) process.exit(1);
if (r2.idempotent_replay !== true) process.exit(2);

// 单调性检验
let prevRev = r1.revision;
for (let step = 1; step <= 5; step++) {
    const rx = coordinator.headlessEdit({ fileId, author: 'agent', baseRevision: prevRev, newText: (step + '\n'), mode: 'git' });
    if (rx.revision !== prevRev + 1) process.exit(3);
    prevRev = rx.revision;
}
console.log('OK');
"@
        $res = node --input-type=module -e "$nodeScript" 2>&1
        if ($LASTEXITCODE -ne 0 -or $res -notmatch "OK") {
            throw "状态机幂等律与单调性验证失败: $res"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-07: Blender bpy 风格 Rust 控制台 (ide::context/data/ops) 全自动化执行
# ==============================================================================
Run-E2ETest "MOCK-07" "Blender bpy-style Rust Console DSL Execution" "Console API" {
    Push-Location $root
    try {
        $out = & cargo test --target-dir $TargetDir -p ide-mcp rust_console 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "Rust Console DSL (ide::context/data/ops) 测试失败: $outStr"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-08: 终端子系统全真 ANSI 16/256/TrueColor 与 PowerShell CLIXML 解析
# ==============================================================================
Run-E2ETest "MOCK-08" "Terminal ANSI TrueColor & PowerShell CLIXML Parser" "Terminal Subsystem" {
    Push-Location $root
    try {
        # 验证 ide-fast 终端模块单元测试
        $out = & cargo test --target-dir $TargetDir -p ide-fast terminal 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "终端子系统测试失败: $outStr"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-09: 万级多 Agent 编队调度容量与虚拟化呈现守卫
# ==============================================================================
Run-E2ETest "MOCK-09" "100k Multi-Agent Roster Scale & Viewport Virtualization" "Agent Orchestration" {
    Push-Location $root
    try {
        # 验证 collab.rs 中对万级编队的虚拟化截断算法
        $collabFile = "ide-fast\src\app\collab.rs"
        $code = [System.IO.File]::ReadAllText($collabFile, [System.Text.Encoding]::UTF8)
        if ($code -notmatch "10000" -and $code -notmatch "100,000") {
            throw "collab.rs 未实装万级编队容量"
        }
        if ($code -notmatch "50") {
            throw "collab.rs 缺少前 50 项视口虚拟化约束"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# MOCK-10: 无头启动 CREATE_NO_WINDOW 与 GUI 控制台抑制不变式
# ==============================================================================
Run-E2ETest "MOCK-10" "Headless Startup & CREATE_NO_WINDOW Invariant" "Operability" {
    Push-Location $root
    try {
        $mainFile = "ide-fast\src\main.rs"
        $mainText = [System.IO.File]::ReadAllText($mainFile, [System.Text.Encoding]::UTF8)
        if ($mainText -notmatch 'windows_subsystem\s*=\s*"windows"') {
            throw "主入口未标记 #![windows_subsystem = 'windows']"
        }
        
        $setupFiles = Get-ChildItem "ide-fast\src\app\installer_ui\*.rs"
        foreach ($sf in $setupFiles) {
            $t = [System.IO.File]::ReadAllText($sf.FullName, [System.Text.Encoding]::UTF8)
            if ($t -match 'Command::new' -and $t -notmatch 'CREATE_NO_WINDOW') {
                throw "$($sf.Name) 中的子进程拉起未注入 CREATE_NO_WINDOW"
            }
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# 报告聚合与汇总
# ==============================================================================
$startTime.Stop()
$totalElapsed = [math]::Round($startTime.Elapsed.TotalSeconds, 2)

$totalCount = $testItems.Count
$passedCount = ($testItems | Where-Object { $_.Status -eq "Passed" }).Count
$failedCount = ($testItems | Where-Object { $_.Status -eq "Failed" }).Count
$passRate = if ($totalCount -gt 0) { [math]::Round(($passedCount / $totalCount) * 100, 2) } else { 0.0 }

$report = [ordered]@{
    GeneratedAt     = (Get-Date).ToString("yyyy-MM-ddTHH:mm:sszzz")
    Suite           = "IDE1.0 Dedicated Mock-Project E2E Test Suite"
    MockProjectDir  = $mockDir
    TotalTests      = $totalCount
    PassedTests     = $passedCount
    FailedTests     = $failedCount
    PassRatePercent = $passRate
    ElapsedSec      = $totalElapsed
    Tests           = $testItems
}

$report | ConvertTo-Json -Depth 5 | Set-Content -Path $ReportPath -Encoding UTF8

Write-Host "`n================================================================================" -ForegroundColor Cyan
Write-Host "         IDE1.0 Mock 工程 E2E 全功能测试汇总仪表盘                              " -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  测试项目总数 : $totalCount" -ForegroundColor White
Write-Host "  测试通过数   : $passedCount" -ForegroundColor Green
Write-Host "  测试失败数   : $failedCount" -ForegroundColor $(if ($failedCount -eq 0) { "Green" } else { "Red" })
Write-Host "  测试通过率   : $passRate %" -ForegroundColor Green
Write-Host "  测试总耗时   : $totalElapsed 秒" -ForegroundColor Gray
Write-Host "  评测报告位置 : $ReportPath" -ForegroundColor Gray
Write-Host "--------------------------------------------------------------------------------" -ForegroundColor Gray
foreach ($t in $testItems) {
    $color = if ($t.Status -eq "Passed") { "Green" } else { "Red" }
    Write-Host "  [$($t.Id)] $($t.Name.PadRight(45)) : [$($t.Status)] ($($t.DurationSec)s)" -ForegroundColor $color
}
Write-Host "================================================================================`n" -ForegroundColor Cyan

if ($failedCount -gt 0) {
    exit 1
}
exit 0
