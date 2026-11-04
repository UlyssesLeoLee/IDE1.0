# ADVERSARIAL_AUDIT.md — 对抗式审查记录（只读取证 → 修复 → 复审）

> - 方法：把自己当成陌生人 PR 的审查者，按「凌晨 3 点被叫醒：这东西把生产搞挂了，最可能是哪一行」去找问题。
> - 判定规则：**只要还有 HIGH 未解决，整体就不判 PASS**（v10.0 `adversarial_review.review_loop`）。
> - 本文所列全部问题**都实际复现、修复并复验过**；没有复现的猜测不写进来。
> - F-01〜F-17 为交付前审查；**F-18〜F-23 为交付后自审（用户要求「自审」）新增**。

---

## 结论速览

| 轮次 | 对象 | 发现 | 其中 HIGH | 状态 |
|---|---|---|---|---|
| 第一轮（交付前） | 5D 图集 | 23 | 8 | 全部修复并复验 |
| 第二轮（对抗式） | 5D 图集 | 4 | 3 | 全部修复并复验 |
| 第三轮（本次） | 需求调整工作台 | 6 | 1 | 全部修复并复验 |
| **合计** | | **33** | **12** | **OPEN HIGH = 0** |

| ID | 严重度 | 类别 | 状态 | 问题 |
|---|---|---|---|---|
| F-01 | HIGH | 实现 | **已修** | 规则 `edgeId` 指向不存在的边，永久不触发且零报错 |
| F-02 | HIGH | 实现 | **已修** | 终态节点不再扩散，导致后续规则永不触发 |
| F-03 | HIGH | 模型 | **已修** | 插件路径在所有桌面 Case 中误触发，污染轨迹 |
| F-04 | HIGH | 视觉/契约 | **已修** | 390px 下检查器被 `display:none` 消失 + 横向溢出 492px |
| F-05 | MEDIUM | 视觉 | **已修** | 3D 标签互相重叠，不可读 |
| F-06 | MEDIUM | 视觉 | **已修** | 3D 相机固定距离，场景出画 |
| F-07 | MEDIUM | 视觉 | **已修** | 2D 边标签与角色副题常显，噪声过大 |
| F-08 | MEDIUM | 视觉 | **已修** | 工具栏按钮文字竖排折行 |
| F-09 | MEDIUM | 集成 | **已修** | `app.js` 硬编码上一轮项目的 ID 与控件 |
| F-10 | LOW | 测试 | **已修** | 跨 realm 数组 strict deepEqual 假阳性 |
| F-11 | LOW | 实现 | **已修** | 省略显示的摘要没有全文 `title` |
| F-12 | INFO | 诚实性 | **已记录** | 5D `compensation` 恒为 null（引擎无该事件类型） |
| F-13 | INFO | 诚实性 | **已记录** | 3 个零转移 Case（规格预期） |
| F-14 | INFO | 诚实性 | **已记录** | 32 条无规则的静态依赖边 |
| F-15 | INFO | 来源隔离 | **已记录** | 源文档陈旧（22 方法 / 13 endpoint / Axum） |
| F-16 | INFO | 协作 | **已记录** | 目标工作树既有未跟踪文件 |
| F-17 | HIGH | 来源隔离 | **已修** | 上一轮产物（他项目事实的文档、失效 E2E 规格）混入 |
| **F-18** | **HIGH** | **功能缺口** | **已修** | **3D 选中不写回检查器 —— 被一条过弱的断言掩盖** |
| **F-19** | **HIGH** | **半成品** | **已修** | **Prototype 的 6 个子标签是装饰按钮，点击无反应** |
| **F-20** | MEDIUM | 规格缺口 | **已修** | 5D 缺规范要求的三种模式 |
| **F-21** | MEDIUM | 规格缺口 | **已修** | 2D 缺 Group 能力 |
| **F-22** | MEDIUM | 交互缺陷 | **已修** | 节点标签溢出方框，导致点击被邻接节点拦截 |
| **F-23** | **HIGH** | **我引入的 bug** | **已修** | **对 `el()` 的返回值调用 `appendChild`，打断原型初始化** |

---

## 交付前审查（F-01〜F-17）

### F-01 [HIGH] 规则的 edgeId 指向不存在的边

- **位置**：`models/causal-rules.json`（R-WB-TERM）/ `models/architecture.json`（E-WB-TO-TERMINAL）
- **复现**：`node .tooling/verify-cases.mjs .` → `rules fired: 29/32, never: R-UNDO-DOC,R-WB-TERM,R-TERM-WB`
- **期望**：每条规则至少在一个 Case 中触发
- **实际**：规则写的是 `E-WB-TO-TERM`，实际边是 `E-WB-TO-TERMINAL`。引擎的 `ruleByEdge` 查不到 → **不报错也不触发**（典型「静悄悄消失」型故障）
- **影响**：4D 的 web 终端链路缺失，5D 部分干预目标失效
- **修复**：改为真实 ID；并新增 `.tooling/verify-refs.mjs` 作为**悬空引用门禁**（规则→边→节点、Case→节点/边/证据、Prompt 四类引用、节点 role/偏移/宝石映射、端口归属）
- **验证**：`verify-refs.mjs` 通过；`rules fired: 32/32`

