# .tooling/ — 构建与验证工具链

> 本目录是**开发工具**，不是交付物本体。`node_modules/` 与构建产物已被 `.gitignore` 排除。
> 交付物在仓库根：`index.html`（双击打开）、`models/`、`src/`、`vendor/`、`tests/`、`docs/`、`qa/`。

## 可复跑的验证脚本（保留在根）

| 脚本 | 作用 | 怎么用 |
|---|---|---|
| `build.mjs` | 构建单文件 `index.html`：内联 CSS / 源码 / 模型 / vendor，并同步写 `dist/index.html` | `node .tooling/build.mjs .` |
| `check-src.mjs` | 用 `node:vm` 逐文件校验 `src/*.js` 语法 | `node .tooling/check-src.mjs src` |
| `vitest.config.mjs` | 单元测试配置（root = 图集根） | `npx vitest run --config .tooling/vitest.config.mjs` |
| `fix-models.mjs` | 模型文件的关键字名/JSON 可解析性批量校验与修补 | `node .tooling/fix-models.mjs models` |
| `apply-audit-fixes.mjs` | **对抗式审核第 1 批修复**（守卫变量、多规则、Case 可达性、悬空边、状态机补全等） | `node .tooling/apply-audit-fixes.mjs .` |
| `apply-audit-fixes2.mjs` | **对抗式审核第 2 批修复**（状态机初/终态、新增 evidence EV-079..082） | `node .tooling/apply-audit-fixes2.mjs .` |
| `fix-edge-id.mjs` | edge id 冲突修复（E-026 已被占用 → 自环改用 E-028）+ 全局唯一性校验 | `node .tooling/fix-edge-id.mjs .` |
| `fix-rule-priority.mjs` | 规则优先级排序（拒绝类在前）+ Case inputs 补齐 | `node .tooling/fix-rule-priority.mjs .` |
| `verify-gates.mjs` | **实证** build 的两道门禁真的会失败（语法错误 / 错模型键 → BUILD ABORTED）。在 TEMP 副本上做，不碰仓库 | `node .tooling/verify-gates.mjs .` |
| `verify-all.mjs` | **全链路验证**：源码语法 → 模型 JSON 可解析 → id 唯一 → 引用完整性 → 全部 Case 实跑转移数 → index/dist 同字节 → 交付物清点。全部通过才退出 0 | `node .tooling/verify-all.mjs .` |
| `mutation-test.mjs` | **变异测试**：在 `.tooling/.mutwork/` 工作副本上注入 9 个真实缺陷，验证测试套件**真的会失败**（不是恒真断言）。每轮约 3 分钟 | `node .tooling/mutation-test.mjs .` |

## 诊断脚本（保留在根，用于复现问题）

| 脚本 | 定位的问题 |
|---|---|
| `diagnose-page.mjs` | `file://` 打开后外壳不渲染（HTML 字符串被当 DOM 节点 `appendChild`） |
| `diagnose-views.mjs` | 2D / 4D / 5D / Prototype 四视图的实际 DOM 状态与 console 输出 |
| `diagnose-2d-proto.mjs` | 2D 画布命中与 Prototype 序列化 / undo-redo 状态 |
| `diagnose-j03-j09.mjs` | 14 种工程图逐个渲染统计 + Prototype 面板语义 |
| `repro-undo.mjs` | undo 前后 Prototype JSON 的逐字符 diff |
| `probe-edge.mjs` | 边的 `elementFromPoint` 命中探测（水平线 rect 高度为 0） |
| `diagnose-chain.mjs` | **维度依赖链实测**：1D→2D 聚焦与反向链接、导览跨视图与解说卡、3D→4D 事件回放、4D→5D 基线继承 |
| `diag-round2.mjs` | 图底座能力自证与 InsertElement 改写后的实际轨迹 |

## 依赖

`.tooling/package.json` 声明的工具链：`esbuild` / `vitest` / `playwright` /
`graphology` / `dagre-graphlib` / `three` / `konva`。

- Playwright **不下载浏览器二进制**：E2E 用 `chromium.launch({ channel: "chrome" })` 走系统安装的 Google Chrome。
- `node_modules/` 在 `.tooling/` 下；`tests/` 通过 `node_modules` junction 解析 `playwright`。
- 第三方库的**运行时**版本与许可以 `vendor/LICENSES.json` 为准（构建期工具链另列 `toolchainPackages`，不随交付物分发）。
