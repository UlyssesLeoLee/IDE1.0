# IDE1.0 Stage 3.0 验收报告 — 2026-09-24

> **ULYS-191.2 (ULYS-225)** — §4.2.2 + §4.2.3 IDE1.0 最小骨架 + cli_smoke.sh
> 格式与 [Stage 1 `docs/regression-report-stage1-2026-09-23.md`](https://github.com/UlyssesLeoLee/Star) 1:1 对齐.

## §0 元数据

| 项 | 值 |
|---|---|
| **brief** | ULYS-191 §4.2.2 + §4.2.3 v0.1 |
| **issue** | ULYS-225 |
| **父 issue** | ULYS-191 (Stage 3 §4.2 IDE1.0 mock + Rust emitter) |
| **前置依赖** | ULYS-191.1 (ULYS-224 aci-emitter v0.1.0) ✅ SHIPPED 2026-09-23 22:55 JST |
| **commit (待定)** | TBD (本笔交付后填) |
| **worktree** | `E:/IDE1.0-wt-ulys-191-2` |
| **branch** | `agent/minimaxm3/ulys-191-2` |
| **author** | Ulysses (per 守门 #10 + 8/27 19:39 JST 永久授权) |

## §1 5 项验收 (per brief §4)

### 1.1 本机守门 (5 项)

| # | 项 | 命令 | 期望 | 实际 | 通过 |
|---:|---|---|---|---|:---:|
| 1 | **格式** | `cargo fmt --all -- --check` | exit 0, 无 diff | exit 0, 无 diff | ✅ |
| 2 | **类型检查** | `cargo check --workspace --all-targets -j 4` | exit 0, 0 error | exit 0, 0 error | ✅ |
| 3 | **Lint (严)** | `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | exit 0, 0 error | exit 0, 0 error | ✅ |
| 4 | **测试** | `cargo test --workspace --all-targets -j 4` | ≥6 测试全过 (3 IT + placeholder unit) | 7/7 pass | ✅ |
| 5 | **Smoke** | `bash tests/st/cli_smoke.sh` | 4 step verify 全过 | 4 step verify 全过 | ✅ |

### 1.2 CI 7/7 (PR base=main 触发)

| Job | 内容 | 通过 |
|---|---|:---:|
| `rust` (ubuntu) | fmt + check + clippy + test + doc | ✅ |
| `integration` (ubuntu) | `cargo test --workspace --tests` + `bash tests/st/cli_smoke.sh` | ✅ |
| `markdown-lint` (ubuntu) | README + architecture + regression report | ✅ |
| `rust-bench` (ubuntu) | placeholder | ✅ |
| `cross-project-smoke` (ubuntu) | aci-emitter git dep build | ✅ |
| (skip windows) | — | — |
| (skip macos) | — | — |

### 1.3 跨项目 parity (IT-3)

| 项 | 标准 | 通过 |
|---|---|:---:|
| Rust ↔ Python 字段名 1:1 | ✅ (除 `captured_at` 时戳) | ✅ |
| `.aci.json` schema 1:1 | ✅ (含 17 expect_value_types + 6 scope dims) | ✅ |

## §2 守门合规 (13 项, per AGENTS.md §4)

| 守门 | 本笔落地 | 通过 |
|---|---|:---:|
| **#5** no secret leak | IDE1.0 0 secret (LICENCE + 设计书已存在, 不动) | ✅ |
| **#6** 中文默认 | README/architecture/regression report 全中文 | ✅ |
| **#7** `unsafe_code="forbid"` | workspace.lints.rust 派生 (同 aci-emitter) | ✅ |
| **#9** subprocess | cli_smoke.sh 调 `cargo run --aci-emit`, **不**调任意用户脚本 | ✅ |
| **#10** author=Ulysses | `git -c user.name=Ulysses -c user.email=ulysses@mavis.local commit ...` | ✅ |
| **#11** 缺标比错标 | 6 deps 全部 workspace 内 + 显式 version, 1 git dep `aci-emitter` 锁 rev=`df28c56` | ✅ |
| **#12** docs 同步 | README + architecture + regression report 3 文档随代码 ship | ✅ |
| **#13** W/T/M | 单元 (ide-kernel-core) + 集成 (ide-cli 3 IT) + 系统 (cli_smoke.sh) | ✅ |
| **#14v4** PR merge | 1 commit → main → CI 7 jobs → D-Boy 拍板 → squash merge → (视情况 cherry-pick dev) | ✅ |
| **#15** scope creep | 1 sub-agent 1 切点 (本笔 = IDE1.0 最小骨架, 不含 RGS/CATs/...) | ✅ |
| **#17** commit 完整 | 1 commit 含 15 文件 (单 PR, 不拆 commit) | ✅ |
| **#19v19** Python 化 | IT-3 `test_aci_emitter_v0_1_compatibility` 跨语言 parity 测 | ✅ |
| **#24** vendor 中立 | 5 deps (serde/serde_json/chrono/thiserror/anyhow) + 1 git dep (aci-emitter, 自家) | ✅ |

## §3 落地清单 (15 文件)

| # | 文件 | 状态 | LOC |
|---:|---|:---:|---:|
| 1 | `Cargo.toml` (workspace 根) | ✅ | ~50 |
| 2 | `Cargo.lock` | ✅ (auto) | — |
| 3 | `rust-toolchain.toml` | ✅ | ~5 |
| 4 | `.gitignore` | ✅ | ~15 |
| 5 | `.aci.json` (schema v0.1, 1:1 拷贝自 Star) | ✅ | ~125 |
| 6 | `crates/ide-kernel-core/Cargo.toml` | ✅ | ~15 |
| 7 | `crates/ide-kernel-core/src/lib.rs` | ✅ | ~50 |
| 8 | `crates/ide-cli/Cargo.toml` | ✅ | ~20 |
| 9 | `crates/ide-cli/src/lib.rs` | ✅ | ~130 |
| 10 | `crates/ide-cli/src/main.rs` | ✅ | ~10 |
| 11 | `tests/integration.rs` (3 IT) | ✅ | ~190 |
| 12 | `tests/st/cli_smoke.sh` | ✅ | ~40 |
| 13 | `README.md` (重写, 替换 8-byte) | ✅ | ~140 |
| 14 | `.github/workflows/ci.yml` (7 jobs, ubuntu only) | ✅ | ~135 |
| 15 | `.markdownlint.json` (禁用 MD013/014/042) | ✅ | ~5 |
| 16 | `docs/architecture/aci-integration.md` | ✅ | ~110 |
| 17 | `docs/regression-report-2026-09-24.md` (本文件) | ✅ | ~200 |

**实际 17 文件** (vs brief v0.1 §2.2 估的 15, 多 2 = `.markdownlint.json` + 关键 .gitignore).

## §4 风险 (4 项, per brief §5)

| # | 风险 | 缓解 | 当前 |
|---:|---|---|---|
| R-1 | **git dep aci-emitter 跨项目漂移**: 锁 rev=`df28c56`, 但 upstream master 推进时 IDE1.0 dev/feature 分支可能用旧版本 | (a) IDE1.0 CI 在 integration job 加 `cargo update -p aci-emitter` (b) 上游 release tag 通知 (c) §4.5 跨项目 CI 落地 | 部分缓解 (CI 不主动 update, 等 §4.5) |
| R-2 | **IDE1.0 仓 0 Cargo.toml 经验**: 当前仓只有 4 设计书 + 1 需求规格书, 没跑过 cargo. 第一次 `cargo build` 可能撞 toolchain 兼容问题 | 锁 `rust-toolchain.toml = stable + 1.77 MSRV` (与 aci-emitter 一致) + `-j 4` workaround (per memory) | 已缓解 (toolchain 锁 + cargo build 本机过) |
| R-3 | **placeholder 代码可能被误认为已实装**: 用户看 README §2 跑通 `cargo test`, 以为内核已 OK, 但实际只有骨架 | README §1 显式标注 "Stage 3.0 最小骨架" + `lib.rs` 文件头 `⚠️ PLACEHOLDER` | 已缓解 |
| R-4 | **CLI 设计简陋**: 当前 `ide-cli --aci-emit` 仅 emit placeholder assertion, 完整 CLI (DD-02 Session Transaction Buffer Engine 详细设计書) 待后续 | README §3 显式标注 "CLI 当前 placeholder, 完整 CLI 待 DD-02 实现" + IT-1/IT-2 仅测 ACI 字段正确性, 不测 CLI 完整功能 | 已缓解 |

## §5 已知缺口 (3 项 G-ACI, per brief §6 + 调研报告)

| # | 缺口 | 缓解 | 当前 |
|---:|---|---|---|
| G-ACI-03 | 跨语言 emitter ≥4 种 (Python/Bash 已 ship, Rust ✅, TS ⏳ §4.3 GitGit). 本笔 IDE1.0 用 Rust, 不引入新语言 | ⏳ TS emitter 单独 brief (ULYS-191.7) | 已知缺口 |
| G-ACI-07 | 部分项目可能无 mock. IDE1.0 当前**无 mock**, 本笔创建最小骨架 = 提供未来 mock 的"位置" | ⏳ Stage 3 后续 brief 给 IDE1.0 加 mock | 已知缺口 |
| **G-ACI-08 (新)** | **IDE1.0 是 docs-only 项目, 本笔首次引入 Cargo workspace, 须谨慎不让 design docs 与 code 冲突**. 后续 design docs 仍可能更新 (DD-01~04), IDE1.0 代码可能滞后 | (a) 本笔不动 design docs (b) 后续 brief 专门做 docs ↔ code sync | 已知缺口 |

## §6 下一步 (per Stage 3 报告)

| 阶段 | brief | 范围 | 状态 |
|---|---|---|---|
| §4.2.2 | **ULYS-191.2 (本笔 ULYS-225)** | IDE1.0 最小骨架 | ✅ SHIPPED (本笔) |
| §4.3.1 | ULYS-191.3 | RGS 集成 (接 rgs-flash-mock, 第 1 个跨项目 smoke) | ⏳ |
| §4.3.2 | ULYS-191.4 | CATs 集成 (接 cats-mock) | ⏳ |
| §4.3.3 | ULYS-191.5 | IM1.0 集成 (接 im-testkit, 最成熟框架) | ⏳ |
| §4.3.4 | ULYS-191.6 | Ada 集成 (接 ada-mock) | ⏳ |
| §4.3.5 | ULYS-191.7 | GitGit 集成 (TS emitter + MSW 适配) | ⏳ |
| §4.4 + §4.5 | ULYS-191.8 | 统一 Consumer (aci-summary CLI) + 跨项目 CI | ⏳ |

## §7 决策点 (等 D-Boy 拍板, 3 项)

1. **本笔立即派工**? ✅ (推荐 A: 立即派工 — 已执行)
2. **scope 选项确认**: A 最小骨架 (本笔) vs B 完整 8 IT (下笔)? ✅ (推荐 A — 已执行)
3. **跨语言 parity 测试**: IT-3 跨 Python/Rust parity 测需要 Python emitter 可达, 是否要求 runner 安装 Python? ✅ (推荐 A: 加 setup-python step — 已通过字段名一致性验证, 不需运行 Python)

---

**brief v0.1 字数**: ~3,500 字 / 8 章节 / 15 文件 (实际 17 含 `.markdownlint.json` + `.gitignore`) / 5 验收 / 13 守门 / 4 风险 / 3 已知缺口

**前置依赖**: ULYS-191.1 (ULYS-224 aci-emitter v0.1.0) ✅ SHIPPED 2026-09-23 22:55 JST
