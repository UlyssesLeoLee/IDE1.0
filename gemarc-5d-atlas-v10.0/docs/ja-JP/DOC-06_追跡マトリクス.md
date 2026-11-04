# [ID1.0-JP-06] 追跡マトリクス

> - 正本: `models/traceability.json`
> - 追跡鎖: Requirement ↔ Prompt ↔ BD/DD ↔ Screen/Node/Edge/Port ↔ API/DB/Event ↔ Case ↔ UT/IT/ST/UAT

## 1. 主鎖（抜粋）

| 要件 | Prompt | DD ノード/エッジ | 画面 | API | Case | テスト |
|---|---|---|---|---|---|---|
| FR-01 ファイルオープン | P-IMPL-DOC-HTTP-API | N-FE-EDITORHTML / N-WB-SERVER, E-FE-TO-WB, E-WB-TO-STORE | SC-EDITOR | GET /editor, POST /api/doc/open | C-NORMAL-EDIT | UAT-E2E-EDITOR-01, UAT-E2E-UI-01 |
| FR-03 タブ切替 | P-IMPL-DOC-HTTP-API | N-KN-SESSION, N-KN-DOCSTORE | SC-EDITOR | GET /api/doc/tabs, POST /api/doc/activate | C-NORMAL-EDIT | UAT-E2E-EDITOR-02 |
| FR-04 3 態編集 | P-IMPL-RPC-KERNEL | N-PT-KERNEL, N-SHELL-TUI | SC-EDITOR | JSON-RPC key_event | C-RPC-NORMAL | UVAL-E2E-EDITOR-03 |
| FR-05 Undo/Redo | P-IMPL-RPC-KERNEL | N-EC-UNDO, N-EC-HISTORY, N-KN-DOCUMENT | SC-EDITOR | POST /api/doc/undo, /redo | C-UNDO-REDO | UT-EDITCORE-UNDO-01 |
| FR-06 検索/置換 | P-IMPL-EDITCORE-SEARCH | N-EC-SEARCHOPS, N-KN-TEXTOPS | SC-EDITOR | POST /api/doc/find, /replace_all | C-NORMAL-EDIT | UT-KERNEL-TEXTOPS-01 |
| FR-07 Grep | P-IMPL-EDITCORE-SEARCH | N-EC-GREP, N-EC-GREPENGINE, N-DT-SEARCH | SC-SCM | POST /api/search | C-SEARCH-RG, C-SEARCH-FALLBACK | UT-EDITCORE-GREP-01, IT-DESKTOP-SEARCH-01/02 |
| FR-09 端末 | P-IMPL-DOC-HTTP-API | N-DT-TERMINAL, N-WB-TERMINAL, N-OS-PWSH | SC-TERMINAL | POST /api/terminal_create, GET /api/terminal_output | C-TERMINAL-PS, C-TERMINAL-MISSING, C-TERMINAL-WEB | UAT-E2E-TERMINAL-01/02 |
| FR-15 自動バックアップ | P-IMPL-DOC-HTTP-API | N-EC-AUTOBACKUP | — | — | C-AUTO-BACKUP | （独立テスト無し＝欠落） |
| NFR-05 ルート内アクセス | P-IMPL-AUTH-TOKEN | N-WB-PATHSAFETY, N-WB-AUTH | SC-DEMO-ERROR | POST /api/read_file | C-PATH-ESCAPE, C-AUTH-REJECT | UT-WEB-PATHSAFETY-01, UT-WEB-AUTH-01 |
| NFR-06 4MB 上限 | P-IMPL-DOC-HTTP-API | N-WB-PATHSAFETY | SC-DEMO-ERROR | POST /api/read_file | C-SIZE-LIMIT | UT-WEB-PATHSAFETY-02 |
| NFR-07 UTF-8 のみ | P-IMPL-DOC-HTTP-API | N-EC-ENCODING | SC-DEMO-ERROR | POST /api/doc/encoding | C-ENCODING-REJECT | UT-WEB-PATHSAFETY-03 |
| IR-01 Tauri invoke | P-IMPL-DOC-HTTP-API | N-DT-TAURI, N-FE-EDITORHTML | SC-EDITOR | doc_open, doc_undo, doc_find | C-DESKTOP-INVOKE | IT-DESKTOP-CMDS-01 |
| IR-02 HTTP | P-IMPL-DOC-HTTP-API, P-ATLAS-SYNC-DOC | N-WB-SERVER | 全画面 | /api/doc/*, /api/terminal_*, /api/scm_*, /api/search, /api/outline | C-NORMAL-EDIT, C-CONCURRENT-TABS | IT-WEB-API-01 |
| IR-03 JSON-RPC | P-IMPL-RPC-KERNEL | N-PT-RPC, N-PT-KERNEL | — | JSON-RPC 2.0 20 method | C-RPC-NORMAL, C-RPC-UNKNOWN-METHOD, C-RPC-BUFFER-NOT-FOUND | UT-PROTOCOL-RPC-01/02/03 |
| IR-04 AI Provider | P-IMPL-AI-PROVIDER | N-PT-AIPROVIDER | — | complete_at_cursor, edit_via_provider, chat | C-AI-COMPLETE, C-RPC-NORMAL | UT-PROTOCOL-AI-01 |
| CR-03 ルート限定 | P-IMPL-AUTH-TOKEN | N-WB-PATHSAFETY | SC-DEMO-ERROR | POST /api/write_file | C-PATH-ESCAPE, C-COMBINED-BREACH | UT-WEB-PATHSAFETY-04 |

## 2. 要求に紐づくテストが無いもの（欠落＝明示）

| 要件 | 理由 |
|---|---|
| FR-11 自動インデント | UAT の手動確認のみ（EV-DOC-0013）。単体テスト ID が無い |
| FR-12 折り畳み | テスト ID の記載なし |
| FR-13 / FR-14 | 将来拡張・未実装 |
| NFR-01/04/10/13/14 | ベンチ・CI・被覆率・配布サイズ・OS Versions は既存文書の測定値のみで、テスト ID を持たない |
| CR-01/CR-02/CR-04/CR-05 | CR-05 のみ UT-DESKTOP-WIKI-02 が対応 |

> 欠落は「欠落である」と記す。空のテスト ID をPASS扱いしない。

## 3. 逆引き（2D → 1D）

`src/app.js` の `buildPromptIndex()` が nodeId/edgeId → Prompt の逆索引を構成し、
2D でノードを選ぶと「どの Prompt がこのノードを使うか」が提示される。
索引対象は `models/prompts.json` の 8 プロンプト（groupA 5 + groupB 3）。

## 4. 図集自身の追跡

| 層 | ファイル | 被覆 |
|---|---|---|
| 事実 | models/evidence.json | 79 証拠（EV-CODE 52 / EV-DOC 27） |
| 要件 | models/requirements.json | 38 要件 + 5 除外宣言 |
| 構造 | models/architecture.json | 46 ノード / 63 エッジ / 138 ポート / 39 コンテナ |
| 空間 | models/spatial.json | 24 部屋 / 46 オフセット / 12 ビュー |
| 動的 | models/cases.json | 24 ケース |
| 因果 | models/causal-rules.json | 32 規則 / 19 状態機械 / 12 干预 |
| 図 | models/diagram-definitions.json | 17 図（14 図種） |
| 原型 | models/prototype.json | 4 実画面 + 3 Designed 示范 + 12 预置控件 |
| 追跡 | models/traceability.json | 22 マトリクス行 / 30 テスト |
