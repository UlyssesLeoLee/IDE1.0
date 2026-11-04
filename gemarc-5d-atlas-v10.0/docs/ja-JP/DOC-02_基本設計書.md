# [ID1.0-JP-02] 基本設計書（SDD 日本語版）

> - 正本: `models/architecture.json` / `models/spatial.json` / `models/diagram-definitions.json`
> - 事実ソース: `E:\IDE1.0` @ `dev` / `76c4e8f9ff4775119d1c98e6403eecb020a905fe`

## 1. 全体アーキテクチャ

```
┌───────────────────────────────────────────────────────────────┐
│ Frontend Layer (vanilla JS, CR-01)                            │
│  editor.html = Activity Bar / Side Bar / Editor /             │
│                Bottom Panel / Status Bar                      │
└───────────────┬───────────────────────────┬───────────────────┘
                │ Tauri invoke              │ HTTP fetch (127.0.0.1:8123)
┌───────────────▼──────────────┐  ┌─────────▼────────────────────┐
│ ide-shell-desktop (Tauri 2)  │  │ ide-shell-web (手書き HTTP)  │
│  terminal.rs / scm.rs /      │  │  auth.rs / path_safety.rs   │
│  search.rs / plugin.rs /     │  │  doc_store 接続             │
│  outline.rs                  │  └─────────┬────────────────────┘
└───────────────┬──────────────┘            │
                │        ┌──────────────────┴──────────────────┐
                │        │ ide-shell-protocol                   │
                │        │  Kernel(多 buffer/3 態 mode)          │
                │        │  handle_rpc(JSON-RPC 2.0, 20 method) │
                │        │  AiProvider trait                    │
                │        └──────────────────┬──────────────────┘
                │                           │
                │        ┌──────────────────▼──────────────────┐
                │        │ ide-kernel                          │
                │        │  Document / DocStore / Session /    │
                │        │  text_ops                           │
                │        └──────────────────┬──────────────────┘
                │                           │
                │        ┌──────────────────▼──────────────────┐
                └───────►│ editcore (40 modules, 0 重型依存)     │
                         │  buffer / undo / history / type_config│
                         │  grep / search_ops / encoding_io ...  │
                         └──────────────────────────────────────┘
```

正本: `models/architecture.json`（46 ノード / 63 エッジ / 138 ポート）。
図: `DG-UML-COMPONENT-CRATES`（8 クレート）、`DG-UML-PACKAGE-EDITCORE`（40 モジュール）。

### 1.1 4 つの通信チャネル

| チャネル | 実装 | 用途 | 根拠 |
|---|---|---|---|
| Tauri invoke | `#[tauri::command]` | desktop ↔ webview | EV-CODE-0028 |
| HTTP (手書き) | `("METHOD","/api/...")` match | web モード | EV-CODE-0019 |
| JSON-RPC 2.0 | `handle_rpc` | AI エージェント統合 | EV-CODE-0010 |
| プロセスパイプ | `Command::new("powershell.exe")` 等 | 端末 / git / rg | EV-CODE-0029, EV-CODE-0030, EV-CODE-0031 |

## 2. コンポーネント構成

正本: `models/architecture.json` の `nodes` / `edges`。

| 層 | ノード | 責務 | 状態 |
|---|---|---|---|
| Frontend | `N-FE-EDITORHTML` | 外殻描画とイベント送出 | Implemented |
| Frontend | `N-FE-CLIENT2` | 第 2 クライアント（並行ケース用推論） | Inferred |
| Web | `N-WB-SERVER` | HTTP 分岐ディスパッチ | Implemented |
| Web | `N-WB-AUTH` | CSPRNG トークン + Host/Origin 検証 | Implemented |
| Web | `N-WB-PATHSAFETY` | ルート内判定 / 4MB / UTF-8 | Implemented |
| Web | `N-WB-TERMINAL` | HTTP ポーリング端末 | Implemented |
| Desktop | `N-DT-TAURI` | invoke 登録表 | Implemented |
| Desktop | `N-DT-TERMINAL` | powershell.exe spawn | Implemented |
| Desktop | `N-DT-SCM` | git porcelain 封装 | Implemented |
| Desktop | `N-DT-SEARCH` | rg 優先 + 内蔵フォールバック | Implemented |
| Desktop | `N-DT-PLUGIN` | wasm プラグイン実行 | Implemented |
| Desktop | `N-DT-OUTLINE` | アウトライン抽出 | Implemented |
| Protocol | `N-PT-KERNEL` / `N-PT-RPC` / `N-PT-AIPROVIDER` | headless カーネルと 20 メソッド | Implemented / **Designed(AI)** |
| Kernel | `N-KN-DOCUMENT` / `N-KN-DOCSTORE` / `N-KN-SESSION` / `N-KN-TEXTOPS` | 文書・セッション状態 | Implemented |
| editcore | 22 ノード | 編集内核（buffer/undo/history/lexer/search/io） | Implemented |
| Placeholder | `N-CLI` / `N-KERNELCORE` | 骨格のみ | Implemented(骨架) |
| External | `N-OS-PWSH` / `N-OS-FS` / `N-OS-WEBVIEW2` / `N-EXT-GIT` / `N-EXT-RG` / `N-EXT-ACI` | 外部プロセス・ランタイム | Implemented |

## 3. 実行形態

| 形態 | 成果物 | サイズ | 起動 | 根拠 |
|---|---|---|---|---|
| MSI インストーラ | `IDE1.0_0.1.9_x64_en-US.msi` | 2.88 MiB | `ide-shell-desktop.exe` | EV-DOC-0018 |
| Web 単体 | `ide-shell-web.exe` | 846 KB | `127.0.0.1:8123` | EV-DOC-0018 |
| ソース | 8 crates | — | `cargo build --release` | EV-CODE-0050 |

図: `DG-UML-DEPLOYMENT-RUNTIME`。

### 3.1 環境要件

Windows 10 1909+ / WebView2 runtime 100.0+ / PowerShell 5.1+ / メモリ 4GB+ / ディスク 50MB+（EV-DOC-0019）。

## 4. データモデル

**IDE1.0 に関係型データベースは存在しない。** 永続状態はディスク上のファイルと、
メモリ上の Document / DocStore / Session である（`models/architecture.json` の `semantics.database`）。

| 構造 | 実体 | 鍵 | 関係 | 根拠 |
|---|---|---|---|---|
| Session | `ide_kernel::session::Session` | active tab (u64) | 1 : 0..N TabState | EV-CODE-0017 |
| DocStore | `ide_kernel::doc_store::DocStore` | doc id | 1 : 0..N DocView | EV-CODE-0016 |
| Document | `ide_kernel::document::Document` | — | 行集合 + caret/selection + encoding | EV-CODE-0012 |
| DocView | 投影 | id | フロント/Tauri へ JSON 投影 | EV-CODE-0015 |
| OpeBuf | `editcore::undo::OpeBuf` | — | Insert/Delete/Replace/MoveCaret の JSON | EV-CODE-0039 |
| 履歴 | `editcore::history` | タブ別パッチスタック | HEAD `76c4e8f` で導入 | EV-CODE-0040 |
| ファイル | ディスク | path | ≤4MB / UTF-8 のみ | EV-CODE-0024 |

図: `DG-ER-SESSION`（内存＋ディスクの関係を表現。業務テーブルではない）、
`DG-DATAFLOW-EDIT`（編集 → OpeBuf → タブ別パッチスタック → 保存）。

### 4.1 設計上の不可変条件

- 文字数ベースのカーソル位置（バイト位置ではない）: EV-CODE-0014
- `LineEnding` / `Encoding` は列挙型で検出可能: EV-CODE-0013
- undo スタック上限 200（protocol 側 Buffer）: EV-CODE-0006

## 5. API 概要

### 5.1 HTTP（web モード, `ide-shell-web/src/lib.rs`）

| 系統 | 代表ルート | 状態 |
|---|---|---|
| 文書 | `/api/doc/open|tabs|active|activate|close|new_tab|insert|type|newline|backspace|delete|delete_range|caret|select_all|clear_selection|undo|redo|add_cursor|clear_cursors|insert_all|find|find_all|replace_all|line_ending|encoding|export|mark_saved` | Implemented（EV-CODE-0020） |
| 端末 | `/api/terminal_create|input|close|output|list` | Implemented（EV-CODE-0021） |
| 版管理 | `/api/scm_status|stage|unstage|diff|commit|log` | Implemented（EV-CODE-0022） |
| 検索 | `/api/search` | Implemented（EV-CODE-0031） |
|  Outline | `/api/outline` | Implemented（EV-CODE-0033） |
| プラグイン | `/api/plugin_load` | Implemented（EV-CODE-0032） |
| フレーム/入力 | `/api/frame|event|key|mouse|reset` | Implemented（EV-CODE-0023） |
| ファイル | `/api/open_project|list_dir|read_file|write_file|pick_folder` | Implemented（EV-CODE-0024） |

### 5.2 JSON-RPC 2.0（`ide-shell-protocol`）

実装済み 20 メソッド: `kernel_info` `open_buffer` `close_buffer` `list_buffers` `set_active_buffer`
`active_buffer_id` `get_buffer` `set_buffer_lines` `insert_at_cursor` `key_event` `set_cursor`
`execute_command` `set_cmd_text` `get_cmd_text` `complete_at_cursor` `edit_via_provider` `chat`
`register_provider` `unregister_provider` `list_providers`（EV-CODE-0011）。

エラー符号: `-32700/-32600/-32601/-32602/-32603`（JSON-RPC 標準）、
`-32001 BUFFER_NOT_FOUND` / `-32002 READ_ONLY`（独自, EV-CODE-0008）。

### 5.3 Tauri command（desktop）

`doc_*` 27 種、`terminal_*` 4 種、`scm_*` 5 種、`search_cmd` `outline` `plugin_load`
`desktop_luminance` `clear_theme_vars`（EV-CODE-0028, EV-CODE-0029, EV-CODE-0030,
EV-CODE-0031, EV-CODE-0032, EV-CODE-0033）。

### 5.4 ACI イベント

`ide-cli --aci-emit [ID]` が `aci-emitter`（git rev `df28c56`）で構造化 assertion を出力（EV-CODE-0003）。

## 6. セキュリティ設計

| 脅威 | 制御 | 根拠 |
|---|---|---|
| 任意 Web ページからの localhost アクセス | 起動毎の CSPRNG トークン + `x-ide-token` ヘッダ | EV-CODE-0025, EV-CODE-0027 |
| Host / Origin 偽装 | Host ルールと Origin 検証 | EV-CODE-0026 |
| プロジェクト外ファイル読み取り | `path_safety`（ verbatim 接頭辞除去 / 8.3 短縮パス展開 / ルート内判定） | EV-DOC-0014 |
| 巨大ファイルによる DoS | 4MB 上限（E002） | EV-DOC-0008 |
| 非 UTF-8 混入 | UTF-8 限定（E003） | EV-CODE-0044 |
| 任意コード実行 | wasm プラグイン（`plugin_load`） | EV-CODE-0032（署名検証の記録は無し＝既知の残余リスク） |

図: `DG-SECURITY-BOUNDARY`（境界・資産・攻撃ベクタ・制御）、`DG-NETWORK-LOCAL`（ループバックのみ）。
