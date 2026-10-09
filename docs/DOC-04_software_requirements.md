# [DOC-04] ソフトウェア要件仕様書 (Software SRS)

> **メタ情報:** DOC-04 / v0.1.0 / 2026-10-09 / ソフトウェア要件分析 (CF2013 §7) / Draft

## 1. 概要

SRS (DOC-02) を **ソフトウェアレベル** で詳細化. モジュール単位の API 契約・エラー処理・データ構造を規定.

## 2. プロセス定義

**プロセス ID:** DOC-04
**プロセス名:** ソフトウェア要件分析
**開始条件:** DOC-02 承認
**終了条件:** 全ソフトウェア要件承認

## 3. アクティビティ

### DOC-04.A モジュール仕様
### DOC-04.B API 契約
### DOC-04.C エラー処理

## 4. タスク

### DOC-04.A.1 sakura-rs (Pure Rust)

| 関数 | 入力 | 出力 | 説明 |
|---|---|---|---|
| `Buffer::new()` | - | Buffer | 空バッファ |
| `Buffer::insert_char(c, pos)` | char, Pos | () | 文字挿入 |
| `Buffer::delete(pos)` | Pos | Option<char> | 削除 |
| `Buffer::undo()` | &mut | () | 元に戻す |
| `Buffer::redo()` | &mut | () | やり直し |
| `Cursor::move_word_fwd()` | &mut Buffer | () | 単語前進 |
| `Cursor::move_word_bwd()` | &mut Buffer | () | 単語後退 |
| `Grep::walk_dir(root)` | &Path | Vec<Match> | ファイル横断 |
| `Grep::grep_in_files()` | &Path, &str | Vec<Match> | ファイル内検索 |
| `TypeConfig::load(path)` | &Path | TypeConfig | 言語別設定 |
| `Outline::dispatch(text)` | &str | Vec<OutlineItem> | アウトライン抽出 |

### DOC-04.A.2 ide-shell-protocol (JSON-RPC)

**22 メソッド (全カテゴリ):**

| カテゴリ | メソッド |
|---|---|
| 接続 | `connect`, `disconnect`, `ping` |
| エディタ | `open_file`, `close_file`, `get_buffer`, `set_buffer` |
| カーソル | `cursor_move`, `cursor_set` |
| 編集 | `insert_text`, `delete_range` |
| Undo | `undo`, `redo` |
| 検索 | `search`, `replace`, `grep` |
| ファイル | `list_dir`, `read_file`, `write_file` |
| ターミナル | `terminal_create`, `terminal_input`, `terminal_output` |
| AI | `ai_completion`, `ai_action` |

### DOC-04.A.3 ide-shell-web (HTTP)

**13 endpoints:**
- `GET /healthz` — ヘルスチェック
- `GET /info` — バージョン情報
- `GET /editor` — エディタ HTML
- `GET /api/files` — ファイル一覧
- `GET /api/file` — ファイル読み込み
- `POST /api/file` — ファイル書き込み
- `GET /api/search` — ファイル内検索
- `POST /api/grep` — 横断検索
- `POST /api/outline` — アウトライン
- `POST /api/terminal_create` — ターミナル作成
- `POST /api/terminal_input` — ターミナル入力
- `GET /api/terminal_output` — ターミナル出力 (query `?id=`)
- `GET /api/wiki` — ドキュメント

### DOC-04.A.4 ide-shell-desktop (Tauri)

| モジュール | 役割 |
|---|---|
| `lib.rs` | メインロジック + wiki テスト |
| `wiki.rs` | 日本語 Wiki テキスト |
| `terminal.rs` | PowerShell 起動 + I/O |
| `build.rs` | Tauri codegen + rerun-if-changed |

### DOC-04.B.1 API 契約の原則
- 全 API は UTF-8 のみ受付
- ファイルパスは絶対パス (verbatim `\\?\` 剥除)
- ファイルサイズ ≤ 4 MB
- プロジェクトルート外はアクセス拒否
- エラーは `Result<T, E>` または HTTP 500 + JSON エラー

### DOC-04.C.1 エラーコード

| コード | 意味 | 発生条件 |
|---|---|---|
| `E001` | ファイル未発見 | open_file (存在しない) |
| `E002` | サイズ超過 | 4MB 越え |
| `E003` | エンコード不正 | UTF-8 以外 |
| `E004` | パス不正 | プロジェクトルート外 |
| `E005` | ターミナル未発見 | 不正な ID |
| `E006` | 内部エラー | - |

## 5. 注記

### DOC-04.A.2.i ide-shell-protocol Python デモ
`crates/ide-shell-protocol/examples/python_client_demo.py` で AI agent 統合例を実装.

## 6. 関連ドキュメント

- [DOC-02 SRS](DOC-02_system_requirements.md) — 上位要件
- [DOC-05 ソフトウェア SDD](DOC-05_software_design.md) — 設計
- [DOC-06 詳細設計](DOC-06_software_detail_design.md) — 実装
- [PARITY.md](../crates/sakura-rs/PARITY.md) — 機能カバー率

**関連実装:**
- `crates/sakura-rs/src/lib.rs` (Buffer/Cursor API)
- `crates/ide-shell-protocol/src/lib.rs` (JSON-RPC)
- `crates/ide-shell-web/src/lib.rs` (HTTP 13 endpoints)
- `crates/ide-shell-desktop/src/lib.rs` (Tauri commands)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |