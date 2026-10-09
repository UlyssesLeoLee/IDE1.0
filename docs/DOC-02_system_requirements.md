# [DOC-02] システム要件定義書 (SRS)

> **メタ情報:**
> - **ドキュメント ID:** DOC-02
> - **バージョン:** v0.1.0
> - **作成日:** 2026-10-09
> - **担当プロセス:** 要件定義プロセス (CF2013 §7)
> - **準拠標準:** IPA 共通フレーム2013 §6 (4階層構造)
> - **ステータス:** Draft

## 1. 概要

IDE1.0 IDE Shell の **システムレベル要件** を定義する. 機能要件 (FR) + 非機能要件 (NFR) + インターフェース要件 (IR) + 制約 (CR) の 4 種類で構成.

## 2. プロセス定義

**プロセス ID:** DOC-02
**プロセス名:** 要件定義プロセス
**目的:** システム全体の要件を確定し, 下流 (設計・実装) へのインプット提供
**開始条件:** DOC-01 Charter 承認
**終了条件:** 全要件のレビュー完了と承認

## 3. アクティビティ

### DOC-02.A 機能要件定義
- ユーザー機能
- 編集機能
- ファイル管理
- 検索・置換
- Grep
- アウトライン
- ターミナル

### DOC-02.B 非機能要件定義
- 性能
- 可用性
- セキュリティ
- 保守性
- 移植性
- i18n

## 4. タスク

### DOC-02.A.1 機能要件 (Functional Requirements)

| ID | 名称 | 説明 | 実装 | 状態 |
|---|---|---|---|---|
| FR-01 | ファイルオープン | ネイティブダイアログで選択 | `doOpenFolder` | ✅ |
| FR-02 | ファイルツリー | プロジェクト階層表示 | `renderTree()` | ✅ |
| FR-03 | タブ切替 | 複数ファイル同時編集 | `switchTab()` | ✅ |
| FR-04 | 編集 (NORMAL/INSERT/VISUAL) | 3 態モード | sakura-rs | ✅ |
| FR-05 | Undo/Redo | 任意回数 | sakura-rs | ✅ |
| FR-06 | 検索/置換 | 文字列ベース | sakura-rs | ✅ |
| FR-07 | Grep | 複数ファイル横断 | sakura-rs | ✅ |
| FR-08 | アウトライン | 関数一覧 | sakura-rs | ✅ |
| FR-09 | ターミナル | PowerShell 実行 | `terminal.rs` | ✅ |
| FR-10 | 19 言語高亮 | rust/python/js/... | `syntax.js` | ✅ |
| FR-11 | 自動インデント | Tab → 2 スペース | editor.html | ✅ |
| FR-12 | 折り畳み | zc コマンド | sakura-rs | ✅ |
| FR-13 | ミニマップ | (未実装) | - | ❌ |
| FR-14 | 矩形選択 | (部分実装) | - | 🟡 |
| FR-15 | 自動バックアップ | (未実装) | - | ❌ |

### DOC-02.A.2 検索/置換の具体的仕様
- 検索方向: 前方/後方
- 検索モード: 大文字小文字区別/区別なし
- 検索履歴: 直近 10 件
- 置換: 単発/全件/確認
- 正規表現: (将来, 計画)

### DOC-02.A.3 ターミナル仕様
- シェル: `powershell.exe` (Windows 標準)
- 起動: CREATE_NO_WINDOW フラグ
- I/O: stdin pipe + stdout/stderr thread reader
- 文字エンコード: UTF-8
- 終了: セッション ID 管理 (現状 1 セッション)

### DOC-02.B.1 非機能要件 (Non-Functional Requirements)

| ID | カテゴリ | 要件 | 値 | 状態 |
|---|---|---|---|---|
| NFR-01 | 性能 | sakura-rs bench | ≥ 50,000 ops/s | 56,971 ✅ |
| NFR-02 | 性能 | ファイル読み込み 4MB | ≤ 100ms | ✅ |
| NFR-03 | 性能 | 起動時間 (desktop) | ≤ 2 秒 | ✅ |
| NFR-04 | 可用性 | CI 成功率 | ≥ 88% | 8/9 (88.9%) |
| NFR-05 | セキュリティ | ファイルアクセス | プロジェクトルートのみ | ✅ path_safety |
| NFR-06 | セキュリティ | ファイルサイズ上限 | 4 MB | ✅ |
| NFR-07 | セキュリティ | 文字エンコード | UTF-8 のみ | ✅ |
| NFR-08 | 保守性 | RUSTFLAGS | -D warnings | ✅ |
| NFR-09 | 保守性 | テスト数 | ≥ 200 | 253 ✅ |
| NFR-10 | 保守性 | カバレッジ | ≥ 60% | 66.25% ✅ |
| NFR-11 | 移植性 | Web 版 | 同一機能 | ✅ |
| NFR-12 | i18n | 中国語/英語 | 切替 | ✅ |
| NFR-13 | 配布 | MSI サイズ | ≤ 5 MB | 2.88 MiB ✅ |
| NFR-14 | 配布 | 起動 | Windows 10+ | ✅ |

### DOC-02.C インターフェース要件 (Interface Requirements)

| ID | 名前 | プロトコル | 用途 |
|---|---|---|---|
| IR-01 | 内部 IPC | Tauri invoke | desktop ↔ webview |
| IR-02 | HTTP API | REST (13 endpoint) | web mode |
| IR-03 | JSON-RPC | stdio/HTTP/WS | ide-shell-protocol |
| IR-04 | AI Provider | trait | 拡張性 |

### DOC-02.D 制約 (Constraints)

| ID | 制約 |
|---|---|
| CR-01 | フロントエンド: vanilla JS, 0 フレームワーク |
| CR-02 | sakura-rs: 0 重型依存 (no tree-sitter WASM, no regex crate) |
| CR-03 | ファイル読み書き: プロジェクトルートのみ |
| CR-04 | 文字エンコード: UTF-8 のみ |
| CR-05 | 命名: vim/Cursor/VSCode 禁止 |

## 5. 注記

### DOC-02.A.3.i PowerShell 実測結果
```
> Write-Host 'Hello from IDE1.0'
Hello from IDE1.0
> Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
2026-10-09 22:30:12
> Get-Location
D:\orcaWork\IDE1.0\dev-3\tests\uat\fixtures\sample-project
```

## 6. 関連ドキュメント

- [DOC-01 企画書](DOC-01_project_charter.md) — 上位プロセス
- [DOC-03 システム HLD](DOC-03_system_hld.md) — 下流設計
- [DOC-04 ソフトウェア要件](DOC-04_software_requirements.md) — 詳細化
- [PARITY.md](../crates/sakura-rs/PARITY.md) — sakura 機能対比

**関連実装:**
- `crates/ide-shell-web/src/editor.html` (FR-01 ~ FR-12)
- `crates/ide-shell-protocol/src/lib.rs` (IR-03)
- `crates/ide-shell-web/src/path_safety.rs` (NFR-05/06/07)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 (Stage 6 完了時点) |