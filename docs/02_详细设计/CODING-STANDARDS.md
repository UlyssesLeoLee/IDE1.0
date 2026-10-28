# 代码规范 (重构后的规则)

> 适用范围: 本仓库全部新增 / 修改的代码。这些规则来自 ARCH-01 之后那一轮重构, 每一条都对应一个**真实出过的问题**或**已经落地的结构**, 不是泛泛的建议。
> 违反规则的改动不要合并; 确实需要例外, 在 PR / 提交说明里写明理由。

## 0. 总原则

1. **语义在 Rust, 前端只搬运和描画。** 任何"这个命令该怎么改文本 / 光标往哪走"的判断都写在 `editcore`, `editor.html` 里不再新增编辑逻辑 (ARCH-01)。
2. **以真实 Sakura 为准。** 能用键盘宏比对的行为, 先写 `tests/sakura-diff` 用例暴露差异, 再改; 不要凭记忆或文档实现 (这一轮矩形输入 / 退格 / 回车就是被实测纠正的)。宏和键盘路径不同的地方记入 `tests/sakura-diff/README.md`。
3. **轻量和速度是产品定位。** 不引入常驻后台线程 / 定时轮询 / 大依赖; 新能力默认零成本 (不用就不占内存、不占 CPU)。见 §8 性能预算。
4. **小步、可验证、可回退。** 重构不改行为 (用快照证明), 行为改动单独提交 (见 §9)。

## 1. 分层与依赖方向

```
editor.html (描画 + 输入搬运)
      │  invoke("命令名", 请求)          ← Tauri 命令 / HTTP /api/*, 两个壳一一对应
      ▼
ide-shell-web / ide-shell-desktop   (传输层: 反序列化、状态码, 不写业务规则)
      │
      ▼
editcore::pure_api                  (类型化入口: 请求结构体 → JSON / 给用户看的错误文字, 只写一遍)
      │
      ▼
editcore 各模块                      (纯计算, 无 UI、无全局可变状态, 除非 §3.4 允许)
```

- 依赖只能**向下**。`editcore` 不依赖壳; 壳不互相依赖; 前端不假设后端内部结构。
- 新增一个无状态端点 = ①在 `editcore` 里写纯函数和请求 / 响应结构体 ②在 `pure_api.rs` 加一个类型化入口 ③web 壳在 `lib.rs` 里用 `respond_pure::<Req>(…)` 一行接上 ④桌面壳加一个 `#[tauri::command]` 一行委托 ⑤前端用 `invoke`。**不要**在两个壳里各写一遍业务逻辑或错误文字。

## 2. 模块与文件

- **一个文件一个职责, 文件首行 `//!` 写清楚它是什么、边界在哪。** 看文件名就知道里面是什么。
- 命令多的模块按**命令族**拆目录, 如 `edit_cmd/{text_ops,line_ops,clipboard,box_ops,multi_cursor,fold}.rs`, 共用的放 `ctx.rs` / `req_res.rs`; **一张分发表**在 `mod.rs` (命令集是封闭的, 用编译器检查的 `match`, 不用 trait 对象 / 动态注册表)。新命令 = 在对应族里加一个函数 + 分发表加一行 + `ALL_COMMANDS` 加名字 (有测试检查两者一致)。
- 参数化变体用**参数**区分, 不要在函数里重新读 `req.cmd` 字符串 (`word_cut(cx, cut: bool)`、`MultiOp::Insert/Backspace`)。
- 共用的小工具放专门的小模块并各有一个领域: 字符下标 `text_util`、Sakura 词模型 `word`。**不要**在新文件里再抄一份 `chars_of` / `slice` / `is_word`。
- 单文件超过 ~500 行 (不含测试) 就该想想是不是该拆; 一个函数超过 ~80 行就该拆成有名字的步骤。
- 命名: Rust 用 `snake_case`; 命令名沿用 `kebab-case` 字符串 (`box-insert`、`fold-all`); Sakura 的 `F_*` / `S_*` 原名只出现在对照表、测试和注释里。

## 3. 请求 / 响应与状态

