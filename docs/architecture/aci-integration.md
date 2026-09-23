# IDE1.0 × ACI emitter 集成设计

> **ULYS-191 §4.2.2 + §4.2.3** — IDE1.0 接入 `aci-emitter v0.1.0` 集成设计.
> Stage 3.0 方案 A 最小骨架 (本笔).

## §1 背景

IDE1.0 当前 docs-only (4 详细设计书 + 1 需求规格书 + LICENSE + 8-byte README),
**0 Cargo workspace / 0 crates / 0 Rust 代码 / 0 integration test / 0 cli_smoke.sh**.

per ULYS-191 §4.2 Stage 3 报告 D-Boy 拍板:

- 方案 A 最小骨架 (本笔): 1 workspace + 2 placeholder crates + 3 IT + 1 cli_smoke.sh
- 方案 B 完整 8 IT (推迟到下笔)

`aci-emitter v0.1.0` ([ULYS-191.1 / ULYS-224](https://github.com/UlyssesLeoLee/aci-emitter)) 已 ship,
IDE1.0 通过 **git dep** 接入.

## §2 集成路径

### 2.1 依赖声明

IDE1.0 仓 `Cargo.toml` (workspace 根):

```toml
[workspace.dependencies]
aci-emitter = { git = "https://github.com/UlyssesLeoLee/aci-emitter", rev = "df28c56" }
```

锁 `rev = df28c56` (= ULYS-191.1 main HEAD), 防止上游 master 推进时漂移.

### 2.2 字段 1:1 对齐

IDE1.0 的 `.aci.json` 1:1 拷贝自 Star [`tools/star-flash-mock/.aci.json`](https://github.com/UlyssesLeoLee/Star/blob/main/tools/star-flash-mock/.aci.json).
所有字段名 (10 必填 + 2 条件)、4 测试层、5 严重度、4 状态、17 expect_value_types、6 scope_dimensions 全部一致.

### 2.3 IDE1.0 端 CLI

`crates/ide-cli/src/lib.rs::run_aci_emit` 调 `aci_emitter::AciEmitter::new(Layer::It).build(...)`,
输出 JSON 经 `to_json` + `serde_json::to_string_pretty` 后 stdout.

CLI 命令:

```bash
ide-cli --version          # 打印版本
ide-cli --aci-emit [ID]    # emit 一条 ACI assertion
ide-cli --help             # 打印 help
```

完整 CLI (subcommands / config / REPL / session) 待 DD-02 实装.

## §3 CI 跨项目验证

### 3.1 当前 CI 范围 (IDE1.0 仓)

`.github/workflows/ci.yml` 7 jobs:

| # | Job | 平台 | 内容 |
|---|---|---|---|
| 1 | `rust` | ubuntu-latest | fmt + check + clippy + test + doc |
| 2 | `integration` | ubuntu-latest | `cargo test --workspace --tests` |
| 3 | `markdown-lint` | ubuntu-latest | README + architecture + regression report |
| 4 | `rust-bench` | ubuntu-latest | placeholder (无 benchmark) |
| 5 | `cross-project-smoke` | ubuntu-latest | 验证 aci-emitter 作为 workspace dep 可编译 |
| (skip) | (windows + macos) | — | IDE1.0 仅 ubuntu (节省 CI 分钟) |
| (skip) | (cross-platform × 3) | — | 同上 |

### 3.2 aci-emitter 上游变更同步

IDE1.0 CI **不主动** 监听 aci-emitter upstream 变更, 但:

- (a) `aci-emitter = { git = "...", rev = "df28c56" }` 锁 rev, 上游 master 推进不影响 IDE1.0
- (b) 上游 release tag 时人工 bump `rev`, 走 PR 流程
- (c) §4.5 跨项目 CI 落地后, 改 crates.io publish 或 path 依赖

## §4 未来 (per Stage 3 报告)

| 阶段 | brief | 范围 |
|---|---|---|
| §4.2.2 | **ULYS-191.2 (本笔)** | IDE1.0 最小骨架 |
| §4.3.1 | ULYS-191.3 | RGS 集成 (接 rgs-flash-mock, 跨项目 smoke 第 1 个) |
| §4.3.2 | ULYS-191.4 | CATs 集成 (接 cats-mock) |
| §4.3.3 | ULYS-191.5 | IM1.0 集成 (接 im-testkit, 最成熟框架) |
| §4.3.4 | ULYS-191.6 | Ada 集成 (接 ada-mock) |
| §4.3.5 | ULYS-191.7 | GitGit 集成 (TS emitter + MSW 适配) |
| §4.4 + §4.5 | ULYS-191.8 | 统一 Consumer (aci-summary CLI) + 跨项目 CI |

## §5 风险

| # | 风险 | 缓解 |
|---|---|---|
| R-1 | **git dep 跨项目漂移** | 锁 rev + §4.5 跨项目 CI 监听 |
| R-2 | **IDE1.0 仓 0 Cargo 经验** | rust-toolchain.toml 锁 stable + 1.77 MSRV + `-j 4` workaround |
| R-3 | **placeholder 被误认为已实装** | README §1 + `lib.rs` 文件头 标注"Stage 3.0 最小骨架" |
| R-4 | **CLI 设计简陋** | README §3 + IT-1/IT-2 仅测 ACI 字段, 不测 CLI 完整功能 |

## §6 参考

- [ULYS-191 §4.2 Stage 3 报告](https://github.com/UlyssesLeoLee/Multica) — 方案 A vs B 决策
- [aci-emitter v0.1.0 (ULYS-224)](https://github.com/UlyssesLeoLee/aci-emitter) — Rust emitter 上游
- [Star `.aci.json` schema v0.1](https://github.com/UlyssesLeoLee/Star/blob/main/tools/star-flash-mock/.aci.json) — IDE1.0 schema 1:1 来源
- [Star `_lib_aci_emit.py`](https://github.com/UlyssesLeoLee/Star/blob/main/tools/star-flash-mock/scripts/_lib_aci_emit.py) — Python emitter (跨语言 parity 对照)
- DD-01~04 详细设计书 (IDE1.0 仓内)
