# 高速版路线图

原则: **语义放在 `editcore` (Rust), GUI 只管画和收输入**; 一个功能如果需要网页能力 (Mermaid 渲染、AI Chat 面板、插件的沙箱 iframe), 留在 Tauri 版, 不在这里重做。

## 里程碑 1 (已完成, 本目录首个提交)
窗口 / 多标签 / 打开·保存·另存为 (编码·BOM·换行保持) / Sakura 光标移动与编辑 (editcore) / 撤销·重做 / 剪贴板 / 查找·置换 / 行号 / 状态栏 / 深浅主题 / 字号 / 拖文件打开 / 未保存提示 / CJK 显示 / 输入法 (候选窗跟随光标, 组字内联显示)。

## 里程碑 2: 日常编辑补全
- 语法高亮 (editcore 已有分词 / 配色表 `syntax_styling`, 先接 Rust / JS / Python / Markdown / JSON)
- 设置落盘 (复用 `settings_store`): 主题、字号、Tab 宽度、最近文件
- 自动换行 (显示行移动: 参考 Tauri 版的几何法, 这里可以直接用 galley)
- 书签、折叠、矩形选择 / 多光标 (editcore 已有 `box_*` / `multi_*` 命令)
- 命令行打开 `文件:行:列`、单实例转发

## 里程碑 3 (进行中): 导航与工具
- 侧栏文件树 + 大纲 (`outline`)、Grep (`grep_engine`, 结果窗口)
- Markdown 预览 (用 `md_render` 的块结构做原生渲染, 不含 Mermaid)
- 外部命令执行 (`exec_cmd`)、宏 (`macro_sakura` / `js_macro` 的 Rust 部分)

## 不做 (留给 Tauri 版)
Mermaid / 插件沙箱 / AI Chat 面板 / 网页版。

## 度量
`IDE_FAST_TRACE=1` 打印各阶段毫秒数。首个版本实测: 窗口 + GL 上下文约 170–220 ms, 首帧约 180–240 ms (冷启动 ~470 ms), 对比 Tauri 版约 1.0–1.5 s。
