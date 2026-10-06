//! IDE1.0 IDE Shell — 完整内置 wiki.
//!
//! 与 CLI `--help` / 应用内 `:help` / 工具栏「帮助」共用, 桌面版与 web 版同源.
//!
//! 桌面版的 APP_WIKI 常量从此文件 include_str! 进来, web 版 HTTP /api/help_wiki
//! 同样从此文件 include_str! 进来 — 单一来源, 改一处同步两边.

/// 完整 wiki 全文 (>=2KB, 含 .web 2 段版).
pub const APP_WIKI: &str = r#"IDE1.0 IDE Shell — 桌面版完整手册 (wiki)
=========================================

[1] 简介
--------
IDE1.0 IDE Shell 是一个轻量的 AI Native 桌面代码编辑器:
  * 界面参考 Cursor (工具栏 / 文件树 / 标签页 / 状态栏) + Vim (键位三态)
  * 纯 Rust 核心 (ide-shell crate) + Tauri 2 webview, vanilla JS 前端, 0 前端框架
  * 安装包 ~3MB, 内存占用低, 启动即开即用
所有可交互控件都带鼠标悬停说明 (tooltip + 状态栏提示区).

[2] 安装与启动
--------------
Windows : 双击 dist/*.msi → Next → Install → 开始菜单 "IDE1.0 IDE Shell"
Linux   : .deb (dpkg -i) 或 .AppImage (chmod +x 后直接运行), 由 CI 产出
命令行  : ide-shell-desktop.exe [--help|-h] [--version|-V]
  注意: Windows release 版是 GUI 子系统, --help 输出需接管道/重定向才可见:
    ide-shell-desktop.exe --help | more
    ide-shell-desktop.exe --help > help.txt
  应用内随时按 :help 或点工具栏「? 帮助」查看同一份 wiki.

[3] 界面布局 (参考 Cursor)
--------------------------
工具栏: 打开文件夹 | 保存 | 还原 | 关闭标签 | ? 帮助 | Shell
侧栏: 资源管理器 (文件树, 懒加载, 可拖宽) | 最近项目
主区: 标签栏 + Vim 编辑器 (行号 + 光标块 + 虚拟滚动) + Shell 面板
状态栏: 模式 | Ln/Col | 文件路径 | 悬停提示 | kernel banner

[4] 项目导入
------------
1. 点工具栏「📁 打开文件夹」(或欢迎页大按钮) → 原生文件夹选择器 → 确定
2. 左侧文件树显示项目根, 点文件夹展开 (懒加载), 点文件打开进标签页
3. 打开过的项目记入「最近项目」(localStorage), 欢迎页一键重开
4. 命令行 :e <相对路径> 也可打开项目内文件
安全: 文件读写被沙箱限制在当前项目根内, 越界路径一律拒绝.
限制: 仅 UTF-8 文本文件, 单文件 ≤ 4MB (保持轻量.)

[5] Vim 键位表 (编辑器焦点时)
-----------------------------
NORMAL 模式 (默认):
  h j k l / ←↓↑→   左 下 上 右移动
  w / b / e        下一词首 / 上一词首 / 词尾
  0 / ^ / $        行首 / 首个非空白 / 行尾
  gg / G           文件头 / 文件尾
  Ctrl+D / Ctrl+U  下翻半页 / 上翻半页
  i / a            光标前 / 后进入 INSERT
  I / A            行首 / 行尾进入 INSERT
  o / O            下方 / 上方新行进入 INSERT
  x                删除光标处字符
  d + 动作         删除 (dd 整行, dw 词, d$ 到行尾, d0 到行首, dj/dk 跨行)
  c + 动作         删除并进入 INSERT (cc / cw / c$ 同上)
  y + 动作         复制到寄存器 (yy 整行, yw 词, y$ 到行尾)
  p                粘贴 (整行粘贴在下方, 字符粘贴在光标后)
  u                撤销 (快照式, 最多 200 步)
  v                进入 VISUAL 字符选择, 再按 d/y/c 对选区操作
  :                进入 COMMAND 模式 (底部命令行)
  Ctrl+S           保存当前文件 (任意模式可用)

INSERT 模式:
  可打印字符        直接插入
  Enter            换行
  Backspace/Delete 删除
  ←↓↑→ / Home/End  移动
  Esc              回 NORMAL

VISUAL 模式:
  移动键扩展选区; d 剪切 / y 复制 / c 剪切并插入; Esc 取消选区

COMMAND 模式 (底部 : 命令行):
  :w               保存
  :q               关闭当前标签 (未保存会弹确认)
  :wq              保存并关闭
  :e <路径>        打开项目内文件 (相对项目根或绝对路径)
  :help            打开本 wiki 浮层
  :setlang         手动设置当前文件语言 (覆盖扩展名识别)
  :template <lang> 在当前标签末尾插入该语言的 hello-world 模板
  :tpl <lang>      同 :template (简写)
  Esc              取消命令行

[5.5] 语法高亮 / Tab / 缩进 / 折叠
------------------------------------
按扩展名 + 文件名自动识别 19 种语言, 关键字 / 类型 / 字符串 / 数字 / 注释 /
属性 / 装饰器 / 操作符 各自配色 (VSCode Dark+ 风格):

  Rust / Python / JavaScript / TypeScript / Go / C / C++ / Java / C# /
  Ruby / Bash / HTML / CSS / JSON / Markdown / YAML / TOML / SQL / Plain

INSERT 模式:
  Tab             转 2 空格 (替换为缩进)
  Enter           自动缩进: 复制前一行 leading whitespace, 行尾 { [ ( 多缩一级

Normal 模式:
  zc              折叠当前行所在 block ({...} 或 indent block)
  zo              展开当前行
  zM / zR         全部折叠 / 全部展开

状态栏新增「语言」字段 (例: 「Rust」), 按 :setlang 可手动覆盖.

[6] Shell 面板 (底部)
---------------------
复用 ide-shell core 的演示 shell (与 web UAT 同一 Rust 状态机):
  * 点面板内部或 Ctrl+` 把键盘焦点交给 Shell (边框高亮), 再按一次回编辑器
  * 三态: NORMAL (左键=INSERT, 右键=NORMAL) / INSERT (打字, Enter 执行) /
    COMMAND (: 前缀命令)
  * 内置命令: :help :version :ai :clear :q
  * 非内置命令回落真子进程执行 (Windows=cmd /C, Unix=sh -c) — 可跑
    git status / cargo build 等, 输出进 history
  * 此面板的 :q 只是 shell 退出信号, 不关应用; 关应用用窗口 X

[7] 鼠标悬停说明
----------------
* 所有按钮 / 标签 / 文件树节点 / 状态栏项都带 data-tip:
  悬停 → 浮动 tooltip + 状态栏提示区同步显示说明
* 文件树节点悬停显示完整路径; 标签页悬停显示文件路径与保存状态

[8] 架构与源码导览
------------------
crates/ide-shell          核心状态机 (mode/input/keymap/app/render), 38 UT
crates/ide-shell-desktop  本 Tauri 2 应用 (11 commands, 文件沙箱, wiki)
crates/ide-shell-web      HTTP server 版 (Playwright 40 e2e case 目标, 独立 UI)
crates/ide-kernel-core    kernel banner / version
前端: crates/ide-shell-desktop/dist/index.html — 单文件 vanilla JS (~1000 行),
  编辑器为自绘虚拟滚动 (行高固定), 支持万行文件不卡顿.
Tauri commands: frame/reset/key/mouse/kernel_banner/get_mode (shell)
  + pick_folder/open_project/list_dir/read_file/write_file/help_wiki (编辑器)

[9] 常见问题 (FAQ)
------------------
Q: 窗口白屏?
A: Windows 需要 WebView2 Runtime (Win10/11 一般自带; 缺则装 Evergreen Bootstrapper).
Q: release 版 --help 没输出?
A: GUI 子系统无控制台, 用 `--help | more` 或 `--help > help.txt` (见 [2]).
Q: 打开文件报「仅支持 UTF-8」?
A: 二进制/GBK 文件不在轻量编辑器范围内, 请用 Shell 面板跑外部命令处理.
Q: 文件树点不动?
A: 先「打开文件夹」导入项目; 沙箱只允许访问项目根内路径.
Q: 编辑到一半崩溃?
A: 未保存内容在内存中, 常按 Ctrl+S; :w 亦可. 撤销栈 200 步.

[10] 版本
---------
版本: 0.2.0 (ULYS-191 §5 Cursor 风格 UI + 项目导入 + wiki help)
上一版: 0.1.0 (ULYS-191 §4 Tauri 桌面 PoC, 单行 buffer UAT 壳)
许可: MIT OR Apache-2.0 — https://github.com/UlyssesLeoLee/IDE1.0
"#;
