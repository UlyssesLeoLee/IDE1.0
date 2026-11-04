# MSI 安装包检查记录

范围: 高速版 `IDE1.0-Fast_0.3.0/0.3.1_x64.msi` 与 Tauri 版 `IDE1.0_0.3.0/0.3.1_x64_en-US.msi`。
方法: Windows Installer COM 读表 + `msiexec /a` 管理安装解包 + WiX `smoke.exe` 配 `darice.cub` 做 ICE 校验。

## 结论

四个包全部是每机安装 (`ALLUSERS=1`, `InstallScope=perMachine`), 安装需要管理员提权 (UAC)。
按"不提权、不应答 UAC"的约束, **没有做真实的 `msiexec /i` 安装 / 升级 / 卸载循环**, 机器上没有装任何东西。以下是静态验证结果。

## 静态结果

| 项 | 高速版 | Tauri 版 |
|---|---|---|
| UpgradeCode | `{EEEC6D8B-4609-4DF9-999B-DDADC720C74E}` (0.3.0 与 0.3.1 相同) | `{C22FF458-C363-5413-A620-66C93FF9844A}` (相同) |
| ProductCode | 每版本不同 (0.3.0 `{BA06F3E3-...}`, 0.3.1 `{FBC5F8A6-...}`) | 每版本不同 |
| 版本 | 0.3.0.0 -> 0.3.1.0 | 0.3.0 -> 0.3.1 |
| 升级 | MajorUpgrade: 旧版 (< 新版) 被检测并移除; 同 UpgradeCode 下新版本较低则报"已安装更新版本" | 同样检测旧版 (Min 0, 属性 257) |
| 安装目录 | `%ProgramFiles%\IDE1.0 Fast` | `%ProgramFiles%\IDE1.0` |
| 文件 | `ide-fast.exe` (0.3.0 15001088 字节 / 0.3.1 16602112 字节) | `ide-shell-desktop.exe` |
| 快捷方式 | 开始菜单 `IDE1.0 Fast` (所有用户) | 开始菜单 + 桌面 + 卸载快捷方式 |
| 卸载入口 | 控制面板 "IDE1.0 Fast" (ARPNOMODIFY, 带图标) | "IDE1.0" |

- 两个产品 UpgradeCode、目录、名称均不同, 可以并存, 互不替换。
- `msiexec /a` 管理安装解包均成功 (退出码 0), 解出的文件列表与上表一致 (高速版仅 `ide-fast.exe` 一个文件)。
- 用户设置在 `%APPDATA%\IDE1.0 Fast`, 不在 MSI 管理的任何组件内, 卸载 / 升级不会触碰 (MSI 的文件表与注册表表里均没有该路径)。

## ICE 校验

- 高速版 0.3.0 / 0.3.1: 仅 `ICE69` 警告 (快捷方式 `IdeFastShortcut` 与其目标 `[#IdeFastExe]` 分属 `StartMenuShortcut` / `MainExe` 两个组件)。
- Tauri 版 0.3.0 / 0.3.1: `ICE03` (自定义动作 Target 超长)、`ICE40` (REINSTALLMODE)、`ICE57` (HKCU 键路径混合)、`ICE61` (升级表无最大版本)。这是 Tauri/WiX 模板生成的, 不在本仓库 wxs 内, 未改。

## 修复

`ide-fast/packaging/ide-fast.wxs`: 快捷方式目标由 `[#IdeFastExe]` 改为 `[INSTALLFOLDER]ide-fast.exe`, 消除 ICE69 (效果相同, 快捷方式仍保留独立的 HKCU 键路径组件)。
曾试过把快捷方式并入 `MainExe` 组件, 会触发 ICE43 / ICE57 错误, 已放弃。
用 HEAD 的干净导出 (`E:\IDE1.0-wt\msi-test`) 重建测试包 `0.3.1.0` 与 `0.3.2.0`: `smoke.exe` 全部 ICE 通过 (退出码 0, 无警告); 升级表 UpgradeCode 不变、版本递增, 可原位升级。测试包未发布。

## 没有改每机为每用户的原因

0.3.0 / 0.3.1 已发布的用户装的是每机包。若 0.3.2 起改为每用户, 同一 UpgradeCode 下新旧范围不同, 会出现两份并存或升级失败, 属于破坏升级链的改动, 应单独决策。
应用内自更新运行 `msiexec` 时会弹 UAC (每机安装的必然结果); 想要无 UAC 更新请用便携版。

## 需要用户手动执行的真实循环 (管理员 PowerShell)

```powershell
$o='E:\IDE1.0-wt\rel-fast-out\IDE1.0-Fast_0.3.0_x64.msi'; $n='E:\IDE1.0-wt\rel-fast-out31\IDE1.0-Fast_0.3.1_x64.msi'
msiexec /i $o /qn /l*v $env:TEMP\f30.log
Test-Path "$env:ProgramFiles\IDE1.0 Fast\ide-fast.exe"
Get-ChildItem "$env:ProgramData\Microsoft\Windows\Start Menu\Programs" -Filter 'IDE1.0 Fast*'
msiexec /i $n /qn /l*v $env:TEMP\f31.log       # 升级; 应只剩一个 "IDE1.0 Fast" 卸载项, exe 为 0.3.1
Get-ItemProperty HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*,HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\* | ? DisplayName -eq 'IDE1.0 Fast' | select DisplayName,DisplayVersion
$env:IDE_FAST_SETTINGS='off'; Start-Process "$env:ProgramFiles\IDE1.0 Fast\ide-fast.exe"   # 应无控制台窗口
msiexec /x $n /qn /l*v $env:TEMP\f31u.log     # 卸载; 文件与快捷方式应消失, %APPDATA%\IDE1.0 Fast 仍在
```