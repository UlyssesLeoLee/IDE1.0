# [ID1.0-JP-05] テスト設計書

> - 対象: IDE1.0 の既存テスト（253 件）と、本図集自身の自己テスト
> - 正本: `models/traceability.json`（テスト一覧）/ `qa/e2e-report.json` / `qa/verify-cases.json`

## 1. テスト戦略

| 層 | 対象 | フレームワーク | 件数（文書記載） | 状態 |
|---|---|---|---|---|
| 単体 | Rust 各 crate | `#[test]` | 253 | 合格率 100%・警告 0（EV-DOC-0015） |
| 結合 | Rust `tests/` | `#[test]` | 同上の一部 | — |
| e2e | ブラウザ操作 | Playwright | 72 | **56 pass / 5 skip / 11 fail**（EV-DOC-0012） |
| UAT | 手動（Windows 11） | 手動 | 21 項目 | 全件 ✅（EV-DOC-0013） |
| 本図集 単体 | 図集モデルと 4D/5D エンジン | `node --test` | 22 | 22/22 pass |
| 本図集 E2E | file:// 実ブラウザ 12 旅程 | Playwright（システム Chrome） | 55 実測 | 55/55 pass |
| 本図集 参照整合 | 全モデル間の ID 参照 | `verify-refs.mjs` | — | 0 件 violation |

## 2. IDE1.0 既存テスト（抜粋）

| テスト ID | 種別 | 対象 | 結果 | 状態 |
|---|---|---|---|---|
| UT-EDITCORE-UNDO-01 | UT | `test_buffer_insert_undo_redo` | 挿入→undo→redo が同一状態 | PASS |
| UT-EDITCORE-GREP-01 | UT | `test_grep_walk_dir` | 再帰走査 | PASS |
| UT-KERNEL-TEXTOPS-01 | UT | find/replace 演算 | 前後/全置換 | PASS |
| UT-WEB-PATHSAFETY-01..04 | UT | `tests/path_safety.rs`（12 件、経路/サイズ/UTF-8/接頭辞/8.3） | 12 件 pass、被覆率 93.94% | PASS |
| UT-WEB-AUTH-01 | UT | token 推測・Origin 偽装の拒否 | pass | PASS |
| UT-PROTOCOL-RPC-01..03 | UT | JSON-RPC 応答・-32601・-32001 | pass | PASS |
| UT-PROTOCOL-AI-01 | UT | `test_ai_provider_trait` | pass | PASS |
| UT-DESKTOP-WIKI-02 | UT | `test_wiki_no_vim_cursor_vscode` | 0 件 | PASS |
| IT-DESKTOP-CMDS-01 | IT | `tests/e2e_commands.rs`（8 件） | pass | PASS |
| IT-DESKTOP-SEARCH-01/02 | IT | rg 経路とフォールバック | pass | PASS |
| IT-DESKTOP-SCM-01/02 | IT | porcelain 解析・非リポジトリ判定 | pass | PASS |
| UAT-E2E-EDITOR-01..04 | UAT | editor.spec.ts（18/18） | pass | PASS |
| UAT-E2E-SYNTAX-01 | UAT | syntax.spec.ts（10/10） | pass | PASS |
| UAT-E2E-PANELS-01 | UAT | panels.spec.ts（6/6） | pass | PASS |
| UAT-E2E-OUTLINE-01 | UAT | outline.spec.ts（8/8） | pass | PASS |
| UAT-E2E-TERMINAL-01 | UAT | terminal.spec.ts（5/5 Windows、Linux は skip） | pass | PASS |
| UAT-E2E-TERMINAL-02 | UAT | powershell 不在環境 | Linux CI で skip、Windows 未カバー | **NOT_TESTED** |
| UAT-E2E-UI-01 | UAT | ui.spec.ts | 17/25、WebView2 で 8 件 fail | **FAIL** |
| CI-UT-01 | UT | `cargo test --workspace`（-D warnings） | 253 件 pass、警告 0 | PASS |
| BENCH-EDITCORE-01 | UT | `cargo bench -p editcore` | 56,971 ops/s | PASS |

