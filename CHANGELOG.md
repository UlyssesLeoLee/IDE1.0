# Changelog

## 0.9.6 — 2026-11-24

### 扩展栏 `Skills` · `Hooks` · `MCP` 交互式添加与管理中心、非阻塞 MCP 工具预热与子进程生命周期管理

#### 1. 扩展栏（`Tab::Plugins`）点击展开交互式添加与独立管理中心 (`app.rs`, `plugin_ui.rs`, `agent_plugins.rs`)
- **点击即展开添加面板与独立管理中心**：在左侧扩展栏点击 **`▸ Skills 专业研发技能包`**、**`▸ Hooks 安全与生命周期拦截器`** 或 **`▸ MCP 模型上下文协议`** 标题行（或点击 `➕ 展开添加与管理` / `🗔 独立管理窗口`），即可在原位展开交互式管理抽屉或打开独立管理中心窗口；插件状态弹窗顶部同步内嵌快捷添加与导入按钮。
- **`Skills` 可视化添加、一键触发与批量目录导入**：支持直接在表单填写技能 ID、名称、`/trigger`、描述与 System Prompt 一键生成 `<workspace>/.skills/<id>/SKILL.md` 并即时热重载；列表支持点击 **`▶ 使用`** 将触发词自动填入右侧 AI 对话框，支持单个 `.md/.json` 导入与 **`📂 导入技能目录`** 批量导入。
- **`Hooks` 可视化规则配置与单/多 Agent 全链路生效**：支持在表单填写 Hook 名称、规则描述、Pre-Input 前置提示词、输入拦截词、禁用工具名、参数拦截串与第三方凭证脱敏前缀，一键写入 `<workspace>/.hooks.json` 并热挂载；单 Agent (`aichat.rs`) 与多 Agent (`collab.rs`) 对话入口统一执行 `Pre-Input`、`Pre-Tool`、`Post-Tool` 与 `Post-Response` 完整拦截链。
- **`MCP` 可视化服务添加、引号参数/环境变量支持与常用第三方模板**：支持在表单直接填写服务 ID、启动命令、支持双引号包裹空格路径的启动参数（`parse_quoted_args`）及 `KEY=VAL` 环境变量（`parse_env_pairs`），内置 `📁 Filesystem`、`🗄 SQLite`、`🌐 Fetch` 常用第三方模板一键填充。

#### 2. MCP 子进程生命周期安全、非阻塞后台预热与服务名最长前缀路由 (`ide-agent-router::mcp`, `agent_plugins.rs`)
- **子进程确定性回收 (`Drop for ClientWorker`)**：为 `ClientWorker` 实现 `Drop` 自动 `kill()` 并 `wait()` 回收外部 MCP Server 子进程，消除热重载或卸载时的孤儿进程泄漏。
- **非阻塞后台工具预热 (`aggregate_openai_tools_nonblocking`)**：对话轮次准备阶段采用非阻塞缓存读取并异步触发后台线程握手与 `tools/list` 预热，彻底避免外部 MCP 子进程冷启动阻塞 UI 主线程。
- **支持含下划线 (`_`) 的 MCP 服务名路由**：`McpManager::dispatch_tool_call` 采用已注册服务器最长前缀匹配替代首下划线切分，确保 `my_custom_mcp` 等包含下划线的服务名准确路由。

---

## 0.9.5 — 2026-11-23

### 中国默认区域用户协议 v1.1、Grep 主编辑区标签页与 SIMD 并行提速、全屏 0px 贴边、无遮罩弹窗及 90%+ 覆盖率门禁

#### 1. 用户协议 v1.1（默认适用区域：中华人民共和国） (`docs/IDE_1.0_用户协议.md`, `installer_ui/*`)
- **默认适用区域与准据法更新**：《最终用户许可与使用协议》升级至 v1.1，默认适用区域设为**中华人民共和国（中国大陆地区）**，准据法更新为**中华人民共和国法律**（引用《民法典》《网络安全法》《数据安全法》《个人信息保护法》），第一审管辖法院更新为发行方所在地或主要运营地有管辖权的中国大陆人民法院，责任兜底上限更新为**人民币 100 元（RMB 100）**。
- **全面移除弹窗全屏遮罩**：移除检查更新弹窗（`upd_window`）、应用内向导弹窗（`installer_window`）、文件选择加载提示与前景警示弹窗（`render_comic_alert_modal`）的全屏遮罩层，保持底层代码与文档始终清晰可读。

#### 2. Grep 检索体验升级与 SIMD + 多核分片并行性能优化 (`editcore::grep_engine`, `ide-fast/src/grep.rs`, `app.rs`, `tabbar.rs`)
- **折叠侧栏 `Ctrl+G` 自动展开**：当左侧边栏处于折叠状态（`side_collapsed = true`）时按下 `Ctrl+G`，自动展开侧栏并聚焦搜索框。
- **主编辑区默认打开 `Grep 结果` 标签页 &「结果不开标签页」开关按钮**：执行 Grep 查询时默认在主窗口编辑区打开（或复用并激活）文本形式的 **`Grep 结果`** 标签页；原“结果开成标签”按钮升级为可切换的 **`结果不开标签页`** 开关按钮，点击开启后自动关闭主编辑区结果标签页，使查询结果仅显示在左侧面板下方的结果栏位中。
- **双击打开文件与全功能右键菜单**：支持在 `Grep 结果` 主编辑区、标签页及左侧结果列表双击跳转到目标文件对应行列，并提供丰富右键菜单（打开文件并跳转、复制完整路径/相对路径/文件名/`路径:行:列`/命中行文本、在资源管理器中定位、设为 Grep 搜索目录、排除该文件/文件夹、复制全部结果、重新检索）。
- **文件夹栏右侧浏览按钮**：在 Grep 文件夹输入栏右侧新增 **`📁 浏览…`** 按钮，一键呼出系统原生目录选择器。
- **SIMD `memmem::Finder` 与多核并行检索**：引入 `memchr::memmem::Finder` 双字节稀有度 SIMD 搜索、纯 ASCII 检索词零拷贝预筛、零分配原地大小写不敏感扫描、`whole_word` 字节级边界检查、预编译 Glob 过滤器（`CompiledFileFilter`）以及多核分片保序并行流水线，在 210 MB / 3000 文件基准测试中吞吐量达 **1,694 MB/s**（耗时仅 123ms）。

#### 3. 编辑器核心修复与 Hooks 等热插拔插件全链路增强 (`titlebar.rs`, `markdown.rs`, `md_view.rs`, `find.rs`, `box_ops.rs`, `plugins/*`)
- **全屏/最大化四周 0px 贴紧边缘**：`MAX_OVERSHOOT` 归零，`maximized_inset` 在全屏与普通窗口均返回 `0.0`。
- **Markdown 加粗与 Alt 矩形列选视觉对齐**：支持 `**bold**` 与 `__bold__`（含对称词边界校验）及独立粗体字族 `editor_bold_family()`；`editcore::text_util` 统一导出包含 `\t` 宽度对齐的 CJK/ASCII 视觉显示列映射，彻底消除 Markdown 表格中英混排 Alt 矩形列选错位。
- **增量查找 (`isearch`) 修复**：完善正反向步进、锚点记忆与清空关键字回退起点。
- **Hooks / Skills / MCP 支持添加用户自定义与第三方扩展**：
  - 插件面板内联展示快捷执行按钮以及 **`➕ 新建/编辑自定义...`** 与 **`📥 导入第三方...`** 操作入口，保存配置文件（`Ctrl+S`）即刻自动热重载；
  - **Hooks**：支持通过工作区 `.hooks.json` / `.hooks/*.json` 或全局 `custom_hooks/*.json` 添加自定义与第三方生命周期规则（`pre_input_prefix`、`pre_input_block_patterns`、`blocked_tools`、`pre_tool_block_patterns`、`mask_patterns`）；
  - **Skills**：支持加载工作区 `.skills/<id>/SKILL.md`、`.skills/*.md`、`.skills/*.json`、第三方兼容目录 `.claude/skills/`、`.claude/commands/*.md`、`.agents/skills/` 及全局 `custom_skills/`；
  - **MCP**：支持合并加载全局 `mcp_servers.json` / `custom_mcp/*.json` 与工作区 `.mcp.json` / `.cursor/mcp.json`，兼容标准 `mcpServers` 与第三方 `servers` 格式；
  - `hooks:audit`、`mcp:status`、`automation:todos` 始终弹出可视化结果详情窗口。
- **零启动开销 Function Calling 与 ReAct 推理闭环增强** (`editcore::agent_api`, `ide-fast/src/ai/*`, `aichat.rs`, `collab.rs`)：
  - **静态 `OnceLock` 工具表缓存**：`editcore::agent_api::tools_json()` 采用 `OnceLock` 惰性初始化缓存，进程冷启动 0 开销，运行期构建工具表零重复 JSON 解析；
  - **ReAct 观测与结构化思考工具集**：新增 `read_doc_lines`（按需读取活跃文档指定行区间，自动注入文档缓冲免模型重传全文）、`workspace_grep`（基于 1,694 MB/s SIMD 引擎跨文件检索）、`react_thought`（显式记录 `thought` / `plan` / `reflection` 推理轨迹）；
  - **多格式 Function Calling 兼容与自我修正**：`SseParser` 同时支持 OpenAI `delta.tool_calls`、旧版 `function_call`、`reasoning_content` 思维链汇聚以及开源/推理模型的内联 `<tool_call>{...}</tool_call>` 零正则提取；`parse_args` 自动容错剥离 Markdown 代码块围栏与双重转义 JSON，`auto_apply` 模式下向下一轮 ReAct 精确回传最新文档行数（`new_total_rows`）。
- **90%+ 覆盖率与真机 GUI 20 步 E2E 门禁**：新增 `e2e_driver.rs` 真机 20 步 GUI 自动化实测与 `run-coverage-gate.ps1` 90%+ 分支/区域覆盖率门禁。

---

## 0.9.4 — 2026-11-22

### 向导全屏最大化与视觉通透优化、移除帮助菜单向导入口、及惰性插件化多 Agent 后端对等

#### 1. 安装向导交互与视觉打磨 (`menubar.rs`, `main.rs`, `installer_ui/*`, `updateui.rs`)
- **精简帮助下拉菜单**：移除「帮助」菜单中的「欢迎与安装向导…」入口。
- **安装向导全屏最大化支持**：独立安装/维护视口允许缩放与最大化，标题栏新增最大化/还原矢量按钮并支持双击标题栏切换；协议滚动区高度自适应拉伸，方便放大查阅《最终用户许可与使用协议》。
- **移除协议警示弹窗暗化蒙版**：未勾选同意点击继续时弹出 `render_comic_alert_modal` 保留透明交互防穿透遮罩，移除 `from_black_alpha(175)` 暗色背景层以保持亮洁清晰。

#### 2. 插件化惰性 Agent 运行时与多 Agent 能力对等 (`agent_plugins.rs`, `aichat.rs`, `collab.rs`)
- **冷启动零开销插件封装 (`AgentPluginRuntime`)**：将 `mcp`、`skills`、`mod`、`hooks`、`memory_engine`、`loop_breaker` 收拢至按需惰性初始化的 `Option<AgentPluginRuntime>`（`AiState::default()` 下为 `None`），并严格按热插拔插件激活状态加载与卸载。
- **多 Agent 与单 Agent 后端能力拉齐**：多 Agent 协同调度（`send_master_with_plugins`、`execute_parallel_edit_on_doc_with_plugins`、`send_individual_with_plugins`、`poll_with_docs_and_plugins`）全面支持 `PreInput`/`PreTool`/`PostTool`/`PostResponse` 钩子、Skills、Mod 模式与规则注入、工作记忆、`LoopBreaker` 熔断以及内建/`plugin_*`/`mcp_*` 多轮工具调用；同步修复部分 Agent 未配置时的分区切分问题与同帧多 Agent 完成写盘时的行偏移累加问题。

---

## 0.9.3 — 2026-11-22

### 恢复维护模式用户协议前置确认、折叠条网点自适应与顶部触发区收紧 (`installer_ui/*`, `docks.rs`, `app.rs`)
- **向导恢复协议前置确认**：从 MSI 安装目录自动进入维护向导（`InstallerLaunchMode::Auto`）时同样先展示 `InstallerStep::Agreement` 并同步 4 步步骤指示条。
- **左右折叠条黄色网点自适应分布**：`adaptive_ben_day_dot_ys` 按实际高度均匀填充中央拉手上下区域，消除大屏留白。
- **右上角 `导航 / MD / NEMA` 统一顶部悬停**：取消未展开项落到底部的跳动行为，并在 `Dock::Right` 折叠条最顶部切割紧凑正方形触发区（点击仍执行面板折叠/展开）。

---

## 0.9.2 — 2026-11-22

### 安装向导布局对齐、消除残留终端闪窗与多 Agent 并行修改净版输出 (`installer_ui/*`, `ide-fast.wxs`, `exec_cmd.rs`, `store.rs`, `collab.rs`)
- **统一底部内边距与警示弹窗**：规范向导各分镜 `640px` 高度与底部 `46px` 操作条布局；未勾选同意协议点击下一步及空路径时统一弹出 `ComicAlertModal`。
- **无头化修复**：MSI 改用 WiX 原生 `<util:CloseApplication>` 替换外部 `taskkill.exe`，为 `exec_cmd` / `store` 补充 `CREATE_NO_WINDOW`。
- **多 Agent 净版写回**：`extract_fenced_patch_lines` 仅从闭合代码块提取写回文本，杜绝流式进度日志或错误说明写入源文件。

---

## 0.9.1 — 2026-11-22

### MCP 最终安全加固、TLS 验证、无头安装与 Blender 式 `ide_rust_eval` (`api/v1/*`, `ide-mcp`, `ide-fast.wxs`)
- 补全 Proposal 工作区 ACL（`SEC-001`）、鉴权 Fail-Closed（`SEC-002`）与 PostgreSQL 严格 TLS 校验（`SEC-003`）。
- 对齐 MCP Streamable HTTP 协议与 `/api/v1/health` 实库探测（`MCP-001`），新增 `ide_rust_eval` 批量文本脚本引擎。

---

## 0.9.0 — 2026-11-22

### MSI 安装后即刻可启动修复、精简安装包与绿色便携版双形态发布、以及 Vercel MCP 持久化加固全量同步

本版本解决 `0.8.9` MSI 安装包安装或覆盖更新后未拉起主程序、缺乏桌面快捷方式及独立 Setup 冗余问题，完成真实开机窗口可见性实测门禁，并同步将 Vercel 生产端与桌面端全部升级至 **`v0.9.0`**。

#### 1. MSI 安装体验与可启动性彻底修复 (`ide-fast.wxs`, `build-msi.ps1`, `installer_ui/deploy.rs`)
- **安装/升级/修复即刻自动拉起主程序**：将 `Launch3DSetup` (`IDE1.0.exe --setup`, `NOT Installed`) 改为直接启动 `LaunchIDE` (`IDE1.0.exe`, `NOT REMOVE="ALL" AND NOT IDE10_NO_LAUNCH`)，彻底消除升级场景跳过启动与向导二次跳转问题。
- **补齐桌面快捷方式 (`DesktopFolder`)**：MSI 现同步创建桌面快捷方式与开始菜单快捷方式，指向 `[INSTALLFOLDER]IDE1.0.exe`。
- **精简发版产物为双形态**：移除独立 `IDE1.0_Setup_*_x64.exe` 冗余副本，仅交付企业/标准安装包 (`IDE1.0_0.9.0_x64.msi`)、单文件绿色便携版 (`IDE1.0-0.9.0-windows-x64.exe`) 与 `SHA256SUMS.txt`。
- **打包自动化解包门禁**：`build-msi.ps1` 构建完成后自动运行 `msiexec /a` 解包 `MSI` 并对解包出的 `IDE1.0.exe` 执行 `--smoke-test` 验证。
- **向导退出不被托盘拦截**：独立向导点击完成并退出时显式置位 `self.force_exit = true`，避免被系统托盘常驻模式拦截隐藏。

#### 2. 真实桌面实测与 Vercel 云端 MCP 全同步
- **桌面双形态实测通过**：`IDE1.0_0.9.0_x64.msi` 解包主程序与 `IDE1.0-0.9.0-windows-x64.exe` 均实测通过 `--smoke-test` 及真实 Win32 窗口渲染可见性验证（`IDE1.0` 主窗口正常创建并可见）。
- **Vercel 生产 MCP (`v0.9.0`)**：线上 `https://ide1d0.vercel.app/mcp` 同步更新至 `0.9.0` 并通过全套 13 项（含 REST 协调与容器重建重启持久化）真实云端 E2E 测试。

---

## 0.8.10 — 2026-11-22

### Vercel MCP 持久化事务存储、权限边界全面加固与 Rust 内核规范治理

本版本聚焦于多 AI 代理并发协同与云端 MCP 安全性全面强化：将 Vercel 云端 MCP 全量迁移至 Neon Serverless Postgres 事务存储，落地 P0-01..P0-05 安全不变量与 P1-01..P1-06 协同统一架构，完成双轴代码审查所有高风险鉴权与规范隐患治理，并同步发布最新 Vercel 生产部署。

#### 1. 云端 Durable Postgres 状态存储与防冲突提案 (`api/v1/_store.js`, `db/migrations/001_mcp_durable_state.sql`)
- **冲突绝不污染 HEAD (`P0-01`)**：重叠修改或语法校验失败时保持正式 HEAD、`revision` 与 `content_hash` 不变，保存独立 `PENDING_REVIEW` 提案并返回结构化冲突差异与预览，支持审核后按基线 CAS 提交。
- **单文件行锁 + 强 CAS + 作用域幂等 (`P0-02/P0-04/P0-05`)**：所有写操作在 `BEGIN ... SELECT ... FOR UPDATE ... COMMIT` 内校验 `(current_revision, content_hash)`、`(tenant_id, workspace_id, file_path, op_id)` 幂等键、`payload_hash` 一致性与单调 `fencing_token`；`openFile` 默认原子防覆盖。

#### 2. 全栈 Bearer JWT / 令牌鉴权与提案/文件级 ACL (`api/v1/_auth.js`, `api/v1/_http.js`, `api/v1/mcp.js`)
- **工作区 + 文件级细粒度 ACL (`P0-03`)**：支持 `files` / `allowed_files` 通配与精确文件权限控制，所有按文件 MCP/REST 工具严格校验目标工作区与文件路径。
- **提案读写闭环防护**：修复 `resolveProposal` 未校验目标工作区写入权限、无 `file_id` 时 `listProposals` 跨工作区返回以及无 `content-length` 时已解析对象 `req.body` 绕过 2 MiB 载荷限制的问题；抽取统一 `_http.js` 中间件直接读取权威迁移 SQL 消除 DDL 重复。

#### 3. 本地 Rust MCP 统一内核与安全规范加固 (`crates/ide-mcp`)
- **单一权威状态 (`DocumentStore`)**：统一 `McpServer` 与 `RustConsoleEvaluator` 共享 `Arc<DocumentStore>`，重构 `merge/` 模块目录并封装 `read_doc_revision`。
- **协议与授权一致**：`handle_tools_call` 按结果 `ok == false` 准确置位 `isError: true`；`force_overwrite` 强制要求 `confirm_force_overwrite=true`（及可选 `IDE_MCP_FORCE_TOKEN`）。
- **无窗进程安全、路径安全与写前备份 (`ADR-003`, `CODING-STANDARDS`)**：`handle_launch_browser` 与 `git_diff_workspace` 严格启用 Windows `CREATE_NO_WINDOW` 并通过环境变量+静态脚本消除命令串拼接；全量锁操作支持中毒恢复；磁盘读写实施路径安全校验及 `auto_backup::backup_before_save` 写前备份。

#### 4. 全量验证与真实云端 E2E 复核
- `editcore`、`ide-mcp`、`nema-codec`、`tests/mcp-vercel-unit.test.mjs` 及真实生产环境 `https://ide1d0.vercel.app/mcp` 全套 E2E 验证全通过。

---

## 0.8.9 — 2026-11-22

### 日志安全清理与美漫进度条、崩溃专属反馈、悬停文字反色彻底修复、停靠区折叠手柄重塑与次世代透明水晶主题统一

本版本聚焦于产品级用户交互与视觉质感全面升级：实装主窗口退出时超过 7 天历史日志的安全清理与美漫进度条模态，崩溃弹窗改用专属反馈问题按钮直达 GitHub Issues，彻底治理顶部下拉菜单与悬停文字动态反色缺陷，重塑停靠区内侧腰部折叠交互，并将透明玻璃与水晶主题深度合璧为次世代透明水晶主题。

#### 1. 退出时历史日志安全清理与美漫进度条 (`lifecycle.rs`)
- **7天过期日志安全清理**：点击关闭窗口退出应用时，后台自动扫描并清理超过 7 天的历史日志与崩溃转储，限制最大超时（3s）或完成即刻退出，绝不阻碍正常关机体验。
- **美漫风格退出模态与进度条**：主线程弹出带有黑曜钛金、赛璐珞高光与坚固外框的美漫进度条模态窗口，实时呈现清理文件进度与日志瘦身字节数。
- **UTF-8 安全截断**：采用 `truncate_log_filename` 字符边界安全截断，彻底消除切片越界与 Panic 隐患。

#### 2. 崩溃诊断专属「反馈问题」与 GitHub Issues 联动 (`diag_log.rs`, `build.rs`, `Cargo.toml`)
- **专属定制操作按钮**：启用 Windows ComCtl32 v6 视觉样式清单，将未捕获异常系统弹窗改用专属 `OkCancelCustom("反馈问题", "关闭")`。
- **GitHub Issues 一键直达**：用户点击「反馈问题」即时调用系统默认浏览器打开 GitHub Issues 页面，自动预填版本环境、错误详情与日志路径。
- **三语 I18N 深度支持 (`i18n.rs`)**：补齐反馈弹窗中文、英文、日文专属文本与错误分类提示，告别乱码与冷冰冰的 Yes/No 语义。

#### 3. 悬停文字动态反色与交互细节全面治理 (`menuui.rs`, `btnstyle.rs`)
- **悬停高对比度文字反色**：彻底解决顶部下拉菜单（分割窗口、全屏等按钮）悬停无视觉反馈、聊天顶部 Agent 下拉菜单悬停文字不反色的视觉缺陷。
- **背景感知 Alpha 合成**：引入 `composite_over` 背景感知计算，在半透明水晶、浅色和深色主题下动态推导悬停文字的高对比度反色。
- **恢复主题自然流转**：清理强制覆盖的 `override_text_color = None` 干扰，恢复主题自然流转。

#### 4. 停靠区折叠手柄重塑与聊天界面优化 (`docks.rs`, `titlebar.rs`)
- **内侧边缘腰部折叠把手**：左右与底部停靠区面板（文件树、AI聊天、Markdown预览、NEMA预览）统一采用腰部居中折叠把手；展开后折叠手柄自适应移动到展开窗口的内侧一边的边缘（紧邻代码编辑区）。
- **全员统筹调度默认展示与极简紧凑顶栏**：聊天界面默认进入「全员协调调度界面」，移除冗余的返回精准控制切换逻辑；模型与接口设置统一移至光波统筹右侧，改用精致齿轮图标 (`⚙`) 紧凑呈现。

#### 5. 次世代透明水晶主题深度统一 (`glass_render.rs`, `app.rs`)
- **双主题合璧**：将原透明玻璃 (`theme-clear`) 与透明水晶 (`theme-crystal`) 各取所长，深度融合成全新的水晶质感透明主题。
- **增强型字符级动态反色与多层光学棱镜焦散**：升级 `adapt_char_color` 字符着色器，保留色彩色相的同时在浅色与光晕背景下实现防眩光高对比度呈现；多层光学焦散背板渲染，集成双发光边缘与细微棱镜光泽。

#### 6. 插件沙箱错误防护与死代码清理 (`plugin_ui.rs`, `aichat.rs`)
- **插件错误反馈弹窗**：修复插件文档关系图点击缺失响应问题，新增插件宿主解析与沙箱错误专用模态弹窗，对禁用状态实施防护拦截与指引。
- **单代理死代码消除**：彻底清理旧版遗留的单代理聊天死代码及未使用的枚举分支，包体与运行时内存开销进一步降低。

#### 7. 全量单元测试、集成测试与启动烟雾门禁全绿
- `editcore`、`ide-mcp`、`nema-codec`、`ide-fast` (676 passed) 及 `--smoke-test` 启动烟雾门禁全量通过，无警告、无编译错误。

---

## 0.8.8 — 2026-11-21

### Myers 差异内核统一收敛、NEMA 架构解耦模块化、诊断日志锁自愈与代码规范全量治理

本版本聚焦于架构治理与健壮性加固：将双向行级 Myers 差异算法收敛至底层核心库 `editcore`，将 3800+ 行 NEMA 单体编解码器解耦拆分，为诊断日志系统引入锁中毒自愈与目录自愈能力，并对美漫三渲二弹窗界面实施数据泥块消除与全量短函数规范治理。

#### 1. Myers 差异比对算法下沉与全仓统一 (`crates/editcore/src/diff.rs`)
- **算法单一事实来源 (Single Source of Truth)**：将原分散在 `ide-mcp` 与 `ide-fast` 中的 Myers 双向行级 diff 算法完全收敛下沉至底层核心库 `editcore`。
- **上层零冗余复用**：`ide-mcp` 直接复用 `editcore::diff` 导出统一接口；`ide-fast` 统一通过 `editcore::diff::diff_lines` 执行文档差异计算，消除算法冗余与维护分叉。

#### 2. NEMA 编解码器架构模块化深度解耦 (`crates/nema-codec`)
- **高内聚职责分离**：将原本 3800+ 行的单体库解耦拆分为 4 个职责单一的子模块：
  - `export.rs`：专注于 Markdown 表格、JSON、YAML 导出。
  - `lease.rs`：租约仲裁与协作并发控制状态机。
  - `benchmarks.rs`：官方基准场景实测数据集及语法矩阵。
  - `tests.rs`：模块化单元测试套件。
- **模块可见性收敛**：在 `lib.rs` 中收敛子模块为私有 `mod`，并通过根级 `pub use *;` 导出，对外公共 API 保持 100% 向后兼容。

#### 3. 诊断日志系统锁中毒自愈与目录自愈 (`ide-fast/src/diag_log.rs`)
- **锁中毒自愈机制**：内存环形追踪缓冲区锁获取引入 `.unwrap_or_else(|e| e.into_inner())` 中毒自愈机制，彻底防止 panic hook 场景下因二次锁中毒导致进程异常中止。
- **目录运行时自愈**：新增 `ensure_log_dir()`，运行时日志目录若被异常移除或未创建，在写入崩溃日志、启动错误或导出诊断前自动重新校验并创建。
- **测试覆盖强化**：强化环形追踪缓冲区滚动容量与省略标记断言，确保启动追踪与最新追踪无损采集。

#### 4. 美漫三渲二弹窗界面重构与代码规范治理 (`updateui.rs`, `frame.rs`)
- **消除数据泥块与参数反模式**：引入 `UpdActions` 状态动作载荷，统一 `upd_badge()` 匹配逻辑，全面消除 `#[allow(clippy::too_many_arguments)]`。
- **短函数规范治理**：拆分 `paint_comic_shell_chrome`、`render_upd_primary_action`、`render_upd_hero_banner`、`render_upd_body_cards`、`render_upd_notes_box`，所有函数单体严格控制在 80 行以内，符合团队最高编码规范。

#### 5. 全量单元测试、集成测试与启动烟雾门禁全绿
- `editcore` (475 UT + parity suites)、`ide-mcp` (21 tests)、`nema-codec` (26/26 tests)、`ide-fast` (667 passed) 及 `--smoke-test` 启动烟雾门禁全量通过，无警告、无编译错误。

---

## 0.8.7 — 2026-11-21

### 多代理并发协同编辑引擎 OCC 高性能架构实装与实测验证

本版本针对 `ide-mcp` 多代理并发协同编辑场景实装了三项核心性能算法改善，并通过 Release 生产优化模式完成数学不变式验证与 300 字级端到端基准实测。

#### 1. OCC 文档锁细粒度解耦 (Arc\<RwLock\<HeadlessDoc>>)
- 将 `ConcurrentCoEditWorkspace` 内部文档映射从 `RwLock<HashMap<String, RwLock<HeadlessDoc>>>` 升级为 `Arc<RwLock<HeadlessDoc>>` 独立智能指针，彻底消除工作区级全局读锁在多代理高并发场景下的争用瓶颈。
- 新增 `get_doc_ref()` 零争用获取接口，所有读写操作直接作用于目标文档锁，完全绕过工作区级锁。

#### 2. 极速直提快径 (Direct Fast-Path)
- 当代理请求基于最新版本 (`base_revision == current_revision`) 且文档写锁空闲时，通过 `try_write()` 在 < 100 ns 内直接完成提交，无需进入三阶段 OCC 状态机。

#### 3. 自适应自旋退避 (Adaptive Spin-Loop Backoff)
- CAS 争用重试时采用 `std::hint::spin_loop()`（CPU `PAUSE` 指令）以指数级退避（4、8、16 周期），彻底消除 Windows 系统下 `thread::yield_now()` 触发的内核态线程休眠颠簸（1~15 ms 非必要调度时延）。

#### 4. 实测性能数据（Release 生产优化模式，300 字级内容，15 次提交）

| 指标 | 单代理（串行基准） | 3 代理并发协同 |
|------|------------------|--------------|
| 15 次提交总耗时 | 8.601 ms | 3.612 ms |
| 端到端提速比 | 1.00x | **2.38x** |
| 并发性能提升 | 基准 | **+138.08%** |
| 耗时节省 | — | **58.00%** |
| 3 线程并行效率 | 100%（基准） | **79.36%** |
| 数据完整性 | 100% | **100%**（rev 16） |
| OCC vs Mutex P95 延迟 | — | **P95 −36.2%** |
| NEMA 语义合并吞吐提升 | 基准 | **+20% ~ +75%** |

- 数学不变式：版本严格单调递增收敛至 rev 16，内容 100% 无损保留

#### 5. 新增三代理基准测试套件 (`tri_agent_bench.rs`)
- 通过 `McpServer::call_tool_concurrent()` 真实 MCP 接口，使用 `std::sync::Barrier` 消除线程启动偏差，对 300 字级内容实施端到端实测。
- 全部 28 项测试通过，`cargo clippy -p ide-mcp -- -D warnings` 零警告。

---

## 0.8.6 — 2026-11-20

### 核心 MCP 服务与跨供应商无头协同编辑、Vercel 云端部署、Blender-Style Rust 控制台、终端深度兼容与万级 Agent 调度性能优化

本版本推出面向 AI 代理协议的完整 **Model Context Protocol (MCP)** 服务体系（本地 `crates/ide-mcp` 与 Vercel 云端 `/mcp` 无头部署）、支持多供应商并发无头编辑与 3-Way 自动合并、在原生桌面版底栏实装 Blender `bpy` 风格专属 **Rust 控制台 (`ide::ops`)**，并完成终端 OSC 133 协议与十万级 Agent 协作调度深度优化。

#### 1. 核心 MCP 服务体系与跨供应商无头协同
- **多供应商并发无头协同编辑 (`headless_coedit`)**：支持 Claude、GPT-4o、DeepSeek、Gemini 等混合智能体在内存中并发编辑相同文件；具备原子 CAS 快径、基准版本快照回溯与 3-Way 行级自动合并，冲突时安全嵌入冲突标记与结构化诊断，通过 `op_id` 实现严格幂等。
- **Myers 文本差异比对引擎 (`diff_unified`, `diff_revisions`)**：实现 Myers 行级差异算法，支持折叠不变上下文并输出标准 Unified Diff 补丁。
- **Vercel 云端 Serverless MCP 部署**：在 `/mcp` 与 `/api/v1/mcp` 实现完整 JSON-RPC 2.0 规范，支持 GET 发现状态与 OPTIONS 跨域，无缝对接 Claude Desktop、Cursor、Antigravity 等任意 MCP 客户端。
- **本地 Stdio MCP 运行时 (`crates/ide-mcp`)**：基于标准 stdio 传输提供完整 MCP 2024-11-05 协议支持，内置 18 项全覆盖单元测试。

#### 2. 原生界面底栏 Blender `bpy` 风格 Rust 控制台
- **`[RUST CONSOLE]` 专属底栏页签**：在原生 `ide-fast` 底层面板中紧邻 `[TERMINAL]` 实装全新 Rust 控制台，支持直接以 Rust 语法操作当前编辑器。
- **三元自动化设计**：提供 `ide::context::*`（状态/主题/活跃文件）、`ide::data::*`（内存文档列表/正文/历史）与 `ide::ops::*`（打开/编辑/替换/双文档比对/协同扩容）。
- **交互控制台 UI**：支持预设宏动作工具栏、彩色语法分级日志回显与交互式 `rust>` 命令行输入。

#### 3. 终端子系统与多 Agent 调度深度调优
- **OSC 133 Shell 标记协议分块安全解析**：重构 `LineBuffer`，消除跨块分段时未闭合 OSC 133 转义序列导致的乱码。
- **PowerShell 流式执行与交互安全**：采用 `-ExecutionPolicy Bypass -Command -` 流式通道，严格将 `Ctrl+C` 信号约束在终端获得焦点时生效，杜绝误截获。
- **十万级 Agent 调度并发控制**：引入 `filter_agents_limited` 快速通道，将并发调度上限收敛至 16，模型隔离绑定至 `model_override`。
- **安装向导安全增强**：安装器子进程采用纯 Rust 读取并修改用户 PATH，彻底杜绝 Shell 注入漏洞。

#### 1. 用户模型配置档案隔离与持久化
- **纯净模型列表**：`configured_models()` 仅展示用户实际配置/保存过的模型档案；彻底杜绝未配置的硬编码占位模型或虚假选项，未配置时呈现清爽引导。
- **多模型档案管理 (`ModelProfile`)**：支持用户为不同接口/模型起名并保存为独立档案，支持即时切换与按模型删除，密钥脱敏不泄漏。
- **配置跨重启持久化**：面板开闭状态、工作模式、上下文开关、自动应用、Agent 编队数量、当前激活 Agent 及每个 Agent 的专属模型绑定均随 `settings.json` / `ai.json` 自动落盘与恢复。

#### 2. 智能助手与多 Agent 协同平台合流 (右侧统一工作台)
- **单 Agent 完整赋能**：原智能助手全部对话与内联编辑能力（上下文装配、Diff/Patch 应用、MCP 工具、Persona 规则）直接赋予单 Agent。
- **可搜索精准控制下拉菜单**：右侧面板顶部引入纯矢量下拉选择器，支持实时按名称与职责搜索，支持一键在「全员协同调度」与任意单 Agent 之间精准切换。
- **会话与草稿彻底隔离 (`AgentSession`)**：每个 Agent 维护独立对话流、输入草稿、执行状态与独立模型覆盖配置，切换无干扰。
- **移除右上角冗余入口**：移除标题栏重复的多 Agent 协同工作台按钮，统一由右侧展开抽屉承载。

