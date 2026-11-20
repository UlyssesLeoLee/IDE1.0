# IDE1.0

> **纯 Rust AI-Native 编辑内核 + 高性能桌面版 (IDE1.0.exe) + Vercel Web 协同云平台**  
> 支持万级 AI Agent 通过 API 并发作业：**文件级路由、内部语义节点级并行作业、Patch 级提交、CAS + Fencing 权威协调**。

---

## 🚀 桌面旗舰版最新发布 (Desktop Release v0.8.4)

- **GitHub Release 官方下载**: [https://github.com/UlyssesLeoLee/IDE1.0/releases/tag/v0.8.4](https://github.com/UlyssesLeoLee/IDE1.0/releases/tag/v0.8.4)
- **核心发布资产**:
  - `IDE1.0_Setup_0.8.4_x64.exe`：旗舰真 3D 赛璐珞水晶徽标安装向导（100% 聚焦正版 3D Crystal Logo，0 冗余视觉噪音，低调奢华暗色系与色彩心理学设计）
  - `IDE1.0-0.8.4-windows-x64.exe`：免安装绿色单文件便携版
  - `IDE1.0_0.8.4_x64.msi`：企业级静默安装包（彻底移除 WixUI 原生灰框与按钮，安装完成自动拉起真 3D 向导）
- **v0.8.4 关键实装特性**:
  - **沉浸式终端子系统与现代 CLI / PowerShell 深度兼容（Terminal Subsystem Parity）**：
    - **全真 ANSI 16/256/TrueColor 渲染引擎**：全面重构 `LineBuffer` 与 `egui::text::LayoutJob`，支持标准 16 色、256 色阶与 24 位 RGB 真彩色，完整支持粗体、斜体、下划线与暗淡样式，消除原本单一白灰文字的廉价感；
    - **PowerShell CLIXML 错误流深度解析**：原生拦截 PowerShell 管道产生的 `#< CLIXML` 原始错误块，自动提取错误消息并高亮转码为标准的 Bright Red ANSI 错误流，告别原生 XML 乱码倾泻；
    - **多行进度条与 Hermes 等现代 CLI 原生兼容**：实现 `CSI n A`（光标上移重绘）、`CSI n K`（擦行）与回车覆盖，完美支持 Hermes Agent CLI、npm、cargo、pip 等现代工具的动态 Thinking 旋转动画（Spinner）与多层进度条；
    - **工作目录追踪与动态 Prompt 实时联动**：实时解析 `cd`、`Set-Location`、`chdir` 指令，动态同步终端会话工作路径与 Tab 补全目录，命令行提示符即时跟随变迁；
    - **全量 VT / TTY 兼容环境变量注入**：自动注入 `TERM=xterm-256color`、`COLORTERM=truecolor`、`FORCE_COLOR=3`、`CLICOLOR_FORCE=1`、`PYTHONUNBUFFERED=1`、`PYTHONUTF8=1`，确保 Node、Python、Rust 与各类 AI Agent CLI 原生识别彩色终端；
    - **非破坏性 Ctrl+C 中断机制**：按 Ctrl+C 向子进程投递 `0x03` 中断信号而非直接暴力击杀常驻 Shell，平滑返回命令行交互。
  - **AI 聊天与协同工作台 UI/UX 全面重塑**：彻底消除零散文本按钮与冗余 Pill 行产生的廉价感，系统性重构为专业级下拉菜单体系（`ComboBox` / `<select>`）：
    - **顶栏模式切换**：优雅切换“💬 智能助手对话”与“🤝 多Agent协同工作台”；
    - **顶栏操作菜单 (`⋮ 操作 ▾`)**：集成“🧹 清空对话历史”、“📋 复制全部对话”、“⚙️ 模型与接口设置…”及“🤝 切换至协同工作台”；
    - **策略与上下文 (`⚙️ 策略 ▾`)**：集成“⚡ 自动应用补丁 / 🛡️ 手动确认补丁”、“📄 携带选区与当前章节上下文”等高级编辑意图；
    - **能力模组 (`🧩 模式 ▾`)**：下拉选择并热切换 `code`、`plan`、`review` 等专精模式；
    - **模型供应商预设 (`🏢 供应商预设 ▾`)**：统一收纳 DeepSeek、SiliconFlow、OpenAI、Anthropic、Ollama、自定义等供应商配置；
    - **快捷技能指令 (`⚡ 快捷技能 ▾`)**：告别平铺方块，一键触发 `/review`、`/test`、`/refactor`、`/commit`、`/nema`、`/explain`、`/collab` 等核心指令；
    - **模型快速切换 (`🤖 模型 ▾`)**：底部工具栏即时切换 `deepseek-chat`、`deepseek-reasoner`、`gpt-4o`、`claude-3-5-sonnet` 等模型；
    - **消息采纳操作 (`⚡ 采纳操作 ▾`)**：单条消息卡片收纳“📥 插入到当前光标处”与“📋 复制全部回复”，配合智能 Agent 协同任务卡片一键识别与分发。
  - **全链路纯粹无头静默启动（Headless Direct Startup）**：主程序无条件采用 Windows GUI 子系统（`#![windows_subsystem = "windows"]`），安装向导的桌面/开始菜单快捷方式部署、PATH 环境变量配置、卸载注册表写入、未保存询问弹窗、外部链接打开均严格注入 `CREATE_NO_WINDOW (0x0800_0000)`，彻底杜绝任何黑色控制台终端窗口闪烁。
  - **全量多语言 (i18n) 与本地文件加载进度条**：原生支持中/英/日三大语言环境，安装向导与首选项支持实时语言切换；文件系统非阻塞启动并提供丝滑进度条与首启友好提示。
  - **全局 Ctrl + 滚轮平滑缩放与 Retro16 像素点阵字体自适应**：支持全局按住 Ctrl 滚动鼠标滚轮任意放大缩小画布；切换 Retro16 主题自动平滑适配 `PixelFont 12` 像素字体。
  - **安装向导排版与响应式微交互升级**：880×600 宽幅布局彻底消除右下角大面积空旷；精简欢迎页实现 3 步直达核心配置（环境配置 -> 极速部署 -> 部署完成）；移除拖拽复位 HUD 胶囊，引入空间感知微交互（移动光标或悬停按钮触发平缓、微随机的高阻尼 3D 宝石旋转）；字体加粗与对比度强化（白金/纯白字符，58px 高级卡片，46px 铺满式主操作栏）。
  - **首启静默原则与用户偏好持久化**：便笺独立视口首次启动默认不自动打开（`open: false`），主编辑器左侧导航栏首次启动默认关闭（`side_open: false`），用户开关状态在变更时立即原子落盘持久化，后续启动严格记忆还原。
  - **便笺格式化目标即选区引擎**：删除线（`~~`）、粗体（`**`）、斜体（`*`）、下划线（`<u>`）与行前缀按钮全面支持“选中文本即为目标”，支持再次点击就地解包（Toggle Off）；在 Markdown 富文本预览模式下以被聚焦块/行为目标实时反映更新，彻底告别在便笺末尾追加模板字符的体验缺陷。
  - **低调奢华真 3D 赛璐珞向导**：参数化正版菱形双棱锥切割冠部/腰棱/亭部，核心立体悬浮 `< I >` 恒光代码符号，三阶赛璐珞阶梯光照，黑曜钛金底板与皇家宝蓝行动条。
  - **便笺体验全面加固**：默认视口拓宽至 440px（12 项格式工具栏全景展开无截断），删除所有便笺保留用户自选颜色记忆，彻底消除拖拽后自我抖动。
  - **核心编辑器大选取复制修复**：彻底消除 >8 行大选区跨行复制/剪切时中间行丢失缺陷，100% 完整无损提取。
  - **AI Agent 线程守护与便笺工具集成**：全链路注入 `std::panic::catch_unwind` 守护，新增 `sticky_note_list / create / update / delete / open` 5 大专属工具。
  - **Agent 命令按钮全主题深度设计**：针对 Comic 贴纸徽章、Retro16 街机像素、Crystal 毛玻璃胶囊与 Fluent 专属重塑。
  - **常驻托盘退出强制置顶提示**：退出遇到未保存文档时，窗口自动解除最小化并激活置顶，弹窗采用 Windows `0x40023` 强制置顶显示。
  - **统一交付物正名**：编译与部署产物全面对齐为主程序 `IDE1.0.exe`。

---

## 🌐 Vercel 部署与云端接入 (Live Deployment)

- **Web Workbench 体验环境**: [https://fleet-acacia-7gzmn0a.tmp.now](https://fleet-acacia-7gzmn0a.tmp.now)  
- **Vercel 生产正式接入点**: `https://ide1d0.vercel.app`
- **MCP (Model Context Protocol) 云端网关**: `https://ide1d0.vercel.app/mcp` (重写至 `/api/v1/mcp`，支持 GET 发现与 POST JSON-RPC 2.0)
- **OpenAPI 3.1 规格**: [`docs/02_详细设计/Router-Worker-API.openapi.yaml`](docs/02_详细设计/Router-Worker-API.openapi.yaml)
- **API 网关根路径**: `/api/v1` (`/health`, `/jobs/dispatch`, `/tasks/claim`, `/patches/commit`, `/events`, `/mcp`)

---

## 🔌 IDE1.0 MCP 服务与跨供应商协同体系 (MCP Protocol & Cross-Provider Co-Editing)

IDE1.0 原生提供 **Model Context Protocol (MCP)** 服务（支持本地二进制 Stdio 与 Vercel Serverless HTTP 两种运行形态），专为多模型混合调度与无头自动化设计：

### 1. 核心 MCP 功能
- **跨供应商多 Agent 内存无头协同编辑器 (`headless_coedit`)**：
  - 支持 Claude 3.7、GPT-4o、DeepSeek-V3/R1、Gemini 等不同厂商的 AI 智能体在云端或本地内存中并发协同编辑同一文档；
  - **原子 CAS 快径**：当智能体提交的基准版本与当前版本一致时，执行零冲突快速写入；
  - **3-Way 行级自动合并引擎**：当两个智能体同时对同一文件的不同区块进行修改时，基于共同基准版本自动执行 3-Way 合并并提交新版本，无需人工干预；
  - **显式冲突标记**：当出现重叠代码修改冲突时，安全插入 `<<<<<<< HEAD (ours)` / `=======` / `>>>>>>> INCOMING (theirs)` 标记并返回冲突分析，杜绝静默代码覆写与数据丢失；
  - **操作幂等守护**：通过 `op_id` 实现严格幂等调用，重复网络重试不产生冗余版本。
- **Myers 文本差异比对引擎 (`diff_unified`, `diff_revisions`)**：
  - 行级 Myers Diff 算法，自动折叠不变上下文并输出标准 Unified Diff (`@@ -s,l +s,l @@`)；
  - 支持任意两段文本比对，或针对指定文档的历史双版本进行差异分析。
- **Blender `bpy` 风格 Rust 自动化脚本引擎 (`ide_rust_eval`)**：
  - 借鉴 Blender Python API 的三层设计，通过 Rust 语法代码操纵编辑器：
    - `ide::context::*`：查看工作区状态、活跃文件、选区与主题元数据；
    - `ide::data::*`：查询内存文件列表、历史版本树、全文正文；
    - `ide::ops::*`：执行打开、按行修改、全文替换、双版本比对、标签切换与协同扩容。
- **`editcore` 12 大标准 Agent 编辑工具**：
  - 完整暴露 `ide_open_file`、`ide_edit_lines`、`ide_replace_all` 等 12 个原生原子工具。

### 2. 各主流 AI 客户端导入与配置指南 (Import Guide)

#### A. 导入 Google Antigravity
在 Antigravity 全局配置文件 `~/.gemini/config/mcp_config.json`（或 `~/.gemini/settings.json`）的 `mcpServers` 字段中添加：

```json
{
  "mcpServers": {
    "ide1d0-mcp": {
      "url": "https://ide1d0.vercel.app/mcp"
    }
  }
}
```
*如需调用本地纯 Rust 高性能 Stdio 引擎，配置如下：*
```json
{
  "mcpServers": {
    "ide1d0-mcp-local": {
      "command": "cargo",
      "args": ["run", "-p", "ide-mcp", "--", "--mcp"]
    }
  }
}
```

#### B. 导入 Claude Desktop
打开 Claude Desktop 配置文件 `%APPDATA%\Claude\claude_desktop_config.json`（Windows）或 `~/Library/Application Support/Claude/claude_desktop_config.json`（macOS）：

```json
{
  "mcpServers": {
    "ide1d0": {
      "url": "https://ide1d0.vercel.app/mcp"
    }
  }
}
```

#### C. 导入 Cursor
在项目根目录 `.cursor/mcp.json` 或 Cursor 设置中的 **Features > MCP Servers** 添加：

```json
{
  "mcpServers": {
    "ide1d0-mcp": {
      "url": "https://ide1d0.vercel.app/mcp"
    }
  }
}
```

#### D. 导入 VS Code / Roo Code / Cline
在插件的 MCP 设置配置文件中添加：

```json
{
  "mcpServers": {
    "ide1d0-mcp": {
      "url": "https://ide1d0.vercel.app/mcp"
    }
  }
}
```

#### E. 命令行验证连通性
```bash
# 验证服务发现与工具清单
curl https://ide1d0.vercel.app/mcp

# 测试 JSON-RPC 2.0 工具调用 (Diff 计算)
curl -X POST https://ide1d0.vercel.app/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"diff_unified","arguments":{"text_a":"old line\n","text_b":"new line\n"}}}'
```

---

## 🦀 原生界面底栏专属 Rust 控制台 (Blender bpy-style Rust Console)

在原生桌面端主界面（`IDE1.0.exe`）底端面板中，除原有 `[TERMINAL]`、`[OUTPUT]`、`[PROBLEMS]` 之外，实装了全新的 **`[RUST CONSOLE]` (Rust 控制台)** 选项卡：
- **开发者与 AI 统一接口**：在终端输入框输入任意 `ide::*` Rust 表达式即可实时读写当前编辑器内存对象。
- **顶部预设宏工具栏**：提供 `status()`、`docs()`、`help()`、`diff_docs(0, 1)`、`collab(10000)`、`Clear` 等快捷一键执行按钮。
- **全彩日志回显与输入提示**：彩色语法分级高亮显示执行结果，支持单行命令与多行/分号分隔的复合自动化脚本。

---

## 🤖 AI Agent 深度调用与智能工具路由指南 (AI Tool Calling & Autonomous Integration)

> **面向 LLM、自主代理系统（AutoGPT, CrewAI, LangChain, Semantic Kernel, Claude Code, Antigravity, OpenDevin）、MCP 服务器与自动化流水线的权威工具定义与集成协议。**

### 1. 工具身份与语义检索索引元数据 (Tool Discovery Metadata)
当外部智能体根据意图检索开发环境、代码编辑器、多智能体协同框架或高性能文本内核时，可直接通过以下标准元数据精确命中：

```json
{
  "name": "ide10_editor",
  "display_name": "IDE1.0 Native AI Development Environment",
  "version": "1.0.0",
  "description": "High-performance, pure Safe Rust AI-native editor and multi-agent coordination platform. Features sub-500ms cold start, O(1) buffer editing, up to 100,000 concurrent agents orchestration, atomic CAS transactional patching, and PowerShell-fidelity terminal subsystem.",
  "keywords": [
    "ide", "code-editor", "rust-ide", "ai-native-editor", "multi-agent-orchestrator",
    "ast-patching", "cas-fencing", "nema-dsl", "powershell-terminal", "headless-editor",
    "collaborative-coding", "mcp-server", "zero-unsafe-code", "simd-text-processing"
  ],
  "capabilities": [
    "buffer.open", "buffer.read", "buffer.patch", "buffer.cas_commit",
    "terminal.execute", "terminal.persistent_session", "terminal.interrupt",
    "collab.dispatch", "collab.scale_roster", "collab.stream_events"
  ]
}
```

### 2. 命令行 CLI 自动化调用接口 (CLI Invocation Specification)

外部系统或脚本可直接以无头或窗口模式调用可执行文件 `IDE1.0.exe`：

```bash
# 1. 唤醒并打开指定文件（若已运行则通过 IPC 毫秒级定向唤醒已打开窗口，否则秒级冷启动）
IDE1.0.exe src/main.rs src/lib.rs

# 2. 强制开启全新的独立编辑器实例（忽略已有窗口互斥体）
IDE1.0.exe --new src/feature.rs

# 3. 运行全新 3D 菱形徽标环境配置与安装部署引擎
IDE1.0.exe --setup

# 4. 运行一体化修复与卸载向导（支持组件校验、注册表清理与偏好重置）
IDE1.0.exe --maintenance

# 5. 指定外部独立配置文件启动（实现沙箱化隔离测试与多租户测试）
IDE10_SETTINGS="C:/tmp/test_settings.json" IDE1.0.exe
```

### 3. 系统环境变量协议规范 (Environment Variables Contract)

| 环境变量名 | 取值类型 | 默认行为 | 针对 AI Agent 与 CI/CD 自动化的用途 |
|---|---|---|---|
| `IDE10_TRACE` | 文件绝对路径 | 无 | 设置后将首屏渲染时间（SLA < 500ms）、内存峰值、IPC 事件以原子 JSON 格式落盘追加，供基准评测。 |
| `IDE10_SETTINGS` | 文件绝对路径 | `%APPDATA%/IDE1.0/settings.json` | 隔离测试或沙箱调度时指定独立的用户配置与模型列表路径。 |
| `IDE10_OFFLINE` / `IDE10_NO_NETWORK` | `1` / `true` | `0` | 严格断绝一切外部网络外呼探针与自动更新检查，供气隙（Air-Gapped）密级环境与纯离线测试。 |
| `IDE10_MULTI` | `1` / `true` | `0` | 允许同一工作区同时并发拉起多个独立的系统进程，不被单实例互斥锁接管。 |
| `IDE10_BACKDROP` | `none` / `acrylic` / `mica` | 系统自适应 | 控制窗口合成器毛玻璃材质（无头截图或 E2E 视觉回归时可设为 `none` 获得确定性像素）。 |

### 4. 进程间管道通信协议 (IPC Wire Protocol: `IDE1.0-OPEN`)

通过 Windows 具名消息向现有窗口派发指令：
```text
IDE1.0-OPEN
<file_absolute_path>
<line_number>:<column_number>
```
- **延迟指标**: 跨进程响应耗时 $\le 12\text{ms}$，自动聚焦并激活对应标签页。

### 5. 多 Agent 规模化并发调度保证 (Multi-Agent Scale & Numeric Display)

- **万级编队支持**: 内置 `CollabState` 运行时支持 **1 至 100,000 个** 智能体并行编队。
- **自适应数值呈现**: 当协同 Agent 数量超出视觉承载上限（阈值 $> 8$）时，系统自动切换为数字标识徽章（如 `数额: 10,000`）；在名录与下拉搜索框中，自动实施前 50 项视口虚拟化与统计汇总（`共 10,000 个 Agent (显示前 50 个匹配项)`），保证 UI 在调度上万级 Agent 时维持 60 FPS 零掉帧。
- **并发互斥隔离**: 单 Agent 独立线程执行与独占取消句柄（`AtomicBool`），任何子 Agent 异常退出不影响总控流水线。

### 6. 原子补丁与 CAS 事务提交流程 (Transactional AST Patching)

AI Agent 对缓冲区做修改时，推荐遵循 CAS + Fencing Token 语义原子提交：

```json
{
  "action": "apply_patch",
  "doc_id": 1,
  "expected_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "fencing_token": 1042,
  "patches": [
    {
      "start_line": 45,
      "end_line": 52,
      "replacement": "    // AI Agent #2 优化写入\n    let result = simd_accelerated_calc(input);\n"
    }
  ]
}
```
- 若 `expected_hash` 与当前内存文档指纹不匹配，内核拒绝执行并返回 `CAS_CONFLICT` 及最新上下文行差分，阻断脏写。

---

## ⚡ 核心架构：Semantic Agent Router & Worker

系统彻底解耦控制面、语义任务面与执行面，在保留 Tauri 桌面端与 `editcore` 纯净编辑特性的同时，赋能无头 AI Agent 高并发安全协同：

```
Browser IDE / Tauri / External Agent / MCP / CLI
                     │
            HTTPS Agent Gateway (JWT / Scope Auth)
                     │
          ┌──────────┴──────────┐
          │  Vercel Router API  │ (Next.js Functions / Edge)
          └──────────┬──────────┘
                     │
      ┌──────────────┴──────────────┐
┌─────▼───────┐             ┌───────▼────────┐
│ PostgreSQL  │ (Ledger)    │  Queue/Outbox  │ (Rate-Limited)
│ Revs/Leases │             │   Dispatcher   │
└─────┬───────┘             └───────┬────────┘
      │                             │
      │                     ┌───────▼────────┐
      │                     │ Vercel Sandbox │ (Worker Fork)
      │                     └───────┬────────┘
      │                             │
      │                     ┌───────▼────────┐
      │                     │Rust AST Engine │ (Parse/Patch)
      │                     └───────┬────────┘
      │                             │
┌─────▼─────────────────────────────▼────────┐
│          File Commit Coordinator           │ (Rust Core CAS)
│      CAS → Anchor Relocate → New Rev       │
└────────────────────────────────────────────┘
```

### 四大非妥协机制
1. **文件是一致性与路由边界**: 权威文件由 Coordinator 进行严格 CAS 串行化推进，彻底杜绝并发覆盖。
2. **语义单元是作业与锁定边界**: 基于 AST（支持 Rust、Python、TypeScript、JavaScript、C、PowerShell）细化至函数与类型级别，多 Agent 可同时并发编辑同一文件的不同函数。
3. **Patch 是交付与验证边界**: Agent 仅提交结构化 Patch Proposal，不直接覆盖权威源文件。
4. **Fencing Token 租约保护**: 认领任务获得 60s TTL 与单调递增 Token，过期或被抢占的提交被严格 412 拒斥。

---

## 📦 Agent SDK 快速接入 (Python / TypeScript)

### Python SDK
```python
from sdk.python.ide_agent_sdk import IDEAgentClient

client = IDEAgentClient(base_url="https://ide1-0.vercel.app/api/v1")

# 1. 认领任务
task = client.claim_task(worker_id="agent_alpha", workspace_id="ws_main")

# 2. 准备并提交 AST Patch
res = client.commit_patch(
    task_id=task["task_id"],
    worker_id="agent_alpha",
    fencing_token=task["fencing_token"],
    file_id=task["file_id"],
    base_revision=task["base_revision"],
    symbol_path=task["symbol_path"],
    replacement_text="pub fn compute() -> i32 { 42 }"
)
print("Committed successfully with new revision:", res["new_revision"])
```

---

## §0 三阶段累计交付 (per 2026-10-03 dev 收尾)

| 阶段 | 内容 | 交付 commit / PR | 状态 |
|---|---|---|---|
| **Stage 1** | `.aci.json` schema v0.1 (跨项目 1:1) | Star PR #93 → 已 ship main + dev | ✅ |
| **Stage 2** | Rust `aci-emitter v0.1.0` (10 必填字段 + 4 layer + 5 severity + 4 status + 17 expect_value_types) | [ULYS-224](https://github.com/UlyssesLeoLee/aci-emitter) rev=df28c56 | ✅ |
| **Stage 3.0** | 最小 Cargo skeleton + `ide-kernel-core` + `ide-cli` + `aci-emitter` 集成 | PR #2 (5af5e7c) | ✅ MERGED |
| **§4.4 L1+L2** | mock `cluster_switch` + `plugin_switch` (2 plugin) | PR #3 (c4b62bb) | ✅ MERGED |
| **§4.4 L3** | mock `module_switch` Stage 1 (8 module: ide-cli×4 + ide-kernel-core×4) | PR #4 (54aacf0) | ✅ MERGED |
| **§4.4 L3 收尾** | regression report 标 IDE1.0 ✅ MERGED,跨项目 5/7 累计 | 48425a2 | ✅ MERGED (含 PR #5 squash) |
| **§4.4 stage6** | mock_switch 11 边界/失败用例 (Python `unittest`) + CI `mock-switch-validate` job | PR #5 (15beb40) | ✅ MERGED |
| **IDE Shell** | `crates/ide-shell` 骨架 — TUI (ratatui) + AI bridge (PlaceholderAi) + Vim normal/insert/command + 鼠标 (crossterm EnableMouseCapture) — 30 UT | PR #6 (f0753ab) | ✅ MERGED |
| **Lint 全清** | `.markdownlint.json` 关 MD060 (版本漂移噪音 141) + 修 16 真错 → 0 issues | PR #7 (42212f4) | ✅ MERGED |
| **四层测试矩阵** | UT (61 Rust) + IT (跨 crate 10) + ST (cli_smoke + 26 Python mock) + **UAT (Playwright e2e 40 case, `crates/ide-shell-web` web 渲染)** | PR #9 (本笔) | 🟡 OPEN |

测试矩阵 (本机 rustc 1.98.1 全绿):

- Rust: `cargo test --workspace --all-targets` → **61 测试** (3 ide-cli + 3 ide-cli IT + 2 ide-kernel-core + 38 ide-shell + 8 ide-shell-web UT + 7 ide-shell-web IT)
- Python mock: `test_ide1_mock_switch.py` → **15** + `test_ide1_mock_switch_edge.py` → **11** = **26**
- System: `cli_smoke.sh` → **4/4 PASS**
- UAT (Playwright): `npx playwright test` → **40/40 PASS** (chromium, 6 spec 文件)
- Markdown lint: **0 issues / exit 0**
- CI: **9 jobs** (Rust + Integration + Mock-switch-validate + UAT-Playwright + Cross-language parity + Markdown lint + Rust bench + Cross-project smoke + CodeRabbit)

## §1 简介

**IDE1.0** = 纯 Rust AI Native CLI Development Kernel (per 需求规格书).
Stage 1 (Star `_lib_aci_emit.py` + `.aci.json` schema v0.1) 已 ship,
Stage 2 (Rust `aci-emitter v0.1.0`) 已 ship ([ULYS-224](https://github.com/UlyssesLeoLee/aci-emitter)).

本仓 (IDE1.0) 当前提供:

- **底层 kernel (Stage 3.0 最小骨架)**:
  - 最小 Cargo workspace (4 crates: `ide-kernel-core` + `ide-cli` + **`ide-shell`** + **`ide-shell-web`**)
  - 3 个 placeholder integration tests (`tests/integration.rs`)
  - Bash smoke (`tests/st/cli_smoke.sh`)
  - 9 jobs GitHub Actions CI (含 UAT Playwright job)
  - `.aci.json` schema v0.1 (1:1 拷贝自 Star)
  - 集成设计文档 (`docs/architecture/aci-integration.md`)

- **IDE Shell 骨架 (ULYS-191 §3, 同产品双外观)**:
  - `crates/ide-shell` — TUI + AI + Vim + 鼠标 (PowerShell 替代/增强内核共用)
  - `crates/ide-shell-web` — 同内核 web 渲染 + HTTP server (Playwright UAT target)
  - `tests/uat/` — Playwright e2e 40 case (UT/IT/ST/UAT 四层矩阵补齐)
  - 见 [§8 IDE Shell 骨架](#8-ide-shell-骨架-ulys-191-3) + [§9 四层测试矩阵](#9-四层测试矩阵-utitstuat)

⚠️ **本笔仅最小骨架**. 完整内核实装待后续 brief (Stage 3 方案 B).

## §2 快速开始

```bash
# 1. 编译
cargo build --workspace

# 2. 单元 + 集成测试
cargo test --workspace --all-targets -j 4

# 3. CLI smoke (调 ide-cli emit 一条 ACI assertion)
bash tests/st/cli_smoke.sh

# 4. Lint (严, -D warnings)
cargo clippy --workspace --all-targets --no-deps -- -D warnings

# 5. Format check
cargo fmt --all -- --check
```

CLI 用法:

```bash
# 打印版本
cargo run --quiet -p ide-cli -- --version

# Emit 一条 ACI assertion
cargo run --quiet -p ide-cli -- --aci-emit ide1.0:smoke:smoke-1

# 启动 IDE Shell demo (TUI + Vim + AI placeholder)
cargo run --quiet -p ide-shell --bin ide-shell
```

## §3 ACI 接口 (per §4.2.3)

IDE1.0 集成 [`aci-emitter v0.1.0`](https://github.com/UlyssesLeoLee/aci-emitter) (ULYS-224 已 ship).
所有 ACI assertion 必须符合 `.aci.json` schema v0.1:

- **10 必填字段**: `assertion_id`, `aci_version`, `layer`, `scope`, `expect`, `actual`, `status`, `severity`, `reasoning`, `captured_at`
- **4 测试层**: `ut`, `it`, `st`, `e2e`
- **5 严重度**: `critical`, `high`, `medium`, `low`, `info`
- **4 状态**: `PASS`, `FAIL`, `WARN`, `SKIP`
- **17 expect_value_types**: `response_within_ms`, `response_status_2xx`, `field_equals`, ...

详细字段语义见 [`.aci.json`](.aci.json).

### 3.1 CLI emit 示例输出

```bash
$ ide-cli --aci-emit ide1.0:smoke:smoke-1
```

输出示例 (sort_keys, ACI v0.1 — 字段按字典序):

```json
{
  "aci_version": "0.1.0-draft",
  "actual": {
    "description": "measured 5ms (placeholder)",
    "type": "response_within_ms",
    "value": 5
  },
  "assertion_id": "ide1.0:smoke:smoke-1",
  "captured_at": "2026-09-24T...",
  "expect": {
    "description": "CLI should start within 100ms",
    "type": "response_within_ms",
    "value": 100
  },
  "layer": "it",
  "reasoning": "actual << expect (20x margin) — placeholder per §4.2.2 brief v0.1",
  "scope": {
    "module": "smoke",
    "project": "ide1.0"
  },
  "severity": "info",
  "status": "PASS"
}
```

### 3.2 Rust 库用法

```rust
use aci_emitter::{AciEmitter, ExpectActual, ExpectValueType, Layer, Scope, Severity, Status};

let em = AciEmitter::new(Layer::It);
let assertion = em.build(
    "ide1.0:it:example-1",
    Scope::new("ide1.0".into(), Some("it".into()), None, None, None, None),
    ExpectActual::new(ExpectValueType::WorkflowCompletes, serde_json::json!(true), "should complete"),
    ExpectActual::new(ExpectValueType::WorkflowCompletes, serde_json::json!(true), "did complete"),
    Status::Pass,
    Severity::Info,
    "placeholder",
).unwrap();
```

完整 API 见 [`aci-emitter` upstream docs](https://github.com/UlyssesLeoLee/aci-emitter).

## §4 架构

IDE1.0 当前 4 个详细设计书 (设计阶段, **未实装**):

| DD | 名称 | 状态 |
|---|---|---|
| DD-01 | Microkernel Core 详细设计書 | 设计 (实装待 ULYS-191 后续 brief) |
| DD-02 | Session Transaction Buffer Engine 详细設計書 | 设计 (实装待 ULYS-191 后续 brief) |
| DD-04 | Plugin Loader / Search / TUI | 设计 (实装待 ULYS-191 后续 brief) |

Stage 3.0 集成设计见 [`docs/01_基本设计/architecture/aci-integration.md`](docs/01_基本设计/architecture/aci-integration.md).

## §5 守门合规 (13 项, per AGENTS.md §4)

- **#5** no secret leak ✅
- **#6** 中文默认 (README/architecture/regression report 全中文) ✅
- **#7** `unsafe_code = "forbid"` (workspace.lints.rust) ✅
- **#9** subprocess (cli_smoke.sh 调 `cargo run --aci-emit`, 不调用户脚本) ✅
- **#10** author=Ulysses ✅
- **#11** 缺标比错标 (7 deps 全 workspace + 显式 version, 1 git dep 锁 rev) ✅
- **#12** docs 同步 (README + architecture + regression report) ✅
- **#13** W/T/M (单元 + 集成 + 系统) ✅
- **#14v4** PR merge ✅
- **#15** scope creep (1 sub-agent 1 切点) ✅
- **#17** commit 完整 ✅
- **#19v19** Python 化 (IT-3 跨语言 parity 测) ✅
- **#24** vendor 中立 (7 deps + 1 git dep) ✅

## §6 已知缺口 (4 项)

- **G-ACI-03** 跨语言 emitter ≥4 种 (Python ✅ / Bash ✅ / Rust ✅ / TS ⏳ ULYS-191.7)
- **G-ACI-07** 部分项目可能无 mock (本笔 IDE1.0 = 最小骨架, 提供未来 mock 位置)
- **G-ACI-08 (新)** IDE1.0 首次引入 Cargo workspace, design docs 与 code 可能滞后
- **G-SHELL-01 (新, ULYS-191 §3)** IDE Shell 仅骨架,真实 LLM 接入 / 多行 buffer / 历史补全 / PSReadLine 插件形态 均留 Stage 4+

## §7 License

GNU Affero General Public License v3.0 (`AGPL-3.0-only`), 见 [LICENSE](LICENSE)。

随程序打包的第三方组件保留各自的许可: 默认插件 Pele (MIT, `crates/editcore/plugins/pele/LICENSE`); `editcore` 中沿用上游 Sakura Editor 的部分为 Zlib 许可。

## §7.1 Code signing policy / 隐私声明

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org). *(申请中: 通过之前发布的 Windows 安装包尚未签名。)*

- **Authors / Reviewers / Approvers**: [@UlyssesLeoLee](https://github.com/UlyssesLeoLee) (当前为单人维护; 有第二位维护者加入后在此分开列出)。
- 只签名由本仓库源码经自动化工作流 ([`release-windows.yml`](.github/workflows/release-windows.yml)) 构建的二进制; 每次发布在签名前都需要人工批准。
- **隐私**: IDE1.0 在本机运行, 没有遥测。除非你主动要求, 程序不会向其它联网系统发送信息: 「扩展」面板的"检查更新 / 安装"只在你点击时访问 GitHub (`api.github.com`、Release 资产); Chat 面板 / "让 AI 给出改善方案"只在你确认后, 把你选定的文字 (选中内容或当前章节) 发往**你自己配置的** AI 端点, API Key 只保存在本机并只发给该端点。

## §8 IDE Shell 骨架 (ULYS-191 §3)

> **同产品双外观,内核相通,尽量在一起做** (per D-Boy 10/3 拍板).
> IDE1.0 仓既承载底层 Rust kernel,也在 `crates/ide-shell` 内做 PowerShell 替代/增强
> 内核的 TUI + AI + Vim + 鼠标 骨架。两种外观(全壳替代 vs PSReadLine 插件)共用 ide-shell 内核。

### 8.1 设计目标

| 维度 | 目标 |
|---|---|
| 内核 | 单仓 `crates/ide-shell` 复用 `ide-kernel-core` 提供 version/kernel_status |
| 外壳 1 (全壳替代) | `cargo run -p ide-shell --bin ide-shell` 启动 ratatui TUI,内置 PS-like prompt |
| 外壳 2 (插件形态) | (Stage 4+ 后续 brief,共内核不同 entry) |
| 键位 | Vim normal/insert/command 三态 (hjkl / i / : / Esc / q) |
| 鼠标 | crossterm `EnableMouseCapture` — 左键 Insert / 右键 Normal |
| AI | `AiBackend` trait + `PlaceholderAi` 占位(返回 `AI suggestion: <input>`);真实 LLM 接入 Stage 4+ |
| 桥接 | 复用 Stage 2 `aci-emitter v0.1` 走 .aci.json schema (ACI 必填字段不变) |

### 8.2 模块分层

```text
crates/ide-shell/
├── Cargo.toml                  # workspace 共享 deps, 锁 ratatui 0.28 + crossterm 0.28
└── src/
    ├── lib.rs                  # 模块声明 + App / Mode re-export
    ├── mode.rs                 # Vim 三态枚举 (serde Serialize/Deserialize)      + 2 UT
    ├── input.rs                # 编辑缓冲 (Vec<char> + cursor + 移动/删除)       + 7 UT
    ├── keymap.rs               # KeyEvent + Mode → Action 映射                   + 11 UT
    ├── ai.rs                   # AiBackend trait + PlaceholderAi                 + 2 UT
    ├── render/
    │   ├── mod.rs              # backend 声明 (ratatui_render + web)
    │   ├── ratatui_render.rs   # TTY backend — 三段布局 ratatui widgets
    │   └── web.rs              # Web backend — WebFrame 结构化数据 (JSON)        + 8 UT
    ├── app.rs                  # App 状态机 + 事件循环 + 双 buffer 执行           + 9 UT
    └── bin/
        └── ide-shell.rs        # TUI binary entry — 终端初始化 + 主循环

crates/ide-shell-web/
├── Cargo.toml                  # 复用 ide-shell core, 0 web framework dep (std TcpListener)
├── src/
│   ├── lib.rs                  # HTTP server (/ + /api/frame + /api/event + /api/reset) + 8 UT
│   ├── index.html              # DOM 渲染 (data-mode/data-buffer/data-cursor 属性供 e2e selector)
│   └── bin/
│       └── ide-shell-web.rs    # web server binary entry (默认 127.0.0.1:8123)
└── tests/
    └── integration.rs          # HTTP server 级 IT — 真 TCP 请求验证端点行为     + 7 IT
```

总计 **38 ide-shell 单测 + 8 ide-shell-web UT + 7 ide-shell-web IT**,全部 `cargo test --workspace` 通过。

两个 render backend 共享同一份 `&App` 状态 → TUI 与 Web 视觉/行为 parity (双外观内核相通)。

### 8.3 键位速查

| Mode | 键 | 动作 |
|---|---|---|
| Normal | `hjkl` / ←→ | 光标左/右 |
| Normal | `0` / `$` / Home / End | 行首/行尾 |
| Normal | `i` / `a` | 进 Insert |
| Normal | `:` | 进 Command (预置 `:`) |
| Normal | `x` | 删除右侧 |
| Normal | `q` | 退出 |
| Insert | 字符 | 插入 buffer |
| Insert | Backspace / Delete | 删左/右 |
| Insert | Enter | 执行 buffer 作为命令 |
| Insert | Esc | 回 Normal |
| Command | 字符 | 追加到 cmd buffer |
| Command | Enter | 执行 command(去掉 `:` 前缀) |
| Command | Esc | 回 Normal |
| 全局 | 鼠标左键 | 切 Insert |
| 全局 | 鼠标右键 | 切 Normal |

### 8.4 内置命令

| 命令 | 效果 |
|---|---|
| `:help` / `help` | 打印键位速查 |
| `:version` / `version` | 打印 IDE1.0 kernel banner |
| `:ai` / `ai` | 调 `ai_suggestion()` 当前 buffer(占位回显) |
| `:clear` / `clear` | 清空 history |
| `:q` / `quit` | 退出 |
| 其它 | 兜底走 `cmd /C <cmd>` (Windows) / `sh -c <cmd>` (Unix) 子进程执行 |

### 8.5 不在本笔范围 (Stage 4+)

- 真实 LLM 接入 (Anthropic / OpenAI / MiniMax 任选)
- 行内补全 ghost text 渲染
- 历史命令补全(`Up`/`Down` 翻历史)
- 多行 buffer (现仅单行)
- PowerShell 真实解析(现仅 shell 兜底)
- PSReadLine 插件形态 entry

## §9 四层测试矩阵 (UT/IT/ST/UAT)

> per 「UT,IT,ST,基于 playwright 的 UAT 都要做到位」(2026-10-03).

| 层 | 位置 | 数量 | 工具 | 覆盖 |
|---|---|---|---|---|
| **UT** (单元) | 各 crate `src/` `#[cfg(test)]` | **61** Rust + **26** Python | `cargo test` / `unittest` | mode 状态机 / input buffer / keymap / AI bridge / web render / HTTP server / mock_switch 数据形状 + 11 边界用例 |
| **IT** (集成) | `crates/ide-cli/tests/integration.rs` + `crates/ide-shell-web/tests/integration.rs` | **3 + 7** | `cargo test --all-targets` | ACI schema roundtrip / emitter 兼容性 / HTTP 端点真 TCP 行为 (frame/event/reset/404/quit 信号) |
| **ST** (系统) | `tests/st/cli_smoke.sh` | **4 step** | bash + cargo run | ide-cli 端到端 emit → 10 必填字段 → schema version |
| **UAT** (验收) | `tests/uat/specs/*.spec.ts` | **40** case / 6 spec | **Playwright 1.63 (chromium)** | 浏览器驱动完整用户流: 页面加载 / mode 切换 / Vim 键位 / 鼠标 / AI 建议 / 内置命令 / DOM 属性追踪 / 端到端工作流 |

### 9.1 UAT 架构

```text
Playwright (chromium)
    │  HTTP (fetch /api/event)          DOM (keydown / mousedown)
    ▼                                    ▼
ide-shell-web server ◄────────── index.html (data-mode/data-buffer 属性)
    │
    ▼
ide-shell core (App 状态机 — 与 TUI binary 100% 同源)
```

- Web backend 渲染 `WebFrame` JSON(与 ratatui backend 共享同一 `&App`),Playwright 双通道验证:API 层 (frame JSON) + DOM 层 (`data-*` 属性 / 键盘鼠标事件)。
- `webServer` 由 `playwright.config.ts` 自动启停(`reuseExistingServer` 本地开发复用)。

### 9.2 UAT 跑法

```bash
# 1. build web server binary
cargo build -p ide-shell-web --bin ide-shell-web

# 2. 装 deps (首次)
cd tests/uat && npm install && npx playwright install chromium

# 3. 跑 40 case (webServer 自动启停)
IDE_SHELL_WEB_BIN=../../target/debug/ide-shell-web npx playwright test
```

CI: `uat-playwright` job (ubuntu-latest, Node 20, chromium + browser cache).

## §10 Tauri Desktop 打包 (ULYS-191 §4)

> per 「我需要的是打包安装文件的桌面版」(2026-10-03).

把 IDE Shell 从 web (HTTP server) 升级为 **Tauri 2 桌面应用** — 用户双击 `.msi` 安装,启动后弹原生窗口。同 `ide-shell` core,双 frontend (web + desktop) 共享后端状态机。

### 10.1 架构

```text
                chromium (Playwright UAT)                    Tauri 2 webview (Windows/macOS/Linux)
                        │                                              │
                   fetch /api/*                                  invoke('frame' / 'key' / ...)
                        │                                              │
                        ▼                                              ▼
                ide-shell-web server                      ide-shell-desktop.exe (Tauri managed state)
                        │                                              │
                        └──────────┬───────────────────────────┘
                                   │
                                   ▼  共用同一 `ide-shell::App` 状态机 (38 UT)
                            crates/ide-shell
                                   │
                                   ▼
                            crates/ide-kernel-core
```

- **Tauri 2 已知限制** (per [issue #9362](https://github.com/tauri-apps/tauri/issues/9362)):
  `#[tauri::command]` 在 `lib.rs` root + 同 crate 有 `main.rs` 时 macro 重复定义。本笔绕法: commands 放 `lib.rs` root 但**不用 `pub`**(`__cmd__xxx` macro 是 private 不能跨 module),`invoke_handler!` 在同 module 引用。

- **前端代码复用**: `crates/ide-shell-web/src/index.html` 拷到 `crates/ide-shell-desktop/dist/index.html`,JS 加 ~10 行 adapter 检测 `window.__TAURI_INTERNALS__` 走 `invoke` vs `fetch`。`tests/uat/` Playwright 仍可跑 (走 fetch 分支) — 桌面/Tauri 与 web UAT 一份前端代码。

### 10.2 模块结构

```text
crates/ide-shell-desktop/
├── Cargo.toml              # 依赖 tauri 2.x + ide-shell + crossterm (for KeyEvent 类型)
├── build.rs                # tauri_build::build()
├── tauri.conf.json         # app 配置 (windows.bundle + identifier)
├── capabilities/default.json   # Tauri 2 必需
├── icons/                  # 占位 PNG/ICO (品牌待设计)
│   ├── 32x32.png  128x128.png  128x128@2x.png  icon.ico  Square30x30Logo.png
├── src/
│   ├── main.rs             # entry — ide_shell_desktop::run()
│   └── lib.rs              # ShellState + 6 commands + 5 UT
├── dist/index.html         # 复用 ide-shell-web 的 index.html + tauri adapter
└── tests/integration.rs   # 3 IT (state 并发 / KeyResponse shape / quit 信号)
```

### 10.3 6 个 Tauri commands

| Command | 签名 | 用途 |
|---|---|---|
| `frame` | `() -> WebFrame` | 拉当前 frame JSON |
| `reset` | `() -> WebFrame` | 重置 app |
| `key` | `(code, modifiers) -> KeyResponse` | 派发键盘事件 |
| `mouse` | `(button) -> WebFrame` | 派发鼠标点击 |
| `kernel_banner` | `() -> String` | 复用 ide-kernel-core |
| `get_mode` | `() -> String` | 取当前 mode (避开 Rust 关键字 `mode`) |

### 10.4 打包跑法

```bash
# 1. 装 Tauri CLI (一次性)
cd tools && npm install  # @tauri-apps/cli 2.12

# 2. 出 Windows .msi
cargo build --release -p ide-shell-desktop
cd crates/ide-shell-desktop && ../../../tools/node_modules/.bin/tauri build --bundles msi --config ./tauri.conf.json
# 产物: target/release/bundle/msi/IDE1.0 IDE Shell_0.1.0_x64_en-US.msi (~2.7 MB)

# 3. macOS / Linux 同理 (换 target)
npx tauri build --bundles appimage --config ./tauri.conf.json  # Linux
npx tauri build --bundles dmg --config ./tauri.conf.json        # macOS
```

### 10.5 验证 (本机 rustc 1.98.1 + Node 26)

| 步骤 | 结果 |
|---|---|
| `cargo test --workspace --all-targets` | ✅ **69 测试** (61 + 8 ide-shell-desktop) |
| `cargo build --release -p ide-shell-desktop` | ✅ 8.1 MB `ide-shell-desktop.exe` |
| `npx tauri build --bundles msi` | ✅ **2.7 MB MSI** (`IDE1.0 IDE Shell_0.1.0_x64_en-US.msi`) |
| `cargo clippy -D warnings` | ✅ 0 warning |
| `cargo fmt --check` | ✅ |
| `markdownlint` (CI 同款) | ✅ 0 issues |

CI 增量: 新增 `tauri-build-linux` job (ubuntu-latest, 出 `.deb` + `.AppImage`)。Windows MSI + macOS DMG 留给后续 brief (需要 Windows runner + Apple Developer ID)。

## Phase 3-6 累计交付 (ulys-191-15 ~ 19)

### Phase 3 — Editor 高级特性

- **3.1 Symbol Outline** (`384f5be`): Rust 源文件 `fn main()` 通过 editcore `outline_dispatch` 解析 → JSON entries → Side Bar 显示 + click jump-to-line
- **3.2 Multi-cursor 视觉 v1** (`cb7b901`): Alt+Click 加光标 + Esc 清空 + status `N cursors` badge
- **3.3 MiniMap** (`cdac369`): 右侧代码缩略图 (line 长度条 + cursor 指示)
- **3.4 Breadcrumb** (`cdac369`): 顶部 path 显示 (drive / folders / file)
- **3.5 Folding gutter** (`cdac369`): ▼/▶ marker + click toggle + indent-based detection

### Phase 4 — AI 集成 (ulys-191-18)

- **ide-shell-protocol**: 22 JSON-RPC methods + 3 protocols (HTTP/WS/stdio)
- **Python client demo** (`examples/python_client_demo.py`): 演示 AI agent 通过 stdio/HTTP/WS 控制 IDE buffer
- **持久 StdioSession class**: 一个进程多 messages, 演示 `kernel_info`/`open_buffer`/`insert_at_cursor`/`get_buffer`/`edit_via_provider`/`chat` 完整流程

### Phase 5 — 商业化打磨 (ulys-191-19)

- **Theme toggle**: 🌙/☀️ 状态栏按钮 + CSS variable swap (Light/Dark)
- **Settings persistence**: Ctrl+= / Ctrl+- / Ctrl+0 font-size 调节, localStorage 保存
- **i18n**: English/中文 auto-detect (navigator.language), localStorage 持久

### Phase 6 — 测试 + CI (ulys-191-19)

- **editcore bench**: `cargo run -p editcore --example bench` — insert_str 10000 ops 175ms / **56,971 ops/s**
- **CI benchmark step**: rust-bench job 加 editcore 吞吐量验证 (target ≥ 50k ops/s)
- **四层测试矩阵**: UT (cargo test --workspace) + IT (cargo test --tests) + ST (cli_smoke.sh) + UAT (playwright 68 cases)

### 累计 commits (dev-3)

```text
2b6e356 Phase 5 Theme + Settings + i18n
87e9b89 Phase 4.1 Python client demo
cdac36 Phase 3.3-3.5 MiniMap + Folding + Breadcrumb
cb7b901 Phase 3.2 Multi-cursor 视觉 v1
384f5be Phase 3.1 Outline 面板
34f64e6 web Terminal
2953aae Bottom Panel + Terminal (desktop)
0cb471e editcore crate 全套
```
