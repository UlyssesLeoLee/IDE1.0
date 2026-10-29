# Microsoft Store + MSIX 发布

> 状态: 打包脚本已做好并在本机验证 (`makeappx pack` 成功, 5.5MB); **提交商店需要你自己的 Partner Center 开发者账号** (个人账号一次性收费), 账号注册、实名、付款、提交认证都只能由你本人完成。

## 1. 已有的东西

| 文件 | 作用 |
|---|---|
| `tools/msix/AppxManifest.template.xml` | MSIX 清单模板 (`runFullTrust` 桌面程序、支持 en / zh-cn / ja-jp) |
| `tools/msix/build-msix.ps1` | 把 release 版 `ide-shell-desktop.exe` + 缩好的图标 + 清单打成 `.msix` (用 Windows SDK 的 `makeappx`) |

Tauri 自己不产出 MSIX (只有 MSI / NSIS), 所以这里用 Windows SDK 工具直接打包同一个 exe。

## 2. 你需要做的 (按顺序)

1. 注册 Partner Center 开发者账号: <https://partner.microsoft.com/dashboard/registration>。
2. 创建新应用, **预留应用名称** (例如 "IDE1.0")。
3. 在 *产品管理 → 产品标识* 里抄下三个值:
   - `Package/Identity/Name` (形如 `12345YourName.IDE10`)
   - `Package/Identity/Publisher` (形如 `CN=XXXXXXXX-XXXX-…`)
   - `Package/Properties/PublisherDisplayName`
4. 用这三个值打包 (版本号必须四段, 且每次提交都要比上次大; Store 要求第四段为 0):
   ```powershell
   .\tools\msix\build-msix.ps1 -Exe <release 版 exe> -Version 0.1.5.0 `
     -IdentityName "<Name>" -Publisher "<Publisher>" -PublisherDisplayName "<显示名>"
   ```
   得到 `tools\msix\out\IDE1.0_0.1.5.0_x64.msix`。**提交到 Store 的包不需要你自己签名**, 商店会用它的证书重新签名。
5. 在 Partner Center 新建提交, 上传 `.msix`, 填商店列表 (见 §3)。
6. 本机想先装来试: 加 `-TestSign` 用自签证书签名, 再把证书导入"受信任的人", 双击安装 (仅测试用, 别发布)。

## 3. 商店列表与审核要点

- **runFullTrust 能力**: 提交时要说明理由 —— 编辑器需要读写用户选择的任意位置的文件、启动终端进程; 这和 Sakura 类编辑器一致。
- **隐私政策 URL** (联网应用必填): 用 README 里的「隐私声明」(`README.md` 的 §7.1) 所在页面链接。内容要与实际一致: 本机运行、无遥测; 联网只发生在用户点"检查更新/安装插件"、或确认后把选定文字发给用户自己配置的 AI 端点。
- **WebView2**: 程序基于 WebView2。Windows 11 自带; Windows 10 需要运行时 (Store 审核通常不挡, 但商店描述里写明"需要 Microsoft Edge WebView2 Runtime")。
- **许可证**: AGPL-3.0, 允许上架; 在商店描述里附源码仓库链接即可满足"提供对应源码"。
- 年龄分级问卷、截图 (≥1 张 1366×768 以上)、描述、搜索关键词、支持联系方式: 你来填。随程序打包的 Pele 是 MIT, 在"第三方声明"里列出。
- 商店版不能自己更新程序本体 (由商店分发); 但**插件**的"从 GitHub 安装/更新"是应用数据, 不受影响。

## 4. 之后接入 CI (可选)

`release-windows.yml` 现在产出并签名 `.msi`; 要自动出 `.msix`, 在同一个工作流里加一步调用 `build-msix.ps1` (identity 三项放 Secrets/Variables)。Store 包不用 SignPath 签名 (商店自己签); SignPath 只管 GitHub Release 上的 `.msi` / `.exe`。