#### 3. 漫反射三维水晶维护/修复/卸载向导与无头启动不变量
- **安装维护/修复/卸载向导**：已安装环境下执行向导自动切换为维护模式，支持就地修复快捷方式/环境变量以及安全无残留卸载。
- **纯矢量漫反射 3D 水晶徽标**：严格对应正版 IDE1.0 水晶切割双棱锥与核心 `< I >` 符号，光标悬停时由确定性哈希驱动微幅缓动扰动，全流程杜绝空心方块或残缺图标。
- **全流程无头启动 (`CREATE_NO_WINDOW`)**：安装向导、后台常驻关闭与托盘操作统一采用无头子系统调用，彻底杜绝 Windows 终端弹窗闪烁。

#### 4. 多主题精致按钮与微质感交互升级
- **全主题自适应按钮体系 (`btnstyle.rs`)**：
  - **美漫风格 (Comic Cel-Shaded)**：增加 1.0px 内边缘高光墨线 (`Color32::from_white_alpha(110)`) 与纯黑硬阴影平滑位移，保留粗犷赛璐珞质感的同时增加印刷级精致度。
  - **16-Bit 街机复古 (Retro 16-Bit Arcade)**：重塑立体浮雕双色光影斜面，增加按压物理凹陷位移与霓虹青/金边框光晕。
  - **玻璃拟物水晶 (Crystal / Clear Glassmorphism)**：引入胶囊微圆角、1.0px 棱镜高光微光反射 (`from_white_alpha(130)`) 与悬停粉色色散微光，呈现通透光学晶石质感。
  - **现代原生 (Fluent Modern Dark / Light)**：增加 1.0px 顶部镜面反光线与微妙微轮廓边框，重构 Primary、Danger、Settings 与 Skill 胶囊按钮为具深度感的高级原生控件。
- **全场景下拉与图标按钮精致化**：将 Agent 切换与模型选择下拉组件升级为深度契合各个主题风格的纯矢量 `dropdown_button`；为 `icon_button` 与 `icon_only_button` 全面引入触感微反光、1px 精细边框与平滑悬停过渡；在抽屉与弹出菜单中全量应用主题交互样式 (`apply_theme_button_style`)。

#### 5. 软件工程学规范与门禁体系
- **静态分析与代码格式**：消除全部 23 处 Clippy 警告（达到 `--all-targets -D warnings` 零警告标准），统一全工作区 Rustfmt 格式。
- **统一质量门禁脚本 (`scripts/verify.ps1`)**：一键自动化串联代码格式检查、Clippy 零警告、全套单元/集成测试、Release 编译尺寸守卫及数学化性能基准测试。
- **架构决策记录 (ADR)**：建立 `ADR-001`（模型档案隔离）、`ADR-002`（Agent 平台深度合流）、`ADR-003`（无头执行不变量）架构文档。

## 0.8.5 — 2026-11-18

### 启动与内核全面性能优化、IDE1.0 品牌与环境变量统一、数学化性能测试框架升级与 10 项对抗式审核加固

本版本聚焦启动链路零冗余 I/O、核心文本操作时间复杂度降阶、全产品命名向 `IDE1.0` 对齐，并依据对抗式审核结果系统性修复了增量缓存不变量、并发搜索复杂度、系统亮度读取开销、Minimap/语法高亮堆分配以及性能统计学与 E2E 测试报告严谨性。

#### 1. 启动与交互性能优化
- **零阻塞冷启动**：延迟启动非首屏字体、图标与系统字体枚举；`apply_settings` 启动路径消除无谓磁盘写入，新增 `theme_tc` 主题缓存与无落盘切换路径 `set_theme_plugin_no_save`。
- **文档与文本内核提速**：`editcore` 引入单遍 SIMD `normalize_to_lf` / `normalize_to_lf_owned`、`detect_charset` 0x1B 提前短路、小选区上下文按需构建 (`rows.len() > 16`)；`Doc` 撤销栈改用 `VecDeque<Undo>` O(1) 环形淘汰、`rows_for_mode` 栈区快路径与单行 `insert-text` / `Doc::splice` 局部行比较。
- **增量 Diff 与渲染缓存**：`Doc::changed_lines` 新增 `line_aligned` 不变量守卫下的等长 O(N) 快路径，在插入/删行导致行对齐失效后自动回退严格 LCS 差分并恢复精度；Minimap 行宽计算改为 256 字符 + 2048 字节探测的零堆分配扫描；`TypeConfig::highlight_line` 新增全 ASCII 零堆分配快路径及整行注释前置检查与 SIMD `str::find`。

#### 2. `IDE1.0` 统一命名与无痛兼容
- 安装包（Setup/MSI/Portable）、版本资源（`IDE1.0.exe`）、打印页眉、标签跳转提示、测试矩阵及文档全面更名为 `IDE1.0`。
- 新增 `IDE10_TRACE` / `IDE10_SETTINGS` / `IDE10_BACKDROP` / `IDE10_MULTI` / `IDE10_NO_NETWORK` / `IDE10_OFFLINE` 与 `"IDE1.0-OPEN"` IPC 协议，同时完全兼容旧 `IDE_FAST_*` 与 `"IDE-FAST-OPEN"`；更新临时文件前缀为 `"IDE1.0-update"`。

#### 3. 数学化性能测试框架升级与对抗式审核修复
- **真实统计学度量**：`perf_bench.rs` 与 `tests/perf-math-bench.ps1` 引入精确双侧 95% Student's $t$ 置信区间（下界非负截断）、Tukey 1.5×IQR 离群值计数与去极值均值、$N < 5$ 样本分位点置空保护、RAII 临时目录自动清理、1MB 单行搜索 SLA 门禁 (`<= 15 ms`)、4 尺度 `[5k, 20k, 80k, 200k]` OLS 对数-对数复杂度拟合（含字形图集预热与每尺度唯一指标名），以及 `-Compare` 基线对比回归门禁（`>+30%` 且 `>+1.0 ms` 即拦截）。
- **并发、终端与系统集成加固**：`search_ops` 消除单行多命中场景的 $O(H \cdot L)$ 退化（ASCII/UTF-8 增量光标 $O(L + H)$）；`terminal` 移除 `\\.\PHYSICALDRIVE0` 提权探测并引入 `output_rev` + 内容哈希缓存键；`theme_luminance` 增加 2s TTL 缓存，并在窗口重新聚焦与主题切换时调用 `invalidate_cache()`；`e2e-run-all.ps1` 严格校验 `$LASTEXITCODE` 与失败用例数并禁止 `0` 套件静默通过。

## 0.8.4 — 2026-11-18

### 便笺体验全面升级、大选取复制零丢失、Agent专属便笺工具与主题按钮定制、托盘退出强制置顶与IDE1.0命名对齐

本版本重点推进便笺系统工业级体验完善与稳定性加固，根治跨编辑器大选取复制数据丢失，重塑 Agent 操作界面美学与便笺工具调度，完善托盘退出未保存提示置顶逻辑，统一构建部署程序命名为 IDE1.0.exe，并彻底消除旧式安装向导原生 UI 残留。

#### 1. 安装向导排版充盈与响应式微交互重塑
- **880×600 充盈版面**：彻底消除右下角大面积空旷，卡片高度由 38px 增至 58px，操作栏高度 46px 铺满底部。
- **精简欢迎页 3 步直达**：去掉冗余欢迎页，直切核心配置流程（1. 环境配置 -> 2. 极速部署 -> 3. 部署完成）。
- **空间感知微交互与阻尼旋转**：移除拖拽复位 HUD 胶囊；光标移动或悬停按钮时，由坐标与时间哈希驱动生成平缓、微随机的 3D 水晶宝石旋转冲量，平滑高阻尼过渡。
- **字体加粗与对比度强化**：白金（#cbd5e1）与纯白字符，色彩心理学宝蓝/翡翠绿/紫罗兰分级，识别度显著提升。

#### 2. 首启静默原则与用户偏好持久化
- **便笺首启不自动弹窗**：首次启动或无配置状态下默认保持关闭（`open: false`），由用户按需主动唤起。
- **左侧导航栏默认关闭**：首次启动默认处于折叠关闭状态（`side_open: false`），给予编辑区最大开阔视野。
- **用户状态原子持久化**：用户对面板折叠、便笺开关的任何操作均触发即刻原子落盘，重启后严格记忆并还原。

#### 3. 便笺格式化目标即选区引擎
- **选区即为目标**：删除线（`~~`）、粗体（`**`）、斜体（`*`）、下划线（`<u>`）在编辑模式下以高亮选中文本为目标直接包裹，支持再次点击就地 Toggle Off 解包。
- **未选区光标就地插入**：光标处就地插入模板，避免简单粗暴在便笺全文末尾追加。
- **富文本预览模式实时反映**：在 Markdown 预览模式下以聚焦行或块为目标即时格式化并更新预览，彻底消除在便笺末尾追加 `~~删除线~~` 的缺陷。

#### 4. 便笺系统体验加固
- **默认视口宽度拓展至 440px**：最小宽度提升至 400px，底部 12 项富文本格式按钮、字数统计与缩放手柄完整排布无截断。
- **删除所有便笺保留颜色记忆**：`preferred_color_idx` 持久化，重新建便笺保留用户喜爱的主题调色板。
- **杜绝拖拽后窗口自我抖动**：消除每帧冗余 `with_position` 发送，仅首次加载初始化位置。

#### 2. 编辑器多行文本复制/剪切数据完整性修复
- **大选取复制无损完整**：拆分行折叠端点映射与完整编辑选取映射 `rows_for_mode(..., full_selection)`，彻底杜绝 >8 行选取复制时中间行被空行覆盖的缺陷。
- **CRLF 换行与空字符安全规范化**：跨窗口、跨便笺粘贴文本 100% 还原。

#### 3. Agent 线程守护与便笺工具体系
- **Panic 熔断守护**：`transport.rs` 全链路包裹 `std::panic::catch_unwind`，防后台任务或请求异常导致线程崩溃。
- **5 项便笺调度工具集成**：支持 Agent 主动查询、创建、更新、删除便笺及唤起独立视口窗口。

#### 4. Agent 命令按钮多主题深度设计重塑
- **Comic**：波普波斯黄/电光青贴纸徽章，2.0px 纯黑墨线描边 + 3.0px 硬阴影。
- **Retro16**：街机怀旧立体倒角像素按钮，高亮与暗部双色阶边框。
- **Crystal**：半透明毛玻璃胶囊，青色棱镜边缘微光。
- **Fluent**：平滑精致现代微光圆角。

#### 5. 托盘退出未保存提示强制自动置顶与构建正名
- **托盘退出强制置顶**：退出触发时主窗口解除最小化并激活置顶，弹窗采用 Windows `0x40023` 自动置顶显示。
- **程序名对齐为 IDE1.0.exe**：构建生成的发布物及快捷方式对齐为主程序 `IDE1.0.exe`。
- **0 原生 UI 残留安装向导**：彻底告别 WixUI 原生灰框与按钮，全自绘无边框真 3D 赛璐珞向导。


## 0.6.2 — 2026-11-08

### 美漫 3渲2 卡通渲染全端默认主题、WiX 安装包波普界面与 Sakura 🌸 常驻托盘

本版本正式将全端默认主题统一为美漫 3渲2 (	heme-comic / comic)，注入 3D 挤出徽章与赛璐珞风格，全面升级 Windows WiX 安装包、原生 PE 多分辨率图标与右下角任务栏系统托盘。

#### 1. 全端默认美漫 3渲2 主题 (3D-to-2D Cel-Shaded Comic)
- **视觉风格**: 波普饱和黄 (#ffcc00) 状态栏、赛璐珞霓虹粉红 (#ff3366) 关键字高亮、青蓝 (#00e5ff) 行号高亮与粗描边。
- **极端对比度抗眩光不变量**: 状态栏采用感知亮度对数模型，当底色亮度 >= 0.5 时文字强制采用 #000000 纯黑，杜绝黄白眩光与视力障碍。
- **全端同步**: Web 端 (editor.html)、Desktop 壳 (index.html) 与极速原生版 (ide-fast) 首次启动与重置默认一致生效。

#### 2. WiX MSI 安装包 3渲2 视觉重塑与 Windows PE 图标注入
- **安装向导美漫图元**: 顶部横幅 Banner (493x58) 与向导背景 Dialog (493x312) 注入网点波普艺术与 3D 挤出立体徽章。
- **多分辨率 PE 图标**: 内嵌 16x16、32x32、48x48、64x64、128x128、256x256 六帧清晰图标，开始菜单与高分屏任务栏告别模糊。

#### 3. Windows 任务栏系统托盘 Sakura 🌸 常驻
- **常驻系统托盘**: 运行后常驻 Windows 屏幕右下角时间旁边，支持左键单击一键最小化/还原置顶、右键菜单快捷唤起与退出。
- **状态栏联动与多语言**: 状态栏最右侧呈现 🌸 Sakura 状态指示器与交互提示，中/日/英三语字典完整覆盖。

#### 4. NEMA 语义解析对抗调优与红绿测试
- **续接行防误判**: 严格区分单空格领域头 ( \1 1) 与 5W1H 自然语言续接行，阻断虚假领域分裂。
- **完整测试矩阵**: 覆盖 64 态组合、调色板对比度不变量、安装包图元规格校验，测试集全绿，Clippy 零警告。

## 0.6.1 — 2026-11-07

### 自适应解释器导入引擎、.NEMA 全语法变化矩阵与 64 态组合验证器

本版本正式引入面向 .nema 活跃调试阶段的用户自定义解释器导入与自适应适配引擎，支持外部 Python 脚本、本地二进制与 CLI 驱动的即插即用，并全面落地《.NEMA 全语法变化矩阵 · 七列完整审查版》交互式看板与 4³ = 64 态穷举测试套件。

#### 1. 自定义解释器导入与自适应适配引擎 (Adaptive Interpreter Engine)
- **多源解释器接入与自动发现**: 自动检测并枚举可用候选（官方 Python 参考实现 `codec/native.py`、`codec/cli.py`、工作区 `.ide/nema_interpreter.py` 等），支持用户自定义文件路径与运行环境（Python / Binary / Node.js / Auto）。
- **动态能力探针 (Adaptive Probing)**: 自动探测 5 种底层调用协议（`PythonNativeView`、`OfficialCliRender`、`GenericJsonAstStdin`、`GenericFileArg`、`BuiltinRust`），实时评测往返延迟与特性兼容性。
- **全链路后端 API 与平滑降级**: 提供 `/api/nema/status`、`/api/nema/probe`、`/api/nema/set_interpreter`、`/api/nema/render` 接口；遇外部解释器语法或进程异常自动安全回退至内置纯 Rust 标准引擎。
- **工具条解释器管理弹窗**: NEMA 工具栏新增 `[🔌 解释器]` 胶囊，点击即可一键管理、实时探测与热切换生效。

#### 2. .NEMA 全语法变化矩阵 · 七列完整审查版交互看板
- **权威规范基线落盘**: 在 `docs/06_功能对标/NEMA_SYNTAX_MATRIX_7COL.md` 建立固定 7 列架构基线（`语法 | 说明 | 具体例子 | 设计理由 | 避免的异常 | 变化写法 | 变化渲染结果`）。
- **交互式 64 态组合验证器 (4³ = 64 Basic Combinations)**: 针对固定三列表格的 4 种基本状态（Value `A` / Null `~` / Empty `\e` / Missing `\_`），支持自由切换状态、实时生成变化写法代码，并由当前解释器实时渲染解析表格。
- **分类筛选与实时检索**: 覆盖 Table、5W1H、Node–Edge、转义字面、NEMAT 传输、全局词法等核心规则，支持一键点击变化写法试算与插入编辑器。

#### 3. 64 态全排列自动化测试套件
- **穷举测试全绿**: 新增 `crates/nema-codec/tests/matrix_64_combinations_tests.rs`，100% 验证 64 种三列字段状态组合在原生引擎与自适应渲染中的确定性语义。
- **边界与错误注入**: 严格校验零编号拒绝、超列拦截、畸形边解析与悬空端点引用。

## 0.6.0 — 2026-11-07

### IDE1.0 × NEMA 交互图拓扑渲染器、基准实验室与新解释器规范对齐

本版本为 IDE1.0 重大里程碑版本，全面对齐 .NEMA 官方参考实现与最新解释器演进规范，引入交互式拓扑图渲染引擎、实时 Benchmark Lab 监控中心，并完成全工作区严格的静态安全自审与 Clippy 零警告加固。

#### 1. .NEMA 官方规范与交互式拓扑渲染器 (Graph Canvas)
- **拓扑图渲染 (Graph Domain Visualizer)**: 深度对齐 .NEMA 官方规范（v1.2 Release R7 及最新解释器），实时解析 ` >G` 领域中的节点 (`n id label [props]`) 与边 (`e src dst [type]`)，支持双向交互、节点选中高亮、边类型标识与动态布局。
- **实时 Benchmark Lab**: 内置与官方基准测试实验室同构的实时分析器，精准测算 .NEMA 与 JSON/YAML/XML 的体积压缩比（Token 节约率高达 70%+）、AST 解析延迟与内存分配开销。
- **三端统一交付**: 网页端 (ide-shell-web)、Tauri 桌面端 (ide-shell-desktop) 与极速桌面端 (ide-fast) 体验保持一致，工具栏底栏一键展开。

#### 2. .NEMA 新解释器规范演进与兼容性支持
- **单空格领域头语法兼容**: 紧跟 `E:\.NEMA\website\download.html` 及官方核心提交，兼容支持单空格前缀领域头（` ?`、` `、` >`）与隐式/显式 Table 结构。
- **全角字符严格拒绝**: 遵循安全与格式一致性原则，严格拒绝 U+3000 全角空格。
- **往返无损保真 (Roundtrip 100%)**: 官方测试用例（`test_v15.py`）及 60+ 官方示例往返还原度 100%。

#### 3. 严格安全自审与代码质量加固 (Clippy Clean & Safe Rust)
- **Clippy 全量零警告**: 全工作区（All targets, tests, benches）在 `-- -D warnings` 严苛模式下 0 警告退出。
- **内存安全保证**: 核心 crate 严格禁止 unsafe 代码（`#![forbid(unsafe_code)]`），确保内存安全与并发无数据竞争。
- **全量自动化测试套件通过**: 1227+ 项单元测试、集成测试、基准测试与并发测试 100% 通过（ide-fast 518 项、editcore 610 项、ide-shell-web 99 项全绿灯）。

#### 4. 高性能二进制交付
- **发布产物**: `dist-fast/ide-fast.exe`（Release 构建优化，体积 18.53 MB）。
- **完整性校验**: SHA256 哈希值 `7A04ADE84EAEEA003CD6FE6435E634BC2E487062DC7EC8F6E44AB69A65A0344D`。

## 0.3.2 — 2026-11-06

### IDE1.0 × NEMA v1.5 多智能体协同开发平台与流水线加固
本版本正式发布面向生产级的 .nema 原生多智能体协同开发平台、NEMA v1.5 无损编解码内核与红绿测试加固，并合并全部 5 项 GitHub Actions 依赖安全升级 PR（#23..#27），全面覆盖网页版 (ide-shell-web)、Tauri 桌面版 (ide-shell-desktop) 与高速版 (ide-fast)。

#### 1. 多智能体四阶段协同流水线 (crates/ide-agent-router)
- **① 路由拆解 (RouterEngine)**: 输入形态智能分类（功能开发/缺陷修复/优化/重构），拓扑 DAG 自动规划，划分 Group A（同文件共编）与 Group B（独立文件并行）。
- **② 并行协作 (Co-editing .nema)**: 细粒度 GuidanceZone 契约与单调递增 encing_token 防脏写，极简 Token 传输开销。
- **③ 汇总统筹与代码转换 (Aggregator & NemaToRustLowering)**:
  - **跨分支字典归一化与 ^N 索引重排**: 统一重排各分支独立词典并建立映射表，自动更新 AST 引用，消除语义漂移。
  - **三路语义合并 (NemaSemanticMerger)**: 领域与记录级结构化合并，严格保证精确 3 个物理 LF（对应 2 个空物理行）的 NEMA 终结不变量。
  - **结构化代码生成与防漂移守护 (NemaRustDriftGuard)**: 将 .nema 降解为标准 Rust 代码，结合 sanitize_rust_ident 自动清洗非法字符与数字前缀，双向关联 SourceMap。
- **④ Git 分支管理与闭环验证 (GitBranchOrchestrator & VerifyPipeline)**:
  - 文件级 Git 分支生命周期管理与原子 PR 合并。
  - 4 重严苛门禁验证（构建、单测、质量、规范），通关签发 AcceptanceCertificate；失败自动逆向生成工单回流；断路器（CircuitBreak）防止同指纹错误死循环。

#### 2. 智能体独立模型与思考配置 (Per-Agent Profiles)
- 7 大智能体槽位（Router、Agent A1..B2、Aggregator、Verify）均支持独立模型、推理强度（Low/Medium/High/Max）、思考模式（Adaptive/Extended/Off）与 Token 预算配置，前端面板实时修改并自动持久化。

#### 3. 三端全能力覆盖与高速版零开销启动
- 网页版与 Tauri 桌面端协同看板 100% 字节级一致。
- 高速版 (ide-fast) 内置纯 Rust 离线四阶段流水线调度（Action::RunCollabPipeline）与中/日/英三语国际化。
- **⚡ 严格零启动开销保证**: 未配置外部 AI 时冷启动绝不读取 i.json、不初始化网络运行时、不派生后台线程。

#### 4. GitHub Actions CI/CD 安全加固 (PR #23 ~ #27)
- ctions/checkout 升级至 7
- ctions/setup-node 升级至 7
- ctions/setup-python 升级至 7
- ctions/upload-artifact 升级至 7
- ctions/download-artifact 升级至 8

## 0.4.0 — 2026-11-05

### IDE1.0 Web 2.0 — 语义路由与多 Agent 协作内核 (Semantic Agent Router & Worker)
本版本在保留原有 Tauri 桌面端、Rust 编辑核心与自包含 Web Workbench 的基础上，正式推出面向大规模 AI Agent 并发协作的纯 Rust 语义路由与一致性协调器 (`crates/ide-agent-router`)，并完成 Vercel 云端部署与多语言 Agent SDK。

#### 1. 核心架构与日本 IPA 要件全覆盖
- **三平面拓扑 (Control Plane + Semantic Task Plane + Execution Plane)**:
  - 控制面: 鉴权、租户路由、任务状态机调度、排队与死信恢复。
  - 语义任务面: 基于 AST 的语义单元抽取、多语言符号定位 (Rust, Python, TS, JS, C, PowerShell)、依赖 DAG。
  - 执行面: 隔离沙盒环境并行执行、仅交付结构化 Patch、不直接覆盖权威源文件。
- **全套日本 IPA 标准研发工件落盘**:
  - `docs/00_要件基线/Web-Agent-Router-Requirements.md`: 8 项核心不变量 (NRP) 与 60 项要件追溯基线。
  - `docs/01_基本设计/Web-Agent-Router-Basic-Design.md`: 整体系统架构、路由与调度拓扑、故障域与灾备。
  - `docs/02_详细设计/Semantic-Units-and-Commit-Protocol.md`: 稳定复合指纹、AST 锚点重定位算法、并发冲突决策矩阵。
  - `docs/02_详细设计/Router-Worker-API.openapi.yaml`: OpenAPI 3.1 规格。
  - `docs/02_详细设计/Postgres-Schema-and-State-Machines.md`: 16 张生产表 DDL、原子 CAS 提交事务与 Transactional Outbox 模板。
  - `docs/03_测试资料/UT-IT-ST-UAT-Web-Agent-Router.xlsx` 与 `.md`: Excel 全量追溯矩阵与测试计划。
  - `docs/04_评审与管理/WBS-Web-Agent-Router.md`: 4 阶段 18 项 WBS 追踪与门禁审查。
  - `docs/05_实装与运维/Vercel-Deployment-and-Recovery.md`: Worker 故障自愈、租约过期重调度与幂等控制。

#### 2. Rust 语义核心与一致性协调器 (`crates/ide-agent-router`)
- **稳定语义指纹与锚点定位**: 基于语法节点特征计算复合哈希，有效防止代码编辑时的行号漂移；同函数被并发修改时精准检出并触发 `SemanticWriteConflict` (HTTP 409)。
- **乐观并发与 CAS 提交协议**: 每个文件维护权威单调自增 `revision`，以 CAS (Compare-And-Swap) 串行化同一文件的最终提交决策。
- **租约与单调 Fencing Token**: 签发 60s 有限期租约并派发全局单调递增 `fencing_token`；配合心跳续约与过期拦截，彻底消除僵尸 Worker 脏写风险。
- **Transactional Outbox 机制**: Patch 提交与领域事件生成在同一原子事务中落盘，解耦异步 Git 投影与外部通知。
- **严苛并发与规模验证**: `tests/concurrency_tests.rs` 7/7 项全绿 (含 100 个并发补丁无损合并、1000 任务批量入队排队分发、指纹漂移防御、重复幂等拦截)。

#### 3. Vercel Serverless 网关与多语言 SDK
- **云端部署与 API 网关**:
  - 生产接入: `https://ide1-0.vercel.app` (临时验证 URL: `https://fleet-acacia-7gzmn0a.tmp.now`)
  - 覆盖 `/api/v1/jobs/dispatch`、`/api/v1/tasks/claim`、`/api/v1/patches/commit`、`/api/v1/events`、`/api/v1/health`。
- **多语言 Agent SDK 与协同示例**:
  - `sdk/python/ide_agent_sdk.py`: 生产级 Python 异步/同步客户端。
  - `sdk/typescript/ide_agent_sdk.ts`: 生产级 TypeScript 客户端。
  - `sdk/examples/multi_agent_collab_demo.py`: 多 Agent 并行领取任务、提交补丁与冲突自愈实跑用例。

### 工程 (WBS-03 O1.1 / O1.7)
- **CI 工作流修复**: `.github/workflows/ci.yml` 之前没有顶层 `jobs:` 键、触发分支写成 `develop`, GitHub 判定无效, 从没真正运行过。现在结构有效、触发分支改为 `dev`; 只启用 `cargo test -p editcore` (三个平台), 其余 job 和 fmt / clippy / `-D warnings` 暂时关闭并注明对应 WBS 编号, 待逐项验证后再打开。
- **`editcore` 测试修复**: `grep_engine::tests::replace_backup_and_skip_list` 在 Linux 上失败, 根因是测试假定 Windows 的路径比较不分大小写; 代码行为是对的, 改测试按平台选文件名。editcore 现在 Linux 上 0 失败。

## 0.3.9 — 2026-11-04

### 高速版: 中 / 日 / 英三语补全
高速版早就有「设置 > 界面语言」(中文 / 日本語 / English, 选择会保存), 但最近几批新增的命令和提示只有中文。这一版把剩下的补齐:
- **命令名与界面文字** (`i18n.rs` 的 `CMD_T`, 新增约 170 行): 第三至七批加的全部命令 (标签分组、SPACE 缩进、代码补全、转到定义、增量查找、标签跳转、列块、Base64 / uudecode、复制为 HTML、剪贴板历史、插入日期 / 时间 / 戳记 / 控制字符、折行方式、20 个「运行宏 N」、窗口一览、关键字帮助、最近的文件…)、小工具窗口 (剪贴板历史 / 控制字符 / 戳记 / 折行列数 / 列块替换)、页面设置与打印预览、Markdown 工具条的悬停提示、面板折叠提示、标题栏按钮、新建 / 重命名文件的对话框标题等。
- **状态栏提示与错误信息** (`msgtr.rs` 的 `MSG_T`, 新增约 150 行): 标签分组、备份、补全 / 定义 / 函数跳转、列块、宏登记与宏运行的各种提示 (含「第 N 行 …」格式的预检错误)、插件与 AI 接口的错误、Base64 / uuencode 解码错误、文件属性对话框的整段文字、Grep 置换确认框、Markdown 预览的占位文字与提示块标题。文档名「无标题 N」也随语言显示。
- **内置手册三语**: 帮助手册 (F1) 和关键字帮助检索跟着界面语言走 —— `help.txt` (中) / `help_ja.txt` / `help_en.txt` 三份正文, 「命令一览」的标题和表头也翻译。
- **防漏翻的测试** (以后新增文字忘了补翻译会在这里变红): 命令表里每个标签在英文界面不含汉字; 源码里每个 `trz("…")` / `app_label(…, "…")` 都在翻译表里; 源码里每条直接写进 `tip` 的中文提示都有 `msgtr` 模板; 三份手册章节号和行数一致。
- **小改动**: `SPACE 缩进 / 反缩进 N 行` 的提示拆成可翻译的两段 (之前把「反」拼进句子里, 译不了)。
- **没改**: 发给 AI 的系统提示词 (面向模型, 保持中文); Sakura 风格的状态栏「挿入 / 上書」和 Grep 结果文档里的日文 (沿用 Sakura 原样, 三种界面语言下都是日文); 终端里的 shell 提示; 用户自己写的宏名 / 文件名。
- **验证**: ide-fast 单测 491 通过 (新增 4 项; 失败的 3 项仍是写死 `C:\` / `D:\` 的 Windows 路径断言, 改动前就有)。**没在真窗口里验证**: 切到日 / 英后各菜单、对话框的实际排版 (日文 / 英文比中文长, 窄窗口下可能被截断)。

## 0.3.8 — 2026-11-04

### 高速版移植补齐 (Tauri → 高速版) 第七批: 窗口一览、关键字帮助、宏里剩下能做的函数
- **通用选择列表** (`pick.rs`): 过滤框 + 方向键 / 回车 / Esc / 点击的小窗口, 命令面板同款手感; 过滤有两种 —— 模糊匹配 (分数高的在前) 和「每个词都出现」(保持原顺序, 帮助检索用)。下面几项都建在它上面, 以后再要「从一列里选一个」不用重写。
- **窗口一览** (窗口 > 窗口一览… / 命令面板 `app.window-list`, 同 Tauri `window-list`): 列出全部标签, ▶ 是当前、● 是有未保存修改, 右侧灰字是路径; 输入文字过滤, 回车切过去。
- **关键字帮助** (帮助菜单 / 命令面板, 同 Tauri `wiki` / `help-search` / `ext-help` / `toggle-key-search`, 默认键也同 Tauri):
  - 帮助手册 (F1): 开成只读标签「帮助手册.txt」, 已开着就刷新并切过去。手册 = `help.txt` 的正文 + 「命令一览」(从命令表现场生成, 用的是此刻的标签和键位, 不会和实际脱节)。
  - 关键字帮助检索… (Alt+Shift+F1): 在手册里查词, 每个空格分开的词都要出现 (不分大小写); 选中一行就跳到手册里那一行。
  - 关键字帮助: 光标处的词 (Ctrl+F1): 带着光标下的词直接打开检索 (含中日文字); 光标处没有单词就只在状态栏提示。
  - 关键字帮助自动显示 (帮助菜单里的勾选项, 设置里记 `key_search`): 光标换到新的单词上时, 状态栏提示显示手册里第一处提到它的行; 词至少 2 个字符 (一个字母几乎每行都命中, 提示没有意义); 先查正文, 正文里没有再查命令一览。
  - 和 Tauri 一样只查内置文本, 没有外部 `.hlp` / `.chm` 帮助文件。
- **宏里 15 个原本「不执行」的 `S_` 函数改成真执行**: `S_WinList` / `S_DlgWinList` (窗口一览)、`S_HelpContents` (手册)、`S_ExtHelp1` / `S_ExtHtmlHelp` (光标处的词)、`S_HelpSearch`、`S_ToggleKeySearch`、`S_FileUsedRecently` (最近的文件)、`S_Favorite` (收藏夹 = 最近的文件 + 最近的文件夹)、`S_InsFileUsedRecently` / `S_InsFolderUsedRecently` (插入最近的路径)、`S_ChgCharset` (指定字符集)、`S_FileReopen` (指定字符集重新打开)、`S_Outline` / `S_FileTree` (侧栏切到大纲 / 文件页)。268 个函数现在是 **142 个在文档上执行 / 118 个转交 App / 8 个明确不执行** (层叠、两种平铺、两种 SQL*Plus、JS 宏、IME 再変換、标签栏开关 `S_ShowTab`, 都写着原因)。
  - 注意: 这几个函数在宏里只是**打开**列表 / 切页, 宏不会停下来等人选, 后面的步骤接着跑 (和已有的 `S_GrepDialog` 一样)。
- **新命令** (都在命令面板里, 能绑键): `app.window-list` / `app.pick-recent` / `app.pick-folders` / `app.pick-favorite` / `app.ins-recent-file` / `app.ins-recent-dir` / `app.pick-charset` / `app.pick-reopen` / `app.help` / `app.help-search` / `app.ext-help` / `app.key-search` / `app.pane-outline` / `app.pane-files`; 中 / 日 / 英标签都补了。
- **代码审查后的修正** (同一版本内):
  - 宏里**带参数**的 `S_ChgCharset(code, bom)` / `S_FileReopen(code, …)` 直接在文档上执行 (号码按 Sakura 的文字コード 0–8: SJIS / JIS / EUC / Unicode / UTF-8 / UTF-7 / UnicodeBE / CESU-8 / Latin1; `bom` 只对 UTF-8 有效); `99` (自動判別) 等不支持的号码整份宏明确拒绝。不带参数的才打开选择列表 —— 之前会忽略参数、打开列表后接着往下跑, 后面的 `S_FileSave` 会用旧编码保存。
  - 分割视图里, 从选择列表切换 / 新开的标签不再被同一帧的「活动标签 = 焦点窗格的文档」盖回去。
  - 选中的行按行里记下的值 (文档 id / 路径 / 手册行号) 找目标, 不再按行号回头查当前的列表 (列表开着期间关掉标签不会切错); 字符集 / 插入路径作用在打开列表时的那个文档上。
  - 重开手册时整份换新 (撤销栈 / 修改标记 / 只读都复位)。
  - 手册正文的换行统一成 LF (Windows 上 git autocrlf 会把 `help.txt` 检出成 CRLF)。
  - 光标处的词改用 `editcore::word` 的 Sakura 词模型 (漢字 / ひらがな / カタカナ 各自成词)。
  - 方向键只在画出来的前 200 行里移动; 命令一览生成从 O(N²) 降到 O(N) (`keymap::display_entry`); 选择列表每帧只过滤一次。
- **验证**: 本批新增 32 项单测 (ide-fast 487 通过; 失败的 3 项仍是写死 `C:\` / `D:\` 的 Windows 路径断言, 改动前就是这样)。其中两项真的跑了 egui 帧: 选择列表单独跑一帧、以及整个界面 (`ui_main`) 里开窗口一览 → 上移 → 回车切标签 → 回车没漏进编辑区 → Esc 关闭。**没在真窗口里验证**: 窗口一览 / 检索窗口的外观、帮助菜单的新项、状态栏提示的实际显示 (本环境没有 Windows)。

## 0.3.7 — 2026-11-04

### 高速版移植补齐 (Tauri → 高速版) 第六批: 宏
- **Sakura 宏函数覆盖**: 宏函数表一共 268 个 `S_` 函数, 以前高速版只有 80 个能跑, 其余报「还不支持」。现在**每个函数都有明确的结局**: **142 个直接在宏的目标文档上执行**, **103 个转交 App 去执行**, **23 个明确不执行并写明原因** (层叠 / 平铺窗口、SQL*Plus、外部帮助、JS 宏、IME 再変換等, 原因登记在 `macros.rs` 的 `NOT_APPLICABLE`)。单测逐个函数核对: 能跑的真的跑得通, 转交的命令 id 在命令表里存在, 不执行的都有原因, 登记表里没有过期的名字。
- **整体预检**: 宏里有一步跑不了, 整份宏都不执行 (以前会做到一半才报错, 文档被改了一半)。提示会指出是第几行、哪个函数、为什么。
- **窗口类步骤按顺序转交 App**: 保存 / 另存 / 打开 / 新建 / 关闭、查找下一个 / 上一个、增量查找、拆分窗口、标签与分组切换、打印、页面设置、标签跳转、比较、显示 / 隐藏各栏…… 这些步骤按命令表里的命令在宏的目标文档上执行; 转交之后活动标签变了 (新建、切换、关闭), 后面的步骤跟着活动标签走。
- **宏里的剪贴板**: `S_Cut` / `S_Copy` / `S_CutLine` 等写进宏自己的剪贴板, `S_Paste` / `S_PasteBox` 读它, 宏跑完同步回系统剪贴板历史。
- **只动文档的函数补齐**: 撤销 / 重做 / 全选、剪切行 / 删除行 / 剪到行首 / 删到行首行尾、Tab 缩进 / 反缩进、插入日期、插入 / 改写切换、范围选择开始、取消模式、书签 (设置 / 下一个 / 上一个 / 清除)、10 个编码转换 (自动判别 → SJIS、E-Mail、EUC、JIS、UTF-7、UTF-8、Unicode 的互转)、按编码重新打开 (有未保存修改时拒绝)、向上 / 向下两行、按段落移动光标。
- **矩形选择的键盘扩展** (`S_Left_BoxSel` 一系列, 共 17 个: 上下左右、上下两行、按词、行首行尾、文首文末、翻页、半页): 矩形从起点拉到新光标处, 往回按会缩小。只在宏里提供 —— Sakura 的 Alt+方向键在高速版里是「上移 / 下移一行」, 不改键位。
- **宏登记槽** (同 Tauri 的「宏登记」): 20 个槽, 每个有名字 + 种类 (Sakura 宏 / PPA 脚本) + 代码。首选项新增「宏」页编辑; 「宏 > 宏登记」点名字运行; 命令 `app.macro-slot-1` … `app.macro-slot-20` 可在键位设置里绑键; 「把当前文档 / 选区登记为宏」存进第一个空槽 (`.ppa` 文档登记成 PPA, `.js` 拒绝, 20 个槽满了会提示)。槽存在设置文件的 `macro_slots` 里, 读入时按不可信数据清洗 (恒为 20 个、名字去控制字符并封顶、种类只认 mac / ppa、代码封顶)。与 Tauri 的差别: 没有 JS 宏这一种。
- **验证**: 本批新增 13 项单测 (ide-fast 455 通过; 失败的 3 项仍是写死 `C:\` / `D:\` 的 Windows 路径断言); `editcore` 加了 1 项 (宏函数表的名字接口)。菜单项、首选项「宏」页、转交 App 的步骤在真窗口里**没有点过**; 会弹窗的函数: 系统文件对话框 (`S_FileOpen`、`S_FileSaveAs`) 是同步的, 宏会停下来等你选完 (按代码判断, 没实测); 应用内的小窗口 (`S_JumpDialog`、`S_SearchDialog`) 弹出后宏不等它关闭, 直接继续后面的步骤。

## 0.3.6 — 2026-11-04

### 高速版移植补齐 (Tauri → 高速版) 第五批
- **代码补全** (Ctrl+Space 或 Alt+/ ; 编辑菜单 / 命令面板, 同 Tauri `completion`): 光标左边的标识符当前缀, 候选 = 当前文件类型的**关键字 > 内建名 > 文件里出现过的标识符** (3 个字符以上); 前缀不分大小写, 同档里短的在前, 最多 40 个。浮层贴在光标行下面: ↑ ↓ 换候选, **Enter / Tab 接受** (Tauri 只能点击接受), Esc 关闭, 也可以点; 接受是一步撤销。打字、移动光标、切标签浮层自动关掉。内建名表是 Tauri 的原表搬来 (19 种语言); 关键字来自 `editcore` 的文件类型表。与 Tauri 的差别: Tauri 用 Ctrl+N / Ctrl+P 换候选, 在高速版里这两个键是新建 / 打印, 没移植。Ctrl+Space 在中文输入法下可能被输入法占用, 所以另给了 Alt+/。
- **转到定义 (本文件)** (搜索菜单 / 命令面板, 同 Tauri `goto-def`): 光标处标识符的定义行 —— 第一行形如 `fn / struct / enum / impl / trait / type / func / class / def / function / interface / const / let` 加这个名字的行; 没有就跳到它的首次出现; 都没有就提示。记进跳转履历。和 F12 的标签跳转 (读 tags 文件、能跨文件) 互补。
- **工具栏按钮可自定义** (首选项 > 工具栏, 同 Tauri 的「快捷按钮」): 「视图 > 工具栏」里的按钮不再写死 —— 可以添加任何命令 (按名字选)、上移 / 下移 / 删除、加分隔线、恢复默认。存在设置文件的 `toolbar_cmds` 里 (导出的偏好也带), 读入时按不可信数据清洗: 不在命令表里的丢掉、重复的只留第一个、连续 / 首尾的分隔线去掉、最多 40 项。默认按钮和以前一样。与 Tauri 的差别: Tauri 的快捷按钮还能放插件命令和宏槽, 这里只能放命令表里的命令。
- **AI 推理强度档位** (AI 面板 > 设置, 同 Tauri): 推理强度从自由文本改成下拉 —— 「默认 (不指定)」+ 档位; 档位优先用「从供应商获取」到的, 其次手填的「自定义档位」(逗号分隔), 都没有才用通用默认 `minimal / low / medium / high`。「从供应商获取档位」点了才联网 (后台线程, 不卡界面), 先查 `<接口>/models/<模型>` 再查 `<接口>/models`; 各家公布档位的写法不一样, 所以按形状找 (键名含 effort 的数组, 或 `{ low: {supported:true} }` / `levels` / `enum` / `values` 这类对象), 结果按强弱排序; 列表里没有这个模型时报错, 不会把别的模型的档位并起来当成它的。档位名只许字母数字和 `_ . -` (1–24 个字符, 最多 12 个), 因为它会原样写进请求。档位存进 `ai.json`。
- **验证**: 本批新增 24 项单测 (ide-fast 442 通过; 失败的 3 项仍是写死 `C:\` / `D:\` 的 Windows 路径断言)。获取档位用本机假服务器测过 (真实的 curl GET、请求头、两个地址的回退、连接被拒、模型不在列表里), **没有连真实的供应商**。补全浮层、首选项的工具栏页、AI 设置界面在真窗口里**没有点过**。

## 0.3.5 — 2026-11-04

### 修正
- **SPACE 缩进 / 反缩进** (0.3.4 引入): Tauri 最终用的是 `editcore::edit_ops` 里 Sakura 实测过的版本 (`tests/sakura-diff` 有对照用例, 含「空行」), 语义是: 选中的行 (没有选区 = 光标行) **行首加 1 个空格, 空行不动**; 反缩进去掉行首的 **1 个空格** (没有就不动, 不碰 Tab)。0.3.4 照着前端里已经被覆盖掉的旧代码写成了「加 Tab 宽度个空格、空行也加」。现在改对了; 有选区时缩进后仍选着这几行, 可以连续按。
- **整理行 / 文本变换改用 `editcore` 里 Sakura 实测的实现** (之前高速版自己抄了一份, 细节和 Sakura 不一样)。行为变化: ① 「Tab → 空格」按制表位对齐 (`a⇥b`、Tab 宽 2 → `a b`), 以前是每个 Tab 换成固定个数的空格; ② 「去除行首 / 行尾空白」只去空格和 Tab, 以前连全角空格 (U+3000) 一起去; ③ 升序 / 降序排序按 UTF-16 顺序 (和 Sakura 一致; 含补充平面字符时顺序和以前不同); ④ 大小写、行首空格 → Tab 同样走 `editcore`。

### 高速版移植补齐 (Tauri → 高速版) 第四批
- **Sakura 默认键补齐**: 把 Tauri `KEYMAP` 逐项核对了一遍, 高速版有对应命令却没绑键的补上了 —— Alt+A / Alt+D (升 / 降序排序)、Alt+L / Alt+R (去行首 / 行尾空白)、Shift+F10 (复制当前行)、Shift+F8 (CRLF 复制)、Ctrl+Shift+D (删除单词)、Ctrl+F6 / Ctrl+F7 (小写 / 大写)、Ctrl+Alt+F5 / Ctrl+Alt+Shift+F5 (Tab ↔ 空格)、Ctrl+F4 (关闭标签)、Alt+Shift+F4 (关闭其它标签)、Ctrl+Alt+F4 (退出)、Ctrl+Alt+O (打开文件夹)、Ctrl+Alt+P (页面设置)、Ctrl+1 / Ctrl+2 (工具栏 / 功能键栏)、Ctrl+M (保存键盘宏)、Ctrl+F5 (执行外部命令)。单测逐项断言。没绑的: Alt+M (被「宏」菜单的助记字母占用, Tauri 同)、Ctrl+Alt+L / Ctrl+Alt+Shift+L (高速版里 Ctrl+Alt+L 是 AI 对话)、Ctrl+Alt+Shift+方向键 (窗口最大化 / 最小化)、帮助检索和 SQL*Plus 相关键。
- **合并连续重复行** (编辑 > 整理行 / 命令面板, 同 Tauri `uniq-consecutive`): 相邻的相同行只留一行 (和「去重」不同, 「去重」是整份里只留首次出现)。
- **执行外部命令** 注册成命令 (工具菜单 / 命令面板, Ctrl+F5): 打开底部面板的终端页。
- **运行 `.mac` 时更多 `S_` 函数能执行**: 带命令 id 的 `S_` 函数一共 85 个, 以前只有 2 个能在高速版里跑, 其余报「高速版还不支持」; 现在能跑的是 21 个 (实测), 增加的都是只动文档本身的: SPACE 缩进 / 反缩进、半角 / 全角英数与假名转换、按段落选择、下一 / 上一处改动 (含选择式)、下一 / 上一个函数、插入日期时间、只读切换、换行符改写 (CR / CRLF / LF)、UnicodeBE → SJIS 转换; `Alt+M` (合并连续重复行) 也认了。窗口 / 标签 / 分组 / 对话框 / 剪贴板类 (关标签、新窗口、分组切换、复制 HTML、各种对话框等) 不是「作用在一个文档上的宏」, 仍然明确报错、不静默跳过。
- **验证**: ide-fast 单测 418 通过 (3 个失败仍是写死 `C:\` / `D:\` 的 Windows 路径断言)。菜单项、快捷键在真窗口里没有点过。

## 0.3.4 — 2026-11-04

### 高速版移植补齐 (Tauri → 高速版) 第三批
- **标签分组命令** (窗口 > 标签分组 / 命令面板, 同 Tauri 的 `next-group` / `prev-group` / `tab-separate` / `tab-joint-next` / `tab-joint-prev` / `bind-window` / `group-close`): 下一个 / 上一个分组 (切到那个分组里最靠左的标签, 到头绕回)、移到新分组、移到下一个 / 上一个分组、合并为一组 (全部回到无分组)、关闭当前分组 (逐个关, 有未保存的会问)。分组还是标签右键菜单里那 6 种颜色分组。与 Tauri 的差别: 切分组时 Tauri 回到该分组最近用过的标签, 高速版回到最靠左的 (标签没有「最近使用」记录)。
- **SPACE 缩进 / SPACE 反缩进** (编辑菜单 / 命令面板, 同 Tauri `indent-space` / `unindent-space`)。**这一条在 0.3.4 里的实现有误 (照了 Tauri 前端里已被覆盖的旧代码, 写成加 Tab 宽度个空格), 0.3.5 已改成 Sakura 实测的语义, 见 0.3.5 的「修正」。**
- **光标行居中** (编辑菜单 / 命令面板, 同 `cur-line-center`)。
- **立即备份** (工具菜单 / 命令面板): 不等定时器, 把所有有未保存修改的文档现在就写进备份目录。自动备份的“收集要写的文档”顺手拆成独立函数并补了测试。
- **选区统计方式** (点状态栏的选区字数, 或命令面板, 同 Tauri `select-count-mode`): 字符数 -> 字节数 (UTF-8) -> 行数 轮换, 中 / 日 / 英提示。
- **切换查找标记 / 回到查找起点** (搜索菜单 / 命令面板, 同 `search-clearmark` / `jump-srchstart`): 前者就是查找栏里的「全部强调」开关; 后者回到这次查找开始时的光标位置 (记进跳转履历)。Tauri 把它们绑在 Ctrl+F3 / Ctrl+Shift+F3, 高速版这两个键已经是「查找当前词」, 所以没有默认键。
- **验证**: 本批新增 6 项单测 (ide-fast 416 通过; 失败的 3 项仍是写死 `C:\` / `D:\` 的 Windows 路径断言)。菜单项、状态栏点击在真窗口里没有点过。

## 0.3.3 — 2026-11-04

### 高速版移植补齐 (Tauri → 高速版) 第二批
- **删除上方行 / 删除下方行** (编辑 > 行): 删掉光标行上面 / 下面那一行, 光标行不动 (editcore 早就有这两条命令, 高速版之前没有入口)。
- **复制路径与光标位置** (编辑菜单 / 命令面板, 同 Tauri `copy-tag`): 复制 `路径(行,列)`, 行列从 1 数; 没存盘的文档用文档名。
- **在折行处加换行复制** (编辑菜单 / 命令面板, 同 Tauri `copy-addcrlf`): 选区 (没有选区 = 当前行) 按折行列数切开, 切口写成 CRLF; 折行列数按窗口宽度时用当前窗口能放的列数; 按字符数切, 不按字节。
- **打开同名 C/C++ 头 / 源文件** (搜索菜单 / 命令面板, Ctrl+Shift+C, 同 Tauri `open-hc`): 源文件 (c cpp cc cxx) 找头文件 (h hpp hh hxx), 头文件找源文件, 按顺序开第一个存在的; 已经开着的直接切过去。
- **(选择) 下一处 / 上一处改动** (搜索菜单 / 命令面板, 同 Tauri `modify-next-sel` / `modify-prev-sel`): 选区从原来的位置 (已有选区则是它的锚点) 拉到改动块首, 连续按继续拉长, 往回按收缩。
- **验证**: 本批新增 8 项单测 (ide-fast 410 通过; 失败的 3 项仍是写死 `C:\` / `D:\` 的 Windows 路径断言)。菜单项没有在真窗口里点过。

## 0.3.2 — 2026-11-04

### 高速版新增
- **单按 Alt 聚焦菜单栏** (同 Tauri / Windows): 按下再松开 Alt, 中间没有别的键 / 鼠标 / 文字 / 失焦, 高亮第一个顶层菜单; ← → 循环切换, ↓ / Enter / 空格 展开并高亮第一项 (↑ 最后一项), 助记字母直接开菜单, Esc 或再按 Alt 退出, 菜单展开时单按 Alt 关菜单。Alt+字母、Alt+点击 / 拖选 (列块) 不受影响。状态机在 `app/menunav.rs` (`AltTap` / `bar_key`, 有单测)。
- **Markdown 工具条矢量图标**: Tauri 的彩色 emoji (链接 图片 任务 引用 结构 Callout Mermaid 导出 检查) 换成 painter 画的矢量图标 (`svgicon.rs`), 跟主题文字色走, 悬停放大 10%, 提示用 Tauri 样式工具提示。
- **结构化搜索** (Markdown 工具条「结构」/ 查找菜单 / 命令面板): `类型 [条件]` (heading #2 关键字 / link / image / fence 语言 / task / todo), 复用 `editcore::md_struct::find` (同 Tauri `md_find`), 命中列表点一条跳到该行; 中 / 日 / 英文案。
- **检查更新…** (帮助菜单 / 命令面板): 直连 GitHub Releases, 稳定 / 开发两个通道 (首选项里选), 可选「每天启动时检查一次」(默认关, 首帧之后才做); 下载后必须通过 `SHA256SUMS.txt` 的 SHA-256 校验才能 "安装并重启" (msi 走 `msiexec /passive`, 便携版自行换文件), 失败保留当前版本。设计见 `ide-fast/docs/UPDATE.md`。
- **更新包验签**: `SHA256SUMS.txt` 必须带发布者的 ed25519 分离签名 `SHA256SUMS.txt.sig` (原始 64 字节), 先验签再信任清单里的哈希; 缺签名 / 验签失败 = 拒绝并删除下载物; 安装前再粗检文件头 (msi 的 OLE 头、exe 的 `MZ`)。公钥内置在 `ide-fast/src/updkey.rs`, **目前是全 0 占位 = 未配置**, 此时更新窗口提示"更新签名密钥未配置"、"安装并重启"保持禁用、只给发布页链接 (发行版里没有任何环境变量可以绕过)。附签名工具 `ide-fast/packaging/sign-release.ps1` + `examples/sign-sums.rs`, 密钥生成 / 保管 / 轮换见 `ide-fast/docs/UPDATE.md`。新增依赖 `ed25519-dalek` (仅验证, 关掉全部默认特性), release exe 体积无可测增长。

### 高速版移植补齐 (Tauri → 高速版)
- **宏录制 (.mac)** (宏菜单 / 命令面板): 开始录, 再执行一次停止并把刚录的键盘宏转成 Sakura `.mac`, 开成未保存的新标签 `录制的宏.mac` (带注释头; 保存后用「运行当前文档 (Sakura .mac)」回放)。「把刚录的键盘宏转成 .mac」可单独导出用 Ctrl+Shift+M 录的宏。生成器就是 Tauri 版 `macro-rec-mac` 用的 `editcore::macro_sakura::record_to_mac`: 键入的文字合成 `S_InsText`, 回车 / 退格 / 删除 / 移动键 / 复制行等按 Sakura 默认键写成 `S_` 函数, 找不到对应 `S_` 函数的步骤列在提示里、不写进宏。录 → 导出 → 运行的往返有单测 (结果与直接回放一致)。
- **运行 `.mac` 时认更多只有默认键的 `S_` 函数**: 以前只认移动键 / 退格 / 删除 / 回车 / Ctrl+I, 其它报「这条命令高速版还不支持」。现在还有 Ctrl+D / Ctrl+Shift+D / Ctrl+K / Ctrl+W / F7–F9 / Shift+F8 (词剪切、删词、剪到行尾、选词、剪切 / 复制 / 粘贴)、选区变换 (Ctrl+F6…F12 大小写与全半角假名、Ctrl+Alt+F5 Tab 与空格)、整理行 (Alt+A 升序、Alt+D 降序、Alt+L / Alt+R 去行首 / 行尾空白)。录制与运行共用同一张键表 (`macros.rs`), 单测保证表里每项两头都对得上。
- **上一个 / 下一个函数** (搜索菜单 / 命令面板, 同 Tauri `funclist-next` / `funclist-prev`): 按大纲条目所在行跳, 到头绕回另一端, 记进跳转履历; 未保存的文件也能用。
- **列块删除 / 列块替换…** (编辑菜单 / 命令面板, 同 Tauri `column-delete` / `column-replace`): 先用 Alt+拖选 建立列块选区; 替换窗口预填列块现在的内容, 多行用换行分隔、一行换一行。
- **验证**: 本轮新增 10 项单测 (ide-fast 402 通过; 失败的 3 项是测试里写死 `C:\` / `D:\` 的 Windows 路径断言, 在 Linux 上必然失败, 与本轮无关)。**菜单项、列块替换弹窗在真窗口里没有点过** (本轮环境没有 Windows), 只验证了逻辑。核对出来仍没有的 Tauri 功能 (再変換、关键字帮助、窗口一览等) 记在 `ide-fast/docs/PARITY-PLAN.md`。

### 高速版外观对标 (第六轮)
- **Markdown 预览默认盖在编辑区上** (同 Tauri `#md-preview`): 编辑区底色、正文最宽 860 居中、点块就地编辑; md 文档上方有同款工具条 (格式按钮 / 章节上下移 / 检查 / 导出 / 预览开关)。视图菜单「Markdown 预览覆盖编辑区」取消勾选可回到右侧停靠模式。
- **工具提示改成 Tauri 样式** (#333 底 / 白字 / 圆角 3 / 11px), 自绘, 不影响别的弹出层。
- **Alt+助记字母打开菜单** (F E S O P C B W M V T H), 先于命令键 (Alt+P 开「插件」菜单, 「上一个位置」仍在书签菜单 / 命令面板), 打开后方向键 / 回车 / Esc 可用; 「只按 Alt + 字母」不再把字母打进文档。
- **半透明 (玻璃) 主题运行中切换, 不再需要重启**: 窗口一律以透明建, 切换主题只开关 DWM 模糊与清屏色; 删除「立即重启」对话框。首帧仍 ≤ 260 ms, 发行版 exe 仍是 GUI 子系统。

### 高速版外观对标 (第七轮: 核对第六轮未确认项)
- **菜单键盘导航重做** (`app/menunav.rs`): 菜单开着时方向键由 `raw_input_hook` 截下, 走一个纯状态机 (第一次 ↓ 高亮第一项, ↑ 高亮最后一项, 项间循环; ← → 切换顶层菜单并清掉高亮, 没高亮时 → 也能切); 焦点在子菜单里时仍交给 egui。Enter 激活、Esc 关闭。真窗口核对: Alt+E ↓x4 落在第 4 项「复制」, → 切到「查找」, ← ↑ 落在「文件」菜单最后一项。
- **Esc 关菜单同帧不再打出多余字母**: Alt+字母附带的 Text 事件同帧和下一帧都吃掉; Esc 与菜单开着 / Alt+字母同批时整批 Text 丢弃。
- **Markdown 预览就地编辑**: 点块进入编辑 (自动取得焦点), 点编辑框外 = 有改动就应用 (没改动直接收起), Esc = 放弃, 应用 / 撤销都是一步; 同一次点击落在另一块时只应用、不再开新编辑 (行号已变)。修了 Esc 被全局快捷键先吃掉的问题。
- **玻璃主题背景材质改用 Acrylic** (`glassfx.rs`): Windows 11 10.0.26300 上 `apply_blur` 背后是纯黑, `apply_acrylic` 截图验证是真模糊 (Mica 只取壁纸色); 默认只试 Acrylic, 调用失败就只剩 alpha 透明。`IDE_FAST_BACKDROP=blur|acrylic|mica|tabbed|none` 可强制某一种。

### 高速版安全修复
- **更新清单绑定版本与通道, 防回滚**: 此前 ed25519 签名只覆盖 `SHA256SUMS.txt` 的内容, 不覆盖它属于哪个版本, 攻击者可把旧版本的签名清单 + `.sig` + 旧安装包挂到新 Release 页上诱导降级。现在签名清单第一行必须是 `version: X.Y.Z channel: stable|dev`, 验签后、下载前客户端要求 (a) 清单版本 = Release 版本, (b) 清单版本 > 当前版本, (c) 清单通道与所选通道相符 (开发通道也接受正式版清单), (d) 安装包文件名含该版本串且在已签名的表中; 任何一项不满足 = 拒绝并删除下载物, 中 / 日 / 英提示。`examples/sign-sums.rs` 的 `sign` 现在要求 `--version` / `--channel` 并核对清单首行 (不符拒绝签名), `sign-release.ps1` 对应 `-Version` / `-Channel`, `publish-dev.ps1` 自动写版本行并把 msi 改名带完整 dev 版本。**发布方需注意**: 没有版本行的旧格式清单会被新客户端拒绝。语法与流程见 `ide-fast/docs/UPDATE.md`。

## 0.3.1 — 2026-11-03

### 修复
- **高速版启动不再带出终端窗口**: 0.3.0 的发行版 exe 是控制台程序, 双击会多出一个黑色终端 (搜狗输入法被注入后还会往里打印 libpng 警告)。现在是 GUI 子系统; 打印、`exec_cmd`、GitHub 插件 curl 等子进程也都不再弹控制台。
- 安全加固 (审查项): Markdown 链接不解析 UNC 路径; Git 状态 / 差异 / 暂存禁用仓库内的外部命令配置与联网; 终端转义序列参数封顶; 插件结果不写到别的文档; AI 工具调用必须是插件清单声明过的; AI 流式读取按上限截断。

### 高速版新增 (批次 10 / 11)
- 编辑: F6 范围选择、拖放选中文字、标签跳转、增量查找条、固定列折行与禁则 (含悬挂)、文本转换 (codecnv / base64 / uudecode / copy-html / 剪贴板历史 / 控制字符表 / 日期时间戳) 。
- 布局: 上下 / 四分割窗格、工具栏、功能键栏、活动栏 (44 px 矢量图标)、欢迎页、自绘标题栏 (可切回系统标题栏)、左 / 右 / 底面板折叠 (按钮与外观对齐 Tauri 版)、页面设置与打印预览。
- 其他: AI 对话面板、自动备份与启动恢复、可选文件锁、半透明主题 (切换后重启)、Markdown 预览编辑与 lint / 导出、GitHub 插件安装与更新检查、sakura.ini 导入补全。
- 外观对标 Tauri 版: 颜色令牌 / 蓝色状态栏 / 标签栏 / 菜单助记与右对齐快捷键 / 浮动查找 / 滚动条 / 编辑区字号与行号栏 / 语法色。逐区域差异和做不到的原因见 `ide-fast/docs/VISUAL-PARITY.md`。
- 性能: 超长单行只排版可见切片 (1 MB 单行每帧 86 ms → 0.38 ms); 窗口先隐藏、首帧画完再显示; 字体后台预读。基线与取舍见 `ide-fast/docs/PERF.md`, 回归工具 `ide-fast/tests/perf.ps1`。

### 高速版 (原生 Rust, `ide-fast/`) — 批次 11 (面板折叠 / 半透明重启 / 自绘标题栏)
- **面板折叠** (`panels.rs`, `app/docks.rs`): 左侧栏 / 右侧面板 (AI 对话、Markdown 预览共用一条) / 底部面板 (终端、输出、问题) 各自可折成 22 点的细条, 点面板头部的箭头或细条上的箭头展开; 命令 `app.side-collapse` (Ctrl+Alt+H) / `app.right-collapse` (Ctrl+Alt+J) / `app.bottom-collapse` (Ctrl+Alt+K), 命令面板和视图菜单里都有。折叠状态与各面板宽高 (`side_w` / `term_h` / `ai_w` / `md_w`) 写进 settings.json; 折叠时面板内容不布局。Ctrl+B / Ctrl+` 在细条状态下先展开。
- **半透明 (玻璃) 主题不能运行中切换, 改为「立即重启」** (`relaunch.rs`): 窗口是否透明在创建窗口时就定了 (WGL 像素格式要带合成标志; winit 0.30 在 Windows 上运行中调 `set_transparent` 只改标志, 不重做 DWM 的 blur-behind), 所以换到玻璃主题时弹「立即重启 / 稍后」。立即重启走正常关闭流程 (有未保存的修改照常先问), 窗口关完、设置落盘后由 `main` 拉起新进程 (`--relaunch`, 不转交给正在退出的旧进程), 带着现在打开的文件。
- **自绘标题栏** (`app/titlebar.rs`, 默认): 无系统标题栏, 栏里有应用图标、紧凑菜单、快捷按钮 (新建 / 打开 / 保存 / 查找 / 命令面板 / 分割) 和三个面板开关 (宽度够时才画)、最小化 / 最大化 / 关闭; 拖空白处移动, 双击空白处最大化 / 还原; 窗口边缘有 5 点的缩放把手 (`BeginResize`); 最大化时内容缩进 8 点 (Windows 会把无边框最大化窗口放大出屏幕一圈)。F11 全屏时不画标题栏 / 窗口按钮, 只留一条菜单行。视图菜单 / 命令 `app.system-titlebar`「使用系统标题栏」可换回系统标题栏, 即时生效 (`ViewportCommand::Decorations`), 设置项 `system_title_bar` 落盘。玻璃主题下标题栏用面板同一个半透明底色。

### 高速版 (原生 Rust, `ide-fast/`) — 批次 10 (窗口与外观)
- **分割窗口**: 窗口菜单新增上下分割 / 四分割 (`split.rs`), 每个窗格各看一个文档, 点哪个窗格哪个收键盘。
- **工具栏 / 功能键栏 / 活动栏**: 视图菜单里开关, 默认关 (极简布局不变); 命令都走注册表, 改键后提示跟着变。
- **窗口图标**: 取自安装包同一个 ide-fast.ico 的 64px 帧。
- **页面设置与打印预览**: 纸张 / 横向 / 字号 / 页边距 / 行号 / 页眉 / 页脚; 应用内分页预览, 再交给浏览器打印; 设置落盘。
- **界面三语**: 查找条 / 侧栏 / Git / 终端 / Grep 面板文字补齐中日英。
- **sakura.ini 导入**补到与 Tauri 版同等: 查找开关、换行方式、`Types(n)` 按扩展名写入覆盖项。

## 0.3.0 — 2026-11-02

### 高速版 (原生 Rust, `ide-fast/`) — 批次 9

#### 命令注册表与键位
- **命令面板** (Shift+F1 / Ctrl+Shift+P): 模糊匹配全部命令 (编辑命令 + 窗口级动作) 和启用的脚本插件命令; 方向键 / Enter / Esc。
- **键位设置** (设置菜单 / 帮助菜单): 列出全部命令和键, 点「改键」后按下新组合; 冲突时问是否抢占 (对方解除绑定, 多备选键的只去掉被抢的那一个); 清除 / 恢复默认 / 全部恢复; 改键立刻生效并存进 `settings.json` 的 `keys` (只存和默认不同的)。普通打字键 (不带 Ctrl / Alt 的字母数字) 和 Esc 不能绑定, 系统键 (Ctrl+X / C / V) 只显示不能改。
- **键位一览** (Ctrl+Q): 可筛选, 可复制全部。
- 快捷键分发改成表驱动 (`keymap.rs`), 修饰键多的先消费; 菜单里的快捷键提示读当前生效的键, 改键后菜单跟着变。
- `settings.json` 里的 `keys` 按不可信数据读: 丢掉未知命令 / 写错的键 / 不安全的绑定, 数量和长度封顶。

#### 其它
- **Migemo 查找**: 查找条的「Migemo」开关, 用罗马字找日文 (kensaku 能找到 検索 / けんさく / ケンサク)。
- **会话恢复**: 启动时 (没有命令行文件) 重新打开上次的标签并回到活动标签; 偏好里可关。路径按不可信数据校验 (拒绝 UNC / 相对路径 / 超长), 超过大文件阈值的跳过。
- **键盘宏保存 / 载入** (宏菜单, JSON): 步数、命令名、文字长度封顶, 整份校验过才替换。
- 编辑: 选到上 / 下一段落; 搜索: 下一处 / 上一处改动 (改动块的起始行); 复制文件名 / 完整路径; 帮助 → 关于。

#### 没做 (原因见 `docs/PARITY-PLAN.md` 状态总表)
- AI Chat 面板、玻璃主题 (需要新依赖 / `unsafe` / 密钥存储, 留在 Tauri 版)。
- 工具栏 / 功能键栏 / 活动栏 / 自绘标题栏 / 应用图标、插入日期时间、F6 范围选择、拖放选中文字、上下 / 四分割、自动备份定时器、文件锁、tag jump、codecnv-*、扩展面板的 GitHub 安装与更新检查、Markdown 预览里直接编辑 / lint / 导出、终端外观设置、`markdown.fence` 接入预览。

## 0.2.0 — 2026-11-02

### 高速版 (原生 Rust, `ide-fast/`) — 第 3–5 期
- **编辑**: 书签 (随行增删移动)、矩形选择 (Alt+拖)、多光标 (Ctrl+点击)、键盘宏录制 / 回放、Sakura `.mac` 宏、**折叠** (按缩进; 行号栏 ▸ 点击 / Ctrl+Shift+[ ]; 光标进入折叠自动展开)、**自动换行** (光标 / 选区 / 点击命中按真实排版, ↑ ↓ Home End 按显示行移动)。
- **面板**: 终端 (后台执行, 可勾「选区作标准输入」「输出替换选区」)、Markdown 预览 (原生块渲染, 点块跳行)、侧栏 (文件树 / 大纲 / 后台 Grep + **Grep 置换**, 自动 .bak 备份, 跳过有未保存修改的已打开文件)。
- **窗口**: **分割窗口** (Ctrl+\, 左右两栏各看一个文档, 键盘只给有焦点的一栏)、**标签分组** (右键标签选颜色组)、**打印 / 打印预览** (Ctrl+P, 排成带行号的打印页交给系统浏览器的打印对话框)、首选项窗口、命令行 `文件:行:列`、单实例 (再次启动把文件交给已运行的窗口)。
- **插件 (声明式部分)**: 同一个插件目录 / 同一套 zip 导入·删除·恢复内置; **主题插件**直接生效 (颜色变量 -> egui 外观, 选择落盘); 带脚本入口的插件 (PPA / 文档关系图 / Pele) 能列出、导入、删除, 但**高速版不嵌 JS 引擎、不运行它们**, 列表里标明。
- 修复: `editcore::highlight_line` 在含中日文的行上按字节切片 panic。
- **未验证**: MSI 的交互式安装 / 卸载 / 升级; 分割窗口、插件菜单、打印页在真实窗口里只做了启动自检和单元测试, 没有逐项人工点按; 无代码签名。
- **没做**: 脚本插件的运行、Mermaid / AI Chat (需要浏览器能力)、换行下矩形选择 / 多光标的视觉 (只画在第一个显示行)。

## 0.1.9 — 2026-11-02

### 插件
- **插件导入改成「选 zip 直接导入」**: 设定菜单「导入插件 (.zip)…」、扩展面板的主按钮「导入插件 (.zip)…」、网页版把 `.zip` 拖进窗口, 三处入口走同一个对话框 —— 解包后给你看插件名 / 版本 / 许可证 / 要求的权限 / 各文件哈希 / 包里自称的来源 (**zip 没有签名, 来源没有被验证**), 你同意权限才安装; 不兼容的包会列出原因而不是装一半。可一次选多个 zip。
- **主题做成插件** (`theme-clear` / `theme-dark` / `theme-light` / `theme-hc` / `theme-crystal` / `theme-retro16`, 宿主新增 `theme` 贡献点), 随程序打包, 也作为 Release 里的插件包提供: 删掉之后可以用 zip 装回来。(这一路改动由另一个会话开发, 随本版一并发布; 水晶主题的字符级反色还在迭代, 见下「已知」。)
- Release 资产里的插件包由 `scripts/package-plugins.mjs` 生成 (全部内置插件 + 校验和)。

### 界面
- **右上角快捷按钮可自定义 (仿 Office 的快速访问工具栏)**: 默认只有 **Grep 搜索**; 「首选项 → 快捷按钮」里可添加任何命令 (菜单里的、插件贡献的)、上移 / 下移 / 移除、恢复默认 (草稿 + OK / 取消); 设置跨重载保留。

### 高速版 (新增独立产品, 原生 Rust)
- `ide-fast/`: 不依赖 WebView2 的原生 (egui) Sakura 风格编辑器, 编辑 / 移动 / 查找复用 `editcore`; 首帧约 0.2 s (Tauri 版约 1.0–1.5 s)。已有: 多标签、打开保存 (编码·BOM·换行保持)、撤销、剪贴板、查找置换 (正则)、语法高亮 (20 种语言)、设置落盘、最近文件、侧栏 (文件树 / 大纲 / 后台 Grep)、拖文件打开、输入法。**还没有**: 自动换行、书签、矩形选择 / 多光标、宏、Markdown 预览、终端、插件 (见 `ide-fast/docs/ROADMAP.md`)。
- **独立安装包**: `IDE1.0-Fast_<版本>_x64.msi` + 便携 `ide-fast-<版本>-windows-x64.exe`, 和 Tauri 版是两个产品、可并存。**MSI 的交互式安装 / 卸载 / 升级尚未在真机验证; 无代码签名。**

### 已知
- 水晶主题: 另一路开发里有一条尚未通过的回归 (`crystal-theme.test.mjs` 期望编辑区用 `mix-blend-mode: difference` 反色, 但本项目的设计结论是不用差值混合), 该测试**没有纳入本版**; 水晶主题的文字反色按现有的统一反色逻辑工作。
- 仍未做: 折行的每类型设置 / 避头尾 / 自由光标、矩形选择按显示列、原生剪贴板格式、大文件惰性加载。

## 0.1.8 — 2026-11-02

### 界面
- **标题栏**: 标题一行显示、窄窗口用省略号收缩 (以前初始就折成两行被裁掉); 左上角 logo 固定是 logo 图, 不再随主题变色。
- **透明玻璃主题下 Markdown 预览**: 预览和原文不再叠在一起 —— 打开预览就只看到预览, 关了回到原文, 和其它主题一致。
- **预览里可以编辑和用目录跳转**: 点预览里的一个块, 它变成该块原文的输入框 (Ctrl+Enter 或点别处保存、Esc 放弃, 一步撤销); 侧栏目录 (大纲) 点击会让预览滚到对应的块。渲染器给每个顶层块带上原文行范围 (`data-l`), 导出 HTML 不带。

### 插件
- **所有插件都能删除 (包括内置的)**, 删掉的内置插件会列在「已删除的内置插件」里, 可「恢复内置」; 也可以用 Release 里的插件包 (`plugin-<id>-<版本>.zip`) 装回来: 扩展面板「安装插件包 (.zip)…」→ 解包 + 兼容检查 + 权限确认 + 哈希显示, 同意才装 (zip 没有签名, 来源声明没有被验证)。包里文件名限一层、拒绝路径穿越 / Windows 设备名 / 大小写重名; 低于内置版本的包会明说装不上; 登记写入失败会回滚到旧版本。插件包同时作为 Release 资产提供。

### AI / 首选项
- **effort 档位不再写死**: 「首选项 → AI 模型」里「从供应商获取」会查模型接口 (数组 / `supported` 对象 / 列表里找本模型, 各家写法不同按形状找), 取不到会提示并允许手动填 (逗号分隔); 聊天窗和首选项的下拉都用这些档位, 换接口地址会作废旧供应商的档位。

### 启动
- 加了启动耗时追踪 (`IDE_STARTUP_TRACE=1` 才记): 实测 Rust 一侧约 12 ms、页面脚本约 0.15 s, **大头 (约 0.7–1.2 s) 是 WebView2 初始化, 不在我们的代码里**; 顺手把启动时十来次重复的菜单快捷键刷新合成一次。没有承诺更大的提速。

### 其它 (自 0.1.7 起)
- 查找 / 替换与 Grep 支持环视和反向引用 (含这类模式时用 fancy-regex); 折行时 ↑ ↓ Home End (含 Shift) 按显示行走; 编辑区内拖动选中的文字移动 / Ctrl 复制; 状态栏显示列 (全角占 2 格) / 字符码位 / REC 指示; 大纲按编辑器正文分析; 大文件上限可在首选项调到最多 32 MB (默认仍 4 MB); 桌面版单实例与文件拖入窗口 (单实例已在真机验证)。

### 仍未做
折行的每类型设置 / 避头尾 / 自由光标 / 逻辑·物理行号、矩形选择按显示列、原生剪贴板格式、大文件的惰性加载 (超 4 MB 的文件编辑会明显变慢)、拖文件进窗口的真机验证。进度见 `docs/06_功能对标/WBS-02_next-steps.md`。

## 0.1.7 — 2026-11-01

### 界面
- **标题栏**: 桌面版改为自绘标题栏 (无系统边框): 图标 + 文档名、菜单、**快捷按钮**、最小化 / 最大化 / 关闭; 每个主题有自己的标题栏外观 (透明玻璃主题是晶面样式)。快捷按钮 (原 Sakura 工具栏) 搬到最顶端, md 文档下面那一行去掉了; 显隐与各按钮在首选项「快捷按钮」页。**桌面版的拖动 / 边缘缩放 / 关闭尚未在真实窗口里端到端验证。**
- **关闭按钮无法关闭软件**: 根因是 Tauri 能力里缺 `core:window:allow-destroy`, 已补。
- **侧栏**: 默认只有竖向导航, 不再有重复的横向标签条; md 文档会多出一个可关闭的「目录」标签 (关掉后同一文档不再自动开)。点已打开的导航按钮再点一次 = 收起。
- **增量搜索**成为终端栏的一个标签页 (自动展开面板, 点 ×、再点标签或 Esc 关闭并还原原状态)。
- **透明玻璃主题**整体再透约 40%; md 预览里的文字按**实际叠出来的底色**自动反色 (代码块 / 表头 / 引用等自带底色的块上不再黑底黑字)。

### 首选项 (共通設定)
- 改动先放草稿, 点 OK 才生效、取消全部丢弃; 新增页: 検索、ファイル、**AI 模型** (接口 / 模型 / Key / effort)、快捷按钮、**宏** (20 个宏槽, 种类 .mac / JS / PPA)、色・フォント (字体 / 行高 / 文字与语法颜色)、导入 (**sakura.ini 子集**)。聊天窗里不再有模型设置, 只留 effort 选择。

### 功能
- **PPA 宏子集**: 做成**随程序打包、可热插拔的插件** `ppa` (扩展面板可禁用): Pascal 风格的 var / if / while / for / repeat / 过程与函数 / 字符串函数 / `S_GetSelectedString` `S_GetLineStr` `S_InsText` 等; 在虚拟文档上运行, 结果一步撤销写回 (宿主新增 `editor.write` 贡献点)。插件文件同时作为 Release 资产提供。
- **查找 / 替换支持环视与反向引用** (`(?=…)` `(?<=…)` `\1`): 普通模式仍走 `regex`, 编译不了的才换 `fancy-regex`。Grep 仍不支持。
- **单实例** (桌面版): 第二次启动把命令行文件交给已运行的窗口并置前; **拖文件进窗口打开** (桌面版原生拖放, 网页版读成文档); 以上两项尚未在真实窗口里验证。
- **大纲**按编辑器里的正文分析 (未保存的修改 / 无标题文档也有); **状态栏**新增光标处字符码位 (U+xxxx / EOL / EOF)、总行数提示、宏录制 ● REC 指示。
- 设置落盘 (`ide10.*` 镜像到磁盘 JSON, 密钥不上盘)、タイプ別設定、JS 宏 `Editor.*` API、キー割り当て「按键查找」、行コメント插入 / 删除。

### 仍未做
折行 / 多字节宽度模型 (WBS 5.2/5.3)、大文件 (5.9)、Grep 的环视、剪贴板格式与编辑区内拖动、5.x 长尾。进度见 `docs/06_功能对标/WBS-02_next-steps.md`。

## 0.1.6 — 2026-10-29

### 文件与启动 (WBS-02 M1)
- **系统「打开 / 另存为」对话框 (桌面版)**: 打开可多选; 你在对话框里选定的**具体文件**被授权, 项目外也能读、写、取指纹 (规范化后精确匹配, 不放行整个目录, 进程内有效), 其余项目外路径照旧被拒; 另存为不需要先打开项目。网页版沙箱不变。
- **`路径:行:列`**: 「打开文件」和命令行启动参数都支持; 启动参数还认 Sakura 风格的 `-Y=行` `-X=列` (作没写位置的文件的默认位置); 命令行里写的文件视同授权。
- **关窗口 / 刷新时有未保存修改会先提示** (网页版 `beforeunload`; 桌面版接了 Tauri 关闭请求 + 主题化确认框, 尚未在真实窗口里端到端验证)。

### 搜索 / 宏 / 外部命令
- **Grep 输出选项**: 标准 / 只文件名 / 只命中部分, 每个文件只取第一处, 路径显示为相对项目根 (相对路径结果可直接跳转)。
- **命令级键盘宏录制**: 菜单「宏录制 (.mac, 命令级)」录下执行的命令和键入的文字, 生成 Sakura 格式 `.mac` (Rust 反查 `S_` 函数, 认不出的命令会列出); 生成的宏能被解析器接受并回放得到同样的结果。表里只给了默认键的 `S_` 函数 (如 `S_DuplicateLine`) 现在也能执行。
- **外部命令执行并捕获输出** (Sakura 的「外部コマンド実行」): 命令在项目根下运行, 选区可作标准输入, 输出开成新标签或插入光标处; 30 秒超时 (杀掉整棵进程树)、输出上限、自动判别输出编码、Windows 下带引号的命令行原样交给 cmd。

### 修复
- 一个会让「Chat 右栏」菜单命令把焦点抢进聊天输入框、导致菜单命令执行后不能直接输入的问题。
- 测试隔离: 插件登记目录不再写到真实的 `%APPDATA%`; `tests/run-all.sh` 的 Rust 单测用单独产物目录。

### 仍未做 (WBS-02 进度快照见 `docs/06_功能对标/WBS-02_next-steps.md`)
设置体系 (タイプ別設定 / 共通設定 / ini 导入)、折行 / 多字节宽度模型、大文件、JS 宏 `Editor.*` API、统一正则引擎、单实例转发等。

## 0.1.5 — 2026-10-29

### 界面
- **查找 / 置換浮动框不再遮挡终端栏**: 命中处被浮动框盖住、框挪到下方时, 现在停在底部面板 (终端栏) 和状态栏的**上方**, 不会盖住「展开终端」的按钮; 终端栏展开 / 折叠或窗口缩放时, 停在底部的浮动框跟着抬高。顺带修了判断"是否盖住命中处"的依据 (焦点在查找框里时编辑区光标是隐藏的, 量不到, 现在改量命中行本身)。
- **桌面版原生标题栏跟随主题**: 浅色主题配亮标题栏, 深色 / 高对比 / 复古配暗标题栏, 透明主题按桌面明暗; 事件驱动 (主题或明暗一变才通知), 不轮询。

### 插件 / Markdown
- **Markdown 快捷键并入键位表**: 只在 Markdown 文档里生效, 与 Sakura 原有键 (Ctrl+I 复制行 / Ctrl+B ブラウズ) 的冲突在「快捷键」对话框里列出, 可重新分配。修了 Ctrl+I / Ctrl+B 被 Sakura 键位抢先、Markdown 斜体 / 加粗按键根本触发不了的问题。
- **结构化搜索 / Git Diff / 导出 HTML (模板可编辑)** (REQ FR-034 / FR-045 / FR-049)。
- **插件 `command` 贡献点 + 第二个默认插件 `doc-graph`** (文档关系图, REQ FR-047): 标题树 + 跨章节引用, 点节点跳转; 禁用它不影响文本编辑。
- **插件 `agent.tool` 贡献点**: 插件可向 Chat 登记只读工具, 需要单独的 `agent.tool` 权限; 补丁提案仍要过 Rust 校验并由用户审核。

### 发布 / 合规
- 项目许可统一为 **AGPL-3.0-only**; README 加 Code signing policy 与隐私声明; 新增 `release-windows` 工作流 (MSI 构建, 配置 SignPath 后签名; 第三方 action 固定到提交 SHA, 最小权限) 与 Dependabot。
- 新增 MSIX 打包脚本 (`tools/msix/build-msix.ps1`) 与 Microsoft Store 提交说明。

### 修复
- 测试不再改到真实的 `%APPDATA%\IDE1.0\plugins` 插件登记 (之前的插件测试会把默认插件禁用在你机器上)。
- `tests/run-all.sh` 的 Rust 单测改用单独产物目录, 避免与开着的 web 二进制抢文件锁。

## 0.1.4 — 2026-10-28

### 界面
- **分屏独立副本**改为点**分割线**(两个视图相接的那条边): 靠近分割线时光标变成"复制", 点一下该视图得到可单独修改、单独保存的副本; 去掉了文字下面多出来的"底边"条。
- **Chat 右栏**的折叠方式和左侧 / 底部一样: 右边缘常驻一条细把手 (‹ / ›), 点它展开 / 折叠, 视图菜单里也有「Chat 右栏」; 去掉了状态栏上单独的 Chat 按钮。右栏占自己的栅格列, 编辑区让位, 底部面板和状态栏仍然通栏。
- **所有确认 / 输入对话框改成主题化的 modal** (关闭有未保存修改的标签、另存为、覆盖、跳转到行、重新载入、全部关闭、退出、快捷键冲突、插件冲突、AI 外发确认……共 20 处), 不再弹白底的系统 confirm/prompt; 加了源码扫描测试, 以后再写原生 confirm/prompt 会直接失败。
- **查找 / 置換对话框**: 浮动、可拖动标题栏、不压暗页面、不拦截编辑区的点击; 输入查找词就自动跳到第一处命中 (命中不在可见范围就居中); 命中处被对话框盖住时对话框自动挪到另一侧 (自己拖过就不再自动动)。

### 合入库接线 (WBS-02)
- 保存走**原子写** (临时文件 + 换名, 中途失败原文件完好), 首选项可勾选"保存时备份旧文件" (`.sakura-backup/`, 保留最近几代)。
- **外部改动检测**: 窗口重新获得焦点时检查一次文件指纹 (不轮询), 没改动的文档问是否重新载入, 有未保存修改的只提示。
- **Grep 置換**: 对话框新增「文字コード」栏和「全部置換 (写回文件)」按钮, 按文件原编码 / BOM / 逐行换行原子写回, 改写前备份, 有未保存修改的已打开文件跳过。
- **Sakura 宏 (.mac)** 可运行: 整份宏先在 Rust 解析规划 (语法错误整体失败, 不执行一半), 前端逐步执行 (命令 / 插入 / 光标 / 跳转 / 检索 / 置換)。

### Markdown
- **章节上移 / 下移** (含子章节, 围栏代码块里的 `#` 不算标题) 与**文档质量检查** (标题跳级 / 重复标题 / 围栏未闭合 / 空链接 / 缺 alt / 空 mermaid), 检查同时是 Agent 工具 `md_lint`。

### 修复
- 一个早已过期的测试 (`test_read_file_non_utf8`: 读文件改成自动判别编码后不再对非 UTF-8 报错) 更新为"必须标出无损失败(lossy)"。
- 回归脚本新增 `tests/run-all.sh` 的使用说明; `*.sh` 固定 LF 换行。

## 0.1.3 — 2026-10-28

### 新功能
- **Markdown 工具条** (REQ 要件基线 FR-005..FR-025): 打开 .md 文档时顶部出现 Markdown 工具条 —— 粗体 / 斜体 / 删除线 / 高亮 / 行内代码, 标题 1–4, 无序 / 有序 / 任务列表, 引用, 链接, 图片, 表格, 围栏代码块 (自动选安全围栏长度), 分隔线, Obsidian callout, Mermaid; 同时显示 Sakura 工具栏 (按钮提示里写出 Sakura 快捷键)。所有按钮和快捷键 (Ctrl+B / I / E / Shift+X / Shift+K / Alt+1..6) 走同一条 `md-*` 编辑命令 (Rust `edit_cmd/md_ops`), 只改选区或光标所在行。**预览**一键切换 (Ctrl+Shift+V): 只读, Obsidian 风格 (Front Matter / callout / wikilink / `==高亮==` / `%%注释%%` / 标签 / 任务列表 / 表格), 用户 HTML 与不安全链接一律转义。非 Markdown 文档零成本。
- **热插拔插件系统**: 沙箱 iframe (仅 allow-scripts + 禁网络 CSP) + 清单 / 权限 / 兼容性检查; **扩展面板**可启停、检查更新、从 GitHub 仓库安装最新 Release 并一键更新; 与编辑器不匹配时报结构化错误并可"让 AI 给出改善方案"; 安装原子换名、失败回滚、清单只下载一次、插件 id 绑定仓库。**默认插件 Pele** (Mermaid → SVG) 随程序打包。插件与内建功能冲突时由 Rust 仲裁: 没激活智能功能时默认插件覆盖原生, 用户偏好最高, 失败回退。设计见 `docs/01_基本设计/PLUGIN-SYSTEM.md`。
- **Chat 右栏** (💬 / Ctrl+Shift+L): 不打开就不存在; 占自己的栅格列不盖编辑区; 在「AI 设置」填接口 / 模型 / Key 后调用编辑器的智能 API; 文档内容外发前确认一次且只发选中内容 / 当前章节; 写类工具只产出补丁提案, 审核差异后点"应用" (一步撤销)。
- **Agent API** (`/api/agent`, Tauri `agent`): 静态工具表 (md_outline / md_render / search_text / propose_patch / plugin_list), OpenAI function-calling 格式, 可直接给 LangGraph 用。
- **分屏独立副本**: 点分屏视图的底边, 该视图得到一个独立副本, 可单独修改、单独保存 (另存为)。

### 修复与改善
- 选中文字时只有选中的部分着底色, 选区前面没选的文字不再着色。
- 安装包不再放进源码库: 只在发版时按版本构建, 作为 Release 资产单独上传 (见 `docs/05_实装与运维/RELEASE.md`); 加了仓库卫生测试。
- 合入三个 WBS 库: 原子保存 / 备份 / 外部改动检测 (`document_io`、`auto_backup`)、编码感知 Grep 与按原编码写回的替换 (`grep_engine`)、Sakura 宏计划 (`macro_sakura`); `/api/search` 改为类型化请求。
- Grep 结果改成主编辑区里的编辑窗口 (Sakura 格式, 边搜边追加, 命中词自动高亮, Enter / 双击跳转)。

### 文档
- `docs/` 按 IPA 共通フレーム 工程重排 (00 要件 / 01 基本设计 / 02 详细设计 / 03 测试资料 / 04 评审与管理 / 05 实装与运维 / 06 功能对标), 纳入 Markdown 要件基线、BD / DD、测试资料; 新增代码规范 `CODING-STANDARDS`; 回归脚本存为 `tests/run-all.sh`。

## 0.1.2 — 2026-10-27

### 修复与改善
- **透明主题**: 子菜单不再被父菜单裁掉 / 盖住 (去掉 `#app` 与菜单上的 `backdrop-filter`); 闪光不再被窗口边缘截断; 动态反色去掉每秒截屏轮询 (录屏会闪 / 黑, 开销大), 改为事件驱动 + 限流。
- **Alt 纵向选区**: 键入 / 粘贴 / 输入法覆盖选中的矩形; Backspace / Delete 批量删除; 回车先删内容再换行。
- **折叠**: 书签 / 查找 / 空白 / 差分标记按可见行对位; 折叠范围不数字符串 / 字符 / 行注释里的花括号。
- PPA 宏每一步落地再做下一步。撤销栈淘汰改为最久没用的标签, 锁中毒后继续可用。

### 重构 (行为不变)
- `edit_cmd` 按命令族拆成模块, 一张分发表; `word.rs` / `text_util.rs` / `pure_api.rs` (web 与桌面两个壳共用的端点入口); `search_ops` 拆小; `doc_store` 改名 `doc_mirror`; 前端编辑桥接层拆成 请求 → 调用 → 套结果 → 重画。
- 测试守卫: 前端两份 HTML 一致性、脚本语法检查、陈旧二进制检查、残留 server 检查。

## 0.1.1 — 2026-10-27

### 界面与品牌
- 新增 **retro16 (16-Bit 游戏机)** 主题; 透明水晶主题的材质更通透, 加边角动态闪光; 内嵌 PixelFont 像素字体 (终端设置里可选)。
- 新增 logo / favicon / 应用图标, 欢迎页显示 logo; 网页版新增字体与图标端点。

### 修复
- 历史管理 (履歴の管理) 对话框的函数残留了旧尾部, 多出一个 `}` 使整页脚本无法加载 (合入主题改动时发现)。

### 架构: ARCH-01 余下的编辑逻辑迁到 Rust
- **键盘输入文字**迁到 Rust (`edit_cmd` `insert-text`): 覆盖模式 (Ins) 吃掉光标后同样个数的字符; 有选区则替换选区。
- **矩形(列块)选区**的输入 / 退格 / 回车 / 删除 / 剪切 / 复制 / 替换 / 粘贴迁到 Rust (`box-*`); 差分用例从 9 条加到 23 条, 与真实 Sakura 逐条一致。新增用例暴露并改正了 5 处与 Sakura 不同的旧行为: 在矩形里输入 / 粘贴**不删**矩形原有内容 (插在每行矩形左端, 多行剪贴板只取第一行); 退格 = Delete (删矩形内容, 零宽矩形什么也不删, 解除矩形, 光标回左上角); 回车 = 先删矩形内容再在左上角换行 (只断一行)。
- **多光标**输入 / 退格迁到 Rust (`multi-insert` / `multi-backspace`): 光标按"拍平偏移"计算, 同一行有多个光标时不再互相错位; 退格并行后重合的光标自动去重; 只交光标所在的行 (不连续的几段各出一个补丁), 光标隔多远都不用发中间的行。撤销 / 重载后越界的旧列块 / 旧光标不再保留 (原先会把最后一行当空行写回去); 有选区时输入 = 替换选区, 多余的光标丢掉。
- **折叠 (zc / zo / zM / zR)** 改成非破坏式: 只隐藏显示, 文字不动; 块边界 (花括号配对 / 缩进块) 在 Rust (`fold-range` / `fold-all`)。旧实现把块内文字换成一行占位符, 展开后变成空行 —— 即**丢内容**; 行数变了或光标进了块会自动展开; 缩进块改为"下一个非空行更深才算", 顶格行也能折。
- **文档正文镜像** (`editcore::doc_mirror`): 大文档 (>=1000 行) 的查找 / 置換不再每次把整篇文字发给 Rust, 第一次登记整篇, 之后只交内容指纹 (行数 + 两路哈希); 指纹对不上才补发整篇。核对的是内容而不是版本号, 前端怎么直接改行数组都不会让 Rust 用到过期正文。关闭标签时释放。

## 0.1.0 — 2026-10-26

第一个功能版本。与 0.0.1 相比, 编辑语义改为**以真实 Sakura Editor 为准逐条比对**, 并把大部分编辑逻辑迁到 Rust (`editcore`)。

### 与 Sakura 的对标 (tests/sakura-diff)
- 新增差分比对: 用真实 Sakura 的键盘宏和我们的编辑器执行同一条命令、同一个起点, 比较结果文本 (含光标位置)。目前 225 条用例 224 条一致, 余下 1 条是 Sakura 宏一侧的偶发。覆盖: 单词 / 行删除与剪切、选词、移动 (含日文字符种类)、复制粘贴 (含整行模式 / 矩形)、缩进 / 排序 / 去空白 / 去重 / 全角半角 / かな 转换、查找 / 置換 / 对括弧 / 书签 / 跳行、撤销 / 重做粒度、コード変換。
- 据此修了一批与 Sakura 不一致的行为, 其中有真实 bug: 多行选区复制 / 删除的首行起点算错; 多行粘贴吞掉后面的行; 变换类命令在没有选区时作用于整个文件; 排序从光标列起比较; Ctrl+Delete 误成「删下一行」。
- 单词移动 / 选词 / 词删除按 Sakura 的字符种类分词 (漢字 / ひらがな / カタカナ / 英数 / 全角英数 / 記号 各成一词, 长音与浊点随假名)。
- 撤销: 替换选区算一步, 撤销之后没有选区。

### 架构: 编辑逻辑迁到 Rust (ARCH-01 进行中)
- 经 `/api/*` (网页版) 与 Tauri 命令 (桌面版) 调用的 Rust 模块: 选区变换与整行操作 (`edit_ops`)、光标移动与范围选择 (`nav`)、删除 / 剪切 / 行结构 / 剪贴板 / 选词 (`edit_cmd`, 28 个命令)、对括弧 (`bracket`)、书签 (`bookmark`)、文档内查找 / 置換 (`search_ops`, 使用 `regex`)、撤销 / 重做栈 (`history`, 按「页面会话 + 标签」登记)。均有 Rust 单测, 桌面版在真实 Tauri 里验证。
- 前端新增输入顺序闸门: 经桥的操作未落地时, 后到的按键排队按原顺序重放; 桥调用 8 秒无响应则放弃, 不会卡死输入。
- 仍在前端 JS 的: 字符输入、矩形选择、多光标、折叠, 以及文档本体 (行数组)。
- 正则查找: 改用 Rust `regex` (不支持反向引用 / 环视断言); 出错时说明原因, 并提示查找字面 `*` 要写 `\*` 或 `[*]` (与 Sakura 一致: 勾选「正则」后单独的 `*` 是语法错误)。

### 修复
- **分割**: 取消分割后编辑器一直被隐藏, 再也无法输入; 分割期间也不能输入。现为多视图: 编辑器占当前格, 其余格是同一文档的实时镜像 (各自滚动), 点击镜像即切换到该视图继续输入。
- **终端**: 新建终端后无法正常输入命令 —— 现为本地回显 + 行编辑 (退格 / 方向键 / 历史 / 输入法 / 粘贴 / Ctrl+L), 原来的 Backspace 会往命令里塞一个空格; shell 回显不再重复显示; shell 退出后可再新建终端。
- **终端 / SQL*Plus 乱码**: 管道式 shell 的输入输出走系统 GBK 代码页导致中文报错变成 U+FFFD。现在启动时统一切到 UTF-8, 并按字符边界解码输出 (不再把一个汉字拆成两个替换符)。
- 对括弧 / 括弧强调、搜索标记改为异步计算并缓存。
- 撤销栈: 刷新页面或开第二个窗口后, 不再撤销到别的文档的补丁; 关闭标签时释放后端的撤销栈。
- 请求窗口里缺少选区两端行 (左右键收缩远处选区会落到第 0 列)、反向选区 / 越界光标在 Rust 端下溢。
- 输入法组合中的按键不再被输入闸门拦下。

### 测试
- mock 回归 119 项、UAT 305 项、Rust 单测 (`editcore` 317 项) 全部通过; 新增 菜单命令全量巡检 (每个命令执行后确认无异常且编辑区仍可输入)。

## 0.0.1 之后、0.1.0 之前新增的功能

### 新增
- **Grep 结果窗口**: 结果在独立浮动窗口里分片到达即显示; 搜索未结束可随时「追加」新条件 (新开一节并行), 每节可单独停止。
- **视图**: 工具栏 / 功能键栏 / 标尺 / 空白·TAB 着色标记 / 当前行下划线 / 閲覧モード (只读) / 文件属性 (Alt+Enter) / 窗口一览 / 关闭其它·左·右标签 / 标签左右移 / F5 重绘。
- **设置**: 共通設定 (TAB 宽度、自动缩进、新文档编码与换行、显示开关), **キー割り当て** (可改键/删键/追加/恢复默认, 落盘, 菜单标注同步)。
- **DIFF**: 与另一标签 / 磁盘文件比较, 新增·变更·删除行着色, 差分一览, 上/下一处, 清除。
- **宏**: 保存 / 读取到文件。
- **印刷**: 印刷 / プレビュー (分页) / ページ設定; **ブラウズ** (sandbox 壳页)。
- **编辑**: SPACE インデント, Base64·uudecode 解码保存, 最近のファイル/フォルダー挿入, 次/前の関数・変更行, 保存して閉じる, 閉じて開く, 折返し位置に改行付きコピー, 选区字数统计 (字符/字节/行)。
- **コード変換** 11 命令 (自動判別・EUC・Unicode・UTF-8/7・JIS ↔ SJIS), 由 Rust `editcore::document_io::reinterpret_text` 实现 (web + 桌面)。
- **MIGEMO インクリメンタルサーチ** (前方/後方, Rust 展开候选: ローマ字 → かな/カナ/半角カナ)。
- **矩形选择**: Ctrl+Alt+←→ 单词、Ctrl+Alt+Home/End 文件首尾、Alt(+Ctrl)+PgUp/PgDn 整页/半页。
- **タグ**: tags 文件生成, 指定关键字直接跳转; 履歴の管理; 指定桁折り返し (临时); 最前面 / 最小化 (桌面)。
- **プラグイン UI**: 登记持久化, 对选区/全文运行并替换·插入 (WASM 运行时仍需 `wasm` feature 构建)。
- **大纲**: C++ / JavaScript / TypeScript / Go / Java / C# / Markdown, 层级缩进 (Rust 实现), 点击跳转到定义行。

- **标签分组 / 窗口**: 分组切换·移动·合并·关闭, 纵向/横向最大化, 全部最小化, SQL*Plus, 再変換, 带选区的変更行。
- **大文件**: >3000 行窗口化渲染 (虚拟滚动)。

### 修复
- Vim NORMAL 下 `z` 前缀键被当作文字插入; `a A I o O` 按 Vim 语义 (后移/行尾/首非空白/新行)。
- 半角カナ展开 (旧公式产出错字, 且不拆浊音) — 影响 MIGEMO 候选。
- Alt 矩形选区输入覆盖选中内容; 左抽屉与底部终端按最近展开重排、互不重叠。
- UAT 全部 305 项通过 (原 13 项旧失败均为用例过期或夹具污染, 已更新)。

## 0.0.1 — 2026-10-20

首个对外版本。目标: 做一个 Rust 版的 Sakura Editor 对标编辑器 (Tauri 桌面版 + 网页版共用同一套前端)。
这是**早期预览版**: 与 Sakura 的对标仍有大量缺口 (见 `docs/06_功能对标/SPA-03_v1-gap-design.md`, 268 条默认命令当时约 20% 一致, 本版之后已显著提高)。

### 编辑
- 键盘移动与范围选择: 方向键 / Home / End / PgUp / PgDn, Ctrl+←→ (单词), Ctrl+Home/End, Ctrl+PgUp/PgDn (半页), Ctrl+↑↓ (滚动), 加 Shift = 范围选择, Shift+Ctrl+↑↓ = 选 2 行。
- 选区替换式编辑: 输入 / 回车 / 删除 / 粘贴先替换选区; Tab 缩进多行选区, Shift+Tab 反缩进。
- Ins 插入/覆盖 (状态栏「挿入/上書」); Tab 键插入 TAB 字符 (宽度 4)。
- 鼠标: 点击落光标, 行内/跨行拖选, Shift+点击扩选; **Alt+拖动 = 纵向 (矩形) 选择**, 可拖到行尾右侧, Alt+方向键, Shift+F6。
- Sakura 默认键位表 (集中一张表, 菜单上的快捷键标注由它生成): 单词切り取り/削除、行末まで切り取り、选单词、排序、LTRIM/RTRIM、行合并、F7/F8/F9 剪切复制粘贴、Shift+F9 矩形贴付け、大小写/全半角/かな 转换、TAB↔空白、日付/時刻挿入、移動履歴 (Alt+P/N)、各种复制变体 (CRLF / 全行 / 引用符 / 行番号 / 彩色 HTML / 路径)。
- 日文输入法 (IME): 预编辑文字显示在光标处, 上屏、替换选区、撤销均正确; 组合中按回车不产生换行。
- 系统剪贴板: 复制写系统剪贴板, 粘贴读系统剪贴板。
- Alt 矩形选择后输入 / 退格会覆盖 / 删除选中的矩形内容。
- 左侧导航与底部终端: 后展开的占整列/整行, 先展开的自动重新排版, 互不重叠。
- 回归测试项目 `tests/mock` (真实按键 / 鼠标 / IME / 编码夹具 / 桌面真机用例)。

### 文件与编码 (Rust, `editcore::document_io`)
- 自动判别并保持 BOM / UTF-8 / SJIS / EUC-JP / JIS / UTF-16LE·BE / Latin1 / GBK / Big5 / EUC-KR / CESU-8 / UTF-7, 保存时原样写回。
- 记住每个文档自己的换行 (CRLF / LF / CR, 混用时标记)。
- 目标编码表示不了的字符不再静默替换: 提示后由用户选择仍保存或取消。
- 文件 > 以指定编码重新打开; 设置 > 文字コードセット指定 / 入力改行コード; 状态栏显示编码与换行。

### 查找 / 替换 / 跳转
- 查找: 区分大小写、单词单位、正则、先頭(末尾)から再検索、全部强调显示、検索マーク切替、検索開始位置へ戻る。
- 置換 (一个 / すべて / 选择范围), 正则分组 `$1` / `\1`。
- インクリメンタルサーチ (前方/後方 × 通常/正規表現)。
- 対括弧の検索 + 对括弧强调, ブックマーク (设置/移动/一览/全解除 + 行号栏标记), タグジャンプ / バック (本文件 → 项目搜索), 同名 C/C++ ヘッダーを開く。
- Grep: 修复桌面版显示 `搜索失败: undefined` (结果仍在对话框内显示, 独立结果窗口 + 边搜边显示计划在下个版本)。

### 透明水晶主题
- 桌面窗口启用 Acrylic; 标题栏 / 导航 / 标签 / 终端 / 状态栏全部半透明, 分区使用不同的玻璃材质 (磨砂 / 亚克力 / 蚀刻 / 烟晶 / 彩釉)。
- **每秒采样窗口背后真实透出的颜色**, 文字随之动态反色 (带滞回, 不闪烁)。

### 修复
- 首次启动不再进入 Vim 模式 (直接可打字); 修菜单漏出源码注释文字、设置菜单「主题」行被截断导致菜单键盘导航整片失效、Shift+Tab 抛 ReferenceError。
- 关闭对话框后焦点还给编辑区。

### 已知问题
- Shift+Ctrl+↑↓ 让给 Sakura 的「选 2 行」, 上移/下移行改为 Alt+Shift+↑↓。
- UAT 约 13 个旧失败 (打开文件夹后左抽屉 / 布局尺寸 / 终端 / Grep フォルダー 栏), 见 `tests/uat`。
- 尚未实现: DIFF/ファイル比較、宏保存/读取、共通設定/タイプ別設定多页对话框、键位编辑、プラグイン UI、大部分窗口/标签分组命令。
- 编辑逻辑目前仍在前端 JS 中 (ARCH-01 计划逐步迁移到 Rust)。
