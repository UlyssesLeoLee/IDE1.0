# 需求调整工作台（Requirement Adjustment Workbench）

> 单文件、离线：`E:\IDE1.0\gemarc-5d-atlas-v10.0\requirement-workbench.html` —— 双击即用。
> 事实来源与 5D 图集同源（`models/*.json` 正本，构建期内联）。

## 它解决什么

改真实代码贵，是因为"要改哪里"要靠记忆和摸索。这个工作台把顺序倒过来：

```
改需求（几分钟） → 自动算出真实代码的触点与波及面 → 产出实现工单 → 再按工单改代码
```

它**不改事实**，只产出「变更提案」，由你或脚本合回 `models/`。

## 四个页签

| 页签 | 作用 |
|---|---|
| **需求详情** | 改意图 / 可验证判据 / 状态 / 负责人；选**变更类型**（澄清·收紧·放宽·废弃·新增），每种类型给出「模型侧要同步什么」和「代码侧要动什么」 |
| **影响面分析** | 这次改动会碰到**几个 .rs**、连带波及**几个文件**、**哪些 Case 要重跑**、**哪些测试要更新**、**哪些图和 Prompt 要同步**，逐条给出 `file:line` |
| **实现工单** | 自动生成交接契约（`objective` / `requirementIds` / `sourceEvidence` / `targetModules` / `inputOutput` / `rules` / `failureModes` / `acceptanceTests` / `dependency` / `rollback` / `cost`），格式与 `docs/AI_IMPLEMENTATION_HANDOFF.md` 一致，复制即用 |
| **变更集** | 累积多条需求改动 → 导出 **Markdown 提案**（含合并后的总成本面）或 **JSON**（可合回 `models/`） |

## 成本口径（看之前先知道它怎么算的）

| 指标 | 含义 |
|---|---|
| **直接代码文件** | 需求落点自身的 `.rs` —— 你大概率要动手改的（实测上限 6 个，有区分度） |
| **连带代码文件** | 下游依赖方的 `.rs` —— 需要复核/回归的（**依赖闭包只走 2 跳**，避免整个外壳都算进来） |
| **直接文档 / 连带文档** | 同上，但 `.md`（改文档的成本低于改代码） |
| **直接 Case** | 覆盖落点本身的 Case —— 必须重跑 |
| **连带 Case** | 只覆盖下游的 Case —— 建议抽样回归 |

成本等级 = `直接代码×5 + 连带代码×2 + 文档 + 直接Case×3 + 连带Case + 测试 + 图 + Prompt`，
阈值按实测 score 分布标定（当前 high 14 / mid 14 / low 10）。

**影响面沿依赖方向闭包计算**：改了 X，波及的是**依赖 X 的一方**（调用方），
被依赖方（X 调用的下游）单列为「需确认契约」。

### 落点来源（界面会显式说明，别猜）

| 来源 | 含义 |
|---|---|
| 追踪矩阵 | 该需求在 `models/traceability.json` 里有登记行（最可信） |
| 证据反推 | **没有追踪行**，落点由需求自身的证据路径反推 —— 直接触点可信，波及面属推断 |
| 使用者指定 | 「新增」类变更，由你在「需求详情」里填落点节点 |

没有触点时，界面会写明是**追踪缺口**，不是「无需改代码」。

## 典型用法

1. 左侧搜「终端」或按分类筛（FR/NFR/IR/CR）→ 选中一条需求。
2. 先看 **影响面分析**：确认这次改动的真实代价（几个代码文件、几个 Case）。
3. 在 **需求详情** 改内容、选变更类型、写理由。
4. 到 **实现工单** 复制工单 → 直接开工。
5. 多条改动在 **变更集** 一起导出成提案，再合回 `models/` 并走 ADR 流程。

## 合回模型（闭环）

工作台只产出提案，**由脚本合回正本**：

```bash
# 1) 工作台里「变更集 → 导出 JSON」
# 2) 试运行（只打印逐字段 diff，不写任何文件）
node .tooling/apply-requirement-changes.mjs --file proposal.json
# 3) 真正写入（自动备份到 models/.backup/）
node .tooling/apply-requirement-changes.mjs --file proposal.json --apply
# 4) 同时补追踪矩阵行（已有行不覆盖，保留原登记）
node .tooling/apply-requirement-changes.mjs --file proposal.json --apply --matrix
```

脚本只改 `models/requirements.json`（意图 / 判据 / 状态 / 负责人 / 变更类型）与可选的
`models/traceability.json`；需求 ID 不存在或提案 schema 不符会直接失败，不做半截修改。

## 与 5D 图集的关系

| | 用途 |
|---|---|
| `index.html`（5D 图集） | **读**：看事实、空间、行为、因果、图与原型 |
| `requirement-workbench.html`（本工具） | **改**：在同��事实之上改需求，并算出改代码的代价 |

两者共享 `models/*.json` 正本。改了模型 → 重新构建两个产物：

```bash
node .tooling/build.mjs .              # 重建 5D 图集 index.html
node .tooling/build-requirements.mjs . # 重建需求工作台
node .tooling/verify-requirements.mjs   # 需求工作台浏览器实测（21 项）
```

## 当前限制（写在明处）

- **浏览器单文件无法写盘**：合回必须走上面的脚本（试运行 → `--apply`）。
- **影响面基于当前追踪矩阵的质量**：16/38 条需求**没有追踪行**，其落点靠证据反推（界面会标明）——
  这本身是在提醒你补追踪。
- **依赖闭包只走 2 跳**：更深的传递影响不在读数里（避免数字失真），需要时看 5D 图集的 3D 依赖视图。
- **成本是「面的大小」，不是工时**：不预测人天，只告诉你这次要碰多少处。
- **Case 影响按节点/边归属判定**，不解析 Case 内部逻辑。
- **变更集不跨会话保存**：刷新页面即清空（导出 JSON 才是持久化手段）。