### F-02 [HIGH] 终态阻断后续扩散

- **位置**：`models/causal-rules.json`（SM-UNDO.terminalStates）
- **复现**：`R-UNDO-DOC` 未触发
- **实际**：引擎的 `isTerminal` 语义是「终态节点不再扩散」。`SM-UNDO` 把 `Completed` 列为终端，undo 结果就永远回不到 `Document`
- **修复**：`SM-UNDO.terminalStates = ["Failed"]`，并在 `SM-UNDO.note` 写明理由
- **验证**：`rules fired 32/32`；C-UNDO-REDO 产生 2 次转移

### F-03 [HIGH] 无关路径互相污染

- **位置**：`models/causal-rules.json`（R-DT-PLUGIN / R-DT-STORE / R-DT-TERM）
- **复现**：`C-TERMINAL-PS` 只有 3 次转移、根本没到 powershell.exe；`SD-INJECT-FAILURE-PSH` 误判为 `NO_DIFFERENCE`
- **实际**：同一入口节点的多个出边都有无条件规则，Case 之间互相污染
- **修复**：引入入口门禁输入（`webModeAvailable` / `desktopModeAvailable` / `docRequested` / `pluginRequested` / `terminalRequested`），每个 Case 显式列举
- **验证**：终端链路真正到达 psh；`SD-INJECT-FAILURE-PSH` 变为 `DIVERGED`（终态差 1）

### F-04 [HIGH] 窄屏检查器消失

- **位置**：`src/styles.css`（旧 `@media (max-width:1024px){ .g-left,.g-right{display:none} }`）
- **复现**：`screenshots.mjs` → `narrow-390 overflow=492 inspector=false`
- **期望**：规范要求「390/768px 下侧栏与检查器改抽屉/底部面板，**因果解释及原型 Tab 不能消失**」
- **修复**：窄屏改抽屉（不隐藏），头部加开合按钮与遮罩；头部换行 + 导航横向滚动
- **验证**：`narrow-390 overflow=0 inspector=true`（全部 6 张）；E2E R01 PASS

### F-05 [MEDIUM] 3D 标签重叠

- **位置**：`src/spatial.js`（`projectLabels`）
- **复现**：3D 截图中「第二客户端窗口 / SCM (git 封装) / ripgrep (rg)」互相压字
- **修复**：按优先级（选中 > 直接相关 > 近 > 远）的贪心屏幕空间避让；LOD 阈值改为相对相机距离
- **验证**：截图目视 + E2E J04/J05

### F-06 [MEDIUM] 3D 相机未适配场景

- **位置**：`src/spatial.js`（`setView` / `resetCamera`）
- **修复**：新增 `fitScene()`，按 FOV **与视口宽高比两侧**同时计算所需距离
- **验证**：3D 截图五层全在视野内

### F-07 [MEDIUM] 2D 标签噪声

- **修复**：边类型标签与角色副题改为「放大/选中/相关时显示」
- **验证**：2D 截图 + E2E J02

### F-08 [MEDIUM] 工具栏竖排

- **修复**：`.g-btn, .g-tlbtn { white-space: nowrap; flex: 0 0 auto; }`

### F-09 [MEDIUM] app.js 硬编码他项目 ID

- **修复**：初始 Prompt 改从 `models/prompts.json` 取；原型标签页按 `primaryScreenGroup` 解析；预置控件改读 `models/prototype.json` 的 `seedWidgets`
- **验证**：E2E J01 / J09

### F-10 [LOW] 跨 realm 数组误判

- **修复**：比较前先转同 realm 数组（`[...op.removedEdgeIds]`），并在注释里写明原因

### F-11 [LOW] 省略文本无全文提示

- **修复**：搜索命中摘要加 `title` 属性

### F-12〜F-16 [INFO] 诚实性登记

补偿恒 null（引擎无 Compensation 事件型）、3 个零转移 Case（规格预期）、32 条静态依赖边、源文档陈旧、目标树既有未跟踪文件 —— 全部以**机器可读**形式固化在 `honestyNotes` / `gaps` / `coverage` 中，并写明「引擎边界 ≠ 项目缺陷」。

### F-17 [HIGH] 上一轮产物污染

- **位置**：`docs/ja-JP/DESIGN_DECISIONS.md`（旧版）、`tests/e2e/atlas.spec.mjs`（旧版）、`models/*.json` 的 schema 标识、`design-system/visual-dictionary.json`
- **实际**：旧文档写的是另一个项目（仓库名、SHA、页面路由）；旧 E2E 规格断言旧标题与旧格式名，**照跑必挂**；数据层残留旧命名
- **修复**：
  1. `DESIGN_DECISIONS.md` 重写为 IDE1.0 专属（DD-01〜DD-15）
  2. `tests/e2e/atlas.spec.mjs` 改为委托到已实现的旅程运行器
  3. 模型与 design-system 的 schema 标识统一为 `ide1-5d-atlas/*`
  4. 视觉词典的宝石适用范围改为 IDE1.0 实体
  5. 25 个一次性修补脚本退避到 `.tooling/retired/`（附误运行说明）
- **验证**：`models/` + `design-system/` + 全部交付文本对外部标记表（见 `.tooling/foreign-markers.txt`）的 grep = **0 件**

---

## 交付后自审（F-18〜F-23）

> 触发：用户要求「自审」。做法：把规范逐条与实现对照，并**用更强的方式重测**（例如把过弱的断言改严）。

### F-18 [HIGH] 3D 选中根本不写回检查器 —— 一条过弱的断言把它掩盖了

- **位置**：`src/spatial.js`（`select()` 无 `onSelect` 回调）、`src/app.js`（`createScene` 未传回调）
- **复现**：
  - 原 E2E：`check("J05", "3D 拾取后在检查器出现对象信息", insp.trim().length > 10, ...)`
  - 空检查器文案是「未选择对象。在 2D 画布或 3D 场景中点击 Node / Edge / Port。」——**长度远大于 10**，所以这条断言在**根本没选中任何东西时也会通过**
- **期望**：规范 `3D_inspector`：「选中 Node/Edge 即按真实 ID 联动 ER、UML、时序、API、状态机、设计文档、证据与 Case」；J05 旅程同样要求
- **实际**：3D 拾取只改 3D 内部 `state.selected`，**从不写回 `S.canvas.selected`**，因此检查器永远显示「未选择对象」；而弱断言让 E2E 报 PASS
- **影响**：3D → 检查器的整条联动链是死的；且**测试绿灯掩盖了功能缺失**（比功能缺失本身更糟）
- **修复**：
  1. `spatial.js`：`select()` 后回调 `opts.onSelect({kind,id})`
  2. `app.js`：`createScene(..., { onSelect: sel => { S.canvas.selected = sel ? [sel.id] : []; renderRight(); } })`
  3. 检查器新增「关联工程图 / 关联 Case / 状态机 / 端口」四行，**按真实 ID 关联**（`diagram-definitions.json` 的 `nodeIds`/`edgeIds`、`cases.json` 的 `nodeIds`/`edgeIds`）
  4. 给 17 张图补 `nodeIds`/`edgeIds`（脚本校验 ID 存在，不存在即失败）
  5. `openDiagramWindow` 提到模块作用域并挂到 `S`，使检查器在任何视图下都能开图窗
  6. **把 J05 断言改严**：必须出现被选中的真实 ID（`N-WB-SERVER`），并断言存在 `data-godiag` / `data-gocase` 链接
- **验证**：E2E J05 由 1 条弱断言扩为 4 条强断言，全部 PASS

### F-19 [HIGH] Prototype 的 6 个子标签是装饰按钮

- **位置**：`src/app.js`（`renderPrototype` 里 `data-sub` 按钮无任何事件）
- **复现**：打开 Prototype → 点「组件库 / 状态与 Case / 设计与测试」→ **什么都不会发生**
- **期望**：规范 `prototype_workbench.sub_tabs` 是必须的真实工作台页签（页面与屏幕 / 可视编辑 / 交互预览 / 状态与 Case / 组件库 / 设计与测试）
- **实际**：6 个按钮只渲染不切换 —— 典型的「看起来有、点不动」半成品
- **修复**：6 个子标签各绑定独立内容块（用 `data-subs` 声明归属，显隐切换；DOM 状态不丢）：
  - 页面与屏幕：屏幕元信息 + DOM 预览 + 原型 JSON
  - 可视编辑：控件清单（含每个控件的 action 数）+ 原型 JSON
  - 交互预览：真实 DOM 预览 + Action DSL 清单 + 可用动作目录
  - 状态与 Case：状态表 / 变量 / Fixture + 5D 绑定表（Case→屏幕→DOM 状态）
  - 组件库：控件目录（**点击即插入画布**）+ 编辑引擎能力清单
  - 设计与测试：按当前屏幕过滤的追踪矩阵（Requirement→Prompt→DD 节点→Case→测试）
- **验证**：E2E J09 新增 6 条断言（子标签齐备 / 切换后内容块变化 / 组件库来自模型 / 从组件库插入后 JSON 变化 / 5D 绑定可见 / 可切回）

### F-20 [MEDIUM] 5D 缺规范要求的三种模式

- **位置**：`src/app.js` / `src/spatial.js`
- **期望**：规范 5D `modes: [Overlay3D, SplitSynchronized3D, BranchTimeline]`，`views: [Overlay, SynchronizedSplit3D, BranchTimeline]`
- **实际**：只有单一叠加模式
- **修复**：
  - 新增模式选择器 `#sIvMode` 与 `S.ivMode`
  - **Overlay3D**：干预结果叠加在同一 3D 场景（分歧节点打环）
  - **SplitSynchronized3D**：`spatial.js` 新增 `setBranchStates/clearBranchStates` + **scissor 双视口**渲染（同一场景、同一相机，左 A 基线 / 右 B 干预后，环色按各分支终态），DOM 侧加 A/B 标题与分隔线
  - **BranchTimeline**：时间轴改为 **A/B 双泳道**，B 泳道在**首次分歧序号**处高亮标记；点击任一泳道事件即把该分支状态回放到 3D
- **验证**：E2E J13 新增 8 条断言（含 `scene3d.state.split.active` 为真、两条泳道、分歧标记）

### F-21 [MEDIUM] 2D 缺 Group 能力

- **期望**：规范 2D canvas 能力含 `Group` / `Collapse` / `MultiSelect`
- **实际**：有多选与折叠，无分组
- **修复**：新增「分组 / 解组」按钮 —— 把当前多选节点框成命名组（虚线框 + 名称 + 成员数），**纯视图态，不进事实图**，并在状态栏注明
- **验证**：E2E J02 新增 3 条断言（分组框出现 / 带名称与成员数 / 可解组）

### F-22 [MEDIUM] 节点标签溢出方框 → 点击被邻接节点拦截

- **位置**：`src/app.js`（2D 节点文本）
- **复现**：实现分组时 Playwright 报 `element ... intercepts pointer events` —— 标签文字比方框宽，使 `<g>` 包围盒大于图形本体，点击其中心时落在**相邻节点**的形状上
- **影响**：真实用户点长名字节点时同样可能点不中（可点区域与视觉不一致）
- **修复**：按框宽截断显示（CJK 记 1 字宽 / ASCII 0.56），完整名放进 SVG `<title>`（hover 与读屏可见）
- **验证**：E2E J02 改为点击**图形本体**并通过；同时截图中的标签拥挤明显缓解

### F-23 [HIGH] 我自己引入的 bug：对 `el()` 的返回值调用 `appendChild`

- **位置**：`src/app.js`（新增的 `renderProtoPanels`）
- **复现**：实现 F-19 后 E2E 出现 `J09 屏幕元信息来源模型 ← (空)`、`DOM 预览 0 个控件`、`J11 widgets:0`
- **实际**：本代码库的 `el()` 返回**HTML 字符串**而不是 DOM 节点，我却在 `renderProtoPanels` 里对返回值调用 `appendChild` → 抛 TypeError → **中断了 `renderPrototype` 的后半段**（预置控件没插入、预览没渲染、JSON 为空）
- **为什么没被立刻发现**：异常发生在**打开 Prototype 之后**，而 E2E 的 console-error 断言只跑在**首页加载**那一次（另一个页面实例）—— 又是「断言覆盖不到真实失败面」
- **修复**：全部改用 `innerHTML` 写入（与代码库既有写法一致），事件在写入 DOM 后再绑定
- **验证**：E2E J09/J10/J11 全部恢复 PASS（75/75）

---

# 第二轮：对抗式审核（13 项攻击 → 可执行门禁）

> 触发：用户要求「对抗式审核并基于结果优化」。
> 做法：把规范 `adversarial_review.tests` 的 13 条攻击项做成**可执行的审核器**
> （`.tooling/adversarial-audit.mjs`），每条都要给出复现命令与实测证据；
> 另派一个**只读独立审查员**（verifier）复核。
> 审核器不是"参观"，它会真的**禁用 WebGL、反复切视图、禁用无关节点、改守卫**来攻击。

## 13 项攻击 → 实测结论

| # | 攻击项 | 结论 | 证据 |
|---|---|---|---|
| 1 | 参考仓库事实污染 | **PASS（修后）** | 交付内容 46 个文件 × 13 个外部标记 = 0 命中（标记表见 `.tooling/foreign-markers.txt`） |
| 2 | 只有构建配置误标 Implemented | **PASS（修后）** | 逐个核对 43 个 Implemented 节点，0 个仅凭 Cargo.toml；补查全部 63 条边亦为 0 |
| 3 | 2D 重复监听 | **PASS** | 48 次视图切换后 DOM 244→501 个节点、console error 0；无 WebGL 上下文泄漏 |
| 4 | 图窗类型误用 | **PASS** | 17 张图逐张渲染，14 种图型全部正确分派，0 个 `Unsupported diagramType`；各图型必需元素字段齐备 |
| 5 | 左手系失配 | **PASS** | 语义 Δx=-14.40 与屏幕 Δx=-106.37 **同号** → 语义 +X 确实在屏幕右侧 |
| 6 | 无 WebGL 伪装 3D | **PASS（修后）** | 以 `--disable-webgl` 启动：显式 `DEGRADED · WebGL 初始化失败` + 状态栏 DEGRADED + 2D 回退元素 1 个，应用不崩 |
| 7 | 无关模块出现假分歧 | **PASS** | `C-PATH-ESCAPE` + `DisableNode(N-OS-PWSH)` → **NO_DIFFERENCE**（禁用与该链无关的终端进程不会造假分歧） |
| 8 | 新节点只是摆件 | **PASS** | 插入后 `removedEdgeIds=["E-WB-TO-STORE"]`、新边 2 条、新规则 2 条；旧边确实从实验副本移除 |
| 9 | 旧 Edge 没被重写 | **PASS** | 同上：旧边不在模型边集中，新边各自带可触发规则 |
| 10 | Konva 只存图没有 Action | **PASS** | 原型 JSON 同时含控件、Action、状态表；DOM 预览为真实控件且可输入 |
| 11 | file:// 网络隐性依赖 | **PASS** | 我方 10 个引擎文件无 eval/动态 import/fetch/XHR；75 趟旅程运行期远程请求 0 |
| 12 | 窄屏因果检查器消失 | **PASS** | 390px 下 `causalInfo` 含 DIVERGED、横向溢出 0px、检查器宽 390px（抽屉形态可达） |
| 13 | 未执行测试被写为 PASS | **PASS** | 报告数字与实测一致（E2E 75/75）；被引用的 `UAT-E2E-UI-01=FAIL`、`UAT-E2E-TERMINAL-02=NOT_TESTED` 保持原状未被升级 |

**审核结论：0 项 OPEN HIGH。**

---

## 本轮新发现并修复的问题

### AF-01 [HIGH] 交付源码里残留另一个项目的实现注释

- **findingId**: AF-01
- **severity**: HIGH
- **fileLine**: `src/engine.js:88`（构建后落在 `dist/index.html` 内的同一段）
- **reproduction**: `node .tooling/adversarial-audit.mjs` → 交付内容全量扫描命中「外部项目实现路径」标记
- **expected**: 交付源码中不出现任何其他项目的实现标识
- **actual**: 引擎的 `matchesDriveLetter` 守卫注释写着「对齐某项目::path_for 的真实规则」——一个**别的项目**的内部函数名，且这段注释会随构建进入产物
- **impact**: 参考仓库事实污染（规范 `adversarial_review.tests` 第 1 项）；读者会误以为 IDE1.0 与该实现有关
- **fixProposal**: 改写为中性且自足的规则说明（Windows 盘符前缀判定），并注明该算子当前未被 IDE1.0 模型使用
- **verificationTest**: 外部标记表全量扫描 0 命中
- **status**: **FIXED**

### AF-02 [HIGH] 只有 Cargo.toml 注释的节点被标成 Implemented

- **findingId**: AF-02
- **severity**: HIGH
- **fileLine**: `models/architecture.json`（节点 `N-FE-JSENGINE`）、`models/evidence.json`
- **reproduction**: `node .tooling/adversarial-audit.mjs` → 「只有构建配置误标 Implemented」检查命中 1 个节点
- **expected**: `Implemented` 必须有**落实了相应行为**的代码行号；依赖声明、目录、接口定义、DTO、Mock、TODO、**注释**都不算数
- **actual**: 「前端 JS 编辑引擎」节点的唯一证据是 `crates/ide-shell-web/Cargo.toml` 里的一句注释「（与 web 前端 JS 引擎并行迁移中）」，代码树中并无独立的 JS 编辑引擎模块
- **impact**: 把一句注释当成已实现 → 2D/3D 会把一个不存在的组件画成"已实现"，直接违反 `implementation_gate`
- **fixProposal**: 节点降级为 `Designed`，新增 `EV-CODE-0053`（kind=SourceCode，status=Designed）如实记录"仅注释提及、无独立实现"；同时把"仅凭 Cargo.toml 证据"的检查**扩展到全部 63 条边**（复查结果 0 条）
- **verificationTest**: `node .tooling/adversarial-audit.mjs` → 43 个 Implemented 节点全部有实现级证据
- **status**: **FIXED**

### AF-03 [HIGH] WebGL 降级路径本身是坏的（降级形同虚设）

- **findingId**: AF-03（审核器编号）/ 对应本轮 F-24
- **severity**: HIGH
- **fileLine**: `src/app.js`（`drawFallback2D`）
- **reproduction**: 以 `chromium.launch({ channel:"chrome", args:["--disable-webgl","--disable-gpu"] })` 打开 → 3D 视图 → 控制台报 `Cannot create property 'innerHTML' on string '<svg …>'`
- **expected**: 无 WebGL 时显式 DEGRADED + 可交互 2D 回退（规范第 6 项）
- **actual**: 降级分支确实设置了 `DEGRADED` 文案与状态栏，但**回退函数本身崩了**：`el()` 返回的是 HTML 字符串，代码却对字符串赋 `innerHTML`（与本轮 F-23 同类错误）；随后回退函数又 `container.innerHTML = ""` 把降级横幅**清空**
- **impact**: 没有 WebGL 的机器上，3D 视图既没有真 3D、也没有可用的 2D 回退 —— 降级路径从未被任何测试覆盖过（此前所有测试环境都有 WebGL）
- **fixProposal**: ① 回退图用 `insertAdjacentHTML` 写入（字符串只能这样用）；② **不再清空容器**，保证 `g-degraded` 横幅留在页面上；③ 标签按框宽截断
- **verificationTest**: 禁用 WebGL 重跑 → `DEGRADED` 横幅存在、状态栏 DEGRADED、2D 回退元素 1 个、无 pageerror
- **status**: **FIXED**

### AF-04 [MEDIUM] 审核器自身两处误报（检查器也要被打假）

- **findingId**: AF-04
- **severity**: MEDIUM
- **fileLine**: `.tooling/adversarial-audit.mjs`
- **reproduction**: 首轮审核报出 3 项 HIGH
- **expected**: 检查结论可复现且不因检查器自身文本而误判
- **actual**: ① 污染扫描把**检查器自己**与**修复脚本**里的标记字面量当成污染；② 网络 API 扫描把 **vendor 压缩包**里的同名字符串当成可达依赖；③ 3D 标签类名与输出目录名属于引擎内部标识符与规范给定目录名，不是业务事实
- **impact**: 误报会掩盖真问题（本轮就差点让人去改引擎命名空间）
- **fixProposal**: ① 标记表独立到 `.tooling/foreign-markers.txt` 并排除 `.tooling/`、`dist/`、`qa/`；② 网络 API 只静态扫我方 `src/`，vendor 的可达性由**运行期远程请求数**证明；③ 在标记表里写明"刻意不列入"的标识符及理由
- **verificationTest**: 审核器复跑 0 误报、0 OPEN HIGH
- **status**: **FIXED**

---

## 独立只读审查员

已派出一个**只读** verifier 子代理，按同一 13 项攻击清单独立复核（含"自己跑门禁、自己看代码"的要求），
产物路径约定在 `qa/audit-tmp/adversarial-report.md`。
本次会话的子代理通道多次出现网络中断（`net::ERR_CONNECTION_CLOSED`），
因此**主审核结论不依赖它**：上述 11 项攻击全部由 `.tooling/adversarial-audit.mjs` 本机实测得出，
每项都有可复现命令与输出。若子代理后续返回结论，将以追加方式并入本文档，不改写既有结论。

---

# 第三轮：对需求调整工作台的对抗式审核

> 触发：用户要求「审核」。对象是**本轮新增的需求调整工作台**（`requirement-workbench.html`）。
> 审核器：`.tooling/audit-requirements.mjs`（10 条挑刺清单）+ `.tooling/verify-requirements.mjs`（21 项功能实测）。
> 审核原则：不接受「看起来没问题」——每条都要么给出可复现证据，要么明确判 FAIL。

## 10 条挑刺清单的结果

| # | 挑刺项 | 结论 | 证据 |
|---|---|---|---|
| AR-01 | 38 条需求全部可算影响面并生成工单 | **PASS** | 0 异常，计算耗时 3ms |
| AR-02 | 无追踪行的需求必须说明原因 | **PASS（修后）** | 16 条无行，界面显式写明「落点来源 + 缺口原因」 |
| AR-03 | 依赖图有自环/环时闭包必须终止 | **PASS** | 自环 `E-KERNELCORE-SELF`、`E-PLUGIN-RETRY` 存在，闭包正常终止 |
| AR-04 | 成本数字不得整体失真 | **PASS（修后）** | 直接代码触点 max=6、7 种取值；等级 high 14 / mid 14 / low 10 |
| AR-05 | 用户输入不得形成 HTML 注入 | **PASS** | 注入 `<img onerror>` 无元素生成、无脚本执行 |
| AR-06 | 宣称「可合回 models/」必须附带可运行的工具 | **PASS（修后）** | `.tooling/apply-requirement-changes.mjs`（试运行/写入/备份/矩阵，端到端已验） |
| AR-07 | 「新增」类变更必须能指定落点 | **PASS（修后）** | 详情页出现 `fLanding` 落点输入（带节点 datalist） |
| AR-08 | 交互元素必须语义化 | **PASS** | 0 个 div onclick |
| AR-09 | 离线：运行期零远程请求 | **PASS** | 远程请求 0、console error 0 |
| AR-10 | 图/Prompt 口径要与 Case 一致 | **PASS（修后）** | 新增 `directDiagrams` / `downstreamDiagrams` |

## 本轮新发现并修复的问题

### AR-01 [HIGH] 宣称「可合回 models/」却没有合并工具 —— 闭环断裂

- **fileLine**: `docs/REQUIREMENT_WORKBENCH.md`（宣称处）、`.tooling/`（缺工具）
- **reproduction**: `node .tooling/audit-requirements.mjs` → AR-06 判 FAIL
- **expected**: 文档承诺的每一步都要有可运行的工具
- **actual**: 工作台能导出 JSON，但没有任何脚本把它写回 `models/requirements.json` —— 用户仍要手工搬字段
- **impact**: 工具链在最后一环断开，「减少调整成本」的承诺只兑现了一半
- **fixProposal**: 新增 `.tooling/apply-requirement-changes.mjs`：默认**只试运行**并打印逐字段 diff；
  `--apply` 才写入，写前自动备份到 `models/.backup/`；`--matrix` 补追踪矩阵行（已有行不覆盖）；
  需求 ID 不存在或 schema 不符直接失败
- **verificationTest**: 在临时副本上跑通「导出 → dry-run（无写入）→ apply（写入+备份+矩阵行）」，正本未受影响
- **status**: **FIXED**

### AR-02 [MEDIUM] 合并脚本的参数契约自相矛盾（实测才暴露）

- **fileLine**: `.tooling/apply-requirement-changes.mjs:19`
- **reproduction**: `node .tooling/apply-requirement-changes.mjs <proposal.json>` → `TypeError: Cannot read properties of undefined (reading '2')`
- **actual**: 同一行把 `argv[2]` 既当 ROOT 又当提案文件，且没有 `--file/--root` 的显式约定
- **impact**: 工具一运行就崩 —— 刚写完的"闭环"其实是坏的
- **fixProposal**: 改为显式 `--file/--root`（首个非选项参数仍兼容为提案文件），ROOT 默认取脚本所在项目根
- **verificationTest**: dry-run / apply 两种模式均通过
- **status**: **FIXED**

### AR-03 [MEDIUM] 成本数字整体失真（21/38 条触点 ≥12 个文件）

- **fileLine**: `src-requirements/app.js`（`closure()` / `impactOf()`）
- **reproduction**: 审核 AR-04 首轮 → 21/38 条触点集合 ≥12 个文件，最大闭包 27/46 节点
- **expected**: 触点集合收敛，"改这条需求要碰几处"才有信息量
- **actual**: 依赖闭包**无深度上限**，顺着外壳链条一路吃到 27 个节点，每个节点又带 1–3 条证据 → 文件数膨胀
- **impact**: 成本徽章几乎全变 "high"，与"哪个需求更贵"无关
- **fixProposal**: ① 闭包加**深度上限 2 跳**（直接调用方及其调用方）；② 阈值按实测 score 分布重新标定
- **verificationTest**: 直接代码触点 max=6、7 种取值；等级 high 14 / mid 14 / low 10
- **status**: **FIXED**

### AR-04 [MEDIUM] 无追踪行的需求显示 0 触点却不说明原因

- **fileLine**: `src-requirements/app.js`（`impactOf()` / `impactHtml()`）
- **reproduction**: 审核 AR-02 首轮 → 16 条需求无追踪矩阵行，界面无任何说明
- **expected**: "没有触点"必须区分「缺追踪」「缺证据」「未指定落点」
- **actual**: 16 条需求（FR-02、FR-11、FR-13、FR-14、NFR-02…等）落到 0 触点，用户会读成"这条需求没有代码"
- **impact**: 掩盖了**追踪缺口**这项真实待办
- **fixProposal**: ① 落点来源显式标注（矩阵 / 证据反推 / 使用者指定）；② 无追踪行时用需求**自身的证据路径**作为直接触点（证据是事实）；③ 0 触点时写明原因与"这是待补的追踪缺口"
- **verificationTest**: 审核 AR-02 判 PASS（样本 FR-02，origin=evidence）
- **status**: **FIXED**

### AR-05 [MEDIUM] 「新增」类变更无法指定落点 → 该类型形同虚设

- **fileLine**: `src-requirements/app.js`（`detailHtml()`）
- **reproduction**: 审核 AR-07 首轮 → 变更类型选「新增」后，页签内无落点控件，影响面恒为 0
- **actual**: 五种变更类型里只有「新增」没有落点来源，导致这一类无法计算影响面
- **impact**: 「新增」是实际工作中最常用的一类变更，却不能用
- **fixProposal**: 变更类型为「新增」时显示**落点节点输入**（带全部节点 id 的 datalist），指定后按该落点重算影响面
- **verificationTest**: 审核 AR-07 判 PASS（控件 `fLanding` 存在）
- **status**: **FIXED**

### AR-06 [LOW] 图/Prompt 影响口径与 Case 不一致

- **fileLine**: `src-requirements/app.js`（`impactOf()` 的 `cost`）
- **actual**: Case 分了直接/连带，图与 Prompt 没有 → 同一页里两套口径，读数会偏
- **fixProposal**: 补 `directDiagrams` / `downstreamDiagrams`，界面同步显示
- **verificationTest**: 审核 AR-10 判 PASS
- **status**: **FIXED**

---

## 三轮汇总（已并入下方第四轮）

## 第四轮：原型 / 模型 ↔ 真实代码 一致性核对

> 触发：用户要求「核对你的原型和代码是否基本一致」。
> 做法：不看文档、不看旧结论，**直接从 IDE1.0 代码抽取事实**，再和图集模型逐条对撞。
> 门禁：`.tooling/verify-consistency.mjs`（8 项检查）+ `.tooling/sync-ports.mjs`（端口由代码生成）。

## 抽到的真实事实

| 面 | 真实数量 | 权威来源 |
|---|---|---|
| HTTP 路由 | **72** | `crates/ide-shell-web/src/lib.rs` 的 `("METHOD","/path") =>` 分支 |
| Tauri command | **64** | `crates/ide-shell-desktop/src/lib.rs` 的 `generate_handler![ ... ]` 注册表（不是 `#[tauri::command]` 计数） |
| JSON-RPC 方法 | **20** | `crates/ide-shell-protocol/src/lib.rs` 的 `dispatch()` 里 `match method` 分发表 |
| Activity Bar | **5 项** | `crates/ide-shell-web/src/editor.html:1445-1449`（explorer/search/scm/outline/extensions） |
| editor.html | **10054 行** | 按 `\n` 切分（含空行） |

## 查出的 4 个不一致（全部已修）

| # | 严重度 | 不一致 | 修法 |
|---|---|---|---|
| CC-A | **HIGH** | **模型少登记 32 条真实 HTTP 路由 + 38 条真实 Tauri command**（只登记了 40/72、26/64）→ 2D 画布看不到近半真实接口，改需求时影响面会漏算 | 新增 `sync-ports.mjs`：端口清单**由代码生成**；保留既有 ID（边的引用依赖），缺失自动补、失效自动报 |
| CC-B | **HIGH** | **Activity Bar 写成 4 项**，实为 5 项（漏「搜索」）；Side Bar 同；Editor 区漏了 Breadcrumb —— 来源是 DOC-03 的旧描述 | 按 `editor.html:1445-1449` 实测修正；新增证据 EV-CODE-0054；同步修正 architecture/prototype/spatial 三处模型与日文画面设计书 |
| CC-C | **HIGH** | **editor.html 行数写成 1348**，实为 10054（ADR-010 的旧快照） | 按实测修正（EV-CODE-0055）。**顺带发现**：`Measure-Object -Line` **不计空行**，我据此写的 9518 仍偏小 536 行 → 改用 `split("\n").length` |
| CC-D | MEDIUM | 同步器早期版本留下 **156 个重复端口**（同接口登记两次，影响面统计翻倍） | 去重逻辑改为「被边引用的优先」；`verify-refs.mjs` 增端口唯一性检查 |

## 现在的一致性状态

| 检查 | 结论 |
|---|---|
| CC-01 HTTP 路由双向一致 | PASS（72 = 72，0 缺失 0 虚构） |
| CC-02 Tauri command 双向一致 | PASS（64 = 64，以注册表为准） |
| CC-03 JSON-RPC 方法双向一致 | PASS（20 = 20） |
| CC-04 原型屏幕路由存在 | PASS |
| CC-05 原型 UI 结构 == 代码实测 | PASS（5 项 Activity Bar + Breadcrumb，minimap=false） |
| CC-06 证据可核验 | PASS（82 条路径存在、区间有效；3 条弱证据为启发式边界，已在 QA_FINAL §3.5 声明） |
| CC-07 无过期规模声明 | PASS（10054 行） |
| CC-08 端口 ID 唯一 | PASS（273 个） |

**结论：原型的界面结构、接口面、规模数据现在都与真实代码逐项对齐**；剩余的已知差异只有
**文档侧的陈旧描述**（DOC-03 的「4 panels」、ADR-010 的「1348 行」）——这些已按"代码为事实来源"原则
在图集里更正，并在 `models/evidence.json` 的 gap 中登记为漂移。

---

## 四轮汇总

| 轮次 | 对象 | 发现 | 其中 HIGH | 状态 |
|---|---|---|---|---|
| 第一轮（交付前） | 5D 图集 | 23 | 8 | 全部修复并复验 |
| 第二轮（对抗式） | 5D 图集 | 4 | 3 | 全部修复并复验 |
| 第三轮 | 需求调整工作台 | 6 | 1 | 全部修复并复验 |
| 第四轮（一致性） | 图集模型 ↔ 真实代码 | 4 | 3 | 全部修复并复验 |
| **合计** | | **37** | **15** | **OPEN HIGH = 0** |

**当前状态**：工作台 21/21 功能实测 + 10/10 审核通过；图集 22/22 单测 + 75/75 E2E + 13 项攻击清单 0 OPEN HIGH +
原型/代码一致性 8/8 通过。
所有 HIGH 均已关闭，且每一项都由**可复现的门禁**守着
（`verify-requirements.mjs` / `audit-requirements.mjs` / `adversarial-audit.mjs` / `verify-consistency.mjs` / `sync-ports.mjs`）。

- 第一轮（交付前）23 项 + 第二轮（对抗式）4 项，**合计 27 项发现**
- **OPEN HIGH: 0**
- 13 项规范攻击：**全部 PASS**（含 3 项经修复后转 PASS）
- 审核器本身的可复现性已纳入门禁：`.tooling/adversarial-audit.mjs` 任一检查失败即非零退出
- 复审判定：**通过**（v10.0 review_loop：HIGH 不残留即可通过）
