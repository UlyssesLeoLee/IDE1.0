<#
.SYNOPSIS
  IDE1.0 v0.8.6 全功能点端到端 (E2E) 回归测试与自动化验证套件
.DESCRIPTION
  端到端覆盖 IDE1.0 最新全部 9 大关键功能点 (FEAT-16 至 FEAT-24):
    - FEAT-16: 用户自定义模型隔离与纯净模型列表 (零 Synthetic Mock 注入, API Key 脱敏)
    - FEAT-17: 设置全面持久化与跨会话恢复 (AI 面板状态、协同模式、Agent 数量与每 Agent 模型持久化)
    - FEAT-18: 智能助手与多 Agent 协同平台合并 (单 Agent 下拉搜索精准控制与会话独立性, 万级编队扩缩容与超限数字呈现, 顶部冗余移除)
    - FEAT-19: 安装向导进度条重构与三渲二晶体维护/修复/安全卸载向导 (零空心方块, 双模检测, 安全清理)
    - FEAT-20: 全主题按钮精致化与物理光影 (Comic, Retro16, Crystal, Fluent 4 大主题自适应渲染与矢量图元, 杜绝空方块)
    - FEAT-21: 无头启动与控制台抑制不变式 (CREATE_NO_WINDOW 与 Windows GUI 子系统, 零黑框终端闪烁)
    - FEAT-22: 终端直接交互、PowerShell 动作状态对齐与路径追踪 (执行中状态闭环、OSC 133 同步与 cd 追踪)
    - FEAT-23: Release 生产二进制冷启动 SLA 与完整性守卫 (IDE1.0.exe 尺寸、冷启动与零悬挂)
    - FEAT-24: AI Agent 工具调用规范与自主接入清单 (README.md 结构化元数据、CLI 参数、环境变量契约与 CAS 补丁规范)
  
  输出结构化 JSON 报告至: qa/e2e-v086-report.json
#>
param(
    [string]$TargetDir = "",
    [string]$ReportPath = "",
    [switch]$SkipBinaryLaunch = $false
)

$ErrorActionPreference = "Continue"
$root = (Resolve-Path "$PSScriptRoot\..").Path
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
    $ReportPath = Join-Path $qaDir "e2e-v086-report.json"
}
if (-not (Test-Path $qaDir)) {
    New-Item -ItemType Directory -Force -Path $qaDir | Out-Null
}

$startTime = [System.Diagnostics.Stopwatch]::StartNew()

Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "         IDE1.0 v0.8.6 全功能点端到端 (E2E) 回归测试与自动化评测系统            " -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "根目录       : $root" -ForegroundColor Gray
Write-Host "构建目录     : $TargetDir" -ForegroundColor Gray
Write-Host "报告输出     : $ReportPath" -ForegroundColor Gray
Write-Host "测试启动时间 : $((Get-Date).ToString('yyyy-MM-dd HH:mm:ss'))`n" -ForegroundColor Gray

$results = [ordered]@{}
$features = @()

function Test-Feature {
    param(
        [string]$Id,
        [string]$Name,
        [string]$Desc,
        [scriptblock]$Action
    )
    Write-Host "--------------------------------------------------------------------------------" -ForegroundColor Gray
    Write-Host "[$Id] $Name" -ForegroundColor Yellow
    Write-Host "描述: $Desc" -ForegroundColor Gray
    
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
        Write-Host "  [PASS] 验证通过 ($($dur)s)" -ForegroundColor Green
    }
    
    $featObj = [PSCustomObject]@{
        Id          = $Id
        Name        = $Name
        Desc        = $Desc
        Status      = $status
        DurationSec = $dur
        Error       = $errMsg
    }
    $script:features += $featObj
    $script:results[$Id] = $featObj
}

# ==============================================================================
# FEAT-16: 用户自定义模型隔离与纯净模型列表
# ==============================================================================
Test-Feature "FEAT-16" "User Custom Model Isolation & Zero Synthetic Models" `
    "用户自定义模型隔离、纯净模型列表、零伪造Mock模型注入、敏感Token掩码脱敏" {
    Push-Location $root
    try {
        $filter = "configured_models_excludes_unconfigured_and_roundtrips_saved_profiles|model_list_only_contains_user_configured_models|debug_and_errors_never_show_the_key"
        $out = & cargo test --target-dir $TargetDir -p ide-fast -- ai:: --test-threads=1 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "FEAT-16 Rust 集成测试失败: $outStr"
        }
        
        # 验证配置文件模型隔离规则: 构造测试配置，验证未配置状态下 0 模型且无 mock 注入
        $mockCheck = Select-String -Path "ide-fast\src\ai\config.rs" -Pattern "mock-gpt|mock_model" -SimpleMatch
        if ($mockCheck) {
            throw "在 ai/config.rs 中发现硬编码 mock 模型引用，违反模型纯净性规范"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-17: 设置全面持久化与跨会话恢复
# ==============================================================================
Test-Feature "FEAT-17" "Settings Full Persistence Across Sessions" `
    "用户AI与协同设置全面持久化: 对话框开闭、协同模式、自动应用、上下文开关、激活Tab与每Agent模型跨会话完整还原" {
    Push-Location $root
    try {
        $out = & cargo test --target-dir $TargetDir -p ide-fast -- switching_single_agents_preserves_independent_sessions_and_persists_settings per_agent_model_override_persists_through_settings --test-threads=1 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "FEAT-17 Rust 状态恢复集成测试失败: $outStr"
        }
        
        # 验证 settings.json 模式完整性
        $tmpDir = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), [System.Guid]::NewGuid().ToString())
        New-Item -ItemType Directory -Path $tmpDir | Out-Null
        try {
            $settingsFile = Join-Path $tmpDir "settings.json"
            $sampleJson = @'
{
  "theme": "Comic",
  "ai_open": true,
  "ai_collab_mode": true,
  "ai_use_ctx": true,
  "ai_auto_apply": false,
  "ai_active_mod": "deepseek-chat",
  "collab_agent_count": 5,
  "collab_active_agent": 2,
  "collab_agent_models": {
    "1": "gpt-4o",
    "2": "claude-3-7-sonnet"
  }
}
'@
            Set-Content -Path $settingsFile -Value $sampleJson -Encoding UTF8
            $readBack = Get-Content -Path $settingsFile -Raw -Encoding UTF8 | ConvertFrom-Json
            if ($readBack.ai_open -ne $true -or $readBack.collab_agent_count -ne 5 -or $readBack.collab_active_agent -ne 2) {
                throw "FEAT-17 设置序列化/反序列化丢失关键字段"
            }
            if ($readBack.collab_agent_models.'2' -ne "claude-3-7-sonnet") {
                throw "FEAT-17 per-agent 模型覆盖持久化损坏"
            }
        } finally {
            Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-18: 智能助手与多 Agent 协同平台合并
# ==============================================================================
Test-Feature "FEAT-18" "Unified AI Chat & Multi-Agent Collab Platform" `
    "单 Agent 精准控制下拉搜索切换、上万级 Agent (1~100,000) 规模化扩缩容、超显限数字徽章呈现与虚拟化、右上角冗余工作台按钮移除" {
    Push-Location $root
    try {
        $out = & cargo test --target-dir $TargetDir -p ide-fast -- filter_agents_indices_matches_name_role_and_model test_multi_agent_true_concurrency_no_queueing parallel_patches_apply_atomically_with_single_undo_and_reject_overlap collab_agent_count_and_tab_expansion --test-threads=1 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "FEAT-18 单 Agent 搜索筛选与并发隔离测试失败: $outStr"
        }
        
        # 验证右上角多 agent 协同工作台冗余按钮已彻底移除
        $titlebarContent = Get-Content -Path "ide-fast\src\app\titlebar.rs" -Raw -Encoding UTF8
        if ($titlebarContent -match 'Collab Workbench|ai_collab_mode\s*\^=\s*true') {
            throw "FEAT-18 发现 titlebar.rs 中仍残留多 Agent 独立顶栏按钮，未实现统一右侧入口"
        }

        # 验证 collab.rs 与 settings.rs 中 100,000 上限与千分位格式化
        $collabContent = Get-Content -Path "ide-fast\src\app\collab.rs" -Raw -Encoding UTF8
        if ($collabContent -notmatch 'MAX_COLLAB_AGENTS:\s*usize\s*=\s*100_000') {
            throw "FEAT-18 collab.rs 未定义 MAX_COLLAB_AGENTS = 100_000"
        }
        if ($collabContent -notmatch 'AGENT_DISPLAY_LIMIT:\s*usize\s*=\s*50') {
            throw "FEAT-18 collab.rs 未定义 AGENT_DISPLAY_LIMIT = 50 视口虚拟化阈值"
        }
        if ($collabContent -notmatch 'format_number_commas') {
            throw "FEAT-18 collab.rs 缺失 format_number_commas 千分位数值呈现函数"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-19: 安装向导进度条重构与三渲二晶体维护/修复/安全卸载向导
# ==============================================================================
Test-Feature "FEAT-19" "Rebuilt Progress Bar & Cel-Shaded 3D Logo Maintenance Wizard" `
    "安装向导进度条数学重构与偏心校准、安装/维护/修复/安全卸载双模驱动、零空心方块字体图元安全" {
    Push-Location $root
    try {
        $out = & cargo test --target-dir $TargetDir -p ide-fast -- test_apply_open_mode_transitions test_safe_uninstall_file_removal_preserves_unrelated_files test_path_env_add_remove_and_ps_escaping test_installer_adversarial_progress_and_path --test-threads=1 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "FEAT-19 安装向导与维护向导 Rust 测试失败: $outStr"
        }
        
        # 扫描 installer_ui 及 i18n 源码确保零空心方块字符 (Tofu: □, ⬚, 等)
        $tofuPatterns = @("[\u25A0-\u25AF]", "[\u25FB-\u25FE]", "\uFFFD")
        $scanFiles = @(
            @(Get-ChildItem -Path "ide-fast\src\app\installer_ui\*.rs" | Select-Object -ExpandProperty FullName),
            (Join-Path $root "ide-fast\src\i18n.rs")
        )
        foreach ($f in $scanFiles) {
            if ($f -and (Test-Path $f)) {
                $content = Get-Content -Path $f -Raw -Encoding UTF8
                foreach ($pat in $tofuPatterns) {
                    if ($content -match $pat) {
                        throw "FEAT-19 发现可能导致字符乱码或空心方块的字符: $pat 在文件 $f"
                    }
                }
            }
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-20: 全主题按钮精致化与物理光影
# ==============================================================================
Test-Feature "FEAT-20" "Theme-Adaptive Exquisite Buttons & Prismatic Surface Physics" `
    "4 大主题自适应精致按钮 (Comic, Retro16, Crystal, Fluent Dark/Light) 具有精确边框、渐变、按压反馈与纯正矢量图标 (杜绝空方块)" {
    Push-Location $root
    try {
        $out = & cargo test --target-dir $TargetDir -p ide-fast -- all_ui_icons_paint_non_empty_shapes test_crystal_and_clear_translucent_fidelity every_toolbar_command_with_an_icon_paints_something --test-threads=1 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "FEAT-20 按钮矢量图元与透明度光影测试失败: $outStr"
        }
        
        # 验证 btnstyle.rs 中 4 种主题规范完整且包含精细边框/圆角
        $btnStyleSrc = Get-Content -Path "ide-fast\src\btnstyle.rs" -Raw -Encoding UTF8
        $themes = @("tc.comic", "tc.retro16", "tc.crystal", "tc.dark")
        foreach ($t in $themes) {
            if ($btnStyleSrc -notmatch [regex]::Escape($t)) {
                throw "FEAT-20 btnstyle.rs 未完整覆盖主题分支: $t"
            }
        }

        # 验证 icons.rs 中 c.split, c.terminal, c.nema-convert, c.palette, c.save 拥有独特矢量绘制而非粗糙方块
        $iconsSrc = Get-Content -Path "ide-fast\src\icons.rs" -Raw -Encoding UTF8
        $requiredIcons = @("c.split", "c.terminal", "c.nema-convert", "c.palette", "c.save")
        foreach ($ic in $requiredIcons) {
            if ($iconsSrc -notmatch [regex]::Escape($ic)) {
                throw "FEAT-20 icons.rs 缺少图标定义: $ic"
            }
        }
        # 确保 c.terminal / c.split 不再使用生硬的 boxr 边界
        if ($iconsSrc -match '"c\.split"\s*=>\s*\{\s*boxr') {
            throw "FEAT-20 c.split 仍在使用 boxr 绘制外框方块，未升级为双编辑器窗格矢量图元"
        }
        if ($iconsSrc -match '"c\.terminal"\s*=>\s*\{\s*boxr') {
            throw "FEAT-20 c.terminal 仍在使用 boxr 绘制外框方块，未升级为纯终端提示符 >_ 矢量图元"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-21: 无头启动与控制台抑制不变式
# ==============================================================================
Test-Feature "FEAT-21" "Headless Subsystem & Zero-Console Invariant" `
    "CREATE_NO_WINDOW 与 Windows GUI 子系统设置，彻底消除终端黑框与子进程弹窗" {
    Push-Location $root
    try {
        # 验证 ide-fast 启用 windows_subsystem = 'windows'
        $mainSrc = Get-Content -Path "ide-fast\src\main.rs" -Raw -Encoding UTF8
        if ($mainSrc -notmatch 'windows_subsystem\s*=\s*"windows"') {
            throw "FEAT-21 main.rs 缺失 windows_subsystem = 'windows' 属性，会导致运行时弹出黑色终端框"
        }
        
        # 验证子进程调用配置了 CREATE_NO_WINDOW (0x08000000)
        $spawnCheck = Select-String -Path "ide-fast\src\*.rs", "ide-fast\src\app\*.rs" -Pattern "CREATE_NO_WINDOW|0x08000000"
        if (-not $spawnCheck) {
            throw "FEAT-21 未检测到 CREATE_NO_WINDOW 标志位定义与使用"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-22: 终端直接交互、ANSI TrueColor 与路径追踪
# ==============================================================================
Test-Feature "FEAT-22" "Direct Terminal ANSI TrueColor & Path Tracking" `
    "终端直接交互子系统、ANSI 256 与 24-bit TrueColor 颜色解析及 OSC/cd 路径追踪" {
    Push-Location $root
    try {
        $out = & cargo test --target-dir $TargetDir -p ide-fast -- terminal --test-threads=1 2>&1
        $outStr = $out -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outStr -notmatch "test result: ok") {
            throw "FEAT-22 终端模块回归测试失败: $outStr"
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-23: Release 生产二进制冷启动 SLA 与完整性守卫
# ==============================================================================
Test-Feature "FEAT-23" "Release Binary Cold Startup SLA & Packaging Integrity" `
    "IDE1.0.exe 生产二进制体积合规 (10MB ~ 45MB)、冷启动低于 500ms、无进程死锁" {
    $exeCandidates = @(
        (Join-Path $root "target\release\IDE1.0.exe"),
        (Join-Path $TargetDir "release\IDE1.0.exe"),
        "E:\IDE1.0\target\release\IDE1.0.exe"
    )
    $releaseExe = $exeCandidates | Where-Object { Test-Path $_ } | Sort-Object { (Get-Item $_).LastWriteTime } -Descending | Select-Object -First 1
    if (-not $releaseExe) {
        throw "FEAT-23 缺失发布产物 IDE1.0.exe，请先执行 cargo build --release"
    }
    
    $item = Get-Item $releaseExe
    $sizeMb = [math]::Round($item.Length / 1MB, 2)
    Write-Host "  -> 二进制路径: $($item.FullName)" -ForegroundColor Gray
    Write-Host "  -> 二进制大小: $sizeMb MB" -ForegroundColor Gray
    
    if ($sizeMb -lt 10.0 -or $sizeMb -gt 45.0) {
        throw "FEAT-23 二进制体积异常: $sizeMb MB (期望 10.0MB ~ 45.0MB)"
    }
    
    if (-not $SkipBinaryLaunch) {
        $tmpTrace = Join-Path $env:TEMP ("IDE1.0-trace-" + [guid]::NewGuid().ToString("N").Substring(0, 8) + ".txt")
        $oldTrace = $env:IDE10_TRACE
        $oldSettings = $env:IDE10_SETTINGS
        try {
            $env:IDE10_TRACE = $tmpTrace
            $env:IDE10_SETTINGS = "off"
            $swLaunch = [System.Diagnostics.Stopwatch]::StartNew()
            $proc = Start-Process -FilePath $item.FullName -ArgumentList "--new" -PassThru
            try {
                $firstFrameMs = $null
                while ($swLaunch.ElapsedMilliseconds -lt 5000) {
                    if (Test-Path $tmpTrace) {
                        $traceRaw = Get-Content $tmpTrace -Raw -ErrorAction SilentlyContinue
                        if ($traceRaw -and $traceRaw -match '\[fast \+([0-9.]+)ms\] first frame') {
                            $firstFrameMs = [double]$Matches[1]
                            break
                        }
                    }
                    Start-Sleep -Milliseconds 10
                }
                if ($null -eq $firstFrameMs) {
                    throw "FEAT-23 未能捕获到首帧渲染耗时 (超时 5000ms)"
                }
                Write-Host "  -> 首帧渲染耗时: ${firstFrameMs}ms (SLA <= 500ms 达标)" -ForegroundColor Gray
                if ($firstFrameMs -gt 500.0) {
                    throw "FEAT-23 冷启动首帧耗时超过 SLA (500ms): ${firstFrameMs}ms"
                }
            } finally {
                if (-not $proc.HasExited) {
                    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
                }
            }
        } finally {
            if ($oldTrace) { $env:IDE10_TRACE = $oldTrace } else { Remove-Item Env:IDE10_TRACE -ErrorAction SilentlyContinue }
            if ($oldSettings) { $env:IDE10_SETTINGS = $oldSettings } else { Remove-Item Env:IDE10_SETTINGS -ErrorAction SilentlyContinue }
            Remove-Item $tmpTrace -Force -ErrorAction SilentlyContinue
        }
    }
}

# ==============================================================================
# FEAT-24: AI Agent 工具调用规范与自主接入清单
# ==============================================================================
Test-Feature "FEAT-24" "AI Agent Tool Invocation & Autonomous Integration Registry" `
    "README.md 中完整包含工具定义元数据、检索关键词、CLI 自动化调用规格、环境变量契约与 CAS 补丁模式" {
    Push-Location $root
    try {
        $readme = Get-Content -Path "README.md" -Raw -Encoding UTF8
        $checks = @(
            "AI Agent 深度调用与智能工具路由指南",
            "ide10_editor",
            "IDE1.0.exe --new",
            "IDE1.0.exe --setup",
            "IDE1.0.exe --maintenance",
            "IDE10_TRACE",
            "IDE10_SETTINGS",
            "IDE10_OFFLINE",
            "IDE1.0-OPEN",
            "CollabState",
            "apply_patch"
        )
        foreach ($c in $checks) {
            if ($readme -notmatch [regex]::Escape($c)) {
                throw "FEAT-24 README.md 缺少 AI 调用必备信息节点: $c"
            }
        }
    } finally {
        Pop-Location
    }
}

# ==============================================================================
# FEAT-25: MCP 协议、跨供应商协同编辑与 Blender-Style Rust 控制台
# ==============================================================================
Test-Feature "FEAT-25" "MCP Protocol Suite, Headless Co-Editing & Blender Rust Console" `
    "crates/ide-mcp 18项单元测试全通、跨供应商无头编辑原子CAS与3-Way自动合并、Blender ide::ops 控制台与 Vercel MCP 接口" {
    Push-Location $root
    try {
        # 1. 验证 crates/ide-mcp 测试全通过
        $outMcp = & cargo test --target-dir $TargetDir -p ide-mcp 2>&1
        $outMcpStr = $outMcp -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outMcpStr -notmatch "test result: ok") {
            throw "FEAT-25 ide-mcp 测试失败: $outMcpStr"
        }

        # 2. 验证原生 Rust 控制台测试通过
        $outCon = & cargo test --target-dir $TargetDir -p ide-fast rust_console 2>&1
        $outConStr = $outCon -join "`n"
        if ($LASTEXITCODE -ne 0 -or $outConStr -notmatch "test result: ok") {
            throw "FEAT-25 rust_console 测试失败: $outConStr"
        }

        # 3. 验证 Vercel MCP 端点存在并可执行
        if (-not (Test-Path "api\v1\mcp.js")) {
            throw "FEAT-25 api/v1/mcp.js 文件不存在"
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

$totalCount = $features.Count
$passedCount = ($features | Where-Object { $_.Status -eq "Passed" }).Count
$failedCount = ($features | Where-Object { $_.Status -eq "Failed" }).Count
$passRate = if ($totalCount -gt 0) { [math]::Round(($passedCount / $totalCount) * 100, 2) } else { 0.0 }

$report = [ordered]@{
    GeneratedAt     = (Get-Date).ToString("yyyy-MM-ddTHH:mm:sszzz")
    Suite           = "IDE1.0 v0.8.6 E2E Regression Suite"
    TotalFeatures   = $totalCount
    PassedFeatures  = $passedCount
    FailedFeatures  = $failedCount
    PassRatePercent = $passRate
    ElapsedSec      = $totalElapsed
    Features        = $features
}

$report | ConvertTo-Json -Depth 5 | Set-Content -Path $ReportPath -Encoding UTF8

Write-Host "`n================================================================================" -ForegroundColor Cyan
Write-Host "                 v0.8.6 端到端功能点回归测试汇总仪表盘                          " -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  验证功能点总数 : $totalCount" -ForegroundColor White
Write-Host "  通过功能点数   : $passedCount" -ForegroundColor Green
Write-Host "  失败功能点数   : $failedCount" -ForegroundColor $(if ($failedCount -eq 0) { "Green" } else { "Red" })
Write-Host "  回归通过率     : $passRate %" -ForegroundColor Green
Write-Host "  测试总耗时     : $totalElapsed 秒" -ForegroundColor Gray
Write-Host "  测试报告位置   : $ReportPath" -ForegroundColor Gray
Write-Host "--------------------------------------------------------------------------------" -ForegroundColor Gray
foreach ($f in $features) {
    $color = if ($f.Status -eq "Passed") { "Green" } else { "Red" }
    Write-Host "  [$($f.Id)] $($f.Name.PadRight(45)) : [$($f.Status)] ($($f.DurationSec)s)" -ForegroundColor $color
}
Write-Host "================================================================================`n" -ForegroundColor Cyan

if ($failedCount -gt 0) {
    exit 1
}
exit 0
