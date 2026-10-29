# 热插拔插件系统 (设计)

> 目标: 编辑器的"可选能力"(Mermaid 渲染、Markdown 工具、AI 对话……) 做成**插件**, 不进核心架构; 插件可以从 **GitHub 仓库的最新 Release** 一键安装 / 更新, 运行中热装热卸, **和宿主不匹配时明确报错**, 并让用户填写的大模型给出改善方案。第一款默认插件是 [Pele](https://github.com/obsidianmd/pele) (Mermaid → SVG, MIT)。
> 遵守 [CODING-STANDARDS](../02_详细设计/CODING-STANDARDS.md): 语义在 Rust、前端只搬运和描画、零成本(不用不占资源)、不轮询。

## 1. 为什么是"沙箱 + 能力声明"

插件代码来自第三方仓库, 在编辑器页面里直接 `eval` 等于把整个编辑器和用户的 API Key 交出去。所以:

- 插件 JS 跑在 **沙箱 iframe** 里 (`sandbox="allow-scripts"`, **不给** `allow-same-origin`): 拿不到宿主的 DOM、localStorage、Key。
- 插件和宿主只通过 `postMessage` 的**RPC 通道**说话; 宿主只暴露插件在 `manifest.json` 里**声明过且用户安装时同意过**的能力 (`permissions`)。
- 插件返回的 HTML / SVG 由宿主**净化**后才插入页面 (去 `<script>`、`on*` 属性、`javascript:` 链接)。
- 安装 / 更新**必须经用户确认** (显示仓库、版本、要的权限、各文件的 SHA-256); 更新时权限变多要重新确认。
- 默认插件随程序打包 (离线可用), 也走同一套沙箱, 不享受特权。

## 2. 插件是什么 (契约)

沿用 Obsidian 的发布习惯 (用户熟悉): 一个 GitHub Release 的**资产**里放

| 资产 | 必需 | 说明 |
|---|---|---|
| `manifest.json` | 是 | 元数据和声明, 见下 |
| 入口脚本 (`manifest.entry`, 默认 `main.js`) | 是 | 在沙箱里运行, 通过全局 `ide` 对象注册能力 |
| `styles.css` | 否 | 只作用于插件自己的沙箱和它贡献的渲染结果 |
| `LICENSE` | 建议 | 安装确认框里显示许可证 |

```jsonc
// manifest.json
{
  "id": "pele",                         // 全局唯一, [a-z0-9-]+
  "name": "Pele (Mermaid)",
  "version": "1.12.0",                  // semver
  "apiVersion": 1,                      // 本插件按哪一版宿主 API 写的
  "minAppVersion": "0.1.2",             // 可选
  "license": "MIT",
  "description": "把 ```mermaid 代码块渲染成 SVG",
  "repo": "obsidianmd/pele",            // 更新源 (owner/name)
  "entry": "main.js",
  "permissions": [],                    // 声明用到的宿主能力, 见 §4
  "contributes": {                      // 贡献点, 见 §3
    "markdown.fence": [{ "lang": "mermaid", "method": "render" }]
  }
}
```

入口脚本里:

```js
ide.register({
  // markdown.fence 贡献点: 输入围栏代码块文本, 返回 { svg } | { html } | { error }
  render(text, opts) { return { svg: pele.render(text).svg }; },
});
```

## 3. 贡献点 (宿主提供的"插座")

| 贡献点 | 谁调用 | 返回 | 状态 |
|---|---|---|---|
| `markdown.fence` | Markdown 预览遇到 ` ```lang ` 代码块 | `{svg}` / `{html}` / `{error}` | **已实现** (默认插件 Pele) |
| `command` | 命令面板 / 工具条 (命令名 `plugin:<id>:<cmd>`) | 有 `editor.read` 才把当前文档文本交给插件; 返回 `{svg\|html, summary}` 或 `{error}`, 宿主净化后放进对话框 | **已实现** (默认插件 `doc-graph`) |
| `agent.tool` | 内建对话 (Chat 右栏) 调用; 工具名 `plugin_<id>_<name>` | JSON 结果; 只读 —— 想改文档只能返回补丁提案, 且要再过 Rust `propose_patch` 校验 | **已实现** (`doc-graph` 的 `stats`) |
| `statusbar.item` / `sidebar.panel` | 宿主在对应位置渲染 | 受限 HTML | 后续 |

新增贡献点 = 宿主加一个"插座"并在本表登记; **不改插件机制本身**。

## 4. 权限 (能力声明)

| 权限 | 能做什么 |
|---|---|
| (无) | 纯函数: 文本进、结果出 (Pele 就是这一类) |
| `editor.read` | 读当前文档 / 选区 (通过 RPC, 只读快照) |
| `editor.write` | 产出补丁请求, 由宿主按一步撤销套用 |
| `net:<host>` | 只能访问声明过的域名 (由宿主代发, 沙箱本身没有网络) |
| `storage` | 插件私有的小键值存储 (不能碰宿主设置) |

没声明就没有: 沙箱里 `fetch` / `XMLHttpRequest` / `WebSocket` 被 CSP 禁掉。

## 5. 生命周期 (热插拔)

```
installed ──enable──▶ enabled ──(首次被用到)──▶ loaded (沙箱 iframe 存在)
    ▲                    │                           │
    └────── disable ─────┴─────── unload/update ─────┘   (移除 iframe, 立即生效, 不用重启)
```

- **懒加载**: 启用 ≠ 加载。只有贡献点被真正用到时才创建 iframe 并执行入口脚本; 用不上的插件**零成本**。
- **热卸载**: 禁用 / 卸载 / 更新 = 移除 iframe + 取消它在宿主登记的贡献点; 正在渲染的内容回退成普通代码块。
- **崩溃隔离**: 插件抛异常 / 超时 (默认 2 s) / 返回过大 (默认 2 MB) 只影响它自己, 宿主记录错误并按贡献点回退。
- 状态 (启用 / 版本 / 同意的权限 / 资产哈希) 存在应用数据目录的 `plugins/registry.json`, 资产存在 `plugins/<id>/`。

## 6. 从 GitHub 安装 / 更新

流程 (全部由 Rust 侧 `editcore::plugin_host` 判定, 前端只显示):

1. 用户输入 `owner/repo` (或粘贴仓库 URL)。
2. 取 `releases/latest`: tag、资产列表。
3. **兼容性检查** (`check_compat`): 有 `manifest.json`? 能解析? `id` / `version` / `entry` 合法? `apiVersion` ≤ 宿主支持的最大版本? `minAppVersion` 满足? `entry` 资产存在? 权限都认识? 文件大小在上限内?
4. 通过 → 弹确认 (仓库、版本、许可证、权限、SHA-256) → 用户确认后下载资产, **原子写**入 `plugins/<id>/`, 登记 registry。
5. **一键更新**: 对每个已装插件比较 `registry.version` 与最新 tag; 有新版则同样走 3–4, 成功后热替换 (卸载旧沙箱, 下次使用时加载新版)。
6. 失败不留半成品: 临时目录 + 原子替换, 出错保留旧版本可用。

网络: 只在用户点"检查更新 / 安装"时才发请求 (**不轮询、不后台检查**)。用系统自带的 `curl` 子进程完成 HTTPS (Windows 10+ 自带), 不给编辑器引入 TLS 依赖; 没有 `curl` 时报清楚的错。

## 7. 不匹配时: 报错 + 让大模型给方案

`check_compat` 返回结构化问题, 不是一句"失败":

```jsonc
{
  "ok": false,
  "repo": "obsidianmd/pele", "tag": "v1.12.0",
  "assets": ["pele.global.js", "..."],
  "problems": [
    { "code": "NO_MANIFEST",  "message": "最新 Release 里没有 manifest.json", "hint": "在仓库 Release 里附上 manifest.json 和入口脚本, 或做一个适配仓库" },
    { "code": "API_TOO_NEW",  "message": "插件要求 apiVersion 3, 宿主最高支持 1", "hint": "升级编辑器, 或使用插件的旧版本" }
  ]
}
```

界面显示这些问题, 并提供 **"让 AI 给出改善方案"** 按钮 (需要用户在首选项里填过 AI Key; 没填就提示去填)。点击后宿主把下面这些**事实**拼成提示词发给用户配置的大模型, 把回答原样显示出来 (不自动执行任何建议):

- 宿主插件契约 (本文 §2–§4 的精简版) 和宿主版本 / 支持的 apiVersion;
- 仓库、tag、Release 资产清单、已读到的 manifest (若有);
- 结构化的 `problems`。

要求模型回答: ① 根因 ② 给插件作者 / 用户的具体修改步骤 ③ 如果能写一个适配用的 `manifest.json` / `main.js` 就给出完整内容 ④ 风险提示。**Key 只在用户点按钮时从本机设置读出, 随请求发往用户自己配置的端点**, 不写日志、不发给插件。

## 8. 和其它功能的衔接 (底座)

- **Markdown 预览** (`markdown.fence`): 核心 Markdown 渲染器不含任何图表代码, 遇到 ` ```mermaid ` 就问插件主机"谁能渲染 mermaid"; 没人 → 普通代码块。
- **AI 对话 / Agent API**: 插件的 `agent.tool` 登记进 `editcore::agent_api` 工具表, 对话面板和外部 LangGraph 都能调用; 写类工具只产出补丁。
- **命令**: `command` 贡献点进命令面板, 和宏 / 菜单共用分发。
- **默认插件**: Pele 随程序打包在 `plugins/pele/`; 也能"更新到最新 Release"(若上游 Release 不符合契约则触发 §7 的报错 + AI 方案, 这正是第一个真实的适配样例)。

## 8.1 插件与内建功能冲突 (例如 Pele 与内建 Markdown 渲染)

同一个"能力位"(贡献点 + 键, 如 `markdown.fence:mermaid`、`markdown.render`) 既有内建实现、又有插件提供时, 由宿主**统一仲裁**, 插件不自己抢:

| 偏好 `prefer` | 含义 |
|---|---|
| `plugin` | 插件优先, 内建作回退 (**默认**) |
| `native` | 内建优先, 插件不被调用 |
| `ask` | 首次冲突时询问用户, 记住选择 |

规则 (按顺序):
1. **没有冲突就不仲裁**: 只有一方存在时直接用它。
2. **用户显式选择永远最高**: 偏好按"能力位"存进设置 (`plugins.prefer["markdown.fence:mermaid"]`), 扩展面板里可改。
3. **没激活智能功能 (没填 AI Key / 没打开 Chat) 时, 默认插件覆盖原生**: 不弹窗、不联网、不调用大模型。
4. **激活了智能功能时**, 冲突首次出现弹一次选择框, 并附上"AI 建议"(由用户配置的大模型根据两边的能力说明、版本、权限给出建议, 点按钮才请求); 建议只是建议, 用户点了才生效, 且仍可随时在面板改回。
5. **回退**: 被选中的一方抛错 / 超时时, 本次自动换另一方, 并在状态栏提示一次, 不改偏好。

仲裁是纯函数 `plugin_host::resolve_conflict(candidates, prefer, smart_active) -> Choice` (Rust 侧, 单测覆盖以上五条), 前端只执行结果。

## 9. 实现拆分 (WBS)

| 编号 | 内容 | 位置 | 估时 |
|---|---|---|---|
| PL-1 | `plugin_host`: manifest 解析与校验、`check_compat`、registry 读写、SHA-256、semver 比较 | `editcore::plugin_host` (纯函数 + 单测) | M |
| PL-2 | GitHub 取 latest release / 下载资产 (curl 子进程)、原子安装 / 更新 / 卸载 | `editcore::plugin_host::github` | M |
| PL-3 | 端点: list / install / check-updates / update / remove / toggle / asset | `pure_api` + 两个壳各一行 | S |
| PL-4 | 前端主机: 沙箱 iframe、RPC、权限闸、净化、超时与大小限制、懒加载 | 新文件 `plugins/host.js` (按需加载, 不进 `editor.html`) | L |
| PL-5 | 扩展面板: 已装插件、启用开关、检查 / 一键更新、安装确认框、不匹配报错与"让 AI 给出方案" | `editor.html` 里的小块 + `host.js` | M |
| PL-6 | 默认插件 Pele: 打包 `manifest.json` + `main.js` (包装 `pele.global.js`) + `LICENSE` | `plugins/pele/` | S |
| PL-7 | Markdown 预览 + 顶部工具条 (Obsidian 风格语法), 接 `markdown.fence` | 新文件 `plugins/md-tools` 思路的内建模块 | L |
| PL-8 | 右侧 Chat 栏 + 首选项 AI 设置 + 按需加载的内建图运行器, 接 `agent.tool` | 新文件 `chat/*` | L |
| PL-9 | `agent_api` 工具表 + `/api/agent/*` | `editcore::agent_api` | M |

PL-1~PL-6 是"底座", 先做; PL-7/8/9 作为底座上的第一批使用者。


## 10. 实现状态 (2026-10-28)

PL-1 ~ PL-9 全部落地: `editcore::plugin_host` (+`store`)、`/api/plugins`、沙箱主机 / 扩展面板 / 冲突仲裁、两个默认插件 (Pele、doc-graph)、`agent_api`、Chat 右栏。尚未实现: `statusbar.item` / `sidebar.panel` (原设计里标为"后续")。外部 LangGraph 目前只能调内建的 `/api/agent` 工具表, 插件工具只在 Chat 右栏里可用 (插件在前端沙箱里运行)。
