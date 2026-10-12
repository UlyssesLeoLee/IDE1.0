# IDE1.0 项目审查 — 改善建议 Feedback

- 审查日期: 2026-10-12
- 审查对象: `main` @ f44479f (178 个受控文件, 7 个 crate, ~27k 行)
- 审查方式: 静态阅读 (目录结构 / Cargo 配置 / CI / 安全相关源码)。**未运行** `cargo test` / clippy / Playwright, 下列"测试数量"等结论以 README 为准, 未复核。
- 严重度: 🔴 高 (应优先处理) · 🟠 中 · 🟡 低

---

## 0. 总评

项目文档体系 (DOC-01~21、DD-01/02、回归报告)、测试分层 (UT/IT/ST/UAT) 与 CI 规模都超出一般同体量项目, `unsafe_code = "forbid"`、路径沙箱的独立测试 (`path_safety.rs`) 也是好习惯。

主要问题集中在三处: **本地 HTTP 服务/终端接口缺少访问控制**、**web 与 desktop 两套代码大量复制**、**仓库卫生 (构建产物入库、文档陈旧、分支堆积)**。

---

## 1. 安全 (优先级最高)

### 🔴 S-1 `ide-shell-web` 全接口开放 CORS `*` 且无任何认证, 含终端与写文件
- 证据: `crates/ide-shell-web/src/lib.rs:184` 响应头固定 `Access-Control-Allow-Origin: *`; 路由含 `/api/terminal_create`、`/api/terminal_input`、`/api/write_file`、`/api/scm_commit` (约 276–440、708 行), 没有 token / Origin / Host 校验。
- 风险: 服务虽绑定 `127.0.0.1`, 但用户浏览器里打开的**任意网页**都能向 `http://127.0.0.1:8123` 发请求 (CORS `*` 允许读响应; POST 简单请求即使不读也会执行), 进而创建 shell 并输入命令 = 本机远程代码执行。DNS rebinding 也能绕过"仅本机"假设。
- 建议:
  1. 启动时生成随机 token, 所有 `/api/*` 要求 `Authorization`/自定义头; 页面通过启动 URL 带入。
  2. 去掉 `Allow-Origin: *`; 校验 `Host` 与 `Origin` 白名单 (`127.0.0.1:端口`)。
  3. 终端/写文件类危险接口默认关闭, 需显式 `--enable-terminal`。
- 同类问题: `ide-shell-protocol` 的 `/rpc`、`/ws`、`/rpcws` (`bin/ide-shell-protocol-server.rs:~125`) 同样无认证, 请一并处理。

### 🔴 S-2 终端 `shell` / `cwd` 参数由前端任意指定
- 证据: `ide-shell-desktop/src/terminal.rs:62-92` — `Command::new(&shell_cmd)` 直接使用前端传入的可执行文件名, `cwd` 不受项目根限制。
- 建议: `shell` 改为枚举白名单 (powershell/cmd/bash/zsh); `cwd` 走与 `list_dir` 相同的 `resolve_in_project` 校验。

### 🟠 S-3 Tauri `csp: null`
- 证据: `ide-shell-desktop/tauri.conf.json` → `"security": {"csp": null}`; 前端 `dist/index.html` 约 2300 行内联脚本。
- 风险: 一旦渲染的文件内容/文件名/终端输出发生 XSS, 即可直接调用全部 `invoke` 命令 (读写文件、终端)。
- 建议: 设置严格 CSP (`default-src 'self'`; 脚本改外置文件后去掉 `unsafe-inline`); 同时核对所有 `innerHTML` 拼接处是否转义。

### 🟠 S-4 `scm_*` 的 `paths` 未经沙箱校验; `scm_commit` 无防护
- 证据: `scm.rs:137` `stage()` 直接把前端路径传给 `git add --`。`--` 已防止参数注入, 但路径不会被限制在项目根内。
- 建议: 复用 `resolve_in_project_pub` 校验后再传给 git。

### 🟠 S-5 `write_file` 缺少大小上限与原子写
- 证据: `read_file` 有 4MB 上限, `write_file_pub` 无上限, 直接 `fs::write` (崩溃时可能留下半截文件)。
- 建议: 加上限; 写临时文件后 rename。另需补测"目标文件尚不存在 / 符号链接指向根外"的行为 (`safe_canonicalize` 对不存在路径的处理我未验证)。

### 🟡 S-6 工具链/依赖
- `aci-emitter` 以 git rev 依赖, 无 `cargo audit` / `cargo deny` 步骤; 建议 CI 增加 `cargo audit`。
- `rust-toolchain.toml` 与 CI 均为 `stable` 浮动, 而 `rust-version = "1.77"`; 建议固定一个具体版本并另设 MSRV job, 否则 `-D warnings` 会在新版 clippy 出现时无故变红。

---

## 2. 架构与代码质量

### 🟠 A-1 web / desktop 两套代码大量复制
- `ide-shell-web` 与 `ide-shell-desktop` 都有 `scm.rs`、`search.rs`、`terminal.rs`, 彼此仍有 80 / 320 / 361 行差异 (diff 行数), 说明已经开始**漂移**; 前端 `desktop/dist/index.html` (2316 行) 与 `web/src/editor.html` (1410 行) 已明显分叉。`ide-shell-protocol` 又是第三套 RPC 面。
- 建议: 抽出 `ide-shell-services` (scm / search / terminal / 沙箱) 库 crate, 三个外壳只做传输适配; 前端单一来源 + 构建时复制, 而非手工同步。S-1~S-5 修一次即可全覆盖。

### 🟠 A-2 手写 HTTP 服务器
- 证据: `ide-shell-web/src/lib.rs:198-760` 用 `TcpListener` 手写解析; `serve()` 在**单线程里顺序处理**连接 (`for stream in incoming { handle_request }`), 一个慢连接 (或 `/api/terminal_output` 轮询) 会阻塞所有人; 请求头无大小/超时限制, `Content-Length` 未设上限 (`vec![0u8; content_length]` 可被构造为超大分配); `try_clone().unwrap()`。
- 建议: 工作区已依赖 axum/tokio, 直接迁移到 axum 并复用 `ide-shell-protocol`, 一并获得超时、体积限制、路由与中间件 (认证/Origin 校验放中间件)。

### 🟠 A-3 `unwrap/expect` 偏多
- `crates/*/src` 中 `unwrap()/expect(`/shell 相关命中约 376 处 (含测试代码, 未剔除)。库/服务路径上的 `unwrap` 建议逐步换成错误返回, 并启用 `clippy::unwrap_used` 仅对非测试代码告警。
- 终端/PTY 使用 piped stdio 而非真正 PTY, 交互程序 (vim、ssh、颜色、窗口大小) 不可用; 如要对标"PowerShell 替代", 建议评估 `portable-pty`。

### 🟡 A-4 `ide-shell/src/app.rs` 兜底执行任意命令
- 行 235-240 对未知命令 `cmd /C` / `sh -c` 执行。作为 shell 属设计意图, 但应在文档中明确, 且在 AI 建议 (`ai_suggestion`) 接入真实模型后, **禁止 AI 输出未经确认直接执行**。

### 🟡 A-5 版本与元数据不一致
- workspace 版本 `0.1.0`, `tauri.conf.json` 为 `0.1.9`, 产物 MSI 名为 `0.1.0`; `homepage` 指向某个 agent 分支 README (`agent/minimaxm3/ulys-191-2`); `authors` 字段是内部流程描述而非作者。建议统一版本来源 (workspace 版本 → tauri 读取) 并整理元数据。

---

## 3. 仓库卫生

### 🔴 R-1 构建产物与生成文件被提交, 且与 `.gitignore` 自相矛盾
- 已跟踪但 `.gitignore` 明确忽略: `dist/IDE1.0 IDE Shell_0.1.0_x64_en-US.msi` (~3MB 二进制)、`crates/ide-shell-desktop/gen/schemas/*` (两个 2315 行 JSON)、`crates/ide-shell-desktop/dist/index.html`。
- 影响: 仓库膨胀 (每次重打包新增一份二进制历史), 评审噪音, 并与 A-1 的前端分叉直接相关。
- 建议: `git rm --cached` 这些文件; 安装包改发布到 GitHub Releases; 前端在 `build.rs`/脚本中由单一来源复制生成。
- 注意: 当前工作区还有未入库的 `tests/uat/node_modules` (19MB), 已被忽略, 无需处理。

### 🟠 R-2 分支堆积
- 本地与远端有 `agent/*`、`dev`、`dev-3`、`backup/mvp1-dev-20261004`、`test/...` 等十余个分支。建议合并后删除已完成分支, 保留 `main` + 短命特性分支, `backup/*` 改用 tag。

### 🟠 R-3 根目录杂乱
- 根目录混有 `req.jsonl`(RPC 样例)、`refs/`、`_upstream/`、两份 DD 文档、`.hermes/`、`.mock-cluster.json`。建议: DD-01/02 移入 `docs/design/`; `req.jsonl` 移入 `examples/`; `_upstream`/`refs` 在 README 说明用途与来源; `.hermes` 如是个人 agent 计划, 加入 `.gitignore` 或移到 `docs/plans/`。

---

## 4. 测试与 CI

### 🟠 T-1 README 数据已陈旧, 与实际不符
- README §0 称 "4 crates / 61 个 Rust 测试 / 9 jobs / PR #9 OPEN", 而 workspace 现有 7 个 crate (含 `sakura-rs`、`ide-shell-desktop`、`ide-shell-protocol`), 并有 `sakura_features_104.rs` 等大量新增测试; 阶段表仍停留在 2026-10-03。
- 建议: 不再手写数字, 改为 CI 徽章 + 由脚本在 CI 生成的测试统计 (`DOC-18_metrics_report` 同理)。

### 🟠 T-2 CI 仅 ubuntu, 但产品主打 Windows
- CI 注释写明"仅 ubuntu (节省分钟)", 而产品是 Windows MSI / PowerShell 替代、代码中有 `#[cfg(windows)]` 分支与 `cmd /C` 路径。建议至少增加一个 `windows-latest` 的 `cargo test` + Tauri build job (可只在 main/发布前跑)。
- 路径沙箱涉及 Windows verbatim 前缀 (`strip_verbatim`)、大小写与 8.3 短名, 建议在 Windows 上补充专项测试。

### 🟡 T-3 缺少针对安全修复的回归测试
- 修复 S-1/S-2 时请同步添加: 无 token → 401; 非白名单 Origin → 403; `shell=任意路径` → 拒绝; `cwd` 越界 → 拒绝。

### 🟡 T-4 Playwright 40 例走 web 外壳, 未覆盖 desktop 外壳
- 实际交付物是 Tauri 桌面版, 其前端 (`dist/index.html`) 与被测的 `editor.html` 已不同。建议让 UAT 对**同一份前端**运行, 或增加 Tauri WebDriver 冒烟用例。

---

## 5. 文档

- 🟡 D-1 `docs/DOC-01~21` 为流程模板化文档, 请核对其内容是否与现状同步 (如 `DOC-13_progress_report`、`DOC-18_metrics_report`、`DOC-14_risk_register`); **本次未逐份阅读**。建议风险登记册中新增 S-1~S-5。
- 🟡 D-2 文档语言为中/日/英混用 (DD 为日文, DOC 为中文, 代码注释中文): 如面向外部贡献者, 建议在 `docs/README.md` 约定各类文档语言, 并提供英文版 README 摘要。
- 🟡 D-3 `sakura-rs/PARITY.md` 的"✅/🟡"状态为手写, 建议由测试名自动校验 (表中已引用测试名, 适合脚本核对)。

---

## 6. 建议执行顺序

| 阶段 | 事项 | 工作量 |
|---|---|---|
| 立刻 | S-1 token + Origin/Host 校验 (web + protocol); S-2 终端白名单 | 0.5–1 天 |
| 本周 | R-1 移除入库产物; S-3 CSP; S-4/S-5 沙箱与写入加固; T-3 回归测试 | 1–2 天 |
| 本月 | A-1 抽 services crate + 单一前端; A-2 web 迁移 axum; T-2 Windows CI; T-1 README 去数字 | 3–5 天 |
| 持续 | R-2/R-3 整理; A-3 unwrap 清理; S-6 cargo audit / 固定工具链 | 穿插 |

---

## 7. 未覆盖 / 需后续核实

- 未运行任何构建与测试, 未验证 CI 当前是否全绿。
- 未审阅: `sakura-rs` 各模块细节、`js_macro.rs`/`plugin.rs` 的脚本执行边界 (若执行用户提供的 JS/PPA 宏, 需单独做沙箱审查)、`ide-shell-protocol` 的 WebSocket 处理、前端 HTML/JS 的 XSS 面、DOC-01~21 内容。
- `safe_canonicalize` 对不存在文件 / 符号链接 / Windows 特殊路径的行为仅看了调用方, 未读实现。
