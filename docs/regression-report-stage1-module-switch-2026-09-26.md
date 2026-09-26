# IDE1.0 Mock module_switch Stage 1 Regression Report

> **生成时间**: 2026-09-26T12:30:00Z (per ULYS-190 dispatcher)
> **范围**: .aci.json + .mock-cluster.json + scripts/_lib_mock_switch_ide1.py + tests/test_ide1_mock_switch.py + crates/ide-cli/src/lib.rs 接入 doc
> **触发**: ULYS-190 §4.4 stage5 派工触发 D-Boy 「按照推荐彻底完成任务」 reply 2026-09-26 02:39 JST (comment `01a0db94-e5b1-7624-bbca-1e5ad124370b`)
> **守门**: 守门 #1+#5+#6+#7+#9+#10+#11+#12+#13+#14v4+#15+#19v19+#20+#24

## §1 范围

| # | 路径 | 类型 | 关键内容 |
|---|---|---|---|
| 1 | `.aci.json` | 修改 | 2 plugin x 8 module 全填 (ide-cli 4 + ide-kernel-core 4) |
| 2 | `.mock-cluster.json` | 修改 | `mock_switch_trace_format` 真拼接模板 + `module_count_total/enabled: 8` |
| 3 | `scripts/_lib_mock_switch_ide1.py` | 新 (~140 LOC) | Python reader, CLI: is-enabled / get-mode / trace / validate-compat / read-plugins |
| 4 | `tests/test_ide1_mock_switch.py` | 新 (~120 LOC) | 15 单元测试 (per RGS test_rgs_mock_switch.py 範式) |
| 5 | `crates/ide-cli/src/lib.rs` | 修改 (+30 doc lines) | `## module_switch 接入` 段落 + 8 module 总览 + 跨语言 dispatch 用法 + 跨项目累计表 |
| 6 | `docs/regression-report-stage1-module-switch-2026-09-26.md` | 新 (本文件) | 7-8 段 per AGENTS.md §3 |

## §2 8 module 落地清单

| Plugin | Module count | Modules | 对应未来 subcommand (per DD-01/02) |
|---|---|---|---|
| **ide-cli** | **4** | `emit` / `version` / `help` / `shell` | (DD-02: 实际 subcommand 占位) |
| **ide-kernel-core** | **4** | `version` / `kernel_status` / `aci_emit` / `init` | (DD-01: 实际 microkernel subcommand 占位) |
| **总计** | **8** | — | — |

**命名决策**: IDE1.0 是 docs-only placeholder (per lib.rs 顶部 doc). 8 module 名映射占位 = 后续 DD-01/02/03/04 brief 实际 subcommand 实现时一一对应. 跨项目粒度: IM1.0=per pub fn; CATs=per Rust 子模块; RGS=per pub mod; IDE1.0=per 占位 subcommand 意图.

## §3 验收脚本结果 (5 个 §)

| § | 验证项 | 命令 | 结果 |
|---|---|---|---|
| §1 | `is-enabled` | `python3 _lib_mock_switch_ide1.py --aci-config .aci.json is-enabled` | ✅ `CLUSTER_ENABLED=true` (exit 0) |
| §2 | `get-mode` | `... get-mode` | ✅ `CLUSTER_MODE=offline` |
| §3 | `trace` | `... trace` | ✅ 真拼接 `cluster.enabled=true,mode=offline,plugins=[ide-cli(4m),ide-kernel-core(4m)]=8/8 modules` |
| §4 | `validate-compat` | `... validate-compat` | ✅ `ACI_COMPAT=OK (cluster=0.1.0-draft aci=0.1.0-draft)` |
| §5 | `read-plugins` (NEW) | `... read-plugins` | ✅ JSON: 2 plugins / 8 modules 全 enabled |

### mock-switch-validate.py 跨项目验证 (Star tools/ 已 ship)

| Project | cluster_ok | enabled | mode | aci_status |
|---|---|---|---|---|
| `ide1` (本 commit) | ✅ True | True | offline | OK |

## §4 mock_switch_trace_format 真拼接 (OLD vs NEW)

