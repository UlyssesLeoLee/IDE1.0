# ULYS-191 §4 — Tauri Desktop 打包 (PR #10)

> 目标: 把 IDE Shell 从 web (HTTP server) 升级到桌面应用 (.msi / .dmg / .AppImage)
> 用户原始诉求: "我需要的是打包安装文件的桌面版"

## 架构

```
现有 (web UAT):                              Tauri 化后 (本笔):
──────────────────                            ──────────────────────
crates/ide-shell-web/                          crates/ide-shell-desktop/  (新)
├ src/lib.rs     HTTP server                    ├ Cargo.toml          依赖 tauri 2.x + ide-shell
├ src/index.html  vanilla JS (fetch /api/*)    ├ build.rs             tauri_build::build()
└ src/bin/        ide-shell-web.exe             ├ tauri.conf.json      app 配置 (windows.bundle)
                                                ├ icons/               .ico / .icns / .png
              chromium (Playwright)              ├ src/lib.rs           tauri::Builder + commands
              ↑ fetch                              ├ src/main.rs          desktop app entry
              │                                    └ dist/index.html      (从 ide-shell-web 拷)
              ↓                                                  ↑ invoke
         ide-shell-web server                                     │
         └ App (Mutex) ───────── 同源内核 ──────────────── App (Tauri managed state)
```

**核心决策**:
1. **同源内核** — `crates/ide-shell-desktop` 复用 `ide-shell` crate(已有 38 UT),
   `App` 实例由 Tauri `manage()` 注入,生命周期跟随 window
2. **前端复用** — `crates/ide-shell-web/src/index.html` 拷到 `dist/index.html`,
   JS 改为检测 `window.__TAURI__`,有则用 `invoke()`,无则用 `fetch()`
   (向后兼容 web server 模式,Playwright UAT 不变)
3. **Tauri 2.x** — 现代 stable,Windows MSI 打包内建支持
4. **不用 cargo-tauri CLI** — 本机 rustup 缺失,用 `npm i -D @tauri-apps/cli` 走 npm
5. **图标占位** — 用 32x32 PNG 极简蓝色 "IDE" 字样,启动资源即可,品牌待设计

## 文件清单 (本笔新增)

```
crates/ide-shell-desktop/
├── Cargo.toml                   (workspace member + tauri deps)
├── build.rs                     (tauri_build::build)
├── tauri.conf.json              (app 配置)
├── icons/
│   ├── 32x32.png                (占位)
│   ├── 128x128.png
│   ├── 128x128@2x.png
│   ├── icon.ico                 (Windows)
│   ├── icon.icns                (macOS, 占位空)
│   └── Square30x30Logo.png      (Windows Store)
├── src/
│   ├── lib.rs                   (tauri::Builder + commands + state)
│   └── main.rs                  (entry)
├── dist/
│   └── index.html               (从 crates/ide-shell-web/src/index.html 拷)
└── tests/
    └── state_integration.rs     (3 IT — 验证 App 状态机在 Tauri managed state 下行为不变)
```

修改:
- `Cargo.toml` — workspace 加 `crates/ide-shell-desktop` member
- `crates/ide-shell-web/src/index.html` — 加 tauri/invoke adapter (一处 JS 检测,~10 行)
- `.gitignore` — 加 `crates/ide-shell-desktop/target/`
- `README.md` — 加 §10 Tauri Desktop 段

## 验证 (本机 rustc 1.98.1 + Node 26)

| 步骤 | 期望 |
|---|---|
| `cargo build -p ide-shell-desktop` | ✅ 编译通过 (binary `ide-shell-desktop.exe`) |
| `cargo test --workspace --all-targets` | ✅ 全 4 仓测试仍过 (加 3 IT) |
| `cargo clippy --workspace -D warnings` | ✅ 0 warning |
| `cargo fmt --check` | ✅ |
| `cargo build --release -p ide-shell-desktop` | ✅ release build |
| `npx tauri build` (Windows) | ✅ 出 `.msi` 在 `target/release/bundle/msi/` |
| 手动运行 `.msi` 安装 → 双击 → 桌面窗口出现 | 验证 (用户) |

CI 增量: 新增 `tauri-build-linux` (ubuntu-latest, 出 .deb + .AppImage),
不强制 mac/win matrix (本机跑 win build OK,其他 OS 留给后续 brief)。

## 不在本笔范围 (Stage 4+)

- mac/win CI matrix (本笔只 Linux CI build, 桌面 app 实跑留给用户)
- 自动更新 (tauri-plugin-updater)
- 系统托盘 / 全局快捷键
- 真正的 LLM 接入 (current: PlaceholderAi)
- 多行 buffer / 历史补全 / ghost text
- 应用签名 (Windows 代码签名证书 / Apple Developer ID)
- 图标品牌设计 (现占位)

## 守门

#1+#5+#6+#7+#9+#10+#11+#12+#13+#14v4+#15+#17+#19v19+#20+#24
scope creep 守门: 单 sub-agent 单切点 (Tauri 桌面化), 不动 Rust 核心, 不动现有测试, 不动 web server
