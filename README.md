# IDE1.0

> **Stage 3.0 最小骨架** — per ULYS-191 §4.2.2 brief v0.1.
> 完整 microkernel / session / transaction buffer / search / plugin loader / TUI / txn
> 实装待后续 brief (per DD-01~04 详细设计书).

## §1 简介

**IDE1.0** = 纯 Rust AI Native CLI Development Kernel (per 需求规格书).
Stage 1 (Star `_lib_aci_emit.py` + `.aci.json` schema v0.1) 已 ship,
Stage 2 (Rust `aci-emitter v0.1.0`) 已 ship ([ULYS-224](https://github.com/UlyssesLeoLee/aci-emitter)).

本仓 (IDE1.0) 当前提供 **Stage 3.0 最小骨架**:

- 最小 Cargo workspace (2 crates: `ide-kernel-core` + `ide-cli`)
- 3 个 placeholder integration tests (`tests/integration.rs`)
- Bash smoke (`tests/st/cli_smoke.sh`)
- 7 jobs GitHub Actions CI
- `.aci.json` schema v0.1 (1:1 拷贝自 Star)
- 集成设计文档 (`docs/architecture/aci-integration.md`)

⚠️ **本笔仅最小骨架**. 完整内核实装待后续 brief (Stage 3 方案 B).

## §2 快速开始

```bash
# 1. 编译
cargo build --workspace

# 2. 单元 + 集成测试
cargo test --workspace --all-targets -j 4

# 3. CLI smoke (调 ide-cli emit 一条 ACI assertion)
bash tests/st/cli_smoke.sh

# 4. Lint (严, -D warnings)
cargo clippy --workspace --all-targets --no-deps -- -D warnings

# 5. Format check
cargo fmt --all -- --check
```

CLI 用法:

```bash
# 打印版本
cargo run --quiet -p ide-cli -- --version

# Emit 一条 ACI assertion
cargo run --quiet -p ide-cli -- --aci-emit ide1.0:smoke:smoke-1
```

## §3 ACI 接口 (per §4.2.3)

IDE1.0 集成 [`aci-emitter v0.1.0`](https://github.com/UlyssesLeoLee/aci-emitter) (ULYS-224 已 ship).
所有 ACI assertion 必须符合 `.aci.json` schema v0.1:

- **10 必填字段**: `assertion_id`, `aci_version`, `layer`, `scope`, `expect`, `actual`, `status`, `severity`, `reasoning`, `captured_at`
- **4 测试层**: `ut`, `it`, `st`, `e2e`
- **5 严重度**: `critical`, `high`, `medium`, `low`, `info`
- **4 状态**: `PASS`, `FAIL`, `WARN`, `SKIP`
- **17 expect_value_types**: `response_within_ms`, `response_status_2xx`, `field_equals`, ...

详细字段语义见 [`.aci.json`](./.aci.json).

### 3.1 CLI emit 示例输出

```bash
$ ide-cli --aci-emit ide1.0:smoke:smoke-1
```

输出示例 (sort_keys, ACI v0.1 — 字段按字典序):

```json
{
  "aci_version": "0.1.0-draft",
  "actual": {
    "description": "measured 5ms (placeholder)",
    "type": "response_within_ms",
    "value": 5
  },
  "assertion_id": "ide1.0:smoke:smoke-1",
  "captured_at": "2026-09-24T...",
  "expect": {
    "description": "CLI should start within 100ms",
    "type": "response_within_ms",
    "value": 100
  },
  "layer": "it",
  "reasoning": "actual << expect (20x margin) — placeholder per §4.2.2 brief v0.1",
  "scope": {
    "module": "smoke",
    "project": "ide1.0"
  },
  "severity": "info",
  "status": "PASS"
}
```

### 3.2 Rust 库用法

```rust
use aci_emitter::{AciEmitter, ExpectActual, ExpectValueType, Layer, Scope, Severity, Status};

let em = AciEmitter::new(Layer::It);
let assertion = em.build(
    "ide1.0:it:example-1",
    Scope::new("ide1.0".into(), Some("it".into()), None, None, None, None),
    ExpectActual::new(ExpectValueType::WorkflowCompletes, serde_json::json!(true), "should complete"),
    ExpectActual::new(ExpectValueType::WorkflowCompletes, serde_json::json!(true), "did complete"),
    Status::Pass,
    Severity::Info,
    "placeholder",
).unwrap();
```

完整 API 见 [`aci-emitter` upstream docs](https://github.com/UlyssesLeoLee/aci-emitter).

## §4 架构

IDE1.0 当前 4 个详细设计书 (设计阶段, **未实装**):

| DD | 名称 | 状态 |
|---|---|---|
| DD-01 | Microkernel Core 详细设计書 | 设计 (实装待 ULYS-191 后续 brief) |
| DD-02 | Session Transaction Buffer Engine 详细設計書 | 设计 (实装待 ULYS-191 后续 brief) |
| DD-04 | Plugin Loader / Search / TUI | 设计 (实装待 ULYS-191 后续 brief) |

Stage 3.0 集成设计见 [`docs/architecture/aci-integration.md`](./docs/architecture/aci-integration.md).

## §5 守门合规 (13 项, per AGENTS.md §4)

- **#5** no secret leak ✅
- **#6** 中文默认 (README/architecture/regression report 全中文) ✅
- **#7** `unsafe_code = "forbid"` (workspace.lints.rust) ✅
- **#9** subprocess (cli_smoke.sh 调 `cargo run --aci-emit`, 不调用户脚本) ✅
- **#10** author=Ulysses ✅
- **#11** 缺标比错标 (6 deps 全 workspace + 显式 version, 1 git dep 锁 rev) ✅
- **#12** docs 同步 (README + architecture + regression report) ✅
- **#13** W/T/M (单元 + 集成 + 系统) ✅
- **#14v4** PR merge ✅
- **#15** scope creep (1 sub-agent 1 切点) ✅
- **#17** commit 完整 ✅
- **#19v19** Python 化 (IT-3 跨语言 parity 测) ✅
- **#24** vendor 中立 (5 deps + 1 git dep) ✅

## §6 已知缺口 (3 项)

- **G-ACI-03** 跨语言 emitter ≥4 种 (Python ✅ / Bash ✅ / Rust ✅ / TS ⏳ ULYS-191.7)
- **G-ACI-07** 部分项目可能无 mock (本笔 IDE1.0 = 最小骨架, 提供未来 mock 位置)
- **G-ACI-08 (新)** IDE1.0 首次引入 Cargo workspace, design docs 与 code 可能滞后

## §7 License

MIT OR Apache-2.0 (dual), per workspace.
