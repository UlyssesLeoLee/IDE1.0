# ide-fast 与 Tauri/Web 版的视觉对照 (VISUAL-PARITY)

目的: 完整界面 (View 菜单里打开的那套) 要看起来像 Tauri 版。默认的极简单文本框界面不动。
本文只记差异, 不含代码改动。Tauri 值读自 `crates/ide-shell-web/src/editor.html` 的 CSS 变量 / 规则, 并用 Playwright 在 1280x800 下取了 `getComputedStyle` 交叉核对;
ide-fast 值读自源码 (`panels.rs` / `app/docks.rs` / `app/titlebar.rs` / `app.rs`) 并从截图取像素。

## 0. 截图与方法

目录 `ide-fast/docs/parity-shots/` (小 PNG, 共约 2 MB)。

| 前缀 | 来源 | 尺寸 | 内容 |
|---|---|---|---|
| `web-min-{dark,light}-*` | Playwright, 默认 **极简布局** (`ide10.layout=min`) | 1280x800 | 01 默认 / 02 左抽屉展开 / 03 底部展开 / 04 右栏 Chat 展开 / 05 右栏收起 / 06 三处全展开 / 07 三处全折叠 / 08 文件菜单 / 09 视图菜单 / 10 键入 Markdown 文本 / 11 查找 (浮动对话框) / 12 Markdown 预览 (仅 dark) |
| `web-full-{dark,light}-*` | 同上, **完整布局** (`ide10.layout=full`) | 1280x800 | 01 默认 (左栏 + 活动栏 + 底部头 + 右细条) / 03 底部折叠切换 / 04-05 右栏 / 08-09 菜单 / 10 键入 / 11 查找 |
| `fast-dark-*` | `tests/gui-shot.ps1`, `IDE_FAST_SETTINGS=off`, 打开 a.md + b.rs | 1100x720 (窗口默认, 与 Tauri 不同宽) | 01 默认 / 02 左栏折叠 / 03 展开 / 04-05 底部展开 / 细条 / 06-07 右栏展开 / 细条 / 08 三处全折叠 / 09 三处全展开 / 10 文件菜单 / 11 视图菜单 / 12 查找栏 / 13-15 Markdown 标签与预览 |
| `fast-light-*` | 同上, 视图菜单切「深色主题」 | 1100x720 | 20 全展开 / 21 全折叠 / 22 查找栏 (截图里叠了一个搜狗输入法候选框, 与 ide-fast 无关) / 23 视图菜单 |

约束与未覆盖: (1) ide-fast 窗口 1100x720 不是 1280x800, 比较的是形状 / 数值而不是逐像素; (2) Tauri 的 Markdown 是编辑区内的整块覆盖预览 + 工具条, 不是右侧停靠区 (见 12 号图), ide-fast 的预览是右侧停靠区, 两者结构不同; (3) hover / active 态没有截图, 值读自 CSS; (4) 没有 Tauri 的「真 .md 文件」截图, 用「键入后把活动标签改名为 a.md」模拟; (5) gui-shot 是「点击后 400 ms 截屏」, egui 不请求重绘时画面落后一拍, 弹出菜单要多截一次才出来, 所以部分 ide-fast 图是点两次后的稳定态。

## 1. 优先级 P0: 三个折叠按钮

### 1.1 总览

| 项 | Tauri | ide-fast 现状 | 差 |
|---|---|---|---|
| 形态 | **细条 (strip) 常驻**: 折叠和展开两种状态细条都在, 按钮在细条顶部, 无边框的文字箭头 | 折叠时才出现 22pt 细条, 里面一个**带底色的圆角小按钮** (egui `small_button`); 展开时按钮移到面板头部右端, 细条消失 | 形态不同: 细条宽 16 对 22; 按钮无框对有框; ide-fast 展开时没有常驻把手 |
| 默认 | 折叠 (极简布局) | 展开 (`ViewState::default` 里 `side_open: true`) | `IDE_FAST_SETTINGS=off` 下与 Tauri 默认不同; 真实默认按「极简」另行处理 |
| 触发 | 点按钮 = 一键切换 (左: 活动栏 + 侧栏一起开合) | 点按钮切换 `*_collapsed`; 另有标题栏 3 个「矩形图标」开关 (Tauri 没有) | 多出的标题栏开关是 ide-fast 独有 |

### 1.2 左 (导航 / 文件)

| 属性 | Tauri (`#left-strip`, `#left-strip-btn`) | ide-fast (`dock_strip` / `dock_chevron`, Side) | 差 |
|---|---|---|---|
| 位置 | 栅格第 1 列, 第 2 行 (菜单栏下到底部面板头上), 全高; 展开后活动栏 (44) + 侧栏 (240) 排到它右边, 细条仍在最左 | 折叠: `egui::Panel::left("side_strip")` 宽 22, 全高, 在窗口最左; 展开: 按钮在侧栏页签行最右 | 展开态没有细条 |
| 细条宽 | `--w-strip: 16px` | `panels::STRIP = 22.0` | +6 |
| 细条底色 / 边 | `--bg-rail`: dark `#333333`, light `#f0f0f0`; 右边框 1px `--border` (dark `#333333` / light `#e5e5e5`); hover 叠 `--bg-hover` (dark `rgba(255,255,255,.08)`, light `rgba(0,0,0,.04)`) | 面板底色 (dark `#1B1B1B`, light `#F8F8F8`, 与侧栏同色); 无右边框 (由面板分隔线代替); 无 hover 叠色 | 细条应用 rail 色而不是面板色; 加 1px 右边框; 加 hover |
| 按钮尺寸 | 100% 细条宽 x 40px 高, 贴细条顶端 (y=30..70), padding 0 | 约 18x15 px (截图实测 x2..19, y58..72), 按内容自适应 | 应改成撑满细条宽 x 40 的无框按钮 |
| 按钮外观 | 背景透明, **无边框**, 圆角 0, 前景 `--fg-muted` (dark `#9d9d9d`, light `#5f5f5f`); hover: 前景 `--fg` (dark `#d4d4d4`, light `#1f1f1f`) + 背景 `--bg-hover`; focus-visible: 内圈 2px `--blue` | `small_button`: 有填充 (egui 弱按钮色 `#3c3c3c` dark, 浅灰 light), 圆角 2, 前景 `#D7D7D7` | 去掉填充和边框, 改用 hover 叠色 |
| 字形 | 文字 `›` (U+203A, 折叠, 表示「展开」) / `‹` (U+2039, 展开, 表示「折叠」), `font-size:16px; line-height:1`, 居中 | 实心三角 `▶` U+25B6 (折叠) / `◀` U+25C0 (展开), 约 11px | 字形 / 大小不同 (实心三角对细线箭头) |
| 提示 | `data-tip` 与 `aria-label`: 折叠时「展开导航 / 文件」, 展开时「折叠导航 / 文件」; 提示气泡: bg `#333` 白字, padding 4px 8px, 圆角 3, 11px, max-width 400, z 1000; 点击后状态栏提示「导航 / 文件: 展开 / 折叠」 | `on_hover_text("折叠左侧栏 (点击展开)")` — 无论哪种状态都是这句 | 文案应随状态变: 折叠时「展开导航 / 文件」, 展开时「折叠导航 / 文件」 |
| 行为 | `toggleLeftDrawer()` 一次 `setFlags({activityBar, sideBar})`; 折叠后焦点回编辑区; 状态落 `localStorage ide10.layout.flags`; 展开时刷新文件树 | `collapse_dock(Side)` 翻转 `cfg.view.side_collapsed`, 落设置文件 | 行为等价, 缺焦点回编辑区 / 状态栏提示 |
| 完整布局 | 完整布局**没有**左细条 (活动栏 + 侧栏始终并排, 无折叠按钮); 靠视图菜单 | 完整布局下同样有细条 | Tauri 只在极简布局有此按钮 |

