# BACKLOG.md — 図集的后続課題（未実施事項を正直に列挙）

> - すべての項目に「なぜ未実施か」と「完了判定」を付す。
> - Atlantis Fail（図集側の失敗）と Target Gap（対象側の欠落）を混在させない。

## A. 対象側（IDE1.0）の缺口 — 図集では解決できない

| ID | 項目 | 種別 | 担当 | 完了判定 |
|---|---|---|---|---|
| A-01 | `ide-kernel-core` / `ide-cli` が placeholder のまま | GAP_SOURCE_DOCUMENT | IDE1.0 開発 | `kernel_status()` が "stable" を返し、CLI に session/txn/config/plugin が必要 |
| A-02 | DOC-04 の「22 メソッド / 13 endpoint / Axum」が実装と不一致 | GAP_SOURCE_DOCUMENT | IDE1.0 設計 | 文書更新＋ADR（Accepted のまま履歴追記） |
| A-03 | DD-02 の transaction buffer（楽観排他・衝突解決）が未実装 | GAP_BUSINESS_DECISION | IDE1.0 設計 | トランザクション境界の仕様確定と実装 |
| A-04 | AI Provider に production 実装が無い | GAP_BUSINESS_DECISION | プロダクト責任者 | provider 実装＋C-AI-COMPLETE が Implemented 扱いになる |
| A-05 | editcore カバレッジ 65%（目標 80%） | GAP_SOURCE_DOCUMENT | IDE1.0 開発 | カバレッジ計測で 80% 以上 |
| A-06 | e2e 11 件が WebView2 環境で fail | GAP_SOURCE_DOCUMENT | CI 担当 | webdriver-bidi 移行、または CI での回避の明文化 |
| A-07 | Python クライアント demo が未着手 | GAP_SOURCE_DOCUMENT | IDE1.0 開発 | `client_demo.py` の実装 |
| A-08 | 実 FPS / メモリ測定が未実施 | — | IDE1.0 開発 | 計測手順を README に明記し再現可能な形で取得 |

## B. 図集側の未了項目（自分で解決できるが残していない）

| ID | 項目 | 理由 | 完了判定 |
|---|---|---|---|
| B-01 | 4D エンジンに `Compensation` イベント型が無い | 既存のイベント型設計を差し替えると既存 Case と 5D 判定が全て影響を受ける。現時点では「补偿＝普通遷移」を honestyNotes で明示 | `src/engine.js` に型追加し、`src/causal.js` の判定が `compensated` を返す。完了後は `models/causal-rules.json` の honestyNotes を更新 |
| B-02 | 3D の辺ラベルは線分中央に固定 | ラベル回避はノード側のみ実装。辺ラベルは Smart モードでは非表示のため現状の可読性は担保されている | 辺ラベルにも screen-space 避让を実装 |
| B-03 | 4D/5D の 2D 画面への同期表示が無い | 3D ベースで両者を統合しており、2D への同期は要求に明記が無い | 同一イベントを 2D キャンバスにも再生できる |
| B-04 | 原型 Fixture の実データ化 | 現状は IDE1.0 の API 形状に沿った宣言的 Fixture | `models/prototype.json` の fixtures に実応答例を反映 |
| B-05 | 1D の Prompt に「差分ベースライン」保存がない | Prompt 履歴はセッション内保持 | Prompt の版管理を `models/prompts.json` 側へ拡張 |
| B-06 | 日本語正式設計書の章が 19 章全てではない | 主要 19 型のうち 7 章を実装。coverage YAML で適用外/TBD を明記 | `docs/ja-JP/` を 19 章へ拡張し、coverage を更新 |
| B-07 | 認知受容性実験（30 秒探索）未実施 | 人間被験者が不在 | 被験者 3 名以上で成功率・操作数・時間を記録 |
| B-08 | 図集自身の Rust 側カバレッジ目標 90% 未計測 | 対象と無関係 | vitest coverage を有効化し 90% を実測 |

## C. 意図的に保留している判断

| 項目 | 判断 | 理由 |
|---|---|---|
| C-01 | 対象リポジトリへの commit / push | `git_governance` が明示的に禁止。作業ツリーも read-only 扱い |
| C-02 | `index.html` / `dist/` / `qa/*.json` のバージョン管理 | `generated_policy` により既定は生成物。`.gitignore` で除外 |
| C-03 | 20 メソッドの RPC 名を文書名へ追従させる変更 | 実装 API の改名acent は後方互換性に影響するため本図集では行わない（gap として記録） |

## D. 自審（2026-10-23）で判明した残存の制約

| ID | 項目 | 現状 | 完了判定 |
|---|---|---|---|
| D-01 | SplitSynchronized3D モードでは 3D ラベル層を隠す | 同一シーンを左右 2 ビューポートで描画するため、DOM ラベルと座標が一致しないため隠す（A/B の見出しは DOM 側に配置） | ラベルをビューポート別に射影する仕組みを実装する |
| D-02 | 2D の「分組」は**表示状態**であり事実グラフに記録しない | 分組は FactGraph ではなく `S.canvas.groups` に持つ（仕様上は可視化であり、モデル変更ではない） | 設計上「分組＝表示層」と確定するか、モデルに Group 記録を追加するかを決定 |
| D-03 | 2D ノードのラベルは幅で切り詰め表示 | 長い名前で `<g>` の境界ボックスが図形より大きくなり、隣接ノードにクリックが奪われるのを防ぐため（ADVERSARIAL_AUDIT F-22） | 同上（全名保持は hover の `<title>` にある） |
| D-04 | Prototype の Fixture は宣言的な写在保持 | 実 API 応答の記録ではない | `models/prototype.json` の fixtures に実応答例を反映 |
| D-05 | 認知受容性実験（30 秒探索）と読屏主観評価 | 未実施（人手が未配置） | 被験者 3 名以上で記録し、自動化完了度と区別して報告 |
| D-06 | Rust ワークスペース 253 件・IDE1.0 e2e 72 件の再実行 | 未実施（本成果物は設計図集であり対象をビルドしない） | 対象側で CI を回して本レポートの引用値を更新 |

