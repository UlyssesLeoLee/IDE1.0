# SPA-03 — v1「与 Sakura Editor 一致」差距设计报告

> - **文档 ID:** SPA-03
> - **日期:** 2026-10-19
> - **基线:** 本仓库 `6bcffbc` (dev, 代码测量) + `4ccc748` (ARCH-01 / WBS-01 入库, 仅文档) / 上游 sakura `628de79` (本地参考 clone `E:/Temp/sakura-ref/sakura`)
> - **关联:** [ARCH-01 纯 Rust 化 ADR](ARCH-01_pure-rust-migration.md) · [SPA-01 Grep/列块规格](SPA-01_parity.md) · [SPA-02 缺口清点](SPA-02_coverage.md) · [WBS-01 未实现项 WBS](WBS-01_feature-parity.md)(本文只引用不修改)
> - **前提:** 本文的实现位置一律遵循 ARCH-01 (2026-10-18 用户决定): 编辑逻辑放 Rust (`editcore` / `ide-kernel`), JS 只保留描画, 不再新增 JS 编辑逻辑
> - **状态:** 草案 — §10 的决策项等待产品负责人拍板

---

## 0. 结论先行

1. **「Rust 版 Sakura」在上游并不存在**: sakura-editor 组织只有 C++/Win32 版。本文说的"Rust 版"就是本仓库自己的重写。所以差距的对照对象是**上游 C++ 版的默认安装**(默认菜单 + 默认键位 + 默认外观)。
2. **按「用户能不能用到」计分, 当前与 Sakura 默认一致的只有约 20%。** Sakura 默认菜单 ∪ 默认键位共 **268 条**不重复命令:

   | 状态 | 含义 | 条数 | 占比 |
   |---|---|---|---|
   | ✅ 可达且一致 | 菜单或真实按键能触发, 行为与 Sakura 基本一致 | 53 | 20% |
   | 🟡 可达但不同 | 能触发, 但语义/键位/选项与 Sakura 不同 | 42 | 16% |
   | ⚪ 仅有代码 | 有处理函数, 但没有菜单项也没有按键能到达 | 12 | 4% |
   | ⬜ 缺失 | 没有实现 | 161 | 60% |

3. **最大的差距不在命令数量, 而在 6 个地基** (§3)。地基不补, 后面补再多命令也不像 Sakura:
   - **F1 文件 I/O 只认 UTF-8**: Shift_JIS / EUC 文件直接打不开 (`read_to_string`)。
   - **F2 文件只能在"项目根"里打开**: 没有文件对话框, 靠 `prompt()` 输入相对路径。最直观的表现是: 启动后的无标题文档按 Ctrl+S, 要先选一个工作文件夹, 再在 `prompt()` 里输入相对文件名。
   - **F3 没有显示宽度模型**: 全角字符按 1 列算, 矩形选择、列号、折返都会错位。
   - **F4 渲染**: 每次重绘都整篇重建 DOM、全文重新高亮; 文件上限 4 MB。
   - **F5 键盘选择**: Shift+方向键不选中; PgUp/PgDn、Ctrl+←→、Ctrl+Home/End 都没反应; 没有覆盖模式。
   - **F6 键位**: 散落在 3 个 keydown 处理器里, 和菜单上的标注互相矛盾。
4. **有 15 处键位冲突**: 同一个键做的是别的事 (§4.1)。Sakura 用户的肌肉记忆会触发错误动作, 这比"缺键"更伤。
5. **v1 的关键路径包含 ARCH-01 Phase 1。** 按 2026-10-18 的纯 Rust 化决定, 新的编辑逻辑不能再写进 JS。所以除了 F1 / F2 (本来就在后端), 其余差距都要先等 JS↔Rust 桥 (ARCH-01 估 8-12 人日) 完成, 再在 Rust 里补 (§10 D15)。
6. **现有文档高估了完成度。** 例如 SPA-02 §3.1 把光标移动 M-01~M-12 标成 ✅, 但对应的 UAT 是直接调用 `dispatchCommand("cursor-page-down")`, 键盘上并没有绑定。本文改用"真实按键 / 菜单点击可达"作为唯一计分口径。

---

## 1. 测量方法与口径

### 1.1 分母为什么是 268

| 口径 | 数量 | 来源 | 本文是否采用 |
|---|---|---|---|
| `Funccode_x.hsrc` 里全部 `F_` 枚举 | 474 | 含别名、宏函数(40000~)、内部码 | 否 — 不是用户可见功能 |
| 可分配到菜单/键的功能码 (30000~32767) | 343 | 同上, 过滤号段 | 否 — 含大量默认不露出的命令 |
| WBS-01 引用值 | 451 | WBS-01 §0 | 否 — 口径未说明 |
| **默认主菜单叶子** | 247 | `src/main/resources/MainMenu.ini`, 去掉 6 个动态列表占位 | 是 |
| **默认键位里不在菜单上的命令** | +21 | `sakura_core/func/CKeyBind.cpp` 默认表(206 条绑定, 去掉自定义菜单) | 是 |
| **合计 (去重)** | **268** | | **是** |

理由: v1 的目标是"装好打开就和 Sakura 一样", 也就是**默认安装能看到、能按到的东西**。

### 1.2 证据来源与局限

- **代码阅读**: `crates/ide-shell-web/src/editor.html`(7,678 行, 桌面版 build.rs 复制同一份)、`ide-shell-desktop/src/lib.rs`、`ide-shell-web/src/path_safety.rs`、`editcore/src/encoding_io.rs`。
- **运行时验证**: 用内置浏览器以 `file://` 打开 `editor.html`(纯前端模式, 极简布局, Vim 关闭)。
  - **真实按键 8 个**: Shift+→、Ctrl+→、PgDn、Ctrl+End、Tab、Ins、F8、F9。
  - **合成按键 80 个**: 对 Sakura 默认键逐个派发 `KeyboardEvent`, 同时给 58 个处理函数插桩, 记录实际调用了谁(结果见附录 B)。
- **未运行后端**: 本会话的 PATH 上没有 cargo。所以后端相关结论(编码、沙箱、4 MB 上限)来自代码阅读。其中 `encoding_rs` 的 UTF-16 行为已对照其 0.8.35 源码确认。
- **鼠标行为未做系统验证**: 双击选词、行号点击选行、拖放等。这几项在附录里标为待核实。
- **运行时验证没有在 Tauri WebView2 外壳里做。** 附录 B 标"无反应"的键, 在桌面版里可能触发 WebView2 自带的浏览器快捷键。编辑区会对大部分 Ctrl 组合 `preventDefault`, 但 F5 没有被拦截, 可能刷新页面并丢失未保存内容。这是**未实测的风险**, 应在 P0 用桌面版逐个确认。

---

## 2. 现状快照(与对标相关的架构事实)

| 方面 | 现状 | 证据 |
|---|---|---|
| 编辑执行路径 | 前端 JS: `t.lines` 字符串数组 + `renderVim()` 整篇重绘 | editor.html:3203 |
| editcore 接线 | UI 只用到 `grep_engine`、`type_config` 大纲、`plugin`、`theme_luminance`; buffer / undo / encoding_io / cursor / ruler / sdi / split_quad 等**没有接到 UI** | `grep -rn "editcore::" crates/ide-shell-*` |
| 读文件 | `std::fs::read_to_string` — 遇到非 UTF-8 直接报错"仅支持 UTF-8 文本" | ide-shell-desktop/src/lib.rs:478, path_safety.rs:159 |
| 写文件 | `fs::write(content.as_bytes())` — 永远写 UTF-8、无 BOM | ide-shell-desktop/src/lib.rs:502 |
| 文件范围 | 只能读写"已打开项目根"里的文件; 打开 / 另存为 / 新建都是 `prompt()` 输入相对路径 | editor.html:2166, 7209, 7220 |
| 文件大小 | `MAX_FILE_BYTES = 4 MB` | ide-shell-desktop/src/lib.rs:66 |
| 命令行 | 只认 `--help` / `--version`, 不能 `ide file.txt` 打开文件 | ide-shell-desktop/src/main.rs |
| 设置持久化 | localStorage (`ide10.*`) | editor.html `<head>` 引擎 |
| 键位 | 3 个 keydown 处理器 (document ×2、edContent ×1), 没有统一的键位表 | editor.html:1979, 4289, 7018 |
| 架构方向 | ARCH-01 已决定纯 Rust 化 (strangler fig); 其 Phase 1 = JS↔Rust 桥 + 把 130 个命令搬到 Rust, 估 8-12 人日 | ARCH-01 §3, §7 |

---

## 3. 地基层差距(必须先做, 否则上层无法"一样")

按 ARCH-01, 各地基的实现层如下:

| 地基 | 落在哪一层 | 对应 ARCH-01 阶段 |
|---|---|---|
| F1 编码 I/O | Rust 后端 + `editcore::encoding_io` | 独立 (I/O 本来就在后端) |
| F2 文件访问 | Rust (desktop: rfd / 命令行 / 拖放事件) | 独立 |
| F3 显示宽度 | `editcore` 提供宽度与列换算, 前端只取结果 | Phase 2 (编辑原语) |
| F4 渲染 | 前端 JS (描画是 ARCH-01 允许保留在 JS 的部分); 行数据来源改为 Rust 文档 | Phase 1-D |
| F5 移动 / 选择 / 覆盖 | Rust 命令 (`F_X` / `F_X_SEL` / `F_X_BOX`) | Phase 1-B / Phase 2 |
| F6 键位表 | 键位数据放 Rust; 前端只把按键事件翻译成命令 ID | Phase 1-C + 6-B |

### F1 文件 I/O 与文字编码 — P0

| | Sakura | 现状 |
|---|---|---|
| 打开 | 自动判别 SJIS / JIS / EUC / UTF-8 / UTF-16LE·BE / CESU-8 / UTF-7 / Latin1 / 代码页; 识别 BOM | 只认 UTF-8, 其它编码直接失败 |
| 保存 | 按原编码 + 原 BOM + 原换行写回; 遇到无法转换的字符会警告 | 永远写 UTF-8、无 BOM |
| 换行 | 按文件检测 CRLF / LF / CR, 有「入力改行コード指定」 | `FILE_EOL` 是**全局变量**, 不跟随文件 |
| 指定编码重开 | `F_FILE_REOPEN` + 按编码重开 `F_FILE_REOPEN_*` 9 条 + `F_CHG_CHARSET` 对话框 | 无 |

**设计**

1. 后端 `read_file` 改为读字节流: 先 `editcore::encoding_io::detect_encoding`, 再解码, 返回 `{content, encoding, bom, eol, had_errors}`。前端把这些字段存进每个 tab (取代全局 `FILE_EOL`)。
2. `write_file` 接收 `{encoding, bom, eol}`。保存前做一次"能否无损编码"检查; 有不可转换字符时弹出警告, 对应 Sakura 的保存确认。
3. `encoding_io` 需要先修 3 个缺陷:
   - **UTF-16 保存实际写成 UTF-8。** `encoding_rs::Encoding::encode()` 对 UTF-16LE/BE 的输出编码是 UTF-8 (已对照 encoding_rs 0.8.35 `output_encoding()` 源码)。需要手写 UTF-16 编码, 并补一个往返单测。
   - **BOM 信息在解码时丢失**, 保存时无法还原。
   - **不可映射字符被静默替换。** `encode` 会把它们变成 `&#NNNN;` 数字字符引用, 没有任何提示。
4. 自动判别要补 ISO-2022-JP (目前永远判不出来), 并用更好的 SJIS / EUC 判别启发式。现在的做法是"能无错解码就认", 短文本容易误判。
5. 状态栏显示编码 / BOM / 换行, 点击可以切换(对应 Sakura 状态栏)。

**验收**: 夹具里放 SJIS / EUC / JIS / UTF-8 BOM / UTF-16LE·BE / CRLF·LF·CR 混合文件; 每个文件做到"打开 → 编辑 → 保存 → 二进制与预期一致"。

### F2 文件访问模型 — P0

Sakura 能打开任意路径: 标准文件对话框(带编码选择)、命令行 `sakura.exe file -X=列 -Y=行 -CODE=编码 -R(只读)`、拖放到窗口、最近使用的文件 / 文件夹、文件被外部修改时提示重载 (`CDlgFileUpdateQuery`)、ビューモード(只读)。

**设计**

- **桌面版**
  - 改用绝对路径。`rfd` 已经是依赖, 直接用原生打开 / 保存对话框。
  - 新增"最近使用的文件夹"(`F_FOLDER_USED_RECENTLY`)。
  - main.rs 解析命令行打开文件。
  - Tauri 拖放事件 → 打开文件。
  - 用文件监视(或保存前比对 mtime)实现外部修改提示。
- **web 版**: 是否保留项目沙箱是一个决策项(§10 D3)。
- **4 MB 上限**: 随 F4 一起放宽。

### F3 显示宽度模型 — P0

Sakura 以"半角 = 1 列、全角 = 2 列、TAB 按 TAB 宽度(默认 4)展开"计算列。矩形选择、状态栏的「桁」、ルーラー、指定桁折り返し、跳转对话框都依赖这个模型。

现状: editor.html 中没有任何显示宽度逻辑。列 = Unicode 码点下标。含中日文的行做矩形选择时会错位; 这正是用户 2026-10-15 提出的"CJK = 2 列"验收项。

**设计**

- 一个 `dispWidth(cp)` 函数: East Asian Width 为 W / F 时返回 2; Ambiguous 宽度做成可配置项, Sakura 默认按 2。
- 每行提供两个换算: `col → index` 和 `index → col`。
- 所有"列"语义的地方统一改走这一层, 包括 `colSel*`、`boxMove`、状态栏、以后的 ruler 和折返。

### F4 渲染与大文件 — P1

现状 `renderVim()` 每次重绘都遍历全部行, 每行重新做 token 高亮, 再拼出整篇 HTML。在 4 MB 上限内都可能明显卡顿; Sakura 能打开上百 MB 的文件。

**设计**

- 虚拟化: 只渲染可视行 ± 缓冲区, 用固定行高 + 占位高度撑出滚动条。
- 按行缓存 token, 编辑时只让受影响的行(以及块注释状态变化波及的后续行)失效。
- 数据结构: JS 字符串数组在百万行以内可以接受; 更大的文件是否改用 piece table / editcore buffer 列为决策项 D13。

**验收**: 10 MB / 20 万行文件 2 秒内打开; 按键到重绘 < 16 ms。

### F5 键盘选择、模式与基础键 — P0

运行时实测(附录 B):

| 按键 | Sakura | 现状 |
|---|---|---|
| Shift+←→↑↓ / Shift+Home·End / Shift+PgUp·PgDn | 范围选择 (`*_SEL` 系列 20+ 条) | 光标移动, **不产生选区** |
| Ctrl+←→ / Ctrl+Home·End / PgUp·PgDn / Ctrl+PgUp·PgDn | 单词 / 文件首尾 / 翻页 / 半页 | **无反应**(处理函数存在, 但没有接到键上) |
| Ctrl+↑↓ | 滚动一行(光标不动) | 无 |
| Ins | 插入 / 覆盖切换 | 无覆盖模式 |
| F6 | 范围选择开始(键盘选择模式) | 无 |
| Tab | 插入 TAB 字符; 有选区时缩进 | 插入 2 个空格 |
| Shift+Tab | 反缩进 | **插入 2 个空格**(缺陷) |
| Shift+Del / Shift+Ins / Ctrl+Ins | 剪切 / 粘贴 / 复制 | 只删除 1 个字符 / 无 / 无 |
| F7 / F8 / F9 / Shift+F9 | 剪切 / 复制 / 粘贴 / 矩形粘贴 | 无 |

**设计**: 把"移动 × {普通, 范围选择, 矩形选择}"做成一张正交表, 由一个函数统一实现, 与 Sakura 的 `F_X` / `F_X_SEL` / `F_X_BOX` 三组命令一一对应。覆盖模式状态放进 tab 和状态栏。

### F6 键位层 — P0

现状有三个问题:

- 键位分散在 3 个 keydown 处理器里, 分支顺序会互相截胡。例如 F2 分支排在 Ctrl+F2 前面, 结果 Ctrl+F2 执行的是"下一个书签"。
- 菜单上的 `data-shortcut` 只是展示, 和真实绑定是两套数据, 已经互相矛盾 (§9)。
- Sakura 的「キー割り当て」允许用户改任何键; 我们没有这样一张表可改。

**设计**

- 建一张 `KEYBIND` 数据表, 初始值照抄 `CKeyBind.cpp` 默认表: 键 × 8 种修饰组合 → 命令 ID。
- 所有按键只经过这一个分发器。
- 菜单的快捷键标注从同一张表生成。
- 以后的键位编辑对话框(共通設定「キー割り当て」页)直接编辑这张表。

---

## 4. 键位对照(附录 B 运行时结果的归纳)

### 4.1 冲突 — 同一个键做了别的事(优先修)

| 键 | Sakura 默认 | 现状 |
|---|---|---|
| Ctrl+H | カーソル行をウィンドウ中央へ | 打开查找替换 |
| Ctrl+R | 置換ダイアログ | 无反应(菜单上却标在"重新载入"和"替换"两行)— 属于未绑定 + 标注错误, 不计入 15 处冲突 |
| Ctrl+K | 行末まで切り取り | Ctrl+K 和弦前缀 (Ctrl+K O 打开文件夹) |
| Ctrl+W | 現在位置の単語選択 | 关闭标签 |
| Ctrl+Del | 単語の右端まで削除 | 删除下方行 |
| Shift+Ctrl+↑/↓ | (範囲選択)上/下移動 2 行 | 上移 / 下移行 |
| Ctrl+F2 | ブックマーク設定・解除 | 下一个书签(缺陷) |
| Ctrl+F3 | 検索マークの切替え | 查找下一个 |
| Shift+F4 | 左右に分割 | 查找下一个 |
| F12 / Shift+F12 | タグジャンプ / タグジャンプバック | 都是"跳到本文件定义" |
| Tab / Shift+Tab | TAB 插入·缩进 / 反缩进 | 都插入 2 个空格 |
| Shift+Del | 切り取り | 删除一个字符 |
| Ctrl+- | このファイル名をコピー | 字体缩小 |
| Ctrl+0 / Ctrl+= | (无) | 缩放 — 不冲突, 可保留(不计数) |

### 4.2 已一致

Ctrl+A / C / V / X / Z / Y / S / Shift+Ctrl+S / F / G / N / O, Ctrl+E, Shift+Ctrl+E, Ctrl+U, Shift+Ctrl+U, Shift+Ctrl+K, Ctrl+BkSp, F2, Shift+F2, F3, Shift+F3, Shift+F6, Alt+←→↑↓, Ctrl+Alt+↑↓, Alt+Home / End, 以及普通方向键 / Home / End / BkSp / Del / Enter / Esc。Ctrl+J 键位一致, 但弹出的是原生 `prompt()` 而不是跳转对话框。