### 1.3 右 (Chat / 右侧面板)

| 属性 | Tauri (`#right-strip`, `#right-strip-btn`, `#chat-dock`) | ide-fast (Right) | 差 |
|---|---|---|---|
| 位置 | 栅格第 4 列 (完整) / 第 5 列 (极简), 全高; **常驻** (即使从没打开过 Chat); Chat 本体 (`#chat-dock`) 在它左边 | 折叠才出现 `Panel::right("right_strip")`; 没有 AI / 预览开着时右边什么都没有 | 缺「常驻把手」: 空闲时 Tauri 右缘有 16px 细条 + 按钮, ide-fast 没有 |
| 细条宽 / 底色 / 边 | 16px; `--bg-rail`; 左边框 1px `--border`; hover 叠 `--bg-hover` | 22; 面板色; 无 | 同左 |
| 按钮 | 100% x 40, 顶端, 无框透明; `--fg-muted` -> hover `--fg` + `--bg-hover`; 字号 16 | 同左的 `small_button` | 同左 |
| 字形 | `‹` (折叠, 表示展开) / `›` (展开, 表示折叠) | `◀` (折叠) / `▶` (展开) | 方向语义一致, 形状不同 |
| 提示 | `data-tip="Chat (Ctrl+Shift+L)"` 固定 (aria-label 随状态: 「展开 Chat」/「折叠 Chat」); Chat 头上另有 `✕` 钮, title「折叠 (Ctrl+Shift+L)」 | 「折叠右侧面板 (点击展开)」 | 文案 |
| 展开宽 | `--w-chat-cur` 实测 340px | `default_size(Right)=340` (AI) / 420 (预览) | 相同 (AI) |
| 触发 | 按钮 / Ctrl+Shift+L / 菜单; `toggleChat()` 首次展开才建 DOM | 按钮 / 标题栏图标 / Ctrl+Alt+J (折叠) / Ctrl+Alt+L (AI) | 快捷键不同, 见 `keymap.rs` |

### 1.4 下 (底部面板)

