# tests/mock — 配套 mock / 回归测试项目

用真实按键、真实鼠标、真实 IME 组合输入驱动编辑器, 把开发中用过的探针固化为回归用例。

## 运行

```bash
cd tests/mock
# Web 版 (需先构建 ide-shell-web)
QA_BIN=E:/IDE1.0-wt/fix-target/debug/ide-shell-web.exe npm run test:web
# 桌面真机 (Tauri / WebView2, 通过 CDP 驱动; 会弹出真实窗口)
QA_DESKTOP=E:/IDE1.0-wt/qa-target/debug/ide-shell-desktop.exe npm run test:desktop
```

Playwright 复用 `tests/uat/node_modules`。

## 环境变量

| 变量 | 作用 |
|---|---|
| `QA_BIN` | ide-shell-web.exe 路径 (默认自动在 target 目录查找) |
| `QA_TARGET_DIR` | 查找二进制的 cargo target 目录 |
| `QA_PORT_BASE` | 起始端口 (默认 8400) |
| `QA_DESKTOP` | 桌面 exe 路径; 未设置则桌面用例自动跳过 |

## 结构

- `lib/harness.mjs` 起服务/开浏览器/真实首次启动; `lib/mock.mjs` 文件拖放、IME、Tauri IPC、pick_folder 等替身; `lib/desktop.mjs` + `lib/backdrop.ps1` 桌面 + 背后白/黑闪烁窗口。
- `fixtures/encodings/` 各编码样本 (SJIS/EUC-JP/JIS/UTF-16LE/BE/UTF-8(BOM)/CR/LF), 由 `fixtures/make-fixtures.mjs` 生成。
- `specs/web/` 首次启动、键盘选择、鼠标/Alt 矩形、编码读写、IME、键位与命令、搜索、抽屉排版、Tauri 通道。
- `specs/desktop/` 真机 Grep、Alt 矩形、透明玻璃按背后颜色反色。

## 约定

每个用例独占端口与临时项目目录; 失败时优先判断是产品回归还是用例假设过期 (如契约有意变更)。