### 4.3 未绑定 — Sakura 有, 我们按了没反应

Ctrl+B / D / I / L / M / P / Q / T, Shift+Ctrl+D / L / M / P / T, Ctrl+Space(补全死代码), Ctrl+[ / ], Ctrl+1~7(工具栏 / 功能键 / 状态栏 / 设置类), Ctrl+Tab, Ins 系列, 全部 Shift / Ctrl 移动系列, PgUp / PgDn 全系列, F4~F11, Alt+F* / Ctrl+F* / Ctrl+Alt+F*(编码变换、按编码重开), Alt+A / D / L / R / M / N / P / ; / :, Ctrl+Enter, Alt+Enter。

---

## 5. 窗口外观差距

| 部件 | Sakura 默认 | 现状(极简布局) |
|---|---|---|
| 主菜单 | 8 个: ファイル / 編集 / **変換** / 検索 / ツール / 設定 / ウィンドウ / ヘルプ | 8 个: 文件 / 编辑 / 搜索 / **视图** / 工具 / 设置 / 窗口 / 帮助。没有「変換」, 转换功能挤在「编辑 > 转换」里 |
| ツールバー | **默认显示** (`m_bDispTOOLBAR = TRUE`) | 无 |
| ファンクションキー | 默认隐藏, 可放在上或下 | 无 |
| タブバー | **默认隐藏** (`m_bDispTabWnd = FALSE`) | 视布局而定 |
| ステータスバー | 默认显示: 行·桁 / 文字コード / 改行 / 挿入·上書 / 記録中 等 | 只有 模式 / 位置 / 文件 / 提示 / 语言; 没有编码、换行、覆盖、字符码 |
| ルーラー / 行番号 | 有 / 有 | 无 / 有 |
| 空白和换行记号 (TAB / 全角空格 / 改行 / EOF)、カーソル行アンダーライン、対括弧強調 | 有(按タイプ別配色) | 都没有 |
| ミニマップ | 默认隐藏 | 有, 可切换 ✅ |
| 折り返し | 默认不折返 (`WRAP_NO_TEXT_WRAP`), 可指定桁 / 右端 | 只有开/关 |

---

## 6. 命令层差距(按默认菜单归组)

| 菜单 | 条数 | ✅ | 🟡 | ⚪ | ⬜ |
|---|---|---|---|---|---|
| ファイル(F) 文件 | 29 | 4 | 5 | 0 | 20 |
| 編集(E) 编辑 | 95 | 25 | 7 | 10 | 53 |
| 変換(C) 转换 | 25 | 6 | 0 | 0 | 19 |
| 検索(S) 搜索 | 32 | 5 | 7 | 0 | 20 |
| ツール(T) 工具 | 9 | 0 | 4 | 0 | 5 |
| 設定(O) 设置 | 22 | 3 | 6 | 0 | 13 |
| ウィンドウ(W) 窗口 | 28 | 0 | 9 | 0 | 19 |
| ヘルプ(H) 帮助 | 7 | 1 | 4 | 0 | 2 |
| (仅默认键位, 不在菜单) | 21 | 9 | 0 | 2 | 10 |
| **合计** | **268** | **53** | **42** | **12** | **161** |

各菜单缺口最大的部分:

- **文件**: 按编码重开 ×9、印刷 ×3、ファイルのプロパティ、保存して閉じる、閉じて開く、最近使ったフォルダー、全終了。
- **编辑**: 範囲選択開始 + 范围选择移动 `*_SEL` ×14, 矩形移动的单词 / 翻页 / 文件首尾 ×7, 矩形贴付け (`F_PASTEBOX`), 选择范围内的各种复制 ×5 (全行 / 引用符 / 行番号 / 色付き HTML), 文件名 / 路径 / タグ复制 ×3, 移動履歴 ×3, 変更行移動 ×2, 関数リストマーク移動 ×2, SPACE 缩进 ×2, 単語切り取り / 削除, 選択行のマージ。
- **转换**: 全半角英数 ×2, 半角カナ→全角 ×2, TAB↔空白 ×2, 编码变换 ×11, Base64 / uudecode。
- **搜索**: 增量搜索 6 种(目前全是普通查找框)、検索マーク切替、検索開始位置へ戻る、ブックマーク一覧 / 全解除、タグ系 ×4 (タグジャンプバック / タグ作成 / ダイレクトタグジャンプ / キーワード指定)、ファイル比較 + DIFF ×5、対括弧の検索、ファイルツリー、同名ヘッダー / ソース。
- **设置**: ツールバー / ファンクションキー显示、指定桁折り返し、文字カウント方法、挿入 / 上書、ビューモード、文字コードセット指定、入力改行コード ×3、履歴の管理。
- **窗口**: タブ操作 ×11(グループ化、左右を閉じる、移動...)、最大化 / 最小化 ×3、ウィンドウ一覧 ×3、常に手前、再描画、アウトプット窗口。另外 cascade / tile 目前是**假实现**(打开新窗口 / 切全屏)。

逐条清单见附录 A。

---

## 7. 对话框与设置层差距

| 对话框 | Sakura 选项 | 现状 |
|---|---|---|
| 検索 (`CDlgFind`) | 単語単位 / 大小区别 / 正規表現 / 見つからないときにメッセージ / 自動的に閉じる / 先頭(末尾)から再検索 / 上·下検索 / 該当行マーク / 履歴 | 模态框, 只有大小写 + 正则 2 个选项 |
| 置換 (`CDlgReplace`) | 上述 + 置換対象(選択範囲 / ファイル全体)/ 挿入·追加 / 一つ置換 / すべて置換 / クリップボードから貼り付け | 只有「全部替换」 |
| インクリメンタルサーチ | 状态栏内联, 前方 / 後方 × 普通 / 正規表現 / MIGEMO | 打开的是普通查找框 |
| 指定行へジャンプ (`CDlgJump`) | 行番号 + 折り返し単位 / 改行単位 + PL/SQL | 原生 `prompt()` |
| Grep / Grep 置換 | 见 SPA-01(已对标大半) | 按 SPA-01 §4.2 继续 |
| 共通設定 (`prop/`) | **18 页**: 全般 / ウィンドウ / タブバー / 編集 / ファイル / バックアップ / 書式 / 検索 / キー割り当て / カスタムメニュー / ツールバー / 強調キーワード / 支援 / マクロ / ファイル名表示 / ステータスバー / プラグイン / メインメニュー | 选项列表弹窗 |
| タイプ別設定 (`typeprop/`) | **6 页**: スクリーン / カラー / ウィンドウ / 支援 / 正規表現キーワード / キーワードヘルプ; 上游 22 个类型模块 | 「类型配置」选项列表 |
| 其它对话框 | 上游 `dlg/` 共 27 个: 文字コード指定、コントロールコード入力、外部コマンド、DIFF、比較、タグジャンプ一覧、タグ作成、履歴管理、ウィンドウ一覧、ファイル更新確認、印刷設定、プロパティ 等 | 绝大多数没有 |

**设计原则**: 对话框统一用页内模态组件(已有 `openModalInput`), 全部去掉原生 `prompt()`(目前还有 11 处)。设置写到用户目录下的配置文件, 不再只存 localStorage; 是否兼容导入 `sakura.ini` 见 D12。

---

## 8. 宏 / 插件 / 外部集成

| 能力 | Sakura | 现状 |
|---|---|---|
| キーマクロ | 记录 / 保存 / 读取 / 执行 (`.mac`); Ctrl+M / L、Shift+Ctrl+M / L | 会话内按键录制; 不能保存 / 读取; 键位不同 |
| 外部マクロ | PPA + WSH(JScript / VBScript); 宏 API = 60 个宏函数 + 全部命令的 `S_` 形式 | `prompt()` 文本 DSL(KEY / TYPE / CMD)和 `new Function` JS |
| プラグイン | `plugin.def` + WSH / DLL; 三类接口 (`CPlugin` / `COutlineIfObj` / `CComplementIfObj`) | wasm `process(text) -> String`, 且 `wasm` feature 默认关闭 |
| 外部コマンド実行 | `CDlgExec`, 输出到アウトプット窗口 | 有终端(形态不同) |
| 入力補完 / キーワードヘルプ | 有 | 有补全(规则不同); 没有键词帮助 |

---

## 9. 调查中发现的缺陷与漂移(建议在 Phase 0 一起修)

1. **Ctrl+F2 执行"下一个书签"**: 全局 keydown 里 `F2` 分支排在 `Ctrl+F2` 之前。editor.html:7025。
2. **Shift+Tab 插入两个空格**, 而不是反缩进。
3. **Ctrl+Space 补全是死代码**: edContent 处理器的 `e.ctrlKey` 分支先 `return` 了。
4. **Ctrl+D、Shift+Ctrl+D、Shift+Ctrl+V 没有任何绑定**: 后两个写在 `e.ctrlKey && !e.shiftKey` 分支里, 永远进不去; 菜单却标着这些快捷键。
5. **`tagJump()` 的提示文案是复制粘贴错误**: 写成了 `"缩略图: ..."`, 而且只改了光标、没有重绘。editor.html:5762。
6. **`VIM_DEFAULT = { min:false, full:true }` 与用户决策不一致**: 2026-10-15 的决策是"所有布局下 Vim 默认关闭"。
7. **`FILE_EOL` 是全局变量**: 切换标签后换行设置会串到别的文件。
8. **菜单快捷键标注重复或错误**:
   - Ctrl+R 同时标在"重新载入"和"替换"两行, 实际按下没有反应。
   - Ctrl+J 同时标在"合并行""跳转到行""底端面板"三行。
   - Ctrl+Shift+V 同时标在"剪贴板历史"和"左右分割"。
   - F12 同时标在"跳转到标签"和"跳转到定义"。
   - "字体"标 Alt+F, 和文件菜单的助记键冲突; "语法高亮"标 Alt+S, 和搜索菜单冲突。
   - Ctrl+I 标为"增量查找", 实际没有绑定。
