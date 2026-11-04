# IDE1.0 · 5D 工程设计图集 v10.0

> **双击 `index.html` 即可离线使用。** 无需 npm、无需服务器、无需 CDN、无需网络。
> 事实来源：`E:\IDE1.0` @ `dev` / `76c4e8f`（纯 Rust AI Native CLI Development Kernel）

## 这是什么

一份把 IDE1.0 的**真实工程事实**渲染成可交互 5D 图集的交付物：
用静态设计文档讲不清的空间关联、行为动态、异常路径、因果差异与恢复过程，在这里都能看见、能重算、能查证。

| 维度 | 内容 |
|---|---|
| **0D** | 有/无/未知判定（内部存在性门禁，不产出装饰图） |
| **1D** | Prompt 工作台：按需求/模块/Case 生成、编辑、校验、复制、导出、版本差分、链接证据 |
| **2D** | 全元素无限画布（46 节点 / 63 边 / 138 端口）+ 17 张独立专业工程图（14 种图型）；支持搜索过滤、图层、多选、**分组/解组**、折叠、聚焦 |
| **3D** | 真实 Three.js 空间图论：System→Building→Floor→Room→Node/Edge，宝石角色 + 自适应标签（屏幕空间避让）+ 首次进入自动 fit |
| **4D** | 24 个确定性离散事件 Case（正常/异常/边界/并发/超时/重试/恢复/组合），可 Seek/Replay |
| **5D** | 12 个因果干预实验：禁用节点/边、改守卫/超时/重试、注入资源故障、**真正插入元素并改写 A→B**；三种模式 **Overlay3D / SplitSynchronized3D（scissor 双视口）/ BranchTimeline（A/B 双泳道）** |
| **Prototype** | 多标签 Konva.js 原型工作区（6 个子标签页各有内容）+ 真实 DOM 控件预览（不是第六个维度） |

## 快速开始

```
双击 index.html                 → 离线打开全部功能
node .tooling/build.mjs .       → 重新构建单文件（带语法门禁 + 模型键名门禁）
node .tooling/verify-refs.mjs . → 引用一致性（悬空 ID 直接失败）
node .tooling/verify-cases.mjs .→ 4D/5D 引擎真跑 24 个 Case 并回写覆盖率
node .tooling/verify-consistency.mjs . E:\IDE1.0 → 原型/模型 ↔ 真实代码一致性核对（8 项）
node .tooling/sync-ports.mjs . E:\IDE1.0 → 端口清单按真实代码重新生成（防漂移）
node tests/unit/atlas.test.mjs  → 单元测试 22 件
node .tooling/e2e-journeys.mjs  → 真实浏览器 12 条旅程（55 实测）
node .tooling/screenshots.mjs   → 12 张视觉证据 + 布局观测
node .tooling/adversarial-audit.mjs → 13 项对抗式攻击（禁 WebGL / 反复切视图 / 无关节点 / 污染扫描…）
```

> E2E 需要 Playwright 与系统 Chrome（`chromium.launch({ channel: "chrome" })`）。
> `.tooling/node_modules` 已内置 esbuild / playwright / vitest / graphology / konva / three。

## 目录结构