### 2.1 未実行のものを PASS 扱いしていないこと

`UAT-E2E-UI-01` は **FAIL** のまま記録する。文書（EV-DOC-0012）は原因を
「CI Linux 環境で WebView2 の `page.click()` が DOM イベントを起動しない」と分析しているが、
これは**環境要因**であり**未修正の不具合**ではない。本図集はこの 2 点を混同しない。

## 3. 本図集の 4D/5D ケース一覧

正本: `models/cases.json`（24 ケース）。すべて実機実行済み（`qa/verify-cases.json`）。

| Case | 種別 | 期待 | 実測イベント | 転移数 |
|---|---|---|---|---|
| C-NORMAL-EDIT | Normal | 遮蔽なし | 18 | 14 |
| C-AUTH-REJECT | Abnormal | 認証辺で阻断 | 16 | 9 |
| C-PATH-ESCAPE | Abnormal | DocStore へ入らない | 17 | 11 |
| C-SIZE-LIMIT | Boundary | 保管辺で阻断 | 18 | 13 |
| C-ENCODING-REJECT | Abnormal | 保管辺で阻断 | 18 | 13 |
| C-FILE-NOT-FOUND | Abnormal | Document 辺で阻断 | 17 | 11 |
| C-RPC-NORMAL | Normal | Kernel へ入る | 4 | 1 |
| C-RPC-UNKNOWN-METHOD | Abnormal | **遷移 0**（仕様どおり） | 2 | 0 |
| C-RPC-BUFFER-NOT-FOUND | Abnormal | Failed 終端 | 1 | 1 |
| C-AI-COMPLETE | Hypothetical | AI 経路に入る | 4 | 2 |
| C-TERMINAL-PS | Normal | 端末が往復 | 8 | 3 |
| C-TERMINAL-MISSING | Environment | 遷移 0（環境不足） | 3 | 0 |
| C-SEARCH-RG | Normal | rg 主経路 | 3 | 2 |
| C-SEARCH-FALLBACK | Abnormal | 内蔵へ回落 | 2 | 1 |
| C-SCM-COMMIT | Normal | git 往復 | 3 | 2 |
| C-GIT-NOT-REPO | Abnormal | Failed 終端 | 1 | 1 |
| C-PLUGIN-TIMEOUT | Timeout | Failed → Retry → Recovered | 3 | 1 |
| C-E2E-CLICK-WEBVIEW2 | Environment | 遷移 0（invoke 未達） | 2 | 0 |
| C-UNDO-REDO | Normal | Ope 記録 | 2 | 2 |
| C-AUTO-BACKUP | Normal | バックアップ生成 | 1 | 1 |
| C-CONCURRENT-TABS | Concurrent | 2 クライアントが直列処理 | 12 | 9 |
| C-COMBINED-BREACH | Combined | 4 重障壁で停止 | 2 | 0 |
| C-DESKTOP-INVOKE | Normal | desktop 経路が無阻断 | 10 | 6 |
| C-TERMINAL-WEB | Normal | web 端末 | 11 | 11 |

**遷移 0 の 3 ケースは「仕様どおり 0」で、理由が `models/cases.json` の coverage に明記されている
（未説明 0 件）。** これを被覆欠落と扱わない。

### 3.1 被覆率（実測）

| 指標 | 実測 |
|---|---|
| ケース数 | 24 |
| 規則が 1 つ以上発火したケース | 24/24 |
| 発火した規則 | **32/32** |
| 規則を持つ辺 | 31/63（残り 32 は静的依存で `deadEdges` に理由付きで登録） |
| 场景クラス被覆 | 10/10（Normal, Abnormal, Boundary, Concurrent, Timeout, Retry, Recovery, Compensation, Combined, Environment） |

## 4. 本図集の 5D 干预（12 预设）

| プリセット | 種別 | 結論 | 終端差 |
|---|---|---|---|
| SD-DISABLE-SANDBOX-ESCAPE | DisableNode | DIVERGED | 0（**守門点はノードでなくガード条件**） |
| SD-DISABLE-SANDBOX-NORMAL | DisableNode | DIVERGED | 0 |
| SD-DISABLE-EDGE-SANDBOX | DisableEdge | DIVERGED | 0 |
| SD-CHANGE-GUARD-STORE | ChangeGuard | DIVERGED | 0（越界系でも進入はしない） |
| SD-CHANGE-GUARD-AUTH | ChangeGuard | DIVERGED | 0 |
| SD-DISABLE-AUTH-NODE | DisableNode | DIVERGED | 0 |
| SD-CHANGE-TIMEOUT-PLUGIN | ChangeTimeout | DIVERGED | **2**（Failed ↔ Completed） |
| SD-CHANGE-RETRY-PLUGIN | ChangeRetry | DIVERGED | 0 |
| SD-INJECT-FAILURE-PSH | InjectResourceFailure | DIVERGED | **1** |
| SD-BYPASS-DOCSTORE | BypassNode | DIVERGED | 0 |
| SD-INSERT-VALIDATOR | InsertElement | DIVERGED | 0（A→X→B への真改写を確認） |
| SD-REPLACE-ROUTE-STORE | ReplaceRoute | DIVERGED | 0 |

12/12 が結論を生成。UNKNOWN は 0（対象は全て事実図に実在）。

## 5. 12 旅程の実測結果

正本: `qa/e2e-report.json`（55 実測、55 pass、0 fail）。

| 旅程 | 内容 | 実測 |
|---|---|---|
| J01 | 1D: Prompt 選択→objective 更新→校验→差分 | PASS |
| J02 | 2D: 検索过滤（46→一部 dim）→Node/Port/Edge 独立選択 | PASS |
| J03 | 2D: 図窓開封、**14 種すべてが正しく分岐**（Unsupported 0 件） | PASS |
| J04 | 3D: WebGL canvas 生成、12 ビュー、复位、**偽 3D への降格なし** | PASS |
| J05 | 3D: ピッキング→検査器連動 | PASS |
| J06 | 4D: Case 切替→実行→タイムライン（Transition と阻断イベントを保持） | PASS |
| J07 | 5D: 干预→実再計算→DIVERGED（UNKNOWN でない） | PASS |
| J08 | 5D: 挿入要素で**旧辺が実際に除去**され A→X→B の 2 辺が生成 | PASS |
| J09 | Prototype: 4 タブ、DOM プレビューは実入力可 | PASS |
| J10 | Konva: 追加→JSON 変化→Undo→Redo | PASS |
| J11 | Prototype JSON が Action と状態表を含む（Stage 形状のみでない） | PASS |
| J12 | 原型↔4D/5D 束縛が存在し Case が実在 | PASS |

回帰: 狭幅（390px）でも 5D の因果結論が可読、R02 として 1D/2D を 10 回往復しても
console error 0・ノード数非倍増。

## 6. 未実施のテスト（明示）

| 項目 | 状態 | 理由 |
|---|---|---|
| Rust ワークスペースの再テスト（253 件） | NOT_TESTED | 本図集のビルドでは Rust をビルドしない（成果物は設計図集） |
| IDE1.0 の e2e 72 件の再実行 | NOT_TESTED | 対象製品が本図集のスコープ外。既存の記録値（EV-DOC-0012）を引用 |
| 実 FPS / メモリ測定 | NOT_TESTED | 計測手順が未整備。**未実測の数値は記載しない** |
| 画面読屏利用者による主観評価 | NOT_TESTED | 自動化のみ実施。`docs/ja-JP/CHAPTER_COVERAGE.yaml` に記録 |
| 認知受容性実験（30 秒探索等） | NOT_TESTED | 人間 被験者が未配置。自動検査の完了と区別して記録 |