| 属性 | Tauri (`#bottom-head`, `.bottom-toggle[data-cmd=bottom-toggle]`) | ide-fast (Bottom) | 差 |
|---|---|---|---|
| 形态 | **没有独立细条**: 面板头 (28px) 永远在, 折叠只是隐藏 `#bottom-body`; 头里仍有 TERMINAL / OUTPUT / PROBLEMS 三个页签 + 「+ 新建终端」+ 折叠钮 | 折叠: `Panel::bottom("bottom_strip")` 高 22, 内容 = `▲` 小按钮 + 弱色文字「终端」(当前页签名); 展开: 页签行右端 `▼` 小按钮 | 折叠态丢了其他页签 / 「新建终端」; 细条高 22 对头高 28 |
| 位置 | `grid-column: 1 / -1` 横跨整个窗口宽度 (活动栏 / 侧栏下方), 状态栏在它下面也是整宽 | 只在编辑区下方 (左侧栏全高, 状态栏和底部面板都从 x=250 开始) | **拓扑不同**: Tauri 底部 + 状态栏整宽; ide-fast 左栏占满全高 |
| 按钮 | 文字钮: 展开时写「折叠」, 折叠时写「展开」, `font-size 11px`, padding `2px 4px`, 实测 32x22 (两个汉字), `align-self:center`; 圆角 `--r-md` 5px (经 1096 行规则), 边 1px `--border-strong` (dark `#454545`); 前景 `--fg-muted`; 背景透明 (dark) | `▲` / `▼` 图标小按钮, 约 21x19 / 27x17 (实测), 无文字 | 文字对图标; 尺寸 / 边框不同 |
| light 外观 | 背景 `#ffffff`, 边 1px `#d0d7de`, 字 `#24292f`, 阴影 `0 1px 2px rgba(27,31,36,.04)`; hover: 背景 `#f6f8fa`, 边 / 字 `#0969da`; active: 背景 `#ebecf0`, 内阴影 `inset 0 1px 2px rgba(27,31,36,.08)` | 浅灰填充 | 同上 |
| dark hover | 背景 `--bg-hover` `rgba(255,255,255,.08)`, 字 `--fg`, 边 `--fg-dim` `#6a6a6a`; active `translateY(1px)` | egui hovered 填充 `#464646` | 值不同 |
| 提示 | `data-tip="折叠/展开"` 固定; 「+ 新建终端」tip「新建终端 (Ctrl+`)」 | 「折叠底部面板 (点击展开)」 | 文案 |
| 展开高 | 极简: `clamp(120px, 40vh, 320px)` (800 高下 320), 顶边 1px `--border-strong`; 完整: `clamp(120px, 40vh, 240px)` | `default_size(Bottom)=180`, 可拖; 展开时可拖大小 (Tauri 固定) | 默认高度不同 |
| 展开后焦点 | 终端 `#terminal-body` 滚到底; 折叠焦点回编辑区 | -- | -- |

### 1.5 P0 建议改法 (按收益排序)

1. 细条宽 22 -> 16, 底色换 rail 色, 加 1px 内侧边框, 去掉按钮填充 / 边框, 按钮撑满细条宽、高 40、贴顶, hover 叠 `--bg-hover`, 字形改 `›` / `‹` (16px)。`panels::STRIP` 的单测要跟着改 (`mode` 逻辑不动)。
2. 展开态也在原位保留 16px 细条 (按钮换成 `‹`), 去掉侧栏页签行里的小三角; 右栏做成常驻 (没有 AI / 预览时也留细条 + `‹`, 点它就是 `collapse_dock(Right)` 里已有的 `ai_toggle()` 分支)。
3. 底部: 折叠态保留头条 (28 高) 与全部页签, 右端换成文字钮「展开」/「折叠」(走 `lk`/i18n), 边框 / 圆角 5 / 字号 11。
4. 提示文案随状态变化 (见表), 提示气泡样式用 `#333` 底 / 白字 / 圆角 3 / 11px。
5. 标题栏的 3 个矩形开关不是 Tauri 的东西, 可保留为 ide-fast 额外入口, 但视觉上应降调 (不要「按下」高亮)。

## 2. 全局令牌

Tauri 的 `:root` 变量与 `THEMES.dark / light` (setTheme 注入); ide-fast 用 egui 默认 `Visuals::dark()/light()` (`app.rs:2477`, 未做任何 `Style` 覆盖), 另有 `view.rs::ThemeColors::builtin`。

| 令牌 | Tauri dark | Tauri light | ide-fast dark (实测) | ide-fast light (实测) |
|---|---|---|---|---|
| 菜单栏 / 标题栏 `--bg-bar` | `#252526` | `#f8f8f8` | `#1B1B1B` | `#F8F8F8` |
| 侧栏 / 编辑区 `--bg-side` `--bg-edit` | `#1e1e1e` | `#ffffff` | 侧栏 `#1B1B1B`, 编辑区 **`#0A0A0A`** | 侧栏 `#F8F8F8`, 编辑区 `#FFFFFF` |
| 活动栏 / 细条 `--bg-rail` | `#333333` | `#f0f0f0` | (无, 面板色) `#1B1B1B` | `#F8F8F8` |
| 标签栏 `--bg-chrome` | `#2a2a2a` | `#f2f2f2` | `#1B1B1B` | `#F8F8F8` |
| 浮层 / 对话框 `--bg-raised` | `#2d2d2d` | `#ffffff` | egui `window_fill` (≈ `#1B1B1B`) | `#F8F8F8` |
| 底部面板 `--bg-bottom` | `#252526` | `#f8f8f8` | `#1B1B1B` | `#F8F8F8` |
| 状态栏 `--bg-status` | **`#007acc`** (蓝) | **`#005fb8`** (蓝) | `#202020` (灰) | `#F0F0F0` |
| 状态栏字 `--fg-on-accent` | `#ffffff` | `#ffffff` | 普通前景 | 普通前景 |
| 边框 `--border` / `--border-strong` | `#333333` / `#454545` | `#e5e5e5` / `#cccccc` | `#3C3C3C` (分隔线) | `#C8C8C8` 附近 |
| 前景 `--fg` / `--fg-muted` / `--fg-dim` | `#d4d4d4` / `#9d9d9d` / `#6a6a6a` | `#1f1f1f` / `#5f5f5f` / `#8a8a8a` | `#D2D2D2` / `#8C8C8C` | `#1E1E1E` / `#787878` |
| 强调 `--blue` | `#569cd6` | `#005fb8` | `(86,156,214)` | `(0,90,180)` |
| green / yellow / red / orange | `#4ec9b0` / `#dcdcaa` / `#f48771` / `#ce9178` | `#0f7b41` / `#7d6608` / `#c72e0f` / `#b35900` | `(106,190,120)` / `(220,200,90)` / `(230,90,90)` / `(214,150,80)` | `(20,130,50)` / `(150,110,0)` / `(190,40,40)` / `(180,90,0)` |
| hover `--bg-hover` / active `--bg-active` | `rgba(255,255,255,.08)` / `.14` | `rgba(0,0,0,.04)` / `.08` | egui 悬停 `#464646` 附近 | egui |
| 活动 / 选中控件填充 | `--bg-active`; 活动页签下划线 `--blue` | 同 | egui `selection.bg_fill` `#005C80` (dark) / `#90D1FF` 附近 (light) 整块填充 | |
| 焦点环 | `0 0 0 1px --bg-edit, 0 0 0 3px rgba(86,156,214,.85)` (light: `#fff` + `#005fb8`) | | egui 默认 | |

间距 / 圆角 / 字号令牌 (Tauri): `--sp-1..6 = 2/4/8/12/16/24px`; 圆角 `--r-sm/md/lg = 3/5/8px`; 字号 `--fs-xs/sm/md/lg = 11/12/13/15px`, 图标 18px, 欢迎页标题 34px; 动效 `--dur-fast 120ms`, `--dur-base 200ms`, 缓动 `cubic-bezier(.4,0,.2,1)`; 层级 popup 50 < menubar 100 < menu 200 < modal 500 < tip 1000。ide-fast: egui 默认 `item_spacing (8,3)`, `button_padding (4,1)`, 控件圆角 2, 窗口圆角 6, 无动效。

最大的两处全局差: **状态栏是蓝底白字 (Tauri) 还是灰底 (ide-fast)**, **编辑区 `#0A0A0A` (ide-fast dark) 对 `#1e1e1e`**。后者只是 egui `extreme_bg_color`; 设 `extreme_bg_color = bg_edit` 并让 `panel_fill = --bg-side`, 状态栏 `fill = bg_status` 即可贴近。

## 3. 分区对照

### 3.1 标题栏 / 菜单栏
| | Tauri | ide-fast | 差 |
|---|---|---|---|
| 高 / 底 / 边 | 30px (`--h-menubar`), `--bg-bar`, 下边 1px `--border` | 30px (`BAR_H`), `panel_fill`, 无下边 | 加下边框、改底色 |
| 左 | logo 16x16 + 标题 12px / 600 (无标题时「IDE1.0」, 文档名 + ` - IDE1.0`), `padding 0 10 0 8`, `gap 6`, 最大 28ch | 16x16 图标, **无标题文字** | 缺标题文字 |
| 菜单钮 | `.mb-btn`: 12px, 前景 `--fg`, padding `2px 8px` + 1px 透明边 + 圆角 3 (实测 54x29), hover / 打开 `--bg-hover`; 每个菜单名后带带下划线的助记字母 (`文件 F`, 11px muted, `margin-left 6`) | egui `menu` 按钮, `button_padding (5,2)`, 无助记字母 | 缺助记字母 |
| 菜单项 | 文件 / 编辑 / 搜索 / 视图 / 工具 / 设置 / 窗口 / 帮助 (8) | 文件 / 编辑 / 查找 / 折叠 / 插件 / 设置 / 书签 / 窗口 / 宏 / 视图 / 帮助 (11) | 结构不同, 功能全保留, 不建议为视觉合并 |
| 右 | 「+ 新建终端」钮 (24 高, 11px/500, 圆角 5, 1px 边, dark `rgba(255,255,255,.05)` 底 / light `#fff` 底 + `#d0d7de` 边) + 内核横幅 (24px 高) ; 桌面版再加窗口钮 (46 宽, 关闭 hover `#e81123`) | 6 个文字快捷钮 (`新 开 存 找 >_ \|\|`, 宽 270, ≥900 窗宽才画) + 3 个面板开关 + 窗口钮 3x44 (关闭 hover `rgb(196,43,28)`) | 文字快捷钮挤在一起读不出 (截图里显示成「新开存找>_\|\|」), 应换图标 (Tauri 的 `#toolbar .tb-btn` 是 26x24 的 SVG 图标, 圆角 4, 间隔 1) |
| 工具栏 | 默认关; 开了是 `#toolbar` 圆角 6 小条 (dark `rgba(255,255,255,.03)` + 1px `rgba(255,255,255,.08)`; light `#f0f0f0` + `#e0e0e0`) 放在标题栏右侧 | 同名选项, 画在标题栏下一行 | 位置 |

### 3.2 菜单下拉
Tauri: `.menu-dropdown` 背景 `--bg-bar`, 1px `--border`, 阴影 `0 2px 8px rgba(0,0,0,.5)`, `min-width 240`, `padding 4px 0`, 圆角 0, 最大高 `calc(100vh - 34px)`; 行 `.menu-row` padding `5px 14px`, 12px, hover `--bg-active`; 快捷键 11px `--fg-muted`, `margin-left 24`; 分隔线 1px; 禁用 `--fg-dim`; 子菜单向右展开 (`left:100%`)。
ide-fast: egui 菜单, 圆角 6, 边 1px, 阴影 egui 默认, 行内带复选框 (勾选方块) 与快捷键, 无 `min-width` 约束。差: 圆角 (0 对 6)、行高 (Tauri 约 26 对 egui ≈ 23)、勾选样式 (Tauri 为 `✓` 文字, ide-fast 为复选框)。

### 3.3 标签栏
Tauri: `#tabs` 28 高, `--bg-chrome`, 下边 1px; `.tab` 12px (实测), padding `0 26px 0 12px` (含关闭钮位), 右边 1px `--border`; 活动: 背景 `--bg-edit` (dark `#1e1e1e`), 前景 `--fg`, 圆角 `3 3 0 0`; 关闭钮 `×` `--fg-muted`, hover `--bg-active`; 面包屑 22 高 11px; 极简布局下标签栏 `auto` (单文档时隐藏)。
ide-fast: 标签 = egui `selectable_label` + 独立 `×` 小按钮, 活动标签为整块 selection 色 (dark `#005C80`, light `#90D1FF` 附近) 圆角 2, 没有 28 高的条; 只有一个标签时整条收起 (同 Tauri 的 `auto`)。差: 活动标签应为「与编辑区同色、无色块」而不是蓝块; 关闭钮应在标签内部; 标签间应有 1px 竖线。

### 3.4 左栏 + 活动栏
| | Tauri | ide-fast |
|---|---|---|
| 活动栏 | 44 宽, `--bg-rail`, `padding 4px 0`, `gap 4`, 按钮 **32x32** (CSS 基础 36, 被后规则覆盖; 实测 32), 圆角 6, 图标 18px SVG (stroke 1.8, 圆角端点), 前景 `--fg-muted`; 活动: dark 背景 `rgba(255,255,255,.12)` 前景 `#fff` / light 背景 `#e8edf3` 前景 `#005fb8`, 左侧 2~3px 强调竖条 (`--blue`); 5 个: 资源管理器 / 搜索 / 源代码管理 / 大纲 / 扩展 | 默认关; 开了是 `Panel::left("rail")` **64 宽**文字按钮 (`activity_rail`) | 宽 64 对 44, 文字对 SVG 图标; 无竖条 |
| 侧栏 | 默认 240 宽 (`--w-side`), `--bg-side`, 右边 1px `--border`; `#side-body` padding 4, 12px; 页签条仅在有额外页时显示 (28 高, 11px, 活动 2px 下划线); 树行 `padding 2px 4px`, 行高 16, 文件夹 500 字重; 区块标题 11px / 600 / 大写 / `letter-spacing .06em` / muted | 默认 250 宽, 顶部是 `文件 / 大纲 / 搜索 / Git` 四个胶囊页签 (活动: selection 色块), 然后内容 | 页签样式 (胶囊块对无页签 / 下划线); 宽 250 对 240 |
| 空状态 | 「未打开项目」+ 「点工具栏「打开文件夹」导入项目」 + 蓝色「打开文件夹」钮 | 「打开文件夹…」灰色钮 | 文案 / 钮色 |

### 3.5 编辑区 (行号 / 当前行 / 选区 / 光标)
| | Tauri | ide-fast |
|---|---|---|
| 字体 | `--font-ui`: ui-monospace, "Cascadia Code", Consolas, "SF Mono", Menlo, "DejaVu Sans Mono", "Liberation Mono", "Microsoft YaHei UI", ..., monospace; 连字关闭 | `fonts::editor_family()` (内置等宽 + 系统 CJK); 默认 `font_px = 15`, `line_height = 1.0` 倍 |
| 字号 / 行高 | 正文 13px / 行高 18px (`--fs-md`; `#ed-content line-height:18px`); 行号 12px / 18px | 15px / 按字体行高 (截图约 17.5px) |
| 行号栏 | `#gutter`: 底 `--bg-side` (与侧栏同, 比编辑区略不同: dark `#1e1e1e` 同色), 前景 `--fg-muted`, `padding 4px 8px 16px`, 右对齐, `min-width 40`, 无分隔线 | 与编辑区同色 (`#0A0A0A`), 前景 muted, 当前行号加亮 (白); 无分隔线 |
| 内边距 | `#ed-content padding 4px 8px` | 约 6px 左 |
| 当前行 | `#curline` (视图菜单「当前行下划线」, 默认关): 1px `--blue` 底线, opacity .75; 完整布局 retro 另有高亮 | 默认开, 1px 灰线 (`#3C3C3C`) 整宽 | 默认开关与色不同 |
| 选区 | `::selection` `--bg-active` (dark `rgba(255,255,255,.14)`, light `#add6ff`); Visual 选区 `#264f78`; 列选 `#7a3e9d`; 补全高亮 `#094771` | egui `selection.bg_fill` (dark `#005C80`) 半透明 90 (`plugins.rs`) | dark 色偏青; light 与 `#add6ff` 近 |
| 查找高亮 | `::highlight(find-marks)` `rgba(255,213,0,.38)`; 括号对 `rgba(120,180,255,.5)` + 1px 外框 | `mark_bg = rgba(255,200,0,70)`, `bracket_bg = rgba(110,190,255,95)` | 近, 可取 Tauri 值 |
| 光标 | 2px 宽 x 18 高, 色 `--fg`, 闪烁 (vim 关时) | egui 文本光标 (细线) | 宽度 |
| 标尺 | 默认开: 16px 高, 10px 字, `--bg-bar` 底 | 默认关 | 默认值 |
| 语法色 (dark) | keyword `#569cd6`, type `#4ec9b0`, string `#ce9178`, comment `#6a9955`, number `#b5cea8` | 关键字蓝 `(86,156,214)`, 字符串绿 (截图 `hello` 为绿), 数字橙 | 字符串: Tauri 橙 `#ce9178`, ide-fast 绿; 数字: Tauri 浅绿 `#b5cea8`, ide-fast 橙 — 色相互换 |

### 3.6 右栏 / Markdown
| | Tauri | ide-fast |
|---|---|---|
| Chat 头 | 加粗「Chat」, 复选框「带上选中内容/当前章节」, `清空` `✕` 两个 `.tb-btn`; 底部: 模型名 + `effort` 下拉 + 「模型设置…」+ 文本框 (2 行) + 「发送」 | 「AI 对话」+ 清空 / 设置 / ▶ / × 四个小钮; 底部: 复选框 + 文本框 + 发送 |
| 钮样式 | 12px, padding `4px 8px`, 1px `--border-strong`, 圆角 5, hover `--bg-hover` | egui 小钮 (填充 `#3c3c3c`, 圆角 2, 无边) |
| Markdown | 编辑区内整块覆盖预览 (`#md-preview` `position:absolute; inset:0; padding 12px 24px`), 正文 `max-width 860`, 行高 1.7, 13px; 上方 Markdown 工具条 (B I S == ` H1-H4 列表 任务 引用 链接 图片 表格 代码块 章节 结构 ...); 表格 `border-collapse` 1px; 代码块 `--bg-raised` 底 | 右侧停靠区 `md_ui`, 顶部 `检查 / 导出 HTML`, 标题 22px 左右, 代码块深灰底 + 语言标注 | 结构不同 (覆盖 vs 并排); 缺 Markdown 工具条 |

### 3.7 底部面板内容
Tauri: 页签 `.bottom-tab` 11px, `padding 4px 12px`, 大写英文 (TERMINAL / OUTPUT / PROBLEMS), 活动: 前景 `--fg` (light `#005fb8`, 字重 500) + 2px `--blue` 下划线 + 底 `--bg-edit`; 圆角 `3 3 0 0`; 终端正文 `--term-fs 13px / 1.4`, 字体 Consolas / Cascadia Mono / MS Gothic, `padding 4px 8px`, 空状态居中双语文案 + `<kbd>` 小块 (`1px 6px`, 圆角 3, 11px)。
ide-fast: 页签 `终端 / 输出 / 问题` 胶囊 (活动: selection 色块), 下一行 `终端` 标题 + `清屏 重启 常驻会话 × 选区作标准输入 输出替换选区` 一排控件, 再是输出区和单行输入框 (`输入命令, Enter 执行`)。差: 页签样式、大写英文 vs 中文、多出一排控件。

### 3.8 状态栏
| | Tauri | ide-fast |
|---|---|---|
| 高 / 字 | 22px (`--h-status`), 11px, `padding 0 8px`, 整窗宽 | 约 27px (`inner_margin (8,3)` + 内容, 实测 y693..720), 约 12.5px, 仅编辑区下方宽 |
| 底色 | 蓝 `#007acc` / `#005fb8`, 字白 | 灰 `#202020` / `#F0F0F0`, 上边 1px 分隔线 |
| 项 | 每项 `padding 0 8px` + 右侧 1px 分隔 (白 20% 透明); 模式胶囊 (NORMAL / INSERT / VISUAL, 加粗, 底色 `color-mix(blue 62%, #000)`); `Ln 1, Col 1` / 挿入 / `UTF-8` / `CRLF` / 文件 / 提示 (弹性) / 语言 (右端) | `Ln, Col` / `UTF-8` 钮 / `LF` 钮 (带框的小钮) / 共 N 行 / 语言 / INS / 提示; 编码与换行是带圆角填充的按钮, 项间 egui 竖分隔 |
| 差 | 需要蓝底白字 + 项分隔线 + 去掉按钮底色; 高度 22 | |

### 3.9 滚动条
Tauri: 全局 `::-webkit-scrollbar` 12x12, 轨道透明, 滑块圆角 6, `border: 3px solid transparent` + `background-clip: content-box` (可见宽 6), 色 `--fg-dim` (dark `#6a6a6a` / light `#8a8a8a`), hover `--fg-muted`; 侧栏 / 终端 10px, `rgba(127,127,127,.45)` 圆角 5; 菜单 8px。
ide-fast: egui 滚动条 (浮动, 悬停时展开, 默认 `bar_width 10`, 静止时约 2px 细线, 色随 widget); 截图右缘可见窄条。差: 静止态宽度 / 色。

### 3.10 查找栏 / 对话框
| | Tauri | ide-fast |
|---|---|---|
| 查找 | 浮动对话框 `#modal.float .md-box`: `top 56 right 28 width min(440px, 92vw)`, 最大高 70vh, 阴影 `0 6px 24px rgba(0,0,0,.35)`; 标题「検索」15px / 500 + `✕`; 输入框占满, 选项复选框 (区分大小写 / 单词单位 / 正则 / 从头(尾)重新查找 / 全部强调显示), 底部 `↑ 上検索` `↓ 下検索` `关闭` | 窗口底部通栏 `Panel::bottom("find")`: `×` + `查找` 标签 + 输入框 + `↓ ↑` + 5 个复选框; **`×` 与「查找」标签重叠** (fast-light-22 左下角) |
| 通用对话框 | `#modal` 遮罩 `rgba(0,0,0,.4)`, `.md-box` 600 宽 (max 80vw / 80vh), 底 `--bg-raised`, 1px `--border-strong`, 圆角 8, 阴影 `0 16px 48px rgba(0,0,0,.4)`, 进入动画 200ms; 头 `padding 8px 12px` + 下边线, 脚 `padding 8px 12px` + 上边线 + `gap 8` 右对齐; 钮 `.md-btn` 12px, `4px 8px`, 1px `--border-strong`, 圆角 5, primary: 底 `--blue` 字 `--fg-on-accent` | egui `Window`: 圆角 6, 默认阴影, 无遮罩 |

### 3.11 欢迎页 / 空状态
Tauri 欢迎页: 居中 logo + 「IDE1.0」34px / 300 / `letter-spacing .14em` + 蓝色「打开文件夹」钮 (`padding 8px 24px`, 13px, 圆角 8) + 一排 `<kbd>` 快捷键提示。ide-fast 无欢迎页 (默认直接是空白文档)。

### 3.12 图标
Tauri: 全部 inline SVG, `viewBox 0 0 24 24`, `fill none; stroke currentColor; stroke-width 1.8; round caps/joins`, 活动栏 18x18, 工具栏 12-13px; 折叠箭头是文字 `›` `‹`。ide-fast: 图标字形来自文字字体 (`▶ ◀ ▲ ▼ × ✓`) + 标题栏自绘窗口钮; 尚无 SVG 图标 (`svg_lite` 目录是 Markdown / 内嵌图用)。

## 4. 截图里发现的缺陷 (与 Tauri 无关, 但影响观感)

1. 标题栏的文字快捷钮 `新 开 存 找 >_ ||` 连成一串, 没有分隔, 读不出是 6 个按钮。
2. 查找栏左端 `×` 压在「查找」标签上。
3. 折叠细条的提示永远写「折叠 ... (点击展开)」, 展开状态下点它却是「折叠」。
4. 折叠后「标题栏三个矩形开关」仍被 hover 高亮, 无法区分「按下」与「悬停」。

## 5. 建议实施顺序

1. P0 折叠按钮 (见 1.5): 细条 16px + rail 色 + 无框 40px 钮 + `›‹` + 文案随状态; 右栏常驻; 底部保留头条与文字钮。
2. 全局色: `extreme_bg_color`、`panel_fill`、状态栏蓝底白字 (一处 `Visuals` 构造即可)。
3. 菜单栏: 加下边框、标题文字、助记字母; 快捷按钮换 SVG 风格图标。
4. 标签栏 / 页签: 去掉整块蓝色填充, 改下划线 / 同色活动标签。
5. 查找栏、对话框: 修 `×` 重叠, 圆角 / 阴影 / 遮罩。
6. 行号 / 滚动条 / 选区色等细节。

每一步都要: 首帧 <= ~260 ms (用 `tests/perf.ps1 -Compare` 对比)、无控制台窗口、不丢功能、`cargo test -p ide-fast` 全绿。

## 6. 仍有差异 (第五轮之后的现状, 只列 egui 做不到的)

- 工具提示气泡 (#333 底 / 白字 / 圆角 3 / 11px): egui 0.37 的 `Tooltip` 用 `Frame::popup(全局 style)` 生成外框, 没有「只改提示」的样式钩子 (`Popup::style` 对 `on_hover_text` 不可达); 单改全局 `window_fill` 会连所有窗口 / 对话框一起变, 想分开只能给每个窗口传显式 Frame。其余第四轮列出的差异 (默认底部头 / 查找框装饰 / 复选框 / 工具菜单 / 700 高菜单裁切 / 标尺 / 终端工具行 / 无标题标签 / 状态栏 / 滚动条变体) 本轮都已对齐, 见第 8 节第五轮。

## 7. 有意保留的结构 / 功能差异 (不是样式问题, 也不是「做不到」)

- 菜单助记字母只做显示: 按 Alt+字母打开菜单是行为 (不是外观); egui 的 `MenuBar` 没有公开的「打开某个菜单」入口, 要做得重写菜单弹出状态, 未做。
- Markdown 预览: Tauri 是编辑区内整块覆盖 + 顶部工具条, ide-fast 是右侧停靠区 (功能结构差异); h1 / h2 下划线沿用 egui 分隔线。
- 「插件」「宏」「书签」「折叠」是 ide-fast 额外的菜单, 为不丢功能保留; Tauri「工具」里的 SQL*Plus / 历史管理在 ide-fast 没有对应功能, 工具菜单不列。
- 光标宽度 / hover / active 态没有逐像素截图对比。截图含窗口隐形边框 (约 7px) 下方的其他窗口像素, 贴边杂点不是 ide-fast 绘制。

## 8. 实施记录 (第一至五轮)

已改 (截图 `parity-shots/fast2-*`, dark 与 light 各取全展开 / 全折叠 / 查找+分栏):
1. 令牌: `ThemeColors` 新增 `bg_side / bg_rail / bg_chrome / bg_hover`, 内置深 / 浅色改成 Tauri 值 (编辑区 #1e1e1e / #fff, 菜单栏 #252526 / #f8f8f8, rail #333 / #f0f0f0, 状态栏 #007acc / #005fb8, 前景 / 弱前景 / 边框 / 蓝); 没选主题插件时由 `ThemeColors::visuals()` 套到 egui `Visuals` (面板 / 编辑区 / 控件填充 / 选区 / 圆角 3 / 菜单圆角 2 / 窗口圆角 8)。主题插件走原来的 `plugins::visuals_glass`, 没给的 rail / chrome / side 跟随该主题的菜单栏 / 编辑区。
2. 折叠按钮: 左 / 右细条常驻 16px, rail 底色, 无框 40px `›` / `‹` (16px), hover 叠 `bg_hover`, 提示文案随状态 (展开导航 / 文件、折叠 Chat 等); 右细条空闲时也在 (点它 = 展开 AI 对话); 面板头里原来的小三角已去掉 (细条取代)。底部: 折叠态保留 28px 头条 + 三个页签, 右端文字钮「展开」/「折叠」(11px, 圆角 5, 1px 边)。
3. 状态栏蓝底 (字色按底色亮度取白 / 前景), 无按钮底色, 悬停叠白; 标签栏 chrome 底色、活动标签与编辑区同色 + 2px 蓝下划线、`×` 无框; 标题栏 / 菜单栏用 bg_bar 并加下边线; 标题栏快捷钮加间距 (不再连成一串); 查找栏 `×` 移到行首, 不再折行压在「置换」上。
已验证 (真窗口, 发行版 / 调试版): 左 / 右 / 底三处折叠与展开 (点击细条 / 底部文字钮 / Ctrl+` / Ctrl+Alt+L), 标签切换, 查找栏, 分栏 (Ctrl+\), AI 面板, 终端面板, 视图菜单切深 / 浅。
第二轮 (外观对标, 本节为最新状态):
4. 布局拓扑: 状态栏与底部面板先于左 / 右栏加入, 横跨整个窗口宽度 (同 Tauri `grid-column: 1 / -1`); 标签栏只盖住编辑区 (右侧 Chat / 预览停靠在它外面)。
5. 活动栏 (View 菜单「活动栏」开): 44 宽 rail 底色, 32x32 按钮, 图标由 `svgicon.rs` 把 editor.html 的 SVG 路径 (资源管理器 / 搜索 / 源代码管理 / 大纲 / 扩展) 直接解析后用画笔画出 (线宽 1.8、18px), 活动项深色白 12% 底 + 白图标 / 浅色 #e8edf3 + 蓝图标, 左缘 2px 竖线, 悬停叠色, 提示文字; 点击 = 切换面板, 再点已显示的 = 收起侧栏。新增「扩展」侧栏页 (插件列表, 与「插件」菜单共用一段代码)。开了活动栏后侧栏顶部的胶囊页签条不再画 (Tauri 侧栏也没有)。
6. 菜单: 菜单名带下划线助记字母 (文件 F / 编辑 E / 查找 S / 折叠 O / 插件 P / 设置 C / 书签 B / 窗口 W / 宏 M / 视图 V / 帮助 H), 下拉框底色 = 菜单栏色、1px 边、直角、阴影 0 2 8、最小宽 240、行内边距 14x5、12px、悬停 bg_active; 勾选 / 单选统一为行首 ✓ 文字 (无复选框方块)。标题栏左侧加标题文字 (无文件「IDE1.0」, 有文件「名 - IDE1.0」, 窗口宽 >= 约 1250 时才画, 否则会挤掉「帮助」); 快捷按钮由文字换成矢量图标。
7. 滚动条: 常显、轨道透明、滑块 6px (含边距共 12)、色 --fg-dim / 悬停 --fg-muted。
8. 语法色: 固定为 Tauri 调色板 (深 #569cd6 / #4ec9b0 / #ce9178 / #6a9955 / #b5cea8 / #dcdcaa; 浅 #0000ff / #267f99 / #a31515 / #1c7a1c / #098658 / #795e26; retro16 用自己的霓虹色), 主题插件按底色明暗取深 / 浅板 (与 Tauri `data-lum` 一致), 关键字 / 注释不再加粗 / 斜体 (retro16 除外)。
9. 编辑区: 默认字号 13px (旧设置文件保留各自的值), 行高倍数 1.0 = 18/13 (即 13px 下 18px); 行号 12px、至少 40 宽、垂直居中、色 --fg-muted, 当前行号 --fg; 正文左边距 8。
10. 查找: 右上角浮动对话框 (距顶 56 / 右 28, 宽 min(440, 92% 窗宽), 圆角 8, 阴影 0 6 24), 头部标题 + ✕, 正文输入框 / 选项 / 置换行, 底部 `置换 | ↑ ↓ 关闭`; 增量查找同一个窗口。
11. 标签栏 (`tabbar.rs`): 28 高自绘, 标签 padding 0 26 0 12、最宽 200 (超出 `…`)、右 1px 竖线; 活动 = 编辑区同色 + 顶部 2px 状态栏蓝线; 已修改 = 左侧黄点; 关闭钮 18x18 仅悬停 / 活动时显示; 悬停叠 bg_hover; 横向滚动保留。开了活动栏时单文档也显示标签栏 (Tauri `tabs: true`), 极简界面单文档仍整条收起。
12. Markdown 预览: 正文 13px, 标题 2em / 1.5em / 1.17em / 1em, 引用左 3px 粗边 + 弱前景, 代码块 bg_raised 底 + 圆角 4 + 内边距 10x8, 表格 13px。


## 第三轮 (真窗口逐项核对 fast3-*)

对照 `web-full-{dark,light}-*` 后已对齐: 标题栏菜单与右侧按钮按宽度逐样撤 (快捷图标 -> 面板开关 -> 标题文字 -> 终端钮 -> 助记字母), 700 / 900 / 1100 / 1280 / 1600 宽都不重叠不溢出 (`plan_bar`); 完整界面 44px 活动栏 + 侧栏头只写面板名, 左侧细条在完整界面不画 (Tauri 同); 活动项左缘 3px 蓝条 (深 `#007acc` / 浅 `#005fb8`); 欢迎页 (徽标 / 字标 / 打开文件夹钮 / 按键提示, 组合键不拆行, 浅色按钮为白底灰边, 标尺开着时让出标尺条); 右栏 Chat 头; 底部 TERMINAL / OUTPUT / PROBLEMS 头 (28px); 完整界面默认开标尺; 菜单项快捷键右对齐; 查找对话框头部关闭钮改 `×` (原 `✕` 字体里没有, 显示成豆腐块)。

## 第四轮独立复核 (真窗口, 深 / 浅 x 900 / 1280)

截图 `parity-shots/fast4-*.png` (深: 默认 / 底部 + 右栏 / 查找 / 折叠 / 分栏 + 终端; 浅: 底部 + 右栏 / 文件菜单 / 查找)。逐项在真窗口里点过: 底部「折叠 / 展开」文字钮、右细条、Ctrl+Alt+H/J/K、活动栏切换 (文件 / 搜索 / Git)、标签切换、查找 (键入 / 命中高亮)、分栏 (Ctrl+\)、AI 面板、终端 (命令真执行)、Markdown 预览。

本轮修掉的问题:
- 侧栏被内容带窄: 切到 Git 页 (内容窄) 后面板宽度被「内容宽」覆盖, 越缩越窄 (~100px); 现在面板撑满当前宽度。
- Esc 关不掉菜单: 全局 Esc 处理先把按键吃掉, 打开的菜单收不起来; 现在有弹出层时 Esc 只关弹出层。
- 窄窗口 (900) 同时开侧栏 + 预览 + Chat 时编辑区被挤成 0 宽 / 被预览盖住; 现在右侧停靠区宽度封顶, 空间不够 (编辑区 < 240 + 停靠区 < 160) 就不画后一个 (预览), 编辑区保住。
- 底部终端 / 输出 / 问题正文贴着窗口边 (无左右内边距): 加 8px。
- 查找输入框补占位字「查找内容」(Tauri 有)。

## 第五轮 (关闭第四轮列出的差异, 截图 `parity-shots/fast5-*.png`)

对照 `web-full-{dark,light}-*`, 真窗口逐项核对 (深色 / 浅色 1280x800, 另有 700 宽、700 高两张):
- 底部面板默认: 完整界面 (活动栏开) 里底部面板没开时也画 28px 的 TERMINAL / OUTPUT / PROBLEMS 头 + 「+ 新建终端」「展开」, 不启动 shell (`panels::bottom_mode`: 只画头条; shell 在第一次点页签 / 展开 / 在终端里按 Enter 时才起)。点任一页签或「展开」= 展开面板。极简界面仍整块不画。终端没有输出时正文显示 Tauri 的空状态「No terminal session — 暂无终端会话」。
- 查找对话框: 关闭钮改成带 1px 边的 28x24 小方钮 (边色深 `#454545` / 浅 `#d0d7de`, 取自 Tauri 截图像素), 正文与底部按钮区之间一条 1px 分隔线, 底部三个按钮同款描边, 输入框底色深 `#3c3c3c` / 浅白 (`btnstyle.rs`)。
- 复选框: 自绘 `ckbox::Check` (13x13、圆角 2、未选白底 `#767676` 边、悬停 `#4f4f4f`、选中 `#0075ff` 底 + 白色折线对勾, 禁用变淡; 色值取自 Tauri 截图像素), 查找 / 全局查找 / 首选项 / 打印 / 终端 / 插件菜单 / 折行菜单里的所有复选框都换成它 (`ui.check(..)`), 不再借用滚动条共享的 `widgets.inactive.bg_fill`, 滚动条不受影响。
- 菜单: 新增「工具」菜单 (Tauri 的 tool 组: 大纲 / 语法类型 / 首选项 / 导出·导入配置 / 打开自动备份 / 文件加锁 / 宏 / 插件 / 执行外部命令), 每项接到 ide-fast 已有功能; Tauri 的 SQL*Plus 与「历史管理」ide-fast 没有对应功能, 不列。多出的折叠 / 插件 / 书签 / 宏菜单保留。「文件 / 编辑 / 查找 / 设置 / 窗口 / 视图 / 工具」菜单正文放进竖向滚动区, 窗口高 700 时「视图」不再被裁, 改成可滚动 (`fast5-dark-05-menu-view-h700.png`); 12 个菜单在 700 宽下仍不重叠 (`fast5-dark-06-w700.png`)。
- 标尺: 改成 Tauri 的字符串标尺 `....+....1....+....2` (每列一字, 等宽对齐正文, `view::ruler_text`)。
- 终端工具行: 去掉「终端」标签 (头条已有), 按钮 11px、弱前景、描边小按钮 (与头条「+ 新建终端」同款); 输出 / 问题页的工具行同样处理。功能一项不少。
- 标签栏: 欢迎页显示时 (唯一的空白无标题文档) 不画「无标题」标签; 另外补了 Tauri 的 22px 面包屑条 (路径分段 + `›`, 当前文件名用前景色, 欢迎页留空)。
- 状态栏: 顺序 / 标签 / 样式照 editor.html: `[列块] [选区字数] Ln r, Col c (表示 n) | [● REC] [U+码位 / EOL / EOF] | 挿入 / 上書 | 编码 | 换行符 | 文件名 (无活动) | 提示 (撑满) ... Plain (靠右)`, 11px 等宽字、每项左右 8px + 右侧 1px 竖线 (字色 20%)。总行数挪进位置项的悬停提示, 换行混用 / 只读 / 范围选择 / 多光标仍在有效时显示。编码 / 换行符 / 转到行点击行为不变。
- 滚动条: 侧栏 / 终端改成 Tauri 的 10px 款 (无边距, 滑块 `rgba(127,127,127,.45)`), 用 `scrollx::show` 在外层 scope 换样式、进内容前还原, 文字 / 按钮颜色不受影响; 其余区域仍是 12px 全局款。