```
index.html              单文件交付物（vendor + 模型 + 源码全内联）
requirement-workbench.html  需求调整工作台（单文件，改需求而非改代码）
dist/index.html         static-http 发布产物（与 index.html 同字节）
src/                    引擎源码（validate/graph/engine/causal/prompt/diagrams/tour/spatial/prototype/app）
src-requirements/       需求工作台源码（app.js / styles.css）
models/                 正本数据（14 个文件）
  architecture.json     事实图：容器/节点/边/端口
  spatial.json          三维空间投影 + 宝石映射
  evidence.json         证据链（path:line）+ 缺口登记
  requirements.json     FR/NFR/IR/CR 全量
  cases.json            4D Case（24）
  causal-rules.json     状态机 + 规则 + 干预目录
  scenario-deltas.json  5D 干预预设（12）
  diagram-definitions.json 17 张工程图定义
  prompts.json          1D Prompt 库（8）
  prototype.json        原型工作区（实屏幕 + 示范屏 + 预置控件）
  traceability.json     追踪矩阵 + 测试清单
  information-placement.yaml 信息放置矩阵（正本）
docs/ja-JP/             日本語 正式設計書（IPA 风格，7 篇 + 章カバレッジ）
docs/AI_IMPLEMENTATION_HANDOFF.md  AI 实现交接契约（8 个任务）
qa/                     质量证据（e2e-report.json / verify-cases.json / screenshots/）
tests/unit/             单元测试
.tooling/               构建与门禁脚本
```

## 需求调整工作台（改需求，而不是直接改代码）

`requirement-workbench.html` —— 同样单文件、离线、双击即用，与图集共用 `models/*.json` 正本。

| | 用途 |
|---|---|
| `index.html`（5D 图集） | **读**：看事实、空间、行为、因果、图与原型 |
| `requirement-workbench.html`（工作台） | **改**：在事实之上改需求，并自动算出改真实代码的代价 |

核心链路：**改需求 → 自动算出真实代码触点（`file:line`、直接/连带、代码/文档分开）→ 需重跑的 Case 与需更新的测试 → 自动生成交接工单**。
成本口径：直接代码×5 + 连带代码×2 + 文档 + 直接Case×3 + 连带Case + 测试 + 图 + Prompt。

使用指南见 `docs/REQUIREMENT_WORKBENCH.md`；实测脚本 `.tooling/verify-requirements.mjs`（21 项全绿）。

## 几条硬规矩
1. **事实来自代码，不来自想象。** 每个 `Implemented` 节点都带 `path:startLine-endLine`；只有设计文档的标 `Designed`。
2. **文档与实现冲突时两边都记。** 例：DOC-04 写「22 方法 / 13 endpoint / Axum」，实现是「另名的 20 方法 / 约 70 路由 / 手写 match」——文档漂移进 `evidence.gaps`，代码为事实来源。
3. **因果结论来自真实重算。** 没有效果就显示 `NO_DIFFERENCE`，没有执行规则就显示 `UNKNOWN`，不伪造分支动画。
4. **干预不污染事实图。** 所有改动落在 `ScenarioGraphDelta`，插入元素会**真正移除旧边**并留痕。
5. **不确定就说不确定。** 未执行的测试标 `NOT_TESTED`，未实测的性能不写数字。

## 本次实测结果

| 门禁 | 结果 |
|---|---|
| 构建（语法 + 键名门禁） | PASS，1561.6 KB 单文件 |
| 引用一致性 | PASS（0 violation） |
| 4D/5D 真跑 | PASS（24/24 Case，规则 32/32，场景类 10/10） |
| 单元测试 | PASS 22/22 |
| 浏览器 E2E（file://） | PASS 75/75（13 段旅程） |
| 视觉 | PASS（1440 与 390 两视口，横滚动 0px） |
| **对抗式审核** | PASS（规范 13 项攻击清单，0 OPEN HIGH） |

详细报告：`QA_FINAL.md`（**中文**）、`ADVERSARIAL_AUDIT.md`（**中文**，27 项对抗性发现，含 11 项 HIGH 的修复记录）。
正式设计书：`docs/ja-JP/`（**日文**，IPA 风格 7 篇 + 章カバレッジ + 设计决定记录）。

## 许可与来源

- 引擎与 vendor：Konva.js（MIT）、Three.js（MIT）、graphology/dagre（MIT/BSD）——见 `vendor/LICENSES.json` 与 `vendor/LICENSES-REVIEW.md`。
- 事实内容来自 IDE1.0 仓库（MIT OR Apache-2.0）。
- 本图集**不修改目标仓库**：未 commit、未 push、未部署。
