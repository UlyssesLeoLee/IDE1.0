# IDE1.0 v0.9.0 — MSI 可启动性修复、安装包 + 绿色便携版双形态发布与 Vercel MCP 持久化全量同步

## 1. Windows 安装包与绿色版改进
- **修复 `0.8.9` MSI 安装后无法直接启动问题**：
  - [ide-fast.wxs](ide-fast.wxs) 安装、升级及修复完成后直接拉起 `[INSTALLFOLDER]IDE1.0.exe`（不再带 `--setup` 再次进入向导，也不再受 `NOT Installed` 限制导致升级后静默不启动）。
  - 补齐桌面快捷方式（`DesktopFolder`）与开始菜单快捷方式。
  - 修复向导完成启动后可能被托盘常驻拦截关闭的问题（`self.force_exit = true`）。
- **精简发版资产（无单独 Setup）**：
  - 仅提供 **MSI 安装包** (`IDE1.0_0.9.0_x64.msi`) 与 **绿色单文件便携版** (`IDE1.0-0.9.0-windows-x64.exe`)，及 `SHA256SUMS.txt` 校验清单。
  - `build-msi.ps1` 打包阶段新增 `msiexec /a` 自动解包并用 `--smoke-test` 校验 MSI 内置可执行文件可启动性。

## 2. Vercel 云端 MCP 与 Rust 协同内核（含 0.8.10 以来全部安全加固）
- **Neon Serverless Postgres 事务存储**：多实例、冷启动及重部署零丢失；冲突不写入正式 HEAD，保留为 `PENDING_REVIEW` 提案与结构化 Diff。
- **细粒度鉴权与提案/文件级 ACL**：支持短期 HS256 JWT / 作用域 Bearer Token、工作区与文件级 `files` 白名单、`resolveProposal`/`listProposals` 权限闭环、2 MiB 载荷硬上限与全量审计日志。
- **Rust 安全与工程规范**：Windows 无窗进程启动 (`CREATE_NO_WINDOW`)、无 shell 插值、`RwLock`/`Mutex` 锁中毒自愈、磁盘路径安全与 `backup_before_save` 写前备份。

## 3. 实测验证结果
- `cargo test -p ide-fast -- --test-threads=1`：全量单元测试通过
- MSI 解包后 `IDE1.0.exe` 与绿色便携版 `IDE1.0-0.9.0-windows-x64.exe`：`--smoke-test` 及真实 Win32 GUI 窗口创建/可见性实测通过
- Vercel 生产端 `https://ide1d0.vercel.app/mcp` (`version: "0.9.0"`)：真实云端 E2E 全部 13 项（含 `12b` REST 协调）PASS

## 4. 发布安装包清单（Windows x64）
- `IDE1.0_0.9.0_x64.msi` — Windows 企业/标准 MSI 安装包（安装后自动启动主程序，含桌面与开始菜单快捷方式）
- `IDE1.0-0.9.0-windows-x64.exe` — 绿色单文件便携版（免安装双击即用）
- `SHA256SUMS.txt` — SHA-256 完整性校验清单
