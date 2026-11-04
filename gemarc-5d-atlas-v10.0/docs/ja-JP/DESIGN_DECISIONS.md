# docs/ja-JP/DESIGN_DECISIONS.md — 図集自身の設計決定記録

対象: IDE1.0 @ `dev` / `76c4e8f9ff4775119d1c98e6403eecb020a905fe`
本書は**本アトラス自身の**設計決定であり、IDE1.0 の仕様書ではない。
IDE1.0 の事実は `models/evidence.json`（79 件、全て `path:startLine-endLine` 付き）と
`models/architecture.json`（FactGraph）を正とする。

---

## DD-01 — 入口は index.html のみ

**決定**: 本アトラスの入口は `index.html` のみ。IDE1.0 側の入口（`GET /editor`、
Tauri アプリ、`ide-cli`）は**設計対象**であり、本アトラスの入口ではない。

**理由**: 対象システムを起動・配備・改変しないことが `task_scope` の最上位制約。
対象 Cesar 側が壊れてもアトラスは開ける状態を維持する。

**結果**: `file://` で全機能が動作（外部リクエスト 0 件を実測）。

---

## DD-02 — コード（実装）を正とする。文書との差分も残す

**決定**: `verification` の判定は**実装**で決める。文書にしかない設計は `Designed`、
文書と実装が食い違う場合は**両方を証拠として残す**。

**具体例**（いずれも `models/evidence.json` に証拠 ID あり）:

| 主張 | 文書 | 実装 | 扱い |
|---|---|---|---|
| JSON-RPC メソッド | 22 件（`connect`/`open_file`…） | 別名の 20 件（`kernel_info`/`open_buffer`…） | 実装を正、`EV-DOC-0006` と `EV-CODE-0011` の**両方**を保持 |
| HTTP エンドポイント | 13 件 | 約 70 件 | 同上（`EV-DOC-0007` / `EV-CODE-0020`） |
| HTTP サーバ | Axum | 手書き match、Cargo.toml に axum 依存なし | 同上（`EV-DOC-0020` / `EV-CODE-0019`） |
| クレート数 | 9 | 8（aci-emitter は外部 git 依存） | 同上 |

**-gap 登録**: 上記 3 系統は `GAP-SRC-API-DRIFT` / `GAP-SRC-AXUM-MISMATCH` として登録し、
**片方を黙って捨てない**。

**理由**: 陳腐化した文書を正本にすると、2D/3D が「存在しない API」を描く。
ソース隔離の観点でも、旧実装の構造を混ぜ込まないことが必須。

---

## DD-03 — 4D と 5D は同一の 3D シーンと同一のhj引擎を共有する

**決定**: 4D のアニメーションは 3D シーン上を流れ、5D の A/B 比較は**同じ決定論的エンジン**を再実行した結果のみとする。
プリセット光軌（pre-baked light track）で因果を偽らない。

**結果**: `.tooling/verify-cases.mjs` で 12 干预すべてが結論を生成（`DIVERGED` 12/12、
`UNKNOWN` 0）。`NO_DIFFERENCE` Returned 0 件だが、これは「介入が意味のある差を生んだ」ため。

---

## DD-04 — 干預は「書き換え」である。装飾ノードの追加は干預ではない

**決定**: `InsertElement` は A→B を**本当に** A→X→B に改写し、旧辺は**実験副本から除去**する。
旧辺の去留は `ScenarioGraphDelta.operations.removedEdgeIds` で可查にする。

**検証**: 単体テスト `tests/unit/atlas.test.mjs` の
「插入元素必须真正把 A→B 改写为 A→X→B，旧边从模型中移除并留痕」が
`removedEdgeIds = ["E-WB-TO-STORE"]`、`旧辺が副本に存在しない`、
`新辺が 2 本` を検証。E2E J08 もブラウザ内で同じ確認を行う。

**理由**: 新規ノードを 3D に置くだけで「干預成功」と称する実装は、因果実験として無意味。

---

## DD-05 — 因果結論は「モデル内因果」であって「経験的因果」ではない

**決定**: 5D の説明には必ず
「因果結論仅来自本模型内的规则重算，不代表对 IDE1.0 运行时的经验验证」を付す。
実行されなかった規則は `UNKNOWN`、効果の無い介入は `NO_DIFFERENCE` とする。

**具体**: 介入対象が事実図に存在しない場合、`Causal.compare` は `UNKNOWN` を返し
「差なし」を報告しない（`NO_DIFFERENCE` と混同させない）。

**副次効果として得られた知見**: `C-PATH-ESCAPE` では `DisableNode(N-WB-PATHSAFETY)` だけでは
越界アクセスは通らない（保管辺のガードに `pathWithinRoot` があるため）。**真の守門点は
ノードではなくガード条件**である。この知見はアトラスの 5D 演算から初めて得られた。

---

## DD-06 — 补偿は「engine の限界」として正直に書く

**決定**: 4D エンジンは `Transition` / `GuardBlocked` / `WhenBlocked` / `InterventionBlocked` /
`Retry` / `InjectedFailure` のみを生成し、`Compensation` イベント型を持たない。
そのため 5D の `compensation` 欄は常に `null` になる。

**書き方**: 「IDE1.0 に补偿機構が無い」という結論には**しない**。
`models/causal-rules.json` の `honestyNotes` に
「这是引擎能力边界，不是 IDE1.0 缺少补偿逻辑的结论」と明記する。

**理由**: 黙って `null` を出すと「补偿なし」と誤読される。欠落を欠落として書く。

---

## DD-07 — 証拠は `path:startLine-endLine` まで落とす

**決定**: `Implemented` の根拠は**コード中の実装位置**でなければならない。
依存宣言・接口定義・DTO・Mock・TODO・コメント・文書記述は実装の証拠にならない。

**門禁**: `.tooling/verify-refs.mjs` が
「規則の edgeId → 辺 → ノード」「Case の nodeIds/edgeIds/evidenceIds」「Prompt の 4 種参照 ID」
「ノードの role / spatial オフセット / 宝石映射」「ポートの nodeId」を全数照合し、
1 件でも欠けたら**非 0 終了**する。

**実際にこの門禁が Effekt をInclude した例**: 規則 `R-WB-TERM` の `edgeId` が
実在しない `E-WB-TO-TERM` を指していたため、**エラーも出ずに永久に発火しなかった**
（発火 29/32）。門禁追加後は 32/32。

---

## DD-08 — 0D は存在判定に閉じて、装飾図を作らない

**決定**: 0D（Present / Absent / Unknown）は内部ゲートとしてのみ実装し、
独立ページ・装飾アーキテクチャ図を作らない。`Unknown` を `Present` に読み替えない。

**具体**: `ide-kernel-core` は `kernel_status()` が `"placeholder"` を返すため、
`N-KERNELCORE` の `verification` は `Implemented`（骨格の実体は存在する）だが、
DD-01 の設計能力は `Designed` として別個に扱う。

---

## DD-09 — 図は 14 種を正式記号で描き、Mermaid に委譲しない

**決定**: `src/diagrams.js` の自研 SVG レンダラで UML（クラス/コンポーネント/パッケージ/時序/状態/活動/配備/用例）、
ER、DFD、BPMN、セキュリティ、ネットワーク、データフローを描く。
基数・Guard・Port・生命線・alt/opt/loop/par を失わないため Mermaid は使わない。
未知の `diagramType` は**時序図へ退化させず明示的に失敗**する。

**検証**: E2E J03 が全 14 図種の分岐を実測し、`Unsupported diagramType` 0 件を確認。

**副則**: 宝石は 3D の**ノード形状**にのみ使う。フィールド・辺・関係記号は宝石化しない。

---

## DD-10 — 空間は「意味の階層」であって、想像の机房ではない

**決定**: 3D のコンテナは IDE1.0 の実際の階層（前端 / 後端 / OS / 外部ツール）を投影する。
物理的なデータセンター構成を捏造しない。

**具体**: 後端ビルディングは 5 層（desktop / web / protocol / kernel / editcore）を持ち、
各層に**実在の**モジュール群が部屋として並ぶ。

**注意**: `N-FE-CLIENT2`（第 2 クライアント）と `N-KERNELCORE` の自己ループは
**推論（Inferred / Designed）**であり、コードに実体がないことを `verification` で明示する。

---

## DD-11 — 狭幅では「隠す」のではなく「形を変える」

**決定**: ≤1024px で側欄と検査器を `display:none` にしない。**ドロワー化**する。
ヘッダに開閉ボタンを置き、背景スクリムで閉じる。

**理由**: v10.0 の回帰条件「窄屏检查器不丢失，5D 因果链仍可查阅」に対し、
`display:none` は検査器ごと因果の追跡経路を消す。

**実測**: 修正前は 390px で横スクロール 492px・検査器不可視。修正後は
横スクロール 0px・検査器はドロワーとして可視（`qa/screenshots.json`）。

---

## DD-12 — 初期表示は「重要ラベルのみ」

**決定**: 2D の辺タイプラベルとノードの役割副題は、拡大時・選択時・関連時のみ表示する。
3D のラベルは距離 LOD に加えて**画面空間の遮蔽回避**を行う（選択 > 直接関連 > 近 > 遠の優先度）。

**理由**: 46 ノード / 63 辺のグラフで全ラベルを常時表示すると可読性が崩壊する。

---

## DD-13 — 原型は「本物の DOM」でプレビューする

**決定**: Prototype のプレビューは本物の HTML 控件（`input`/`textarea`/`select`/`button`）で、
Konva shape で入力可能にに見せかけない。IME は素の入力要素で保证する。
Action / Guard / State / Fixture は Konva の Stage 形状だけでなく **Prototype JSON に直列化**する。

**検証**: E2E J09（実 DOM 入力可）、J11（JSON が Action と状態表を含む）。

**前提**: 実画面は `editor.html` 及其パネルに限定し、存在しない画面は `Designed` と明示
（`SC-DEMO-ERROR` / `SC-DEMO-TIMEOUT` / `SC-DEMO-ENV`）。

---

## DD-14 — 生成物は生成物。対象リポジトリは触らない

**決定**: `index.html` / `dist/` / `qa/*.json` はビルド生成物として `.gitignore` に列挙し、
既定では追跡しない。IDE1.0 の作業ツリーには**一切書かない**（commit / push / merge / deploy もしない）。

**現状**: 本アトラスは未追跡ディレクトリ `gemarc-5d-atlas-v10.0/` としてのみ存在し、
HEAD は `76c4e8f` のまま。既存の未追跡ファイル群は本作業以前から存在し、触っていない。

---

## DD-15 — エンジン内部の名前空間は対象外だが、データの schema 識別子は宿主に合わせる

**決定**: モデルの `schema` 識別子は `ide1-5d-atlas/*` に統一する。
一方、`src/*.js` の内部名前空間（`window.GemArcEngine` 等）と 3D ラベル CSS クラス
（`.gemarc-node-label`）は、**自研 5D エンジン実装の内部識別子**であり
IDE1.0 の業務事実ではないため据え置く（全面改名は 11 ファイルAcross の回帰リスクがあり、
本アトラスの価値的重点ではない）。

**Requirement**: データ層に旧プロジェクトの固有名が残らないことを grep で確認できる状態にする
（`models/` / `design-system/` は 0 件）。
