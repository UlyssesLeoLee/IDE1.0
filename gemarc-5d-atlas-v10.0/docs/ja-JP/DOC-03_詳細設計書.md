# [ID1.0-JP-03] 詳細設計書（DDD 日本語版）

> - 正本: `models/causal-rules.json`（状態機械・遷移規則）/ `models/cases.json`（24 ケース）
> - 事実ソース: `E:\IDE1.0` @ `dev` / `76c4e8f9ff4775119d1c98e6403eecb020a905fe`

## 1. モジュール責務

| モジュール | 責務 | 公開面 | 根拠 |
|---|---|---|---|
| `editcore::buffer` | 論理行/物理行の行ストア、文字単位カーソル | `DocLineMgr`, `DocLine`, `LogicPos` | EV-CODE-0038 |
| `editcore::undo` | 操作バッファ `Ope`/`OpeBlk`/`OpeBuf`（Insert/Delete/Replace/MoveCaret、JSON 可） | `OpeBuf` | EV-CODE-0039 |
| `editcore::history` | タブ単位パッチスタック（`76c4e8f` で導入） | history API | EV-CODE-0040 |
| `editcore::type_config` / `types` | 行ベース行解析器と 19 言語の定義 | `TypeRegistry` | EV-CODE-0041 |
| `editcore::grep` / `grep_engine` | ファイル列挙と内容検索（regex 非依存） | `walk_dir`, `grep_in_files` | EV-CODE-0042 |
| `editcore::search_ops` | 文書内検索/置換演算 | 検索/置換 API | EV-CODE-0043 |
| `editcore::encoding_io` / `document_io` | UTF-8 制約下の入出力 | IO API | EV-CODE-0044 |
| `ide_kernel::document` | 文書モデル（caret/selection/encoding/dirty/履歴） | `Document` | EV-CODE-0012 |
| `ide_kernel::doc_store` | 多文書ストアと編集アクション、`DocView` 投影 | `DocStore` | EV-CODE-0016 |
| `ide_kernel::session` | タブ列・激活・dirty 集合 | `Session` | EV-CODE-0017 |
| `ide_kernel::text_ops` | 純粋関数としての検索/置換 | `find_*`, `replace_*` | EV-CODE-0018 |
| `ide_shell_protocol::Kernel` | headless カーネル（複数 buffer/3 態 mode/undo） | `Kernel` | EV-CODE-0006 |
| `ide_shell_protocol::handle_rpc` | JSON-RPC 2.0 ディスパッチ（20 method） | `handle_rpc` | EV-CODE-0011 |
| `ide_shell_protocol::AiProvider` | AI 拡張点（complete/edit/chat） | trait | EV-CODE-0009 |
| `ide_shell_web::handle_http` | HTTP ディスパッチ（約 70 route） | 内部関数 | EV-CODE-0019 |
| `ide_shell_web::auth` | トークン/Origin/Host 検証 | `Auth` | EV-CODE-0025 |
| `ide_shell_web::path_safety` | ルート内判定・サイズ・エンコード | 検査関数群 | EV-DOC-0014 |
| `ide_shell_desktop::*` | Tauri command 群 | `#[tauri::command]` | EV-CODE-0028 |
| `ide_shell::App` | TUI アプリ状態（mode/buffer/history/quit） | `App` | EV-CODE-0034 |
| `ide_cli::run` | placeholder CLI（3 flag のみ） | `run` | EV-CODE-0002 |
| `ide_kernel_core` | placeholder（`version()`, `kernel_status()`） | 2 関数 | EV-CODE-0001 |

## 2. 状態遷移

図: `DG-UML-STATE-MODE`、`DG-UML-STATE-DOCUMENT`。

### 2.1 編集モード（3 態）

| 状態 | 入口 | 遷移 |
|---|---|---|
| NORMAL | 初期 | `i` → INSERT（`set_mode(Insert)` は `push_undo` を伴う, EV-CODE-0006） |
| INSERT | `i` | `Esc` → NORMAL |
| COMMAND | `:` | `Enter`/`Esc` → NORMAL |
| VISUAL/SELECT | `v` | `Esc` → NORMAL |

### 2.2 文書ライフサイクル

| 状態 | 遷移条件 |
|---|---|
| 未オープン | `open` → オープン済(clean) |
| オープン済(clean) | 編集 → dirty / `close` → 終了 |
| dirty | `mark_saved` / `write_file` → 保存済 |
| 保存済 | 再編集 → dirty / `close` → 終了 |

### 2.3 セッション/バッファ（protocol 側）

`KernelInner` は `buffers: RwLock<HashMap<u32, Buffer>>` と `active: RwLock<u32>` を持つ。
buffer は `undo_stack`（上限 200）を保持し、`set_mode(Insert)` 時にスナップショットを積む（EV-CODE-0006）。

### 2.4 4D/5D モデル上の状態機械

正本: `models/causal-rules.json` の `stateMachines`（19 個）。
各ノードの取りうる状態は Idle / Running / Waiting / Completed / Failed / Recovered / Unknown。
`Failed` と `Completed` は終端、`Recovered` は插件ノードでのみ終端扱い。

> 設計注記: `SM-UNDO` の `Completed` を終端にしていないのは、undo 結果が `Document` へ戻る必要があるため
> （終端ノードは以後の拡散停止となる）。この判断は `models/causal-rules.json` の `SM-UNDO.note` に明記。

## 3. シーケンス

図: `DG-UML-SEQUENCE-OPEN`、`DG-UML-SEQUENCE-TERMINAL`。

### 3.1 ファイルオープン（web モード）

1. `editor.html` → `POST /api/doc/open`（`fetchApi`, EV-CODE-0052）
2. ディスパッチ → `auth::check`（token / Host / Origin）
3. ディスパッチ → `path_safety`（ルート内・4MB・UTF-8）
4. `DocStore::open(path, text)` → `Document::from_text` → `DocLineMgr`
5. `DocView` を返す → JSON 化して前端へ

失敗時: E001（未発見）/ E002（サイズ）/ E003（エンコード）/ E004（パス）が対応する。

### 3.2 端末（desktop）

1. `invoke terminal_create` → `Command::new("powershell.exe")` + `CREATE_NO_WINDOW`
2. `terminal_input` → stdin へ書込
3. reader スレッドが stdout/stderr を回収
4. `terminal_output(id)` で前端へ

実機検証: `Write-Host` / `Get-Date` / `Get-Location` すべて出力確認（EV-DOC-0013）。

## 4. エラー処理

### 4.1 HTTP エラー符号（DOC-04 定義）

| 符号 | 意味 | 発生条件 | 根拠 |
|---|---|---|---|
| E001 | ファイル未発見 | open_file（存在しない） | EV-DOC-0008 |
| E002 | サイズ超過 | 4MB 超 | EV-DOC-0008 |
| E003 | エンコード不正 | UTF-8 以外 | EV-DOC-0008 |
| E004 | パス不正 | プロジェクトルート外 | EV-DOC-0008 |
| E005 | 端末未発見 | 不正な ID | EV-DOC-0008 |
| E006 | 内部エラー | — | EV-DOC-0008 |

### 4.2 JSON-RPC エラー符号

`-32700 / -32600 / -32601 / -32602 / -32603`（標準）、`-32001 BUFFER_NOT_FOUND`、`-32002 READ_ONLY`（独自, EV-CODE-0008）。

### 4.3 4D 上の失敗分類

| エンジンイベント | 意味 |
|---|---|
| `Transition` | 正常遷移 |
| `GuardBlocked` | ガード不成立（例: パス越界、トークン不正） |
| `WhenBlocked` | 前提状態（`when`）不成立 |
| `InterventionBlocked` | 5D 干预により経路が塞がれた |
| `Retry` | 退避付き再試行 |
| `InjectedFailure` | 5D のリソース障害注入 |

正本: `src/engine.js` の `run()`。失敗は色だけでは表現せず、種別イベントとして保持する。

## 5. 冪等性・順序性・トランザクション

| 項目 | 現状 | 根拠 |
|---|---|---|
| 冪等性 | 実装値の記録なし。`register_provider`/`unregister_provider` は名前で台帳を管理 | EV-CODE-0009, EV-CODE-0011 |
| 順序性 | JSON-RPC は request id 付き。イベント順序は (logicalTime, sequence) | EV-CODE-0007 |
| トランザクション | DD-02 が設計する transaction buffer は**未実装**。実装は `OpeBuf` とタブ別パッチスタック | EV-CODE-0039, EV-CODE-0040, EV-DOC-0026 |
| 補償 | 実装の補償経路の記録なし。4D 上では「通常遷移」として記録され、専用の Compensation イベント型は無い | `models/causal-rules.json` の `honestyNotes` |

> 欠落坦率: 5D の `compensation` 欄が常に null になるのは **引擎側のイベント型不足**であり、
> IDE1.0 に補償機構が無いという結論ではない。この区別は `honestyNotes` に明記している。

## 6. リソース制限と性能目標

| 項目 | 値 | 出典 |
|---|---|---|
| バッファ undo 上限 | 200 スナップショット | EV-CODE-0006 |
| ファイルサイズ上限 | 4MB | EV-DOC-0008 |
| 4D 計算予算 | maxEvents 500 / maxSteps 5000 / maxIterations 20000 / maxMillis 2000 | `models/causal-rules.json` |
| bench 目標 | ≥ 50,000 ops/s（記載 56,971） | EV-DOC-0002 |
| 起動 | ≤ 2s（記載 1.5s） | EV-DOC-0002 |

## 7. テストからivable契約

`models/traceability.json` が「要件 → プロンプト → DD ノード/エッジ → 画面 → API → Case → テスト」の
一意の追跡鎖を保持する。図集側の自己テストは:

- `tests/unit/atlas.test.mjs` — 22 件（モデル契約・決定性・予算・因果の真改写）
- `.tooling/e2e-journeys.mjs` — 12 旅程 55 実測
- `.tooling/verify-cases.mjs` — 全 24 Case の実走と被覆率の実測
- `.tooling/verify-refs.mjs` — 参照整合性（悬空 ID があれば即失敗）