3.1 **无状态窗口**: 前端只交命令需要的几行 (`rows`, 光标周围、选区各行、多光标所在行) + 光标 / 选区; Rust 返回补丁 (`start/remove/insert` 或 `patches`)、新光标、剪贴板、提示。**不要**把整篇文档塞进每个请求。
3.2 **响应只用构造器**: `EditRes::splice / replace_line / nothing / clip_only / selecting` + 链式 `.tip() .clip() .box_sel()`。**不要**手写满字段的结构体字面量 (加一个字段就要改一圈)。
3.3 **没交全的行 = 什么也不做**: `Ctx::has_row` 先查, 绝不拿 `Ctx::line` 给的空行去改文档 (会把真实内容写没)。
3.4 **全局可变状态只允许两处, 且都要有上限和淘汰**: 撤销栈 `history` (按标签, LRU, 上限)、文档正文镜像 `doc_mirror` (按标签, 上限)。新增任何进程内全局状态要先在 PR 里说明理由、上限和淘汰策略; 锁中毒后要继续可用 (`unwrap_or_else(|e| e.into_inner())`)。
3.5 **内容校验优于版本号**: 缓存 / 镜像用内容指纹核对, 不依赖"所有改动处都通知了我"。
3.6 **异步结果要防过期**: 前端经桥的操作都进 `enqueueInput` 串行; 请求期间用户点了鼠标 (`uiVer`)、桥超时 (`ctx.dead`)、标签已切走, 迟到的结果不能再覆盖用户的光标 / 选区。
3.7 **错误**: 请求本身有问题 = `Err(给用户看的文字)` (HTTP 400); "没有结果"(对括弧没找到、不处理的键) 返回 `{}`, 不算错误。错误文字只在 `pure_api` 里写一遍。

## 4. 前端 (`editor.html`)

- 只有一个文件, 且 `crates/ide-shell-desktop/dist/index.html` 必须**逐字节相同** (`cargo build -p ide-shell-desktop` 会同步; `crates/ide-shell-web/tests/frontend_sync.rs` 守卫)。改完运行 `node tests/uat/check-editor-syntax.mjs`。
- 桥接固定四段: 请求构造 (`editRequest` / `rowsWindow` / `selTuple`) → 调用 → 套结果 (`applyEditResult` / `applyUnchangedResult`) → 重画 (`repaintAfterEdit`)。新命令族只需在 `editFamily` 里登记, 不要另起一套。
- **不新增编辑逻辑**; 只新增界面、输入搬运、渲染。需要新语义 → 先写 Rust。
- **不轮询、不常驻定时器。** 需要"变了才做"就用事件 + 去抖 + 限流 (参考 `requestLuminanceCheck`)。动态反色曾经每秒截一次屏, 录屏会闪、开销大 —— 不要重蹈。
- `position: fixed` 的浮层 (子菜单、对话框) 的祖先**不能**带 `backdrop-filter` / `filter` / `transform` (会变成包含块、被 `overflow` 裁掉); 玻璃效果放在不含浮层的节点上。
- 折叠: 隐藏行用 `display:none` 但保留 DOM 里一行一个 `.hl-row` (光标 / 点击按行下标找 DOM); 任何按像素换算行号的地方要用 `visibleRows`。
- 大文件 (`vwActive`) 走窗口化渲染; 新功能要说明在窗口化下怎么处理 (不支持就要明确提示, 不要静默坏掉)。
- 用户文案走三语 `I18N`; 菜单命令必须三语齐全 (有测试)。

## 5. 测试要求 (一个改动要带什么)

| 改动 | 必须有 |
|---|---|
| Rust 逻辑 | `editcore` 单测, 紧挨着模块 (`#[cfg(test)]`); 边界值 (空文档、越界列、反向选区、缺行) 都要有 |
| 能用键盘宏比对的编辑语义 | `tests/sakura-diff` 用例 (以真实 Sakura 为准); 不能比对的写进 README "比对不了" |
| 前端行为 | `tests/mock/specs/web/*.test.mjs` (Playwright, 真实按键); 涉及桌面壳加桌面用例 |
| 重构 | 行为不变的证明: 命令矩阵快照 (改前改后逐字节相同) + 整套回归 |
| 修 bug | 先写能复现的失败用例, 再修 |

- 测试里**不能**直接断言"调用不报错"当作对标完成; 必须通过真实按键 / 菜单触发并断言结果。
- 异步操作落地之后再取快照 (`window.__editIdle()`); 不要靠 `waitForTimeout` 猜。
- 测试网自己要可信: 起 server 失败 / 超时直接报错并收掉子进程; 启动后核对内嵌页面与 `editor.html` 一致 (没重新编译就报错, `QA_ALLOW_STALE=1` 可跳过); UAT 默认不复用已有 server。
- 提交前至少跑: `cargo test -p editcore`、受影响的 mock 用例; 合并前跑整套 mock + UAT + `sakura-diff` (需要本机装有 Sakura)。

## 6. 文件 / 编码 / 安全

