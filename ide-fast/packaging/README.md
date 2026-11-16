# IDE1.0 桌面安装包

`ide-fast`（原生 Rust / egui）现作为 **IDE1.0** 官方唯一桌面发行版本（原 Tauri 版本已停止维护并移除）。默认安装目录为 `Program Files\IDE1.0`。release 时提供以下构建资产:

| 资产 | 说明 |
|---|---|
| `IDE1.0_<版本>_x64.msi` | 安装包 (WiX v3, 每机安装, 开始菜单快捷方式, 可从「应用」里卸载) |
| `IDE1.0-<版本>-windows-x64.exe` | 便携版 (单个 exe, 不写注册表) |

## 构建

```powershell
powershell -ExecutionPolicy Bypass -File ide-fast/packaging/build-msi.ps1 -Target <cargo 产物目录> -Out <输出目录>
```

脚本会先 `cargo build --release -p ide-fast`, 再用 WiX v3 (`candle` / `light`) 打 msi, 并把 exe 复制成便携版。WiX 默认取 `C:\Program Files (x86)\WiX Toolset v3.14\bin` 或 `%LOCALAPPDATA%\tauri\WixTools314`, 也可设 `WIX_BIN` 指向含 `candle.exe` / `light.exe` 的目录。版本号取仓库根 `Cargo.toml` 的 workspace 版本 (msi 补成四段)。

## 发布流程

沿用 `reference_release_procedure` 里的做法 (干净导出 -> 构建 -> 校验和 -> `gh release create`), Release 资产包括:

1. `build-msi.ps1` 产出的 `IDE1.0_<版本>_x64.msi` 与 `IDE1.0-<版本>-windows-x64.exe`;
2. `SHA256SUMS.txt` 与 `SHA256SUMS.txt.sig`。

## 已验证 / 未验证

- 已验证: msi 能构建; 管理映像解包 (`msiexec /a`) 得到的 exe 与构建产物逐字节一致; 便携 exe 能启动、渲染、接收输入、剪贴板可用。
- 未验证: 交互式安装 / 卸载 / 升级覆盖安装 (每机安装需要 UAC, 自动化里点不了)。**发布前请在真机上装一次。**
- 没有做: 代码签名 (SignPath 申请进行中, 见仓库根的签名说明); 文件关联与「用 IDE1.0 打开」右键菜单。

## 自更新对 Release 的要求

「检查更新…」(见 `docs/UPDATE.md`) 认下面的资产名（同时向下兼容旧版 `ide-fast-*` / `IDE1.0-Fast_*`）, 并且**必须有 `SHA256SUMS.txt`** (每行 `<hex>  <文件名>`, 含便携 exe 与 msi), 缺了会拒绝安装:

- `IDE1.0-<版本>-windows-x64.exe` (便携)
- `IDE1.0_<版本>_x64.msi` (安装包)
- `SHA256SUMS.txt`
- `SHA256SUMS.txt.sig` (对 `SHA256SUMS.txt` 的 ed25519 分离签名, 原始 64 字节; 用 `sign-release.ps1` 生成, 私钥在仓库之外; 缺了或验不过会拒绝安装)

开发预发布用 `publish-dev.ps1` (tag 形如 `v0.3.2-dev.20261103.1`, `gh release create --prerelease`); 它只是文档化的脚本, 不会被自动运行。
