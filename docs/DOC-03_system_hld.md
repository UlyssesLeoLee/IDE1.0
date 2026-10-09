# [DOC-03] システム方式設計書 (HLD)

> **メタ情報:**
> - **ドキュメント ID:** DOC-03
> - **バージョン:** v0.1.0
> - **作成日:** 2026-10-09
> - **担当プロセス:** システム方式設計 (CF2013 §7)
> - **準拠標準:** IPA 共通フレーム2013 §6
> - **ステータス:** Draft

## 1. 概要

システム全体のアーキテクチャ・コンポーネント構成・デプロイメント・外部インターフェースを定義する.

## 2. プロセス定義

**プロセス ID:** DOC-03
**プロセス名:** システム方式設計
**目的:** システム全体の方式 (アーキテクチャ) を確定
**開始条件:** DOC-02 SRS 承認
**終了条件:** HLD レビュー完了

## 3. アクティビティ

### DOC-03.A アーキテクチャ設計
- レイヤ構成
- コンポーネント分割
- データフロー

### DOC-03.B デプロイメント設計
- 配布形態 (MSI / Web)
- 環境要件
- インストール手順

## 4. タスク

### DOC-03.A.1 システムアーキテクチャ

```
┌─────────────────────────────────────────────────────────┐
│                IDE1.0 IDE Shell システム                  │
├─────────────────────────────────────────────────────────┤
│  Frontend Layer (vanilla JS)                            │
│  ┌──────────────────────────────────────────────────┐  │
│  │ editor.html                                       │  │
│  │ - Activity Bar (4 panels)                         │  │
│  │ - Side Bar (Explorer/Outline/SCM/Extensions)      │  │
│  │ - Editor (Tabs + Gutter + Content)                │  │
│  │ - Bottom Panel (Terminal/Output/Problems)         │  │
│  │ - Status Bar (Mode/Pos/File/Tip/Lang)             │  │
│  └──────────────────────────────────────────────────┘  │
│           ↕ Tauri invoke / HTTP / WebSocket             │
├─────────────────────────────────────────────────────────┤
│  Backend Layer (Rust)                                    │
│  ┌─────────────┬─────────────┬─────────────┬──────────┐ │
│  │ ide-shell-  │ ide-shell-  │ ide-shell-  │  sakura- │ │
│  │ desktop     │ web         │ protocol    │  rs      │ │
│  │ (Tauri 2)   │ (Axum)      │ (3 proto)   │ (crate)  │ │
│  └─────────────┴─────────────┴─────────────┴──────────┘ │
│           ↕                                              │
│  ┌──────────────────────────────────────────────────┐  │
│  │ OS Layer (Windows 10+)                            │  │
│  │ - powershell.exe                                  │  │
│  │ - file system                                     │  │
│  │ - WebView2 runtime                                │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### DOC-03.A.2 Crate 構成 (9 個)

| Crate | 役割 | 行数 (約) |
|---|---|---|
| `ide-cli` | 4 モジュール CLI | 1,200 |
| `ide-kernel-core` | コア層 (4 module) | 800 |
| `ide-shell` | シェル (cluster_switch 等) | 600 |
| `ide-shell-desktop` | Tauri 2 デスクトップ | 3,500 |
| `ide-shell-web` | Axum ウェブエディタ | 3,000 |
| `ide-shell-protocol` | JSON-RPC 22 メソッド | 2,500 |
| `sakura-rs` | sakura editor 重写 | 1,800 |
| `aci-emitter` | 構造化イベント出力 | 200 |
| **合計** | | **13,600** |

### DOC-03.A.3 コンポーネント関係

```
ide-shell-desktop (exe)
  ├─ tauri 2.x
  │   ├─ frontend: dist/index.html
  │   └─ webview2 (Windows runtime)
  ├─ terminal.rs (PowerShell 起動)
  ├─ lib.rs (wiki テスト含む)
  └─ wiki.rs (IPA wiki text)

ide-shell-web (exe)
  ├─ axum (HTTP server)
  │   └─ 13 endpoints (/api/* + /editor)
  ├─ terminal.rs (HTTP polling)
  └─ editor.html (HTML+JS 同梱)
```

### DOC-03.A.4 デプロイメント構成

| 形式 | パッケージ | サイズ | 起動 | 配布 |
|---|---|---|---|---|
| **MSI installer** | `IDE1.0_0.1.9_x64_en-US.msi` | 2.88 MiB | `ide-shell-desktop.exe` | GitHub Releases |
| **Web 単体** | `ide-shell-web.exe` | 846 KB | `127.0.0.1:8123` | Cargo / Self-contained |
| **ソース** | 9 crates + Cargo.toml | 13,600 行 | `cargo build --release` | GitHub main |

### DOC-03.A.5 環境要件

| 項目 | 最小 | 推奨 |
|---|---|---|
| OS | Windows 10 1909 | Windows 11 |
| WebView2 runtime | 100.0+ | 120.0+ |
| PowerShell | 5.1 | 7.x |
| メモリ | 4 GB | 8 GB |
| ディスク | 50 MB | 100 MB |

### DOC-03.A.6 起動シーケンス (desktop)

```
[1] ide-shell-desktop.exe 起動
    ↓
[2] Tauri Builder 起動 → webview2 プロセス生成
    ↓
[3] dist/index.html ロード
    ↓
[4] JS init() → Backend 接続 (Tauri invoke)
    ↓
[5] Activity Bar / Side Bar / Editor 描画
    ↓
[6] Welcome page 表示
    ↓
[7] ユーザー操作待ち
```

### DOC-03.A.7 起動シーケンス (web)

```
[1] ide-shell-web.exe 起動
    ↓
[2] axum HTTP サーバー起動 (127.0.0.1:8123)
    ↓
[3] GET /editor → editor.html 返却
    ↓
[4] GET /api/terminal_create → terminal session ID
    ↓
[5] ユーザー操作待ち
```

## 5. 注記

### DOC-03.A.4.i MSI と Web の差
- **MSI:** フル機能 + ネイティブダイアログ + 起動アイコン
- **Web:** 軽量 + ブラウザベース + PowerShell ローカル spawn

### DOC-03.A.6.i Tauri codegen キャッシュ
`build.rs` で `cargo:rerun-if-changed=dist/index.html` を emit することで
HTML 変更時に再ビルドが走る. バージョン番号を conf.json で bump する対策も併用.

## 6. 関連ドキュメント

- [DOC-02 SRS](DOC-02_system_requirements.md) — 上位要件
- [DOC-04 ソフトウェア要件](DOC-04_software_requirements.md) — 詳細化
- [DOC-15 構成管理計画書](DOC-15_config_mgmt_plan.md) — ブランチ戦略

**関連実装:**
- `Cargo.toml` (workspace)
- `crates/ide-shell-desktop/tauri.conf.json` (デスクトップ設定)
- `crates/ide-shell-web/Cargo.toml` (Web 設定)
- `.github/workflows/ci.yml` (CI 9 jobs)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 (Stage 6 完了時点) |