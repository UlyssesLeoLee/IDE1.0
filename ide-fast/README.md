# IDE1.0 高速版 (ide-fast)

原生 Rust (egui / OpenGL) 的 Sakura 风格文本编辑器。**不依赖 WebView2**, 进程一起来就直接建原生窗口。

> 和 Tauri 版的关系: **两个产品并行**。
> - `crates/ide-shell-desktop` (Tauri + 网页前端) 继续保留, 它是后续**移植到别的 agent / 网页版**的基底 (前端协议、插件沙箱、Markdown 预览等都在那边)。
> - `ide-fast` (本目录) 是**高速版**: 为的是启动快、占用小, 复用同一个 Rust 内核 `editcore`, 作为**独立安装包**发布。

## 为什么要单做一个

Tauri 版的界面是网页。Windows 上每次启动都要先拉起 WebView2 (浏览器内核的几个进程), 实测约 0.7–1.2 s, 这一段不在我们的代码里 (见 `CHANGELOG.md` 0.1.8「启动」)。原生版没有这一步。

## 复用了什么 (不重写第二份)

`editcore` 的编辑 / 移动 / 查找都是**无状态**的: 调用方交上光标周围需要的几行, 它返回一个行补丁 + 新光标 / 选区。Tauri 版的前端是这个协议的 JS 调用方, `ide-fast/src/doc.rs` 是 Rust 调用方:

| 功能 | 来自 editcore |
|---|---|
| 键入 / 退格 / 删除 / 换行 (自动缩进) / 词删除 / 整行复制·删除·移动 / 剪贴板语义 | `edit_cmd::run` |
| ← → ↑ ↓ Home End PageUp PageDown (含 Ctrl 词移动 / Shift 选择 / 目标列) | `nav::step` |
| 查找 / 置换 (含正则、环视、反向引用) | `search_ops::run` |
| 读写文件 (自动判别编码 / BOM / 换行, 原子写, 不静默替换无法表示的字符) | `document_io` |

所以 Sakura 的键位行为和 Tauri 版一致; 这里只有 GUI 自己的部分: 窗口、绘制、鼠标命中、撤销栈、标签。

## 目录

```
ide-fast/
  Cargo.toml          独立 crate (workspace 成员), 二进制名 ide-fast
  src/main.rs         入口 + 启动耗时追踪 (IDE_FAST_TRACE=1)
  src/app.rs          菜单 / 标签 / 编辑区绘制与输入 / 查找条 / 状态栏
  src/doc.rs          文档模型: 行向量 + 光标 / 选区 + 撤销栈, 桥接 editcore
  src/fonts.rs        系统 CJK 字体接成 fallback (不往程序里打包字体)
  docs/               设计说明、与 Tauri 版的差异清单、进度
  packaging/          独立安装包 (MSI / 便携 exe) 的构建脚本与说明
```

## 现在有的 / 还没有的 (里程碑 1)

有: 多标签、打开 / 保存 / 另存为 (编码·BOM·换行保持)、Sakura 光标移动与编辑、撤销 / 重做 (连续键入合并)、剪贴板、查找 / 置换 (正则)、Tab 展开、行号、状态栏 (行列 / 编码 / 换行)、深浅主题、字号缩放、拖文件进窗口打开、未保存提示、中日文显示 (系统字体) 与输入法提交。

还没有 (按 `docs/ROADMAP.md` 往后做): 语法高亮、Markdown 预览、侧栏 / 文件树、终端、插件、矩形选择 / 多光标、宏、Grep、设置落盘、折行 / 书签 / 折叠、打印。需要网页能力的功能 (Markdown 预览里的 Mermaid 插件、AI Chat 面板) 仍然只在 Tauri 版。

## 构建与测量

```bash
cargo build --release -p ide-fast          # 产物: <target>/release/ide-fast.exe
cargo test -p ide-fast                     # 文档模型单测 (桥接 editcore 的行为)
IDE_FAST_TRACE=1 ide-fast.exe              # stderr 打印 "进程入口 -> 窗口 + GL -> 字体 -> 首帧" 的毫秒数
```

独立安装包见 `packaging/README.md`。
