# [DOC-20] 運用手順書

> **メタ情報:**
> - **ドキュメント ID:** DOC-20
> - **ドキュメント名:** 運用手順書
> - **バージョン:** v0.1.0
> - **作成日:** 2026-10-09
> - **最終更新:** 2026-10-09
> - **担当プロセス:** 運用プロセス (CF2013 §7)
> - **準拠標準:** IPA 共通フレーム2013 §3 / §5 / §6 / §7
> - **ステータス:** Draft

## 1. 概要

IDE1.0 の **日常運用** 手順. インストール, 起動, 設定, トラブルシューティング.

## 2. プロセス定義

**プロセス ID:** DOC-20
**プロセス名:** 運用プロセス
**対象:** デスクトップ版 + Web 版
**環境:** Windows 10/11

## 3. アクティビティ

### DOC-20.A インストール
### DOC-20.B 起動
### DOC-20.C 設定
### DOC-20.D トラブルシューティング

## 4. タスク

### DOC-20.A.1 インストール (デスクトップ MSI)

```powershell
# 1. MSI ダウンロード
# https://github.com/UlyssesLeoLee/IDE1.0/releases/latest

# 2. インストール実行
msiexec /i IDE1.0_0.1.9_x64_en-US.msi /qn

# インストール先: C:\Program Files\IDE1.0 IDE Shell\
# ショートカット: スタートメニュー + デスクトップ
```

### DOC-20.A.2 インストール (Web 単体)

```bash
# 1. ソース clone
git clone https://github.com/UlyssesLeoLee/IDE1.0.git
cd IDE1.0

# 2. ビルド
cargo build --release -p ide-shell-web

# 3. 起動
./target/release/ide-shell-web.exe
# → http://127.0.0.1:8123
```

### DOC-20.B.1 起動 (デスクトップ)

```
方法 1: スタートメニュー → "IDE1.0 IDE Shell"
方法 2: デスクトップ ショートカット
方法 3: コマンドライン
  C:\Program Files\IDE1.0 IDE Shell\ide-shell-desktop.exe
```

**初回起動時の確認:**
- WebView2 runtime あり (Windows 11 標準)
- powershell.exe 利用可能 (Windows 標準)
- プロジェクトルート選択 (rfd ダイアログ)

### DOC-20.B.2 起動 (Web)

```bash
# 既定ポート: 8123
ide-shell-web.exe

# カスタムポート
IDE_SHELL_WEB_PORT=8080 ide-shell-web.exe
```

ブラウザで `http://127.0.0.1:8123` を開く.

### DOC-20.C.1 環境変数

| 変数 | 既定 | 説明 |
|---|---|---|
| `IDE_SHELL_WEB_PORT` | 8123 | Web 版 ポート |
| `IDE_SHELL_WEB_TEST_ROOT` | (なし) | テスト用 プロジェクトルート |
| `RUST_LOG` | info | ログレベル |
| `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` | (なし) | WebView2 設定 |

### DOC-20.C.2 設定ファイル

| 場所 | 用途 |
|---|---|
| `%APPDATA%\IDE1.0\settings.json` | ユーザー設定 (将来) |
| `tauri.conf.json` | デスクトップ設定 (ビルド時) |
| `Cargo.toml` | ワークスペース設定 |

### DOC-20.D.1 トラブルシューティング

| 症状 | 原因 | 対処 |
|---|---|---|
| 起動しない | WebView2 未インストール | https://developer.microsoft.com/microsoft-edge/webview2/ |
| Terminal が起動しない | powershell.exe なし | PowerShell 7 インストール |
| ファイル開けない | パス不正 | プロジェクトルート内を確認 |
| 文字化け | 非 UTF-8 ファイル | 別エディタ使用 |
| ビルド遅い | cargo cache なし | `cargo clean && cargo build` |

### DOC-20.D.2 ログ取得

```bash
# ログレベル設定
RUST_LOG=debug ide-shell-desktop.exe 2> log.txt

# Web ログ
RUST_LOG=info ide-shell-web.exe 2> log.txt
```

### DOC-20.D.3 サポート

- GitHub Issues: https://github.com/UlyssesLeoLee/IDE1.0/issues
- Discussions: https://github.com/UlyssesLeoLee/IDE1.0/discussions
- Wiki: https://github.com/UlyssesLeoLee/IDE1.0/wiki (将来)

## 5. 注記

### DOC-20.A.1.i サイレント インストール
`/qn` で UI 非表示. 企業向け展開に便利.

### DOC-20.D.1.i 既知の制限
- Linux: デスクトップ版 非対応 (Web 版は OK)
- macOS: 未対応 (将来)

## 6. 関連ドキュメント

- [DOC-19 保守計画](DOC-19_maintenance_plan.md) — 保守
- [DOC-15 構成管理](DOC-15_config_mgmt_plan.md) — リリース
- [DOC-21 廃棄計画](DOC-21_disposal_plan.md) — 廃棄

**関連ファイル:**
- `README.md` (クイックスタート)
- `CHANGELOG.md` (将来)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |