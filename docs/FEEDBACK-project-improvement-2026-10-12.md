# IDE1.0 项目改善建议 Feedback

> 分析日期: 2026-10-12 · 分析对象: `main` @ f44479f · 范围: 7 个 crate (79 个 .rs / ≈1.7 万行)、CI、文档、仓库卫生
> 方法: 静态阅读关键代码 (web/desktop/protocol 三个 shell、terminal/scm/search、沙箱、CI、Cargo 配置、文档)。
> **局限**: 本次会话的 shell 中找不到 `cargo`,**未实际运行 build/test/clippy**。文中测试数量、CI 状态均来自 README 与源码,未经本次验证;未逐行审阅 `sakura-rs` 全部模块。

---

## 0. 总体评价

项目工程纪律良好: `unsafe_code = "forbid"`、CI 开启 `-D warnings`、四层测试矩阵 (UT/IT/ST/UAT)、路径沙箱有专门测试、sakura-rs 有对照 PARITY 表。主要问题集中在三类:

1. **安全边界**: web shell / protocol server 暴露「任意命令执行」且无任何认证 (P0)。
2. **"内核"名不副实**: 品牌是 microkernel,`ide-kernel-core` 实际只有 `version()` 和一个恒返回 `"placeholder"` 的函数;真正的编辑能力在 `sakura-rs`,却没有接入 TUI shell。
3. **文档/仓库漂移**: README 数字过期、21 份 DOC 多为模板、二进制与多余分支入库。

优先级: **P0** 立即修 · **P1** 近期 · **P2** 中期 · **P3** 机会性。

---

## 1. 安全 (P0)

### 1.1 Web shell 无认证 + `Access-Control-Allow-Origin: *` = 任意网页可在本机执行命令 (P0)
- 位置: `crates/ide-shell-web/src/lib.rs:182-190` (`send_response` 对所有响应写 `Allow-Origin: *`)、`:276` (`/api/terminal_create`,`shell` 与 `cwd` 取自请求 JSON 直接 `Command::new(shell)`),`crates/ide-shell-web/src/terminal.rs:59`。
- 攻击面: 服务虽然绑定 `127.0.0.1:8123`,但用户浏览器中任何恶意网页都能 `fetch("http://127.0.0.1:8123/api/terminal_create", {method:"POST", ...})`。Content-Type 用 `text/plain` 可免预检,且 CORS 为 `*`,响应也可读;随后 `/api/terminal_input` 写入命令即 RCE。另外存在 DNS rebinding 风险 (不校验 `Host`)。
- 同类问题: `/api/open_project` 接受任意路径并把它设为「沙箱根」,沙箱因此可被调用方自行放宽到 `C:\` (`lib.rs` 的 `open_project` 路由) —— 沙箱的信任边界等于没有。
- 建议:
  1. 启动时生成随机 token,要求 `Authorization: Bearer` / 自定义头;自定义头会强制 CORS 预检。
  2. 删除 `Allow-Origin: *`;只允许同源,或白名单明确的 origin。
  3. 校验 `Host` 头必须是 `127.0.0.1:<port>` / `localhost:<port>`,防 DNS rebinding。
  4. `terminal_create` 的 `shell` 参数改为枚举白名单 (powershell/cmd/bash/zsh),不接受任意可执行路径。
  5. `open_project` 在 web 模式下只允许启动参数指定的根,或必须经过本机交互确认。
  6. 如果该 web 模式仅用于 Playwright UAT,考虑用 feature flag `--features uat-server` 隔离,不放进发布产物。

### 1.2 `ide-shell-protocol-server` 同样无认证 (P0/P1)
- `crates/ide-shell-protocol/src/bin/ide-shell-protocol-server.rs:120-131` 暴露 `/rpc`、`/ws`、`/rpcws`。默认绑 127.0.0.1,但 `--addr` 可被设为 `0.0.0.0` 且没有任何警告/认证。
- 建议: 非 loopback 绑定时拒绝启动,除非显式 `--insecure-allow-remote` 并配 token;WebSocket 升级时校验 `Origin`。引入的 `tower-http` 已含 `cors` feature,可直接加 `CorsLayer` 白名单。

### 1.3 Tauri 配置过宽 (P1)
- `tauri.conf.json`: `"csp": null`(关闭 CSP)。桌面端提供 `write_file` / 终端 spawn,一旦 webview 有 XSS (例如渲染 markdown/wiki/文件名),就是本机代码执行。建议设置严格 CSP (`default-src 'self'`),并审计所有 `innerHTML` 使用。
- `bundle.targets: "all"` 会尝试构建所有平台包格式,建议收窄到实际交付的 (msi/nsis)。

### 1.4 命令/参数注入的次要面 (P1)
- `scm.rs` 的 `git add -- <paths>` 已用 `--` 隔离,这是好的;但 `stage`/`unstage`/`commit` 的路径未经项目根沙箱检查,可传入 `../..`。统一走 `resolve_in_project`。
- `search.rs` 调用 `rg` 时 `include_globs/exclude_globs` 直接成为参数,`pattern` 作为最后一个位置参数: 若 pattern 以 `-` 开头会被当成选项 (参数注入,如 `--pre=<cmd>` 可使 rg 执行预处理程序)。修复: 在 pattern 前加 `--`(或用 `-e pattern`)。
- 另外 exclude 实现为 `--glob X --invert-match`,语义是反转匹配行而非排除文件,疑似逻辑错误,应使用 `--glob '!X'`。

### 1.5 沙箱实现重复且有"补丁式"逻辑 (P1)
- `path_safety.rs` 注释写明"复制自 desktop,改动时两边同步";其中 Windows 8.3 回退逻辑用 `replace("~","")` 比较路径字符串 —— 这会让 `C:\PROGRA~1\x` 与 `C:\PROGRA1\x`/其它目录误等价,是弱化沙箱的潜在洞。
- 建议: 抽出独立 crate (如 `ide-path-safety`) 唯一实现;Windows 用 `dunce::canonicalize` 或 `GetLongPathNameW` 规范化,删除 `~` 字符串替换。补充符号链接/junction 逃逸、`write_file` 新建文件经父目录 symlink 的测试 (`safe_canonicalize` 对不存在路径只 canonicalize 祖先,存在 TOCTOU 窗口,可接受但应在文档中说明)。

### 1.6 DoS / 健壮性 (P1)
- `ide-shell-web` 是手写的**单线程**阻塞 HTTP server: `serve()` 循环里逐连接同步处理。任何一个半开连接 (只发请求行不发完头部) 会永久阻塞所有请求;没有读超时。
- `Content-Length` 直接 `vec![0u8; content_length]` —— 恶意头部可一次申请数 GB 内存。无上限。
- 建议: 直接迁移到 workspace 已依赖的 `axum`(协议 crate 已在用),或至少加 `set_read_timeout`、body 上限 (如 8 MB)、线程池。手写 HTTP 解析 (没有分块编码、没有 header 大小限制) 不值得继续维护。

---

## 2. 架构 (P1/P2)

### 2.1 `ide-kernel-core` 仍是占位 (P1)
- 全部代码是 `version()` 与 `kernel_status() -> "placeholder"`,但被 5 个 crate 依赖,README/DD-01~04 设计书有 microkernel / session / transaction buffer / plugin loader 的宏大叙述。
- 风险: 设计文档(DD-01 ≈ 7.7 万字符、DD-02 ≈ 10 万字符)与实现差距巨大,读者会误以为已有内核能力。
- 建议: 二选一并写入 decision log —— (a) 把 `sakura-rs` 中通用的 Buffer/Undo/Session 事务能力上提为 kernel 的真实 API,让 TUI/Web/Desktop 三个 shell 共用;(b) 如短期不做,把 crate 与文档状态明确标为 "experimental/stub",README 首屏写清实际交付范围。

### 2.2 编辑引擎未接入 TUI (P1)
- `ide-shell` 的 `app.rs` 只有一个 buffer 字符串 + Vim 三态 (README 测试里 `f.buffer == "abc"`),而 `sakura-rs` 已有完整 Buffer/Undo/Cursor/Grep/Encoding/FileLock。依赖图显示 `sakura-rs` 仅被 `desktop` 与 `web` 的 outline 功能使用 (Cargo.toml 注释"outline parser")。
- 结果: 同一产品「三外观」编辑语义不一致,sakura-rs 的数百个测试保护的是一个没有被产品主路径使用的库。
- 建议: 以 `sakura-rs::Buffer` 为单一事实来源重构 `App`,三个 shell 只做渲染;这是项目最高价值的架构整合。

### 2.3 三份 shell 后端大量重复 (P2)
- `ide-shell-web` 与 `ide-shell-desktop` 各自拥有一份 `scm.rs / search.rs / outline.rs / terminal.rs`,以及 `list_dir/read_file/write_file`。已出现分叉 (web 的 terminal 是 HTTP 轮询,desktop 是事件推送;web 的 `outline` 路由甚至注释"sandbox 的 `\\?\` prefix 已知 bug,所以绕过沙箱直接读")。
- 建议: 抽 `ide-shell-backend` crate 承载与传输无关的逻辑 (scm/search/outline/terminal/fs),Tauri command 与 HTTP route 只做薄适配。同时顺带消除 1.5 的沙箱复制。
- `outline` 路由绕过沙箱读文件应一并修掉 —— 这是任意文件读取。

### 2.4 全局可变状态 (P2)
- `terminal.rs` 使用 `static SESSIONS: Lazy<Mutex<HashMap>>` + 全局原子 COUNTER,到处 `.lock().unwrap()`。进程内无法多实例,测试之间互相污染,且一旦某线程在持锁时 panic 会让后续所有请求 panic (poison)。
- 建议: 把 session 表放进显式 `TerminalManager` 结构,由 app state 持有;锁中毒用 `unwrap_or_else(PoisonError::into_inner)` 或换 `parking_lot::Mutex`。
- 终端子进程使用管道而非 PTY: 交互式程序 (vim、ssh、REPL、颜色/光标控制) 无法正常工作。作为 "IDE 终端" 建议改用 `portable-pty`。

### 2.5 `.unwrap()/.expect()` 密度 (P2)
- `crates/**/src` 中 `unwrap/expect/lock()` 共约 237 处 (含测试代码)。多数在测试内可接受;生产路径 (`terminal.rs: child.stdin.take().unwrap()`、各处 `SESSIONS.lock().unwrap()`、`state.project_root.lock().unwrap()`) 建议改为返回 `Result`。
- 建议在 workspace lints 中加 `clippy::unwrap_used = "warn"`,并对 `#[cfg(test)]` 豁免。

---

## 3. 代码质量与可维护性 (P2)

- **大文件**: `ide-shell-protocol/src/lib.rs` 1176 行、`sakura-rs/src/types.rs` 988 行、`ide-shell-web/src/lib.rs` 838 行 (含 HTTP 解析、路由、状态、测试混在一起)。建议拆模块: `server/{http,routes,state}.rs`。
- **路由用字符串元组 match** 且每个路由重复「取 root → 解析 JSON → 构造响应」样板,约 15 处。迁移到 axum 后可用 extractor 消除。
- **JSON 容错吞错**: 大量 `serde_json::from_slice(&body).unwrap_or_default()` 与 `unwrap_or("")`,畸形请求被静默当成空参数处理,难排查。应返回 400 并带错误信息。
- **`#![allow(dead_code)]`** 位于 `ide-shell-web/src/terminal.rs` 文件头,会掩盖真正的死代码;改成针对具体项的 allow,或删除未使用代码。
- **`missing_docs = "allow"`**: 对库 crate (`sakura-rs`、`ide-shell-protocol`) 建议改 `warn`,配合 `cargo doc` CI 步骤。
- **版本号三处不同步**: workspace `0.1.0`、`tauri.conf.json` `0.1.9`、README 阶段号。建议 tauri 版本读 `Cargo.toml` (Tauri 2 支持 `"version": "../Cargo.toml"`),避免漂移。
- **MSRV 声明不可验证**: `rust-version = "1.77"`,而 `rust-toolchain.toml` 用 `stable`,CI 也只跑 stable;README 称本机 rustc 1.98.1。1.77 很可能早已被依赖 (tauri 2.x、axum 0.7、clap) 打破。要么在 CI 加一个 MSRV job,要么删除该声明。
- **注释风格**: 注释里混有流程性标记 ("守门 #7"、"ULYS-191 §4.2.2"、"per brief"),对新读者是噪音。内部追踪编号保留在文档/commit,代码注释只讲 why。

---

## 4. 测试与 CI (P1/P2)

- 现有矩阵很全 (README 称 UT 61 + Python 26 + ST 4 + UAT 40),但 README 数字停在早期;之后新增的 sakura-rs 测试 (`sakura_features_104.rs`、`sakura_official_parity.rs`)、desktop e2e 等没有被统计。→ 用 CI 产出的真实数字替换手写数字,或删除这类易过期的数字。
- **缺少覆盖率门禁**: 有 `test/ulys-191-21-coverage-up` 分支但 CI 无 `cargo llvm-cov`。建议设最低阈值并只对新增代码卡线。
- **缺少安全回归测试**: 沙箱有测试,但没有针对 1.1 的「跨域 POST 创建终端」「畸形/超大 Content-Length」「rg 选项注入」「symlink/junction 逃逸」的测试。修复 P0 时同步补上。
- **缺少依赖审计**: 加 `cargo deny check` / `cargo audit` job;`aci-emitter` 通过 git `rev` 依赖外部仓库,供应链上建议 vendor 或发布到 registry,并 pin 校验。
- **CI 仅 ubuntu**,但产品主打 Windows (msi、PowerShell、Windows 路径/8.3/`\\?\` 处理、`creation_flags`)。最易出 bug 的 Windows 路径逻辑恰好没在 CI 上跑。→ 至少对 `ide-shell-web / desktop` 的测试加 `windows-latest` job。
- **Playwright UAT** 依赖单个 server 实例并用 `/api/reset` 隔离,这使测试不能并行、且 reset 会清空项目根;可为每个 worker 起独立端口。
- **Python mock 测试** (`test_ide1_mock_switch*.py`、`scripts/_lib_mock_switch_ide1.py`) 属于早期阶段的 mock 切换演练,与当前产品关联弱;评估是否仍需保留在主 CI 的关键路径。
- 引入 `cargo-nextest` 与 `Swatinem/rust-cache` (已用) 可缩短 CI。

---

## 5. sakura-rs (P2)

- 对标 sakura-editor 的 PARITY 表很好,但统计显示仍有 1 个 ❌、1 个 🚧、5 个 🟡 —— 其中「矩形选择/粘贴」只是部分实现。建议把 PARITY 表变成**由测试生成**的产物 (每行绑定测试名),CI 校验表与测试一致,避免「计划」状态长期挂着。
- `js_macro.rs` 命名为 JS 宏引擎;须确认它**不**执行任意 JS/系统命令 (实现看起来是自定义解释器,但未逐行审阅)。若将来接入真实 JS 运行时,应沙箱化并禁用文件/进程 API,与 `plugin.rs` 的能力模型一起写入威胁模型。
- `file_lock.rs`、`auto_backup.rs`、`cursor_persistence.rs` 涉及文件系统写入 `unwrap/expect` 最密集 (13/12/18 处,含测试)。确认生产路径对「磁盘满、权限不足、网络盘」返回错误而非 panic,并确保备份/锁文件的**原子写入** (写临时文件 + rename)。
- `encoding_io.rs`: 验证对 Shift-JIS/EUC-JP/UTF-16 BOM 与非法序列的无损往返 (读后写回不改字节),这是日语编辑器的核心承诺,建议加属性测试 (`proptest`)。
- `types.rs` 988 行单文件,建议按领域拆分。

---

## 6. 文档与流程 (P2)

- **README 过载且过期**: 21k 字符,头部「Stage 3.0 最小骨架」「PR #9 本笔 🟡 OPEN」「4 crates」与现状 (7 crates、桌面端 0.1.9、玻璃主题) 不符。建议 README 压缩为: 一句话定位 / 当前实际能力 / 快速开始 / 架构图 / 链接。阶段流水账移至 `docs/DOC-13` 或 CHANGELOG。
- **DOC-01~21**: 抽查 `DOC-13` 仍是章节骨架 (只有 "DOC-13.A 進捗測定" 标题),状态 Draft。21 份 IPA 共通フレーム 文档约 3560 行,多数是模板。若无人维护,过时文档比没有更糟。建议: 只保留与代码同步的 (要件/设计/测试/决策日志),其它标记 `Draft - not maintained` 或移到 `docs/archive/`。
- **多语言混杂** (中/日/英在同一文件、同一表格内),并有 `GitRecon` 类流程关键词。至少在 README 提供统一的主语言和简明英文摘要,便于外部贡献者。
- **缺少** `CONTRIBUTING.md`、`SECURITY.md`(应写明漏洞报告渠道与"web shell 仅供本机开发/测试"的警告)、`CHANGELOG.md`、威胁模型文档。
- `Cargo.toml` 的 `homepage` 指向某个 agent 分支的 README (`agent/minimaxm3/ulys-191-2`),分支删除后链接失效 → 改为 main 或仓库根。
- `authors` 字段包含长段流程说明 (含"per 守门 #10…授权"),应改为简洁的姓名/邮箱。
- LICENSE 约 35KB (看似 GPL-3 全文) 而 `Cargo.toml` 声明 `MIT OR Apache-2.0` —— **许可证声明与文件不一致**,务必核对: 若 LICENSE 是 GPL,则与 Cargo 元数据冲突;若依赖 sakura 代码移植则更需确认许可兼容性 (sakura-editor 为 zlib 风格许可)。(此条未看全文确认,请人工核对。)

---

## 7. 仓库卫生 (P2/P3)

- **二进制入库**: `dist/IDE1.0 IDE Shell_0.1.0_x64_en-US.msi` (3 MB) 已被 git 跟踪,.git 已 37 MB。每次发布都会膨胀历史。→ 移除并改用 GitHub Releases,必要时 `git filter-repo` 清理历史;且 msi 版本 0.1.0 与 tauri.conf 0.1.9 不一致,实际已是旧产物。
- **临时/调试文件入库**: 根目录 `req.jsonl` (一条 JSON-RPC 示例)、`.mock-cluster.json`、`.aci.json`、`.hermes/plans/*`(agent 计划文件)。→ 移到 `examples/` 或加 `.gitignore`。
- **生成物混入**: `crates/ide-shell-desktop/gen/schemas/*` 在 `.gitignore` 中声明忽略 (`crates/ide-shell-desktop/gen/`),但却出现在 `git ls-files`,说明是在加 ignore 之前提交的 → `git rm --cached`。同理 `crates/ide-shell-desktop/dist/index.html`、`icons` 若是构建产物需确认。
- **分支蔓延**: 本地/远端有 `agent/*`、`dev`、`dev-3`、`backup/mvp1-dev-20261004`、`test/...` 等十余个分支;最近的提交是 "merge(dev) → main"。建议约定:短命 feature 分支合并后删除;`backup/*` 改用 tag。
- `_upstream/` 与 `refs/` 存放上游规格书全文 (中日文 .md),与 `docs/` 并列容易混淆;加一个 README 说明它们是只读参考资料。
- `.markdownlint.json` 关掉了整类规则 (MD060) 以消除噪音 —— 版本漂移应固定 markdownlint 版本,而不是关规则。
- `.gitignore` 缺少 `node_modules/` 的顶层覆盖 (目前只有 `tests/uat/` 和 `tools/`),建议通配 `**/node_modules/`。

---

## 8. 产品/体验 (P3)

- AI bridge 目前是 `PlaceholderAi` (回显)。如果这是「AI Native」卖点,需定义 `AiProvider` 的实际接口: 流式输出、取消、上下文预算、凭据存储 (使用 OS keyring,不落盘明文)、以及发送到外部模型前的**用户同意与脱敏**。
- 终端: 参见 2.4 (PTY、窗口 resize、ANSI 渲染、输出缓冲上限 —— 目前 `VecDeque<String>` 无界增长,长输出会吃内存,需要环形缓冲 + 背压)。
- 桌面端 `read_file` 4 MB 上限且只读 UTF-8:提供「二进制/编码/大文件」的友好提示 (项目号称日语编辑器对标,应复用 `encoding_io`)。
- 三态主题 (浅色/黑曜石/玻璃) 与原生透明窗口 (`transparent: true`) 在部分 Windows/GPU 组合上有渲染问题,建议提供「禁用透明」设置与 fallback,并在无障碍层面验证对比度。

---

## 9. 建议的落地顺序

| 阶段 | 事项 | 对应章节 |
|---|---|---|
| **本周 (P0)** | web shell 加 token + 去掉 CORS `*` + Host 校验;`shell` 参数白名单;`open_project` 限制 | 1.1 |
| | protocol server 非 loopback 拒绝启动 / 加 Origin 校验 | 1.2 |
| | `rg` 参数加 `--`,修 exclude 语义;scm 路径走沙箱;`outline` 路由走沙箱 | 1.4, 2.3 |
| **2 周内 (P1)** | HTTP 读超时 + body 上限 (或迁移 axum);Tauri 设 CSP | 1.3, 1.6 |
| | CI 加 windows job、`cargo deny/audit`、安全回归测试 | 4 |
| | 确认 LICENSE 与 Cargo 声明一致 | 6 |
| | 决定 kernel 的去向 (实装或标记 stub);移除入库 msi 与生成物 | 2.1, 7 |
| **1-2 月 (P2)** | 抽 `ide-shell-backend` + 唯一沙箱实现;`TerminalManager` + PTY | 1.5, 2.3, 2.4 |
| | 让 `ide-shell` 复用 `sakura-rs::Buffer` | 2.2 |
| | README 瘦身、文档归档、CHANGELOG/SECURITY | 6 |
| **持续 (P3)** | PARITY 表自动化、proptest、覆盖率门禁、AI 真实接入 | 4, 5, 8 |

---

## 10. 本次分析未覆盖 / 需要验证

- 未运行 `cargo build/test/clippy/fmt`,也未执行 Playwright;所有「测试全绿」结论沿用 README。
- 未审阅: `ide-shell-protocol/src/lib.rs` 的 RPC 方法授权细节、`ide-shell/src/render`、前端 `editor.html/index.html/syntax.js` (XSS 风险需专项检查)、`sakura-rs` 中除 PARITY/js_macro 外的多数模块、`tests/uat/specs`。
- 许可证、`js_macro` 能力范围、`outline` 路由绕过沙箱的实际可利用性,建议人工二次确认后再按 P0 处理。