- **写盘一律原子**: 同目录临时文件 + 重命名 (`document_io::atomic_write`); 保存前对有损编码给出明确标志, 不静默替换字符。
- 保存前先备份原文件 (`auto_backup::backup_before_save`); 保存前核对外部改动 (`FileStamp`)。
- 按文件编码读写, 不用 `from_utf8_lossy` 当默认; Grep / 替换都要遵守。
- 路径: 桌面版可访问任意路径, 网页版保持项目根沙箱; 任何新 IO 入口先过路径策略 (`path_safety`)。
- 外部输入 (宏、插件、命令行、剪贴板) 一律当不可信: 校验长度 / 类型, 不拼 shell 命令串。

## 7. 智能化功能与 Agent API

- 编辑器**不内置模型, 不发网络请求**。智能能力 (补全、解释、重构、批量改写……) 由外部编排器 (如 LangGraph) 通过 **Agent API** 调用编辑器的能力; 编辑器负责提供**上下文**和**安全地应用结果** (补丁、撤销一步)。
- 每个可智能化的功能必须有**无 UI 依赖的纯 API** (本规范 §1 的 `pure_api` 入口), 并登记进 `editcore::agent_api` 的工具表: 名字、一句话描述、输入 JSON Schema、读 / 写类别。这样新增一个功能时, Agent 侧自动可见, 不用另写适配。
- 只读工具可以直接调用; **写工具只产出补丁, 不直接改文档**, 由前端按 §3.6 的流程套用 (一步撤销)。
- 零成本: 工具表是静态数据, 不用不占内存、不起线程、不加依赖; Agent API 默认只监听本机并复用现有鉴权。

## 8. 性能预算 (轻量高速的底线)

| 项 | 预算 |
|---|---|
| 按键到屏幕 (单字符输入) | 一次桥往返 + 一次重画, 不做整篇扫描 |
| 请求大小 | 与光标周围窗口成正比, 与文档大小无关 (大文档查找走指纹镜像) |
| 常驻 CPU | 空闲时为 0: 无轮询、无定时器、无动画循环 (闪光动画只在可见且主题需要时) |
| 内存 | 全局状态有上限 + 淘汰 (撤销 200 步 / 512 标签; 正文镜像 64 份) |
| 新依赖 | 先问"标准库 / 已有依赖能不能做"; 加依赖要写明体积和理由 |
| 启动 | 不在启动路径上加同步 IO 或网络 |

任何会改变上表的改动, 提交说明里要写出对比数据。

## 9. 提交与分支

- 一个提交一件事; **重构提交** 与 **行为改动 / bug 修复提交** 分开, 重构提交说明里写"行为不变"和证据。
- 提交信息用中文, 说明"改了什么 + 为什么 (出过什么问题)", 末尾带协作署名行。
- 分支: `dev` 是唯一长期分支, 其余临时分支合入后删除; 并行开发时每个分支登记要改的文件清单且互不重叠, 共用文件 (`lib.rs`、`pure_api.rs`、`editor.html`) 的接线由一个人在合并后统一做。
- 发版: 升版本号 (`Cargo.toml`、`tauri.conf.json`、`Cargo.lock`) + `CHANGELOG.md` + 带注解的 tag。
- 不提交运行产物 (`tests/sakura-diff/report.md`、`.clip.txt`、`uat-cmd-*.txt` 等已在 `.gitignore`)。

## 10. 提交前自查清单

- [ ] 语义在 Rust? 前端没新增编辑逻辑?
- [ ] 新端点走了 `pure_api` + 两个壳各一行? 错误文字只写了一遍?
- [ ] 响应用了构造器? 缺行 / 越界 / 反向选区都处理了?
- [ ] 能和真实 Sakura 比对的, 有 `sakura-diff` 用例?
- [ ] 没有轮询 / 常驻定时器 / 大依赖? 全局状态有上限?
- [ ] `editor.html` 与 `dist/index.html` 一致, 语法检查通过?
- [ ] 新功能登记进 Agent API 工具表 (如果可智能化)?
- [ ] 测试: 单测 + 受影响的 mock; 合并前整套 + sakura-diff?
- [ ] 文档 (`CHANGELOG.md`、本规范、对应 WBS) 同步了?

## 附: 回归脚本 (已存进仓库, 不要每次现写)

- 全量回归: `bash tests/run-all.sh` (Rust 单测 -> 编译 -> mock -> UAT -> Sakura 差分; `--skip-rust` / `--skip-sakura`), 日志在 `$TMPDIR/ide-regress/`。
- 单个 mock 用例: `cd tests/mock && QA_BIN=<ide-shell-web 二进制> node --test specs/web/<name>.test.mjs`。
- 跑的过程中不要改 `editor.html` (测试会检查二进制内嵌页面与源文件一致)。
