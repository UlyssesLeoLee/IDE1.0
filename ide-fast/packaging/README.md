# 高速版的独立安装包

高速版和 Tauri 版是**两个产品**: 各自的安装包、安装目录 (`Program Files\IDE1.0 Fast`)、升级码、注册表键, 可以并存。release 时作为**独立的资产**提供:

| 资产 | 说明 |
|---|---|
| `IDE1.0-Fast_<版本>_x64.msi` | 安装包 (WiX v3, 每机安装, 开始菜单快捷方式, 可从「应用」里卸载) |
| `ide-fast-<版本>-windows-x64.exe` | 便携版 (单个 exe, 不写注册表) |

## 构建

```powershell
powershell -ExecutionPolicy Bypass -File ide-fast/packaging/build-msi.ps1 -Target <cargo 产物目录> -Out <输出目录>
```

脚本会先 `cargo build --release -p ide-fast`, 再用 WiX v3 (`candle` / `light`) 打 msi, 并把 exe 复制成便携版。WiX 默认取 `%LOCALAPPDATA%\tauri\WixTools314` (Tauri 打包时下载过), 也可设 `WIX_BIN` 指向含 `candle.exe` / `light.exe` 的目录。版本号取仓库根 `Cargo.toml` 的 workspace 版本 (msi 补成四段)。

## 发布流程 (跟 Tauri 版同一个 Release, 多两个资产)

沿用 `reference_release_procedure` 里的做法 (干净导出 -> 构建 -> 校验和 -> `gh release create`), 在它的资产清单里加上:

1. `build-msi.ps1` 产出的 msi 与便携 exe;
2. 把它们的 SHA-256 一并写进 `SHA256SUMS.txt`;
3. Release 说明里写明「高速版 (原生 Rust) 与 Tauri 版是两个独立安装包」。

## 已验证 / 未验证

- 已验证: msi 能构建; 管理映像解包 (`msiexec /a`) 得到的 exe 与构建产物逐字节一致; 便携 exe 能启动、渲染、接收输入、剪贴板可用。
- 未验证: 交互式安装 / 卸载 / 升级覆盖安装 (每机安装需要 UAC, 自动化里点不了)。**发布前请在真机上装一次。**
- 没有做: 代码签名 (SignPath 申请进行中, 见仓库根的签名说明); 文件关联与「用 IDE1.0 Fast 打开」右键菜单。

## 自更新对 Release 的要求

「检查更新…」(见 `docs/UPDATE.md`) 只认下面的资产名, 并且**必须有 `SHA256SUMS.txt`** (每行 `<hex>  <文件名>`, 含便携 exe 与 msi), 缺了会拒绝安装:

- `ide-fast-<版本>-windows-x64.exe` (便携)
- `IDE1.0-Fast_<版本>_x64.msi` (安装包)
- `SHA256SUMS.txt`
- `SHA256SUMS.txt.sig` (对 `SHA256SUMS.txt` 的 ed25519 分离签名, 原始 64 字节; 用 `sign-release.ps1` 生成, 私钥在仓库之外; 缺了或验不过会拒绝安装)

开发预发布用 `publish-dev.ps1` (tag 形如 `v0.3.2-dev.20261103.1`, `gh release create --prerelease`); 它只是文档化的脚本, 不会被自动运行。
