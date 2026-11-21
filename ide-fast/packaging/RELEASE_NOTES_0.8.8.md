# IDE1.0 v0.8.8 — Myers 差异比对内核统一下沉 · NEMA 编解码器架构解耦 · 诊断日志锁自愈与代码规范全量治理

## 1. 架构统一与核心解耦
- **Myers 差异比对内核下沉 (`editcore::diff`)**：
  - 将原分散在 `ide-mcp` 与 `ide-fast` 中的 Myers 双向行级 diff 算法完全收敛下沉至底层核心库 `editcore`。
  - `ide-mcp` 直接复用 `editcore::diff` 导出统一接口；`ide-fast` 统一通过 `editcore::diff::diff_lines` 执行文档差异计算，彻底消除算法冗余与维护分叉。
- **NEMA 编解码器架构模块化深度解耦 (`crates/nema-codec`)**：
  - 将原本 3800+ 行的单体库解耦拆分为 4 个职责单一的子模块：
    - `export.rs`：专注于 Markdown 表格、JSON、YAML 导出。
    - `lease.rs`：租约仲裁与协作并发控制状态机。
    - `benchmarks.rs`：官方基准场景实测数据集及语法矩阵。
    - `tests.rs`：模块化单元测试套件。
  - 在 `lib.rs` 中收敛子模块为私有 `mod`，并通过根级 `pub use *;` 导出，对外公共 API 保持 100% 向后兼容。

## 2. 诊断系统鲁棒性与异常自愈
- **锁中毒自愈机制 (`ide-fast/src/diag_log.rs`)**：
  - 内存环形追踪缓冲区锁获取引入 `.unwrap_or_else(|e| e.into_inner())` 中毒自愈机制，彻底防止 panic hook 场景下因二次锁中毒导致进程异常中止。
- **目录运行时自愈**：
  - 新增 `ensure_log_dir()`，运行时日志目录若被异常移除或未创建，在写入崩溃日志、启动错误或导出诊断前自动重新校验并创建。
- **测试覆盖强化**：
  - 强化环形追踪缓冲区滚动容量与省略标记断言，确保启动追踪与最新追踪无损采集。

## 3. 弹窗界面重构与规范治理
- **消除数据泥块与参数反模式 (`updateui.rs`)**：
  - 引入 `UpdActions` 状态动作载荷，统一 `upd_badge()` 匹配逻辑，全面消除 `#[allow(clippy::too_many_arguments)]`。
- **短函数规范治理 (`updateui.rs`, `frame.rs`)**：
  - 拆分 `paint_comic_shell_chrome`、`render_upd_primary_action`、`render_upd_hero_banner`、`render_upd_body_cards`、`render_upd_notes_box`，所有函数单体严格控制在 80 行以内，符合团队最高编码规范。
- **美漫三渲二视觉规范闭环**：
  - 维持黑曜钛金底板、硬阴影、暗化墨水遮罩与真 3D 赛璐珞水晶展台的严格视觉标准。

## 4. 全量验证与自动化测试全绿
- `cargo fmt -p ide-fast -p editcore -p nema-codec -p ide-agent-router -- --check`：100% 通过
- `cargo clippy -p editcore -p nema-codec -p ide-mcp -p ide-fast -- -D warnings`：0 警告
- `editcore` (475 UT + parity suites)、`ide-mcp` (21 tests)、`nema-codec` (26/26 tests)、`ide-fast` (667 passed) 全量通过。
- `cargo run -p ide-fast --bin ide-fast -- --smoke-test`：启动烟雾门禁 100% 通过 (`IDE1.0_SMOKE_TEST_OK`)。

## 5. 发布安装包清单（Windows x64）
- `IDE1.0_Setup_0.8.8_x64.exe`（图形安装向导版，内置真 3D 水晶展台与协议触底门禁）
- `IDE1.0_0.8.8_x64.msi`（企业级 WiX 静默安装包）
- `IDE1.0-0.8.8-windows-x64.exe`（绿色单文件便携版）
- `SHA256SUMS.txt`（SHA-256 完整性校验清单）
