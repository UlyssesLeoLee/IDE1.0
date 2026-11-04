# [ID1.0-JP-04] 画面設計書

> - 正本: `models/prototype.json`
> - 対象画面の実体は `crates/ide-shell-web/src/editor.html`（単一ページ）

## 1. 画面一覧

### 1.1 実在画面（Implemented）

| 画面 ID | ルート | コンポーネント | 根拠 |
|---|---|---|---|
| SC-EDITOR | `GET /editor` | `editor.html` | EV-CODE-0052, EV-DOC-0017 |
| SC-TERMINAL | `POST /api/terminal_create` / `GET /api/terminal_output?id=` | `editor.html#BottomPanel-Terminal` | EV-CODE-0021 |
| SC-SCM | `POST /api/scm_status` / `scm_diff` / `scm_commit` | `editor.html#SideBar-SCM` | EV-CODE-0022 |
| SC-OUTLINE | `POST /api/outline` | `editor.html#SideBar-Outline` | EV-CODE-0033 |

非ルート状態: Bottom Panel（Terminal/Output/Problems の単層タブ切替, ADR-005）、
Status Bar（Mode/Pos/File/Tip/Lang）。

### 1.2 設計示范画面（Designed — 目標製品には存在しない）

| 画面 ID | 用途 | なぜ設計したか |
|---|---|---|
| SC-DEMO-ERROR | 4D/5D の阻断状態表示 | IDE1.0 的统一エラー画面は未整備。阻断を可視化するため設計 |
| SC-DEMO-TIMEOUT | 超時→再試行→復旧の可視化 | プラグイン超過時限の規程が未定義（Designed） |
| SC-DEMO-ENV | 実行環境の縮退表示 | CI Linux で powershell/WebView2 が無い実情に対応 |

> これらは「目標製品の画面」を偽装しない。`models/prototype.json` の `knownIssue` にDesigned である理由明記済み。

## 2. 画面構成

```
editor.html（実測 9518 行 / EV-CODE-0055）
├─ Activity Bar (5 項)          资源管理器 / 搜索 / 源代码管理 / 大纲 / 扩展
├─ Side Bar                    Explorer / 搜索 / SCM / Outline / Extensions
├─ Editor                       Tabs + Breadcrumb + Gutter + Content
├─ Bottom Panel (単層)          Terminal / Output / Problems
└─ Status Bar                   Mode / Pos / File / Tip / Lang
```

> **実測が文書より優先**：DOC-03 は「Activity Bar (4 panels)」と記載するが、
> `editor.html:1445-1449` の実体は **5 個**のボタン
> （`data-pane=explorer/search/scm/outline/extensions`）で、「検索」が旧文書に欠落していた。
> 本図集はコード実体を正とする（EV-CODE-0054）。実機の `minimap` は `false`（FR-13「ミニマップ=将来拡張」と一致）。

図: `DG-UML-USECASE-SHELL`（利用シナリオと画面の対応）。

### 2.1 操作と結果

| 操作 | 結果 | 根拠 |
|---|---|---|
| ファイルオープン | エディタに内容、タブ追加 | EV-CODE-0020 |
| タブ切替 | 各タブの caret/selection を保持 | EV-CODE-0017 |
| 3 態モード切替 | キー割当がモードで変化 | EV-CODE-0035 |
| 検索/置換 | 一致箇所の選択・置換 | EV-CODE-0043 |
| Grep | プロジェクト全体の検索結果 | EV-CODE-0031 |
| 端末入力 | powershell.exe へ書込、結果をポーリング取得 | EV-CODE-0021 |
| SCM 操作 | git porcelain 結果表示 | EV-CODE-0030 |
| i18n 切替 | zh ↔ en | EV-DOC-0013 |

## 3. 画面遷移

IDE1.0 は単一ページ構成であり、**ルーティングテーブルは実装に存在しない**（EV-DOC-0017）。
したがって画面遷移は次の 2 種類のみ:

| 種別 | 内容 | 状態 |
|---|---|---|
| タブ遷移 | Editor のタブ群（複数同時保持） | Implemented |
| パネル遷移 | Activity Bar / Side Bar / Bottom Panel の切り替え | Implemented |

図集側は Prototype ワークスペースで画面をタブとして扱い，切替・順序変更・復元が可能（`multipleScreens`）。

## 4. Prototype ワークスペース仕様

| 項目 | 仕様 | 根拠 |
|---|---|---|
| サブタブ | ページと画面 / 可視編集 / 交互プレビュー / 状態と Case / 組件庫 / 設計とテスト | `models/prototype.json` |
| 編集引擎 | Konva.js（OSS コア、MIT）。Polotno 等の有償版で代替しない | vendor `konva.bundle.js` |
| 編集能力 | Stage / Layer / Transformer / Drag / Resize / Selection / MultiSelection / Snap / Grid / Alignment / Group / ZOrder / Undo / Redo / Hotspot | `src/prototype.js` |
| プレビュー | 本物の HTML DOM/CSS 控件。Konva shape で入力可能なふりをしない | `src/prototype.js` |
| IME | テキスト入力は素の `input`/`textarea` | `src/prototype.js` |
| 状態モデル | Page / Component / Action / Guard / State / Fixture / Route / Variant を版管理し、Prototype JSON に直列化 | `models/prototype.json` |
| 出力 | PrototypeJSON / ActionGraphJSON / StateTransitionTable / ImportAndReplay | `models/prototype.json` |

### 4.1 5D 束縛

| Case | 画面 | DOM 状態 | 種別 |
|---|---|---|---|
| C-AUTH-REJECT | SC-DEMO-ERROR | 令牌校验失败、链路在認証辺で阻断 | Designed |
| C-PATH-ESCAPE | SC-DEMO-ERROR | 路径越界、沙箱と保管守卫が二重に遮断 | Designed |
| C-PLUGIN-TIMEOUT | SC-DEMO-TIMEOUT | Waiting → Error → Recovered | Designed |
| C-TERMINAL-MISSING | SC-DEMO-ENV | powershell.exe 不可用 | Designed |
| C-SEARCH-FALLBACK | SC-DEMO-ENV | rg 欠落、内蔵検索へ回落 | Designed |
| C-TERMINAL-PS | SC-TERMINAL | 実 DOM の入力欄と出力カード | **Implemented**（実画面へ束縛） |

## 5. 無障礙とアクセシビリティ

| 項目 | 実装 | 根拠 |
|---|---|---|
| タブ | `role=tablist` / `role=tab` / `aria-selected`、キーボード操作 | `src/app.js` |
| 図形のみ操作 | ノード/エッジ/ポートは `tabindex` と `role=button` を保持 | `src/app.js` |
| 文字の省略 | 2D/3D のラベルは既定で重要項目のみ、Smart モードは遮蔽回避 | `src/app.js` / `src/spatial.js` |
| 動きの抑制 | `prefers-reduced-motion` で遷移を 0ms に | `src/styles.css` |
| 狭幅 | ≤1024px で側欄と検査器を**破棄せず**ドロワー化（ヘッダのボタンで開閉） | `src/styles.css` |
| コントラスト | 状態色は赤緑だけに依存せず、破線・アイコン・文字と併用 | `src/styles.css` |

実測: `qa/screenshots.json` — 1440×900 と 390×844 の全 12 ショットで
横スクロール 0px、検査器は両ビューポートで可視。