| 阶段 | 字符串 |
|---|---|
| **OLD** (PR #3 `c4b62bb` §4.3, 静态占位符) | `cluster.enabled={cluster.enabled}, cluster.mode={cluster.mode}, plugins=[ide-cli,ide-kernel-core], modules=per_plugin (TBD)` |
| **NEW** (本 commit 真拼接) | `cluster.enabled={cluster.enabled},mode={cluster.mode},plugins=[ide-cli(4m),ide-kernel-core(4m)]=8/8 modules` |

**实测输出**:
```
cluster.enabled=true,mode=offline,plugins=[ide-cli(4m),ide-kernel-core(4m)]=8/8 modules
```

**trace 长度**: ~92 chars (per G-MS-08 ~80 字 阈值, **超 12 字**, 跨项目截断 batch 跨 session per G-MS-BRIEF-S44-02).

## §5 _lib_mock_switch_ide1.py 扩展 (mirror RGS / Star / IM1.0 範式)

### MockSwitchReader class
- `__init__(aci_config_path, cluster_config_path)` 读两 JSON
- `is_enabled()` / `get_mode()`: cluster 基础
- `validate_compat()`: 校验 cluster.aci_compat_version == aci.aci_compat_version
- `build_trace()`: 用 `str.replace()` 手动 substitute `{cluster.enabled}` 和 `{cluster.mode}` placeholder (per RGS 範式 fix, 因为 str.format() 不支持 dotted kwargs)
- `read_plugins()`: 读 plugins.<id>.modules.<id> 树

### CLI subcommands (5 个)
- `is-enabled` (exit 0=enabled, 1=disabled)
- `get-mode` (print CLUSTER_MODE=...)
- `trace` (print 真拼接 trace)
- `validate-compat` (exit 0=OK, 1+FAIL)
- `read-plugins` (print JSON, exit 0)

注意: argparse order — `--aci-config` 必须在 subcommand 前 (`--aci-config .aci.json read-plugins`), per RGS 範式.

## §6 14 守门合规

✅ #1 code can be tested (Python unittest) - 15 tests ALL PASS
✅ #5 secrets 0 泄露 (IDE1.0 无 secret)
✅ #6 mock 项目存在 (`.aci.json` + `.mock-cluster.json` 在 IDE1.0 root)
✅ #7 mock 不改真实 schema (并存扩展: 仅在 PR #3 v0.1 base 之上加 plugins.<id>.modules.<id> 子字段, 0 改既有 schema_required_fields / emitter_compatibility / backward_compat 等 v0.1 字段)
✅ #9 commit message 完整 + author=Ulysses (per 守門 #10)
✅ #10 author=Ulysses ulysses@mavis.local
✅ #11 透明披露 (本报告 §7)
✅ #12 docs 同步 (本 regression report 7-8 段 per AGENTS.md §3)
✅ #13 W/T/M (Write: lib.rs doc comment; Test: 15 unit tests; Maintain: regression report)
✅ #14 v4 Mavis 审核决策 / 独立审核
✅ #15 1 sub-agent 1 切点 (per D-Boy 「彻底完成」batch override; parent agent 接力 sub-agent 失误)
✅ #19 v19 self-driven (extends RGS/Star/IM1.0/CATs pattern)
✅ #20 documentation 同步
✅ #24 documentation + 跨 session 续做 8 项

✅ #3 D-Boy 「按照推荐彻底完成任务」三 project 一次性派工 override per 守門 #15 v3 + AGENTS.md §4 #3 等价条件

## §7 已知缺口 (5 项, per 守門 #11 透明披露)

### G-MS-BRIEF-S44-01-ide1
**Python helper, Rust native 跨 session** — 当前 `_lib_mock_switch_ide1.py` 是 Python subprocess 形式. Rust native 跨项目 batch 推广 跨 session per G-MS-BRIEF-S44-01.

### G-MS-BRIEF-S44-02-ide1
**trace_format ~92 字 vs G-MS-08 ~80 字, 跨 session 截断** — 本 stage output `cluster.enabled=true,mode=offline,plugins=[ide-cli(4m),ide-kernel-core(4m)]=8/8 modules` 超 12 字 (跨项目: IM1.0 ~120 + CATs ~92 + Star ~139 + RGS ~94 + **IDE1.0 ~92 字** 都超阈值). 跨项目 batch 截断 跨 session.

### G-MS-BRIEF-S44-04-ide1
**不支持 hot reload** — 当前 `.aci.json` change 后必须 restart 进程或 reload per Python helper invocation. v0.2 hot reload 跨 session 评估.

### G-MS-BRIEF-S44-05-ide1
**8 module 命名跨项目一致性, 待 GitGit stage6 验证** — IDE1.0 8 module 命名基于 subcommand 占位意图, 跟 IM1.0/CATs/RGS pub fn 命名粒度不完全可比. 跨项目 naming convention 跨 session 评估.

### G-MS-IDE1-SPECIFIC-01
**IDE1.0 实际跨项目 validate-by-toml 5 plugin profile (per ULYS-191 §4.2.2)** — IDE1.0 docs-only placeholder, 但 mock_switch schema 1:1 对齐 Star. 后续 brief DD-01/02/03/04 实装后, 实际 subcommand 跟 module 名 1-1 对应需 reviewer 验证.

## §8 跨 session 续做入口 (per 守門 #24)

1. 🟡 **§4.4 stage6 GitGit** 派工 (TypeScript MSW frontend 範式, base=dev, 3 plugin: health/repo/vault)
2. 🟡 **§4.4 stage7 Ada** 永久跳过 (降級 L1 only by design)
3. 🟡 **Star design-analysis v0.4 → v0.5** 加 §10 module_switch 落地回顧 (跨项目, 跨 session)
4. 🟡 **G-MS-08 trace_format 截断到 ~80 字** (跨项目 batch: IM1.0/Star/RGS/IDE1.0 + GitGit 6/7 项目都超阈值)
5. 🟡 **G-MS-05 開關變更審計日誌** (Transaction audit SCD-2, 跨项目)
6. 🟡 **G-MS-09 hot reload** (v0.2 評估, 跨项目)
7. 🟡 **Rust native `_lib_mock_switch.rs`** (跨项目, 替换 Python subprocess per G-MS-BRIEF-S44-01 推广)
8. 🟡 **CI mock-switch-validate 加 module 级校验** (本 stage 只校验 cluster, module 级跨 session)
9. 🟡 **Star CI workflow fix** (per §4.4 stage3): `Generate cross-project status report` 应在 `fail>0` 时 `|| true`
10. 🟡 **`db_wtm` / `langgraph` fixture_count 补登** (Star 第 4 / 5 plugin TBD)
11. 🟡 **RGS `scene` module 真实业务验证** (per G-MS-RGS-SPECIFIC-01)
12. 🟡 **Layer 1→Layer 2→Layer 3 贯通验收** (per AGENTS.md §3, 顶层 sub-task)
13. 🟡 **ULYS-190 状态 `in_review` → `done` 最终 flip** (per AGENTS.md, `done` stays human)

## §9 跨项目累计 (per §4.4 stage 1+2+3+4+5, **5/7 项目 module_switch 落地**)

| 项目 | Plugin count | Module count | PR | merged at (JST) | 状态 |
|---|---|---|---|---|---|
| IM1.0 | 5 | 28 | #24 | 9/26 00:18 | ✅ MERGED |
| CATs | 4 | 13 | #18 | 9/26 01:36 | ✅ MERGED |
| Star | 7 | 7 | #151 | 9/26 02:36 | ✅ MERGED |
| RGS | 5 | 12 | #51 | 9/26 12:18 | ✅ MERGED |
| **IDE1.0** | **2** | **8** | **(本 stage 跟踪)** | **(squash merge commit)** | **🟡 待 merge** |
| GitGit | 0 | 0 | — | — | 🟡 pending |
| Ada | 0 (降級 L1 only) | 0 | — | — | 🚫 by design |
| **合计** | **23/23 plugin 累计** | **68/100+ module 累计** | **4/7 MERGED + 1/7 待 merge** | — | — |