9. **`encoding_io` 的三个问题**: UTF-16 保存写成 UTF-8、BOM 丢失、不可映射字符被静默替换成 `&#NNNN;`(见 F1)。
10. **完成度高估**: SPA-02 §3.1 的 M-01~M-12 被标为 ✅, 但 UAT `cursor-movement.spec.ts` 用 `dispatchCommand` 直接调用, 键盘上并未绑定。
11. **cascade / tile 是假实现**: 实际是"打开新窗口 / 切换全屏"(editor.html:5646)。

---

## 10. v1 范围决策清单(需要产品负责人拍板)

默认答案是"全部和 Sakura 一样"。下列各项要么在跨平台 / WebView 架构下没有直接对应物, 要么成本远高于收益, 需要逐项确认: 「做」/「跨平台等价」/「v1 不做」。

| # | 决策项 | 选项 | 备注 |
|---|---|---|---|
| D1 | UI 文案语言 | 日文原文 / 中文 / 可切换 | Sakura 本体有日文和英文资源 (`sakura_lang/en_US`) |
| D2 | 窗口模型 | Sakura 式"一个文件一个窗口 + 可选タブ化" / 现在的单窗口多标签 | 影响「ウィンドウ」菜单 28 条中的大半 |
| D3 | web 版的文件访问 | 保留项目沙箱 / 和桌面版一样开放 | 桌面版建议必须开放(F2) |
| D4 | 菜单结构 | 照搬 Sakura 8 个菜单(含「変換」, 去掉「视图」) / 保留现结构 | IDE 扩展(终端 / SCM / 侧栏)放到哪个菜单 |
| D5 | Win32 专属功能 | SQL*Plus ×2、タスクトレイ常駐、外部 HTML ヘルプ、IME 再変換、VBScript | 建议: 标注"不适用"或给跨平台等价 |
| D6 | 稀有编码 | CESU-8、UTF-7、任意代码页、E-Mail(JIS) 变换、uudecode / Base64 解码 | encoding_rs 不支持 UTF-7 / CESU-8, 需要自己写 |
| D7 | 宏兼容级别 | `.mac` 键宏格式 / PPA / JS 宏 API(`Editor.*`)兼容 | WBS-01 U-01: 兼容级别会让工作量翻倍 |
| D8 | 插件兼容 | Sakura `plugin.def` 兼容 / 自有 wasm ABI | WBS-01 U-02 |
| D9 | 印刷 | v1 做 / 不做 | WebView 打印能力有限; 页眉页脚、行号需要自己排版 |
| D10 | DIFF / ファイル比較 | v1 做 / 不做 | 需要引入 diff 实现(SPA-02 列为未处理依赖) |
| D11 | Migemo | v1 做 / 不做 | 需要字典(cmigemo-dict, 体积和许可都要评估) |
| D12 | 设置格式 | 自有格式 / 兼容导入 `sakura.ini` | |
| D13 | 实现位置 | **已决定 (ARCH-01)**: Rust 优先, 不再新增 JS 编辑逻辑 | WBS-01 U-04 已关闭 |
| D14 | 非 Sakura 功能 | v1 是否保留 Vim、终端、SCM、侧栏 | 建议保留, 但默认隐藏(已是极简布局策略) |
| D15 | v1 对标与 ARCH-01 Phase 1 的先后 | (a) 先做 Phase 1 (桥 + 命令 Rust 化), 再在 Rust 里补 Sakura 差距 / (b) 地基 F1 / F2 (本来就在后端) 与 Phase 1 并行, 其余等桥 | ARCH-01 禁止新增 JS 编辑逻辑, 所以 F3 / F5 / F6 及 §6 的命令补齐都要等桥 |
| D16 | §9 缺陷修复是否允许直接改 JS | 允许 (属于修复, 不是新逻辑) / 不允许 (随 Phase 1 一起搬到 Rust 再修) | 建议允许: 这些都是几行级的分支顺序和标注错误, 用户现在就会踩到 |

**D13 已定, 带来的影响**: 本文 §3~§8 描述的是 v1 必须达到的**行为**, 不是 JS 里的改法。按 ARCH-01, 它们要实现在 `editcore` / `ide-kernel`, 前端只负责描画和把按键 / 菜单翻译成命令 ID。所以 v1 的关键路径是 **ARCH-01 Phase 1 → 本文 F1~F6 → 命令补齐**; ARCH-01 Phase 1 的 8-12 人日要计入 v1 工期。NEW-02 (Web 版是否存续) 与 D3 是同一个问题, 应一起决定。

---

## 11. 建议的实施顺序与验收门

| 阶段 | 内容 | 规模 | 退出条件 |
|---|---|---|---|
| **P0 准备** | §9 缺陷修复 (按 D16); 拍板 §10 决策; 定验收规则; F1 编码 I/O + F2 文件访问 (都在 Rust 后端, 不依赖桥, 可与 ARCH-01 Phase 1 并行) | M | 编码往返夹具全绿; 桌面版能用原生对话框打开任意路径的 SJIS 文件 |
| **P0' 桥** | ARCH-01 Phase 1: JS↔Rust 桥、命令层、`dispatchCommand` 委托给 Rust、文档状态统一到 Rust; 同时把 `KEYBIND` 表 (F6) 建在 Rust 侧, 菜单标注由它生成 | L (8-12 人日, ARCH-01 估) | ARCH-01 Phase 1 完成定义 + 附录 B 冲突全部消除或登记为决策项 |
| **P1 地基** | F5 移动 / 选择 / 覆盖 / Tab → F3 宽度模型 (Rust) → F4 虚拟化描画 (JS) | L | 268 表中的移动 / 选择类全部 ✅ |
| **P2 外观** | 状态栏字段、ルーラー、ツールバー、ファンクションキー、空白记号、カーソル行下線、対括弧、指定桁折返; 按 D4 调整菜单 | M | 与 Sakura 截图逐部件对照 |
| **P3 命令补齐** | 编辑 → 转换 → 搜索 → 文件 → 窗口(按 D2) | L | 附录 A 中按 §10 决定要做的行全部 ✅ |
| **P4 对话框 / 设置** | 查找 / 替换 / 跳转全选项、增量搜索、共通設定和タイプ別設定各页、键位编辑 | L | 每个对话框的选项逐项有 UAT |
| **P5 生态** | 键宏保存 / 读取、外部宏、插件、印刷、DIFF(按 D7~D11) | L | 按决策 |

**验收规则(替代现在"调用不报错"的口径)**: 只有通过 Playwright `keyboard.press()` 或真实菜单点击触发、并对结果做断言的条目, 才能标 ✅。直接调用 `dispatchCommand()` 的用例只算单元级测试, 不计入对标完成度。

**验收规则也要适配 ARCH-01**: 命令搬到 Rust 后, 行为正确性用 Rust 单测保证 (ARCH-01 §6.2); "用户能否触发"仍要靠上面的真实按键 / 菜单 UAT。两者缺一不可, 否则会重演"函数在、键没接"的问题。

**并行性提醒**: 桥完成之前, 前端工作都落在 `editor.html` 这一个 7.7k 行的文件里, 只能串行。桥完成后, 命令逻辑在 `editcore` 里按模块分文件, P3 / P4 可以按菜单分批并行; `editor.html` 只剩描画和对话框外壳, 冲突面会小很多。

---

## 附录 A — 268 条命令逐条状态

图例: ✅ 可达且一致 · 🟡 可达但不同 · ⚪ 仅有代码 · ⬜ 缺失。"默认键"来自 `CKeyBind.cpp` 默认表。同一命令在多个菜单出现时, 只记在第一次出现的位置。

