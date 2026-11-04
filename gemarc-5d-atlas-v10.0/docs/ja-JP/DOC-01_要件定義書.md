# [ID1.0-JP-01] 要件定義書（SRS 日本語版）

> - 対象: IDE1.0 IDE Shell（純 Rust AI Native CLI Development Kernel）
> - 事実ソース: `E:\IDE1.0` @ `dev` / `76c4e8f9ff4775119d1c98e6403eecb020a905fe`
> - 正本: `models/requirements.json` / `models/evidence.json`（本書は正本の日本語投影）
> - 準拠: IPA 共通フレーム2013 相当の章構成を**参照**（公式認証ではない）

## 1. システム概要

IDE1.0 は Windows 10 1909+ を対象とした、純 Rust 実装の AI ネイティブな開発カーネルである。
同一の Rust コアを **デスクトップ（Tauri 2 + WebView2）** と **Web（localhost HTTP）** の 2 つの外観で提供し、
外部製品名を製品名・識別子・文書に用いない命名規約（ADR-006）を課している。

構成クレートは 8 個（`Cargo.toml`）: `ide-kernel-core` / `ide-kernel` / `ide-cli` / `ide-shell` /
`ide-shell-web` / `ide-shell-desktop` / `ide-shell-protocol` / `editcore`。

## 2. 業務要件

| ID | 要件 | 実装の根拠 |
|---|---|---|
| BR-01 | 開発者が複数ファイルを同時編集し、タグ切替・保存・Undo/Redo を連続して行う | EV-CODE-0016 / EV-CODE-0017 |
| BR-02 | 開発者がプロジェクト全体を検索し、編集対象を特定できる | EV-CODE-0042 / EV-CODE-0031 |
| BR-03 | 開発者が.PowerShell を IDE 内から実行し、結果を即時確認する | EV-CODE-0029 / EV-DOC-0013 |
| BR-04 | 開発者が変更内容を確認して版管理的操作を行う | EV-CODE-0030 |
| BR-05 | AI エージェントが IDE のバッファ/カーソル/モードを外部から操作できる | EV-CODE-0009 / EV-CODE-0011 |

## 3. 機能要件

正本: `models/requirements.json`。ID は `docs/DOC-02_system_requirements.md` の記載と一致する。

| ID | 名称 | 実装 | 状態 | 根拠 |
|---|---|---|---|---|
| FR-01 | ファイルオープン | `doOpenFolder` / `POST /api/doc/open` | Implemented | EV-CODE-0020 |
| FR-02 | ファイルツリー | `renderTree()` / `POST /api/list_dir` | Implemented | EV-CODE-0024 |
| FR-03 | タブ切替 | `switchTab()` / `DocStore` | Implemented | EV-CODE-0016 |
| FR-04 | 3 態編集 (NORMAL/INSERT/VISUAL) | `Mode` enum + `map_key` | Implemented | EV-CODE-0006 |
| FR-05 | Undo/Redo | `editcore::undo` / `editcore::history` | Implemented | EV-CODE-0039 |
| FR-06 | 検索/置換 | `ide-kernel::text_ops` / `editcore::search_ops` | Implemented | EV-CODE-0018 |
| FR-07 | Grep | `editcore::grep` + `grep_engine` | Implemented | EV-CODE-0042 |
| FR-08 | アウトライン | `outline.rs` | Implemented | EV-CODE-0033 |
| FR-09 | ターミナル (PowerShell) | `terminal.rs` | Implemented | EV-CODE-0029 |
| FR-10 | 19 言語ハイライト | `type_config.rs` + `types.rs` | Implemented | EV-CODE-0041 |
| FR-11 | 自動インデント | editor.html | Implemented | EV-CODE-0052 |
| FR-12 | 折り畳み | editcore | Implemented | EV-CODE-0005 |
| FR-13 | ミニマップ | — | 将来拡張（実装なし） | EV-DOC-0001 |
| FR-14 | 矩形選択 | — | 将来拡張（実装なし） | EV-DOC-0001 |
| FR-15 | 自動バックアップ (5 分/10 世代) | `auto_backup.rs` | Implemented | EV-CODE-0045 |

## 4. 非機能要件

値は `docs/DOC-02_system_requirements.md` および `docs/DOC-10_unit_test_spec.md` の**記載値**であり、
本図集のビルドで Rust ワークスペースを再測定したものではない（`docs/ja-JP/CHAPTER_COVERAGE.yaml` の TBD 参照）。

| ID | カテゴリ | 要件 | 値 | 状態 |
|---|---|---|---|---|
| NFR-01 | 性能 | editcore bench | ≥ 50,000 ops/s（記載 56,971） | Implemented |
| NFR-02 | 性能 | 4MB ファイル読込 | ≤ 100ms | Implemented |
| NFR-03 | 性能 | desktop 起動 | ≤ 2s（記載 1.5s） | Implemented |
| NFR-04 | 可用性 | CI 成功率 | ≥ 88%（記載 88.9%） | Implemented |
| NFR-05 | セキュリティ | ファイルアクセス | プロジェクトルート内のみ | Implemented |
| NFR-06 | セキュリティ | ファイルサイズ上限 | 4MB | Implemented |
| NFR-07 | セキュリティ | 文字エンコード | UTF-8 のみ | Implemented |
| NFR-08 | 保守性 | RUSTFLAGS | `-D warnings`、警告 0 | Implemented |
| NFR-09 | 保守性 | テスト数 | ≥ 200（記載 253） | Implemented |
| NFR-10 | 保守性 | カバレッジ | ≥ 60%（記載 66.25%） | Implemented |
| NFR-11 | 移植性 | Web 版 | 同一機能 | Implemented |
| NFR-12 | i18n | 中国語/英語 | 切替可 | Implemented |
| NFR-13 | 配布 | MSI サイズ | ≤ 5MB（記載 2.88MiB） | Implemented |
| NFR-14 | 配布 | OS | Windows 10+ | Implemented |

### 4.1 品質の実測値に関する注意

`docs/DOC-10` は editcore カバレッジを 65%（目標 80%）と記録し「要改善」と明記している。
NFR-10 の 66.25% とは対象範囲が異なる。**両方を併記し、片方を他方の根拠として用いない。**

## 5. インターフェース要件

| ID | 名前 | プロトコル | 用途 | 実装の実体 |
|---|---|---|---|---|
| IR-01 | 内部 IPC | Tauri invoke | desktop ↔ webview | `#[tauri::command]` 30+ 件（EV-CODE-0028） |
| IR-02 | HTTP API | REST 風 | web モード | 手書き match 分岐、約 70 ルート（EV-CODE-0020） |
| IR-03 | JSON-RPC | stdio / HTTP / WS | ide-shell-protocol | JSON-RPC 2.0、**実装済み 20 メソッド**（EV-CODE-0011） |
| IR-04 | AI Provider | trait | 拡張性 | `AiProvider`（complete/edit/chat）。**実装は PlaceholderAi のみ**（EV-CODE-0009, EV-CODE-0036） |

### 5.1 文書と実装のドリフト（重要）

| 項目 | 文書の記述 | 実装の事実 | 出典 |
|---|---|---|---|
| JSON-RPC メソッド | 22 メソッド（connect/disconnect/ping/open_file/...） | 別名の 20 メソッド（kernel_info/open_buffer/...） | EV-DOC-0006 / EV-CODE-0011 |
| HTTP エンドポイント | 13 件（/healthz, /info, /editor, /api/files ...） | 約 70 件（/api/doc/*, /api/terminal_*, /api/scm_*, /api/search, /api/outline, /api/key, /api/mouse ...） | EV-DOC-0007 / EV-CODE-0020 |
| HTTP サーバ実装 | Axum | 手書き (`METHOD, path)` match) — Cargo.toml に axum 依存なし | EV-DOC-0020 / EV-CODE-0019, EV-CODE-0027 |
| クレート数 | 9 | 8（aci-emitter は外部 git 依存） | EV-DOC-0020 / EV-CODE-0050 |

本図集は**実装を正本**として扱う。文書側の陳腐化は `models/evidence.json` の `gaps` に
`GAP-SRC-API-DRIFT` / `GAP-SRC-AXUM-MISMATCH` として登録済み。

## 6. 制約

| ID | 制約 | 実装の担保 |
|---|---|---|
| CR-01 | フロントエンドは vanilla JS、フレームワーク禁止 | editor.html（EV-CODE-0052） |
| CR-02 | editcore は 0 重型依存（tree-sitter WASM / regex crate 禁止） | 自作行ベーススキャナ（EV-CODE-0004, EV-CODE-0041） |
| CR-03 | ファイル入出力をプロジェクトルート内に限定 | `path_safety`（EV-DOC-0014） |
| CR-04 | UTF-8 のみ | `encoding_io`（EV-CODE-0044） |
| CR-05 | 外部エディタ/ツールの製品名を名称に使用しない | ADR-006、UT `test_wiki_no_vim_cursor_vscode`（EV-DOC-0010） |

## 7. 未確定事項（Gap）

| ID | 種別 | 内容 | 担当 |
|---|---|---|---|
| GAP-SRC-PLACEHOLDER | GAP_SOURCE_DOCUMENT | `ide-kernel-core` / `ide-cli` は placeholder。DD-01/02/04 の能力は未実装 | IDE1.0 開発 |
| GAP-BIZ-AI-PROVIDER | GAP_BUSINESS_DECISION | AI Provider の実供应商・課金・プライバシー方針が未決 | プロダクト責任者 |
| GAP-BIZ-TXN-BUFFER | GAP_BUSINESS_DECISION | トランザクション境界と競合解決方針が未決 | IDE1.0 設計 |