### ファイル(F) 文件

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| ✅ | `F_FILENEW` | 新規作成 | Ctrl+N |  |
| ⬜ | `F_FILENEW_NEWWINDOW` | 新規作成（新しいウインドウで開く） | — |  |
| 🟡 | `F_FILEOPEN` | 開く | Ctrl+O | prompt() 输入项目根内相对路径; 无文件对话框; 仅 UTF-8; ≤4MB |
| ✅ | `F_FILESAVE` | 上書き保存 | Ctrl+S |  |
| 🟡 | `F_FILESAVEAS_DIALOG` | 名前を付けて保存 | Shift+Ctrl+S | prompt(); 限项目根; 无编码/BOM/改行选择 |
| ✅ | `F_FILESAVEALL` | すべて上書き保存 | — |  |
| ⬜ | `F_FILESAVECLOSE` | 保存して閉じる | — |  |
| 🟡 | `F_WINCLOSE` | 閉じる | — | = 关闭标签 |
| 🟡 | `F_FILECLOSE` | 閉じて(無題) | — | = 关闭标签 |
| ⬜ | `F_FILECLOSE_OPEN` | 閉じて開く | Shift+Ctrl+F4 |  |
| 🟡 | `F_FILE_REOPEN` | 現在と同じ文字コードで開き直す | — | 只按 UTF-8 重读 |
| ⬜ | `F_FILE_REOPEN_SJIS` | SJISで開き直す | Ctrl+Alt+F12 |  |
| ⬜ | `F_FILE_REOPEN_JIS` | JISで開き直す | Ctrl+Alt+F11 |  |
| ⬜ | `F_FILE_REOPEN_EUC` | EUCで開き直す | Ctrl+Alt+F10 |  |
| ⬜ | `F_FILE_REOPEN_LATIN1` | Latin1で開き直す | — |  |
| ⬜ | `F_FILE_REOPEN_UNICODE` | Unicodeで開き直す | Ctrl+Alt+F9 |  |
| ⬜ | `F_FILE_REOPEN_UNICODEBE` | UnicodeBEで開き直す | — |  |
| ⬜ | `F_FILE_REOPEN_UTF8` | UTF-8で開き直す | Ctrl+Alt+F8 |  |
| ⬜ | `F_FILE_REOPEN_CESU8` | CESU-8で開き直す | — |  |
| ⬜ | `F_FILE_REOPEN_UTF7` | UTF-7で開き直す | Ctrl+Alt+F7 |  |
| ⬜ | `F_PRINT` | 印刷 | Ctrl+P |  |
| ⬜ | `F_PRINT_PREVIEW` | 印刷プレビュー | Shift+Ctrl+P |  |
| ⬜ | `F_PRINT_PAGESETUP` | 印刷ページ設定 | Ctrl+Alt+P |  |
| ⬜ | `F_PROPERTY_FILE` | ファイルのプロパティ | Alt+Enter |  |
| ⬜ | `F_BROWSE` | ブラウズ | Ctrl+B |  |
| ✅ | `F_FILE_USED_RECENTLY` | 最近使ったファイル | — |  |
| ⬜ | `F_GROUPCLOSE` | グループを閉じる | — |  |
| ⬜ | `F_EXITALLEDITORS` | 編集の全終了 | Shift+Alt+F4 |  |
| ⬜ | `F_EXITALL` | サクラエディタの全終了 | Ctrl+Alt+F4 |  |

### 編集(E) 编辑

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| ✅ | `F_UNDO` | 元に戻す(Undo) | Ctrl+Z / Ctrl+_ / Ctrl+_(PC-98) |  |
| ✅ | `F_REDO` | やり直し(Redo) | Ctrl+Y |  |
| ✅ | `F_CUT` | 切り取り(選択範囲をクリップボードにコピーして削除) | F7 / Shift+Del / Ctrl+X |  |
| ✅ | `F_COPY` | コピー(選択範囲をクリップボードにコピー) | F8 / Ctrl+Ins / Ctrl+C |  |
| ✅ | `F_PASTE` | 貼り付け(クリップボードから貼り付け) | F9 / Shift+Ins / Ctrl+V |  |
| ✅ | `F_DELETE` | 削除 | Del |  |
| ✅ | `F_SELECTALL` | すべて選択 | Ctrl+A |  |
| ⬜ | `F_RECONVERT` | 再変換 | — |  |
| ⬜ | `F_COPY_CRLF` | CRLF改行でコピー | Shift+F8 |  |
| ⬜ | `F_COPY_ADDCRLF` | 折り返し位置に改行をつけてコピー | — |  |
| ⬜ | `F_PASTEBOX` | 矩形貼り付け(クリップボードから矩形貼り付け) | Shift+F9 |  |
| ✅ | `F_DELETE_BACK` | カーソル前を削除 | BkSp |  |
| 🟡 | `F_INS_DATE` | 日付挿入 | Alt+; | 日期时间合一 |
| 🟡 | `F_INS_TIME` | 時刻挿入 | Alt+: | 日期时间合一 |
| ⬜ | `F_CTRL_CODE_DIALOG` | コントロールコードの入力(ダイアログ) | — |  |
| ⬜ | `F_INS_FILE_USED_RECENTLY` | 最近使ったファイル挿入 | — |  |
| ⬜ | `F_INS_FOLDER_USED_RECENTLY` | 最近使ったフォルダー挿入 | — |  |
| ✅ | `F_WordDeleteToStart` | 単語の左端まで削除 | Ctrl+BkSp |  |
| ✅ | `F_WordDeleteToEnd` | 単語の右端まで削除 | Ctrl+Del |  |
| ⬜ | `F_SELECTWORD` | 現在位置の単語選択 | Ctrl+W |  |
| ⬜ | `F_WordCut` | 単語切り取り | Ctrl+D |  |
| ⬜ | `F_WordDelete` | 単語削除 | Shift+Ctrl+D |  |
| ✅ | `F_LineCutToStart` | 行頭まで切り取り(改行単位) | Ctrl+U |  |
| ⬜ | `F_LineCutToEnd` | 行末まで切り取り(改行単位) | Ctrl+K |  |
| ✅ | `F_LineDeleteToStart` | 行頭まで削除(改行単位) | Shift+Ctrl+U |  |
| ✅ | `F_LineDeleteToEnd` | 行末まで削除(改行単位) | Shift+Ctrl+K |  |
| ✅ | `F_CUT_LINE` | 行切り取り(折り返し単位) | Ctrl+E |  |
| ✅ | `F_DELETE_LINE` | 行削除(折り返し単位) | Shift+Ctrl+E |  |
| ✅ | `F_DUPLICATELINE` | 行の二重化(折り返し単位) | Shift+F10 / Ctrl+I |  |
| 🟡 | `F_INDENT_TAB` | TABインデント | Tab | 菜单可用; Tab 键插 2 空格而非 TAB |
| 🟡 | `F_UNINDENT_TAB` | 逆TABインデント | Shift+Tab | 菜单可用; Shift+Tab 反而插 2 空格 |
| ⬜ | `F_INDENT_SPACE` | SPACEインデント | Space |  |
| ⬜ | `F_UNINDENT_SPACE` | 逆SPACEインデント | Shift+Space |  |
| ⬜ | `F_COPYLINES` | 選択範囲内全行コピー | — |  |
| ⬜ | `F_COPYLINESASPASSAGE` | 選択範囲内全行引用符付きコピー | Ctrl+. |  |
| ⬜ | `F_COPYLINESWITHLINENUMBER` | 選択範囲内全行行番号付きコピー | — |  |
| ⬜ | `F_COPY_COLOR_HTML` | 選択範囲内色付きHTMLコピー | — |  |
| ⬜ | `F_COPY_COLOR_HTML_LINENUMBER` | 選択範囲内行番号色付きHTMLコピー | — |  |
| ⬜ | `F_COPYFNAME` | このファイル名をクリップボードにコピー | Ctrl+- |  |
| ⬜ | `F_COPYPATH` | このファイルのパス名をクリップボードにコピー | Ctrl+\\ |  |
| ⬜ | `F_COPYTAG` | このファイルのパス名とカーソル位置をコピー | — |  |
| ⚪ | `F_UP2` | カーソル上移動(２行ごと) | — |  |
| ⚪ | `F_DOWN2` | カーソル下移動(２行ごと) | — |  |
| ⚪ | `F_WORDLEFT` | 単語の左端に移動 | Ctrl+← |  |
| ⚪ | `F_WORDRIGHT` | 単語の右端に移動 | Ctrl+→ |  |
| ⚪ | `F_GOPREVPARAGRAPH` | 前の段落へ | — |  |
| ⚪ | `F_GONEXTPARAGRAPH` | 次の段落へ | — |  |
| ✅ | `F_GOLINETOP` | 行頭に移動(折り返し単位) | Home |  |
| ✅ | `F_GOLINEEND` | 行末に移動(折り返し単位) | End(Help) |  |
| ⚪ | `F_1PageUp` | １ページアップ | PgUp(RollDn) |  |
| ⚪ | `F_1PageDown` | １ページダウン | PgDn(RollUp) |  |
| ⚪ | `F_GOFILETOP` | ファイルの先頭に移動 | Ctrl+Home |  |
| ⚪ | `F_GOFILEEND` | ファイルの最後に移動 | Ctrl+End(Help) |  |
| ⬜ | `F_CURLINECENTER` | カーソル行をウィンドウ中央へ | Ctrl+H |  |
| 🟡 | `F_JUMP_DIALOG` | 指定行へジャンプ | Ctrl+J | Ctrl+J 弹原生 prompt(), 非跳转对话框 |
| ⬜ | `F_JUMP_SRCHSTARTPOS` | 検索開始位置へ戻る | Shift+Ctrl+F3 |  |
| ⬜ | `F_JUMPHIST_PREV` | 移動履歴: 前へ | Alt+P |  |
| ⬜ | `F_JUMPHIST_NEXT` | 移動履歴: 次へ | Alt+N |  |
| ⬜ | `F_JUMPHIST_SET` | 現在位置を移動履歴に登録 | — |  |
| ⬜ | `F_FUNCLIST_NEXT` | 次の関数リストマークへ | — |  |
| ⬜ | `F_FUNCLIST_PREV` | 前の関数リストマークへ | — |  |
| ⬜ | `F_MODIFYLINE_NEXT` | 次の変更行へ | — |  |
| ⬜ | `F_MODIFYLINE_PREV` | 前の変更行へ | — |  |
| ⬜ | `F_BEGIN_SEL` | 範囲選択開始 | F6 |  |
| ⬜ | `F_UP2_SEL` | (範囲選択)カーソル上移動(２行ごと) | Shift+Ctrl+↑ |  |
| ⬜ | `F_DOWN2_SEL` | (範囲選択)カーソル下移動(２行ごと) | Shift+Ctrl+↓ |  |
| ⬜ | `F_WORDLEFT_SEL` | (範囲選択)単語の左端に移動 | Shift+Ctrl+← |  |
| ⬜ | `F_WORDRIGHT_SEL` | (範囲選択)単語の右端に移動 | Shift+Ctrl+→ |  |
| ⬜ | `F_GOPREVPARAGRAPH_SEL` | (範囲選択)前の段落へ | — |  |
| ⬜ | `F_GONEXTPARAGRAPH_SEL` | (範囲選択)次の段落へ | — |  |
| ⬜ | `F_GOLINETOP_SEL` | (範囲選択)行頭に移動(折り返し単位) | Shift+Home |  |
| ⬜ | `F_GOLINEEND_SEL` | (範囲選択)行末に移動(折り返し単位) | Shift+End(Help) |  |
| ⬜ | `F_1PageUp_Sel` | (範囲選択)１ページアップ | Shift+PgUp(RollDn) |  |
| ⬜ | `F_1PageDown_Sel` | (範囲選択)１ページダウン | Shift+PgDn(RollUp) |  |
| ⬜ | `F_GOFILETOP_SEL` | (範囲選択)ファイルの先頭に移動 | Shift+Ctrl+Home |  |
| ⬜ | `F_GOFILEEND_SEL` | (範囲選択)ファイルの最後に移動 | Shift+Ctrl+End(Help) |  |
| ⬜ | `F_MODIFYLINE_NEXT_SEL` | (範囲選択)次の変更行へ | — |  |
| ⬜ | `F_MODIFYLINE_PREV_SEL` | (範囲選択)前の変更行へ | — |  |
| ✅ | `F_BEGIN_BOX` | 矩形範囲選択開始 | Shift+F6 |  |
| ✅ | `F_UP2_BOX` | (矩形選択)カーソル上移動(２行ごと) | Ctrl+Alt+↑ |  |
| ✅ | `F_DOWN2_BOX` | (矩形選択)カーソル下移動(２行ごと) | Ctrl+Alt+↓ |  |
| ⬜ | `F_WORDLEFT_BOX` | (矩形選択)単語の左端に移動 | Ctrl+Alt+← |  |
| ⬜ | `F_WORDRIGHT_BOX` | (矩形選択)単語の右端に移動 | Ctrl+Alt+→ |  |
| ⬜ | `F_GOLOGICALLINETOP_BOX` | (矩形選択)行頭に移動(改行単位) | — |  |
| ✅ | `F_GOLINETOP_BOX` | (矩形選択)行頭に移動(折り返し単位) | Alt+Home |  |
| ✅ | `F_GOLINEEND_BOX` | (矩形選択)行末に移動(折り返し単位) | Alt+End(Help) |  |
| ⬜ | `F_1PageUp_BOX` | (矩形選択)１ページアップ | Alt+PgUp(RollDn) |  |
| ⬜ | `F_1PageDown_BOX` | (矩形選択)１ページダウン | Alt+PgDn(RollUp) |  |
| ⬜ | `F_GOFILETOP_BOX` | (矩形選択)ファイルの先頭に移動 | Ctrl+Alt+Home |  |
| ⬜ | `F_GOFILEEND_BOX` | (矩形選択)ファイルの最後に移動 | Ctrl+Alt+End(Help) |  |
| ✅ | `F_LTRIM` | 左(先頭)の空白を削除 | Alt+L |  |
| ✅ | `F_RTRIM` | 右(末尾)の空白を削除 | Alt+R |  |
| 🟡 | `F_SORT_ASC` | 選択行の昇順ソート | Alt+A | 经「行排序」面板; Alt+A 未绑 |
| 🟡 | `F_SORT_DESC` | 選択行の降順ソート | Alt+D | 经「行排序」面板; Alt+D 未绑 |
| ⬜ | `F_MERGE` | 選択行のマージ | Alt+M |  |

### 変換(C) 转换

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| ✅ | `F_TOLOWER` | 小文字 | Ctrl+F6 / Ctrl+Alt+L |  |
| ✅ | `F_TOUPPER` | 大文字 | Ctrl+F7 / Shift+Ctrl+Alt+L |  |
| ✅ | `F_TOHANKAKU` | 全角→半角 | Ctrl+F8 |  |
| ✅ | `F_TOZENKAKUKATA` | 半角＋全ひら→全角・カタカナ | Ctrl+F9 |  |
| ✅ | `F_TOZENKAKUHIRA` | 半角＋全カタ→全角・ひらがな | Ctrl+F10 |  |
| ⬜ | `F_TOHANEI` | 全角英数→半角英数 | — |  |
| ⬜ | `F_TOZENEI` | 半角英数→全角英数 | — |  |
| ✅ | `F_TOHANKATA` | 全角カタカナ→半角カタカナ | — |  |
| ⬜ | `F_HANKATATOZENKATA` | 半角カタカナ→全角カタカナ | Ctrl+F11 |  |
| ⬜ | `F_HANKATATOZENHIRA` | 半角カタカナ→全角ひらがな | Ctrl+F12 |  |
| ⬜ | `F_TABTOSPACE` | TAB→空白 | Ctrl+Alt+F5 |  |
| ⬜ | `F_SPACETOTAB` | 空白→TAB | Shift+Ctrl+Alt+F5 |  |
| ⬜ | `F_CODECNV_AUTO2SJIS` | 自動判別→SJISコード変換 | Alt+F12 |  |
| ⬜ | `F_CODECNV_EMAIL` | E-Mail(JIS→SJIS)コード変換 | Alt+F11 |  |
| ⬜ | `F_CODECNV_EUC2SJIS` | EUC→SJISコード変換 | Alt+F10 |  |
| ⬜ | `F_CODECNV_UNICODE2SJIS` | Unicode→SJISコード変換 | Alt+F9 |  |
| ⬜ | `F_CODECNV_UNICODEBE2SJIS` | UnicodeBE→SJISコード変換 | — |  |
| ⬜ | `F_CODECNV_UTF82SJIS` | UTF-8→SJISコード変換 | Alt+F8 |  |
| ⬜ | `F_CODECNV_UTF72SJIS` | UTF-7→SJISコード変換 | Alt+F7 |  |
| ⬜ | `F_CODECNV_SJIS2JIS` | SJIS→JISコード変換 | Shift+Alt+F11 |  |
| ⬜ | `F_CODECNV_SJIS2EUC` | SJIS→EUCコード変換 | Shift+Alt+F10 |  |
| ⬜ | `F_CODECNV_SJIS2UTF8` | SJIS→UTF-8コード変換 | Shift+Alt+F8 |  |
| ⬜ | `F_CODECNV_SJIS2UTF7` | SJIS→UTF-7コード変換 | Shift+Alt+F7 |  |
| ⬜ | `F_BASE64DECODE` | Base64デコードして保存 | Alt+F6 |  |
| ⬜ | `F_UUDECODE` | uudecodeして保存 | Alt+F5 |  |

### 検索(S) 搜索

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| 🟡 | `F_SEARCH_DIALOG` | 検索(単語検索ダイアログ) | Ctrl+F | 模态框, 仅「大小写/正则」2 个选项 |
| ✅ | `F_SEARCH_NEXT` | 次を検索 | F3 |  |
| ✅ | `F_SEARCH_PREV` | 前を検索 | Shift+F3 |  |
| 🟡 | `F_REPLACE_DIALOG` | 置換(置換ダイアログ) | Ctrl+R | Ctrl+R 无反应 (绑在 Ctrl+H); 只有「全部替换」 |
| ⬜ | `F_SEARCH_CLEARMARK` | 検索マークの切替え | Ctrl+F3 |  |
| 🟡 | `F_ISEARCH_NEXT` | 前方インクリメンタルサーチ | — | 打开普通查找框, 不是增量 |
| ⬜ | `F_ISEARCH_PREV` | 後方インクリメンタルサーチ | — |  |
| ⬜ | `F_ISEARCH_REGEXP_NEXT` | 前方正規表現インクリメンタルサーチ | — |  |
| ⬜ | `F_ISEARCH_REGEXP_PREV` | 後方正規表現インクリメンタルサーチ | — |  |
| ⬜ | `F_ISEARCH_MIGEMO_NEXT` | 前方MIGEMOインクリメンタルサーチ | — |  |
| ⬜ | `F_ISEARCH_MIGEMO_PREV` | 後方MIGEMOインクリメンタルサーチ | — |  |
| 🟡 | `F_BOOKMARK_SET` | ブックマーク設定・解除 | Ctrl+F2 | 菜单可用; Ctrl+F2 实际执行「下一个书签」 |
| ✅ | `F_BOOKMARK_NEXT` | 次のブックマークへ | F2 |  |
| ✅ | `F_BOOKMARK_PREV` | 前のブックマークへ | Shift+F2 |  |
| ⬜ | `F_BOOKMARK_RESET` | ブックマークの全解除 | Shift+Ctrl+F2 |  |
| ⬜ | `F_BOOKMARK_VIEW` | ブックマークの一覧 | Alt+F2 |  |
| ✅ | `F_GREP_DIALOG` | Grep | Ctrl+G |  |
| 🟡 | `F_GREP_REPLACE_DLG` | Grep置換 | — | 有对话框, 未与上游逐项对照 |
| 🟡 | `F_OUTLINE` | アウトライン解析 | F11 | 侧栏面板, F11 未绑 |
| ⬜ | `F_FILETREE` | ファイルツリー | — |  |
| 🟡 | `F_TAGJUMP` | タグジャンプ | F12 / Ctrl+T | F12=跳本文件定义; 菜单项实为「下一个定义」且有提示文案 bug |
| ⬜ | `F_TAGJUMPBACK` | タグジャンプバック | Shift+F12 / Shift+Ctrl+T |  |
| ⬜ | `F_TAGS_MAKE` | タグファイルの作成 | — |  |
| ⬜ | `F_DIRECT_TAGJUMP` | ダイレクトタグジャンプ | — |  |
| ⬜ | `F_TAGJUMP_KEYWORD` | キーワードを指定してダイレクトタグジャンプ | — |  |
| ⬜ | `F_OPEN_HfromtoC` | 同名のC/C++ヘッダー(ソース)を開く | Shift+Ctrl+C / Shift+Ctrl+H |  |
| ⬜ | `F_COMPARE` | ファイル内容比較 | Ctrl+Enter |  |
| ⬜ | `F_DIFF_DIALOG` | DIFF差分表示(ダイアログ) | — |  |
| ⬜ | `F_DIFF_NEXT` | 次の差分へ | — |  |
| ⬜ | `F_DIFF_PREV` | 前の差分へ | — |  |
| ⬜ | `F_DIFF_RESET` | 差分の全解除 | — |  |
| ⬜ | `F_BRACKETPAIR` | 対括弧の検索 | Ctrl+[ / Ctrl+] |  |

### ツール(T) 工具

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| 🟡 | `F_RECKEYMACRO` | キーマクロの記録開始／終了 | Shift+Ctrl+M | 会话内录制; Shift+Ctrl+M 未绑 |
| ⬜ | `F_SAVEKEYMACRO` | キーマクロの保存 | Ctrl+M |  |
| ⬜ | `F_LOADKEYMACRO` | キーマクロの読み込み | Ctrl+L |  |
| 🟡 | `F_EXECKEYMACRO` | キーマクロの実行 | Shift+Ctrl+L | 回放; Shift+Ctrl+L 未绑 |
| 🟡 | `F_EXECEXTMACRO` | 名前を指定してマクロ実行 | — | prompt() 文本 DSL |
| ⬜ | `F_EXECMD_DIALOG` | 外部コマンド実行 | Ctrl+F5 |  |
| ⬜ | `F_ACTIVATE_SQLPLUS` | Oracle SQL*Plusをアクティブ表示 | Shift+F11 |  |
| ⬜ | `F_PLSQL_COMPILE_ON_SQLPLUS` | Oracle SQL*Plusで実行 | — |  |
| 🟡 | `F_HOKAN` | 入力補完 | Ctrl+Space / Ctrl+/ | 菜单可用; Ctrl+Space 被编辑区吞掉 (死代码) |

### 設定(O) 设置

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| ⬜ | `F_SHOWTOOLBAR` | ツールバーの表示 | Ctrl+1 |  |
| ⬜ | `F_SHOWFUNCKEY` | ファンクションキーの表示 | Ctrl+2 |  |
| ✅ | `F_SHOWTAB` | タブの表示 | — |  |
| ✅ | `F_SHOWSTATUSBAR` | ステータスバーの表示 | Ctrl+3 |  |
| ✅ | `F_SHOWMINIMAP` | ミニマップの表示 | — |  |
| 🟡 | `F_TYPE_LIST` | タイプ別設定一覧 | Ctrl+4 | 同左 |
| 🟡 | `F_OPTION_TYPE` | タイプ別設定 | Ctrl+5 | 同左 |
| 🟡 | `F_OPTION` | 共通設定 | Ctrl+6 | 选项列表弹窗, 非多页共通設定 |
| 🟡 | `F_FONT` | フォント設定 | Ctrl+7 | 选项列表 |
| ⬜ | `F_FAVORITE` | 履歴の管理 | — |  |
| 🟡 | `F_TMPWRAPNOWRAP` | 折り返さない（一時設定） | Ctrl+Alt+X | 只有自动换行开/关 |
| ⬜ | `F_TMPWRAPSETTING` | 指定桁で折り返す（一時設定） | Ctrl+Alt+S |  |
| 🟡 | `F_TMPWRAPWINDOW` | 右端で折り返す（一時設定） | Ctrl+Alt+W | 只有自动换行开/关 |
| ⬜ | `F_WRAPWINDOWWIDTH` | 現在のウィンドウ幅で折り返し | Ctrl+Alt+U |  |
| ⬜ | `F_SELECT_COUNT_MODE` | 文字カウントの方法を取得、設定する | — |  |
| ⬜ | `F_CHGMOD_INS` | 挿入／上書きモード切り替え | Ins |  |
| ⬜ | `F_VIEWMODE` | ビューモード | — |  |
| ⬜ | `F_TOGGLE_KEY_SEARCH` | キーワードヘルプ自動表示 | — |  |
| ⬜ | `F_CHG_CHARSET` | 文字コードセット指定 | — |  |
| ⬜ | `F_CHGMOD_EOL_CRLF` | 入力改行コード指定(CRLF) | — |  |
| ⬜ | `F_CHGMOD_EOL_LF` | 入力改行コード指定(LF) | — |  |
| ⬜ | `F_CHGMOD_EOL_CR` | 入力改行コード指定(CR) | — |  |

### ウィンドウ(W) 窗口

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| 🟡 | `F_SPLIT_V` | 上下に分割 | F4 / Shift+Ctrl+- | 有分割; F4 未绑 |
| 🟡 | `F_SPLIT_H` | 左右に分割 | Shift+F4 / Shift+Ctrl+\\ | 有分割; Shift+F4 被占为「查找下一个」 |
| 🟡 | `F_SPLIT_VH` | 縦横に分割 | Shift+Ctrl+; | 有四分割 |
| 🟡 | `F_WIN_CLOSEALL` | すべて閉じる | — | = 关闭全部标签 |
| ⬜ | `F_TAB_CLOSEOTHER` | このタブ以外を閉じる | — |  |
| 🟡 | `F_NEXTWINDOW` | 次のウィンドウ | Ctrl+Tab | 菜单「下一个标签」; Ctrl+Tab 未绑 |
| 🟡 | `F_PREVWINDOW` | 前のウィンドウ | Shift+Ctrl+Tab | 同左 |
| ⬜ | `F_WINLIST` | ウィンドウ一覧 | — |  |
| 🟡 | `F_CASCADE` | 重ねて表示 | Ctrl+Alt+E | 假实现 (打开新窗口) |
| 🟡 | `F_TILE_V` | 上下に並べて表示 | Ctrl+Alt+H | 假实现 (切全屏) |
| 🟡 | `F_TILE_H` | 左右に並べて表示 | Ctrl+Alt+T | 假实现 (切全屏) |
| ⬜ | `F_TOPMOST` | 常に手前に表示 | — |  |
| ⬜ | `F_BIND_WINDOW` | グループ化 | — |  |
| ⬜ | `F_TAB_CLOSELEFT` | 左をすべて閉じる | — |  |
| ⬜ | `F_TAB_CLOSERIGHT` | 右をすべて閉じる | — |  |
| ⬜ | `F_NEXTGROUP` | 次のグループ | — |  |
| ⬜ | `F_PREVGROUP` | 前のグループ | — |  |
| ⬜ | `F_TAB_MOVERIGHT` | タブを右に移動 | — |  |
| ⬜ | `F_TAB_MOVELEFT` | タブを左に移動 | — |  |
| ⬜ | `F_TAB_SEPARATE` | 新規グループ | — |  |
| ⬜ | `F_TAB_JOINTNEXT` | 次のグループに移動 | — |  |
| ⬜ | `F_TAB_JOINTPREV` | 前のグループに移動 | — |  |
| ⬜ | `F_MAXIMIZE_V` | 縦方向に最大化 | Shift+Ctrl+Alt+↑ |  |
| ⬜ | `F_MAXIMIZE_H` | 横方向に最大化 | Shift+Ctrl+Alt+→ |  |
| ⬜ | `F_MINIMIZE_ALL` | すべて最小化 | Shift+Ctrl+Alt+↓ |  |
| ⬜ | `F_REDRAW` | 再描画 | F5 |  |
| ⬜ | `F_WIN_OUTPUT` | アウトプットウィンドウ表示 | — |  |
| ⬜ | `F_DLGWINLIST` | ウィンドウ一覧ダイアログ | — |  |

### ヘルプ(H) 帮助

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| 🟡 | `F_HELP_CONTENTS` | ヘルプ目次 | Alt+F1 | 内置 wiki |
| ⬜ | `F_HELP_SEARCH` | ヘルプキーワード検索 | Shift+Alt+F1 |  |
| 🟡 | `F_MENU_ALLFUNC` | コマンド一覧 | Shift+F1 | 快捷键列表 |
| 🟡 | `F_CREATEKEYBINDLIST` | キー割り当て一覧をコピー | Ctrl+Q | 快捷键列表, 不是复制到剪贴板 |
| ⬜ | `F_EXTHELP1` | 外部ヘルプ１ | Ctrl+F1 |  |
| 🟡 | `F_EXTHTMLHELP` | 外部HTMLヘルプ | F1 | F1 = 内置 wiki |
| ✅ | `F_ABOUT` | バージョン情報 | Shift+Ctrl+F1 |  |

### （仅默认键位, 不在默认菜单）

| 状态 | 命令 | Sakura 名称 | 默认键 | 现状说明 |
|---|---|---|---|---|
| ✅ | `F_CANCEL_MODE` | 各種モードの取り消し | Esc |  |
| ✅ | `F_LEFT` | カーソル左移動 | ← |  |
| ⬜ | `F_LEFT_SEL` | (範囲選択)カーソル左移動 | Shift+← |  |
| ✅ | `F_LEFT_BOX` | (矩形選択)カーソル左移動 | Alt+← |  |
| ✅ | `F_UP` | カーソル上移動 | ↑ |  |
| ⬜ | `F_UP_SEL` | (範囲選択)カーソル上移動 | Shift+↑ |  |
| ⬜ | `F_WndScrollDown` | テキストを１行下へスクロール | Ctrl+↑ |  |
| ✅ | `F_UP_BOX` | (矩形選択)カーソル上移動 | Alt+↑ |  |
| ✅ | `F_RIGHT` | カーソル右移動 | → |  |
| ⬜ | `F_RIGHT_SEL` | (範囲選択)カーソル右移動 | Shift+→ |  |
| ✅ | `F_RIGHT_BOX` | (矩形選択)カーソル右移動 | Alt+→ |  |
| ✅ | `F_DOWN` | カーソル下移動 | ↓ |  |
| ⬜ | `F_DOWN_SEL` | (範囲選択)カーソル下移動 | Shift+↓ |  |
| ⬜ | `F_WndScrollUp` | テキストを１行上へスクロール | Ctrl+↓ |  |
| ✅ | `F_DOWN_BOX` | (矩形選択)カーソル下移動 | Alt+↓ |  |
| ⚪ | `F_HalfPageDown` | 半ページダウン | Ctrl+PgDn(RollUp) |  |
| ⬜ | `F_HalfPageDown_Sel` | (範囲選択)半ページダウン | Shift+Ctrl+PgDn(RollUp) |  |
| ⬜ | `F_HalfPageDown_BOX` | (矩形選択)半ページダウン | Ctrl+Alt+PgDn(RollUp) |  |
| ⚪ | `F_HalfPageUp` | 半ページアップ | Ctrl+PgUp(RollDn) |  |
| ⬜ | `F_HalfPageUp_Sel` | (範囲選択)半ページアップ | Shift+Ctrl+PgUp(RollDn) |  |
| ⬜ | `F_HalfPageUp_BOX` | (矩形選択)半ページアップ | Ctrl+Alt+PgUp(RollDn) |  |
---

## 附录 B — 按键运行时结果

方法: 内置浏览器 `file://` 打开 editor.html(极简布局, Vim 关闭), 光标放在 `foo bar baz` 第 0 行第 4 列。向编辑区派发 `KeyboardEvent`, 并给 58 个处理函数插桩, 记录实际被调用的函数。打开文件 / 保存 / 全屏 / 帮助类函数只记录、不执行。标 * 的 8 个键额外用真实按键复测, 结果相同。

| 键 | 实际调用 | 观察 |
|---|---|---|
| Ctrl+A | selectAllText | 产生选区 |
| Ctrl+B | — | |
| Ctrl+C | copySelection | |
| Ctrl+D / Shift+Ctrl+D | — | 菜单标了 Ctrl+D「删除行」 |
| Ctrl+E | cutLine | 文本变化 |
| Shift+Ctrl+E | deleteLine | 文本变化 |
| Ctrl+F | openFind | 模态框「查找」 |
| Ctrl+G | openGrep | |
| Ctrl+H | openReplace | 模态框「查找和替换」 |
| Ctrl+I | — | 菜单标了「增量查找」 |
| Ctrl+J | openGotoLine | 原生 `prompt()` |
| Ctrl+K | — | 和弦前缀 |
| Shift+Ctrl+K | deleteToLineEnd | 文本变化 |
| Ctrl+L / Shift+Ctrl+L / Ctrl+M / Shift+Ctrl+M | — | |
| Ctrl+N / Ctrl+O | createNewFile / promptOpenFile | |
| Ctrl+P / Ctrl+Q / Ctrl+R / Ctrl+T | — | |
| Ctrl+S / Shift+Ctrl+S | saveByKey | |
| Ctrl+U / Shift+Ctrl+U | cutToLineStart / deleteToLineStart | 文本变化 |
| Ctrl+V / Ctrl+X / Ctrl+Y / Ctrl+Z | paste / cut / redo / undo | |
| Ctrl+W | — | 代码里是"关闭标签" |
| Ctrl+Space | — | 补全死代码 |
| Ctrl+[ / Ctrl+1 / Ctrl+6 / Ctrl+Tab | — | |
| Shift+Tab | insertCharMulti | 插入 2 个空格 |
| Tab* | — | 插入 2 个空格 |
| Ctrl+BkSp | deleteToWordStart | 文本变化 |
| Ctrl+Del | deleteLineDown | 删除下方行 |
| Shift+Del | — | 删除 1 个字符 |
| Ins* / Shift+Ins / Ctrl+Ins | — | 无覆盖模式 |
| Ctrl+Home / Ctrl+End* / Shift+Home | — | |
| Ctrl+←/→* / Shift+→* / Ctrl+↑ | — | Shift+→ 只移动光标, 不产生选区 |
| Shift+Ctrl+↑ | moveLines | 上移行 |
| PgDn* / Ctrl+PgDn / Alt+PgDn | — | |
| Alt+↓ / Ctrl+Alt+↓ / Alt+Home | boxMove / boxEdge | 产生矩形选区 ✅ |
| F1 | showHelp | |
| F2 | bookmarkNext | |
| Ctrl+F2 | bookmarkNext | **应为设置/解除书签** |
| F3 / Ctrl+F3 / Shift+F4 | findNext → openFind | 无检索词时弹查找框 |
| F4 / F5 / F6 / F7 / F8* / F9* / Shift+F9 / Shift+F10 / F11 | — | |
| Shift+F6 | startColumnSelect | 产生矩形选区 ✅ |
| F12 / Shift+F12 | gotoDefinitionLocal | |
| Alt+A / L / R / N / ; | — | |

---

## 附录 C — 实现进度(2026-10-21 更新)

附录 A 是 2026-10 初的快照。此后已补齐的命令族(每项都有 `tests/mock` 里的真实按键/鼠标回归用例):

| 命令族 | 状态 |
|---|---|
| 键盘移动 / 范围选择 / 矩形选择(含单词·页·文件首尾) / 覆盖模式 / TAB 字符 | ✅ |
| 文件编码·BOM·换行读写保持, 以指定编码重新打开, コード変換 11 命令 | ✅(Rust) |
| 日文输入法(IME)组合输入 | ✅ |
| 检索 / 置換 / インクリメンタル(通常·正则·MIGEMO) / 検索マーク / 対括弧 / ブックマーク / タグ(生成·跳转·关键字) / DIFF / 次の関数·変更行 | ✅ |
| Grep: 独立结果窗口, 边搜边显示, 可追加 | ✅ |
| 复制变体 / 转换 / 整形 / SPACE インデント / Base64·uudecode 保存 / 挿入系 | ✅ |
| 宏: 录制·保存·读取·回放 + Sakura 默认键 | ✅(按键序列宏) |
| 表示: 工具栏·功能键栏·标尺·空白标记·当前行·閲覧モード·文件属性·窗口一览·F5 | ✅ |
| 印刷 / プレビュー / ページ設定 / ブラウズ | ✅ |
| 共通設定 / キー割り当て(可改键) / 履歴の管理 / 指定桁折り返し | ✅ |
| 大纲: Rust·Python + C++·JS·TS·Go·Java·C#·Markdown 层级 | ✅(Rust) |
| プラグイン UI | ✅ UI;WASM 运行时需 `wasm` feature 构建 |

**此前列为范围外、现已补上**: 标签分组(下/上一分组、移到新分组/下一/上一分组、合并、关闭分组, 标签条只显示当前组)、纵向/横向最大化与全部最小化(桌面窗口插件)、SQL*Plus(在终端里 `sqlplus` / `sqlplus @文件`)、再変換(把原生选区对齐到编辑器选区, 由 IME「変換」键接手 — 浏览器无法编程触发 TSF 再変換)、带选区的次/前の変更行。

**不做**: Oracle SQL*Plus 的专用窗口激活(只提供终端方式)。

**仍然是已知差距**:编辑逻辑大部分仍在前端 JS(ARCH-01 的迁移未做; 本轮新增的编码转换、MIGEMO 展开、层级大纲、半角カナ表在 Rust); 大文件已做窗口化渲染(>3000 行, 自动换行关闭时; 5 万行键入个位数 ms)。

## 附录 D — 逐条覆盖审计(2026-10-21)

`tests/mock/specs/web/sakura-coverage.test.mjs` 对附录 A 的**每一条** `F_*` 命令(解析本文档得到, 对照表 `tests/mock/fixtures/sakura-map.mjs`)做一次真实驱动:
有 Sakura 默认键的用真实按键, 没有默认键的走命令; 驱动前后抓取「文档 / 光标 / 选区 / 标签 / 对话框 / 状态栏 / 剪贴板 / 滚动 / 界面标记」快照, 必须有可观察的变化且不抛异常。
首轮审计发现并已修复的缺口: 重新打开编码的 Ctrl+Alt+F7–F12、Ctrl+. / Ctrl+\ 复制、F6 范围选择模式、(範囲選択)段落移动、(矩形選択)逻辑行首、半角カナ→全角、F4/Shift+F4 分割、Ctrl+Tab 窗口切换、Ctrl+Q 键位一览、Shift+F1 全機能一覧、Ctrl+4–7 设置对话框、Ctrl+Alt+S/W/U/X 折返し、キーワードヘルプ自动表示。
注意 `Ctrl+Alt+S` 现在是 Sakura 的「指定桁で折り返す」, 原先本项目占用它的「全部保存」改为 `Ctrl+Alt+Shift+S`。

**口径说明**: 该审计证明「每条命令都可达且会产生效果」, 不等于逐字节与 Sakura 行为一致; 行为细节另有各专项用例(编码字节往返、查找/置換、矩形选择、IME 等)。
