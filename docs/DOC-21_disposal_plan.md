# [DOC-21] 廃棄計画書

> **メタ情報:** DOC-21 / v0.1.0 / 2026-10-09 / 廃棄プロセス (CF2013 §7) / Draft

## 1. 概要

IDE1.0 の **将来の廃棄** 計画. データ移行, アンインストール, 環境復元.

## 2. プロセス定義

**プロセス ID:** DOC-21
**プロセス名:** 廃棄プロセス
**開始条件:** プロジェクト終了 (将来, 後継 IDE リリース時)
**終了条件:** 全環境から完全削除

## 3. アクティビティ

### DOC-21.A 事前通知
### DOC-21.B データ移行
### DOC-21.C アンインストール
### DOC-21.D 環境復元

## 4. タスク

### DOC-21.A.1 事前通知

- **6 ヶ月前:** 公式アナウンス (GitHub Releases, Discussions)
- **3 ヶ月前:** 最終版リリース (LTS ブランチ)
- **1 ヶ月前:** 移行ガイド公開
- **廃止日:** v1.0.0 以降は別プロジェクトへ

### DOC-21.B.1 データ移行

| データ | 移行先 | 方法 |
|---|---|---|
| プロジェクトファイル | 影響なし (ユーザー管理) | そのまま |
| 設定 | 後継 IDE | JSON export / import |
| プラグイン | 後継 IDE プラグイン | 移行スクリプト |
| ドキュメント | GitHub Wiki | Markdown 形式 |

### DOC-21.B.2 エクスポート機能 (将来実装)

```bash
# 全設定 export
ide-shell --export-settings > settings.json

# 全プロジェクト メタデータ
ide-shell --export-projects > projects.json
```

### DOC-21.C.1 アンインストール手順

#### デスクトップ MSI

```powershell
# 1. コントロール パネル → プログラムと機能
# 2. "IDE1.0 IDE Shell" を選択
# 3. アンインストール

# コマンドライン
msiexec /x IDE1.0_0.1.9_x64_en-US.msi /qn

# 手動削除 (完全クリーンアップ)
Remove-Item -Recurse "C:\Program Files\IDE1.0 IDE Shell"
Remove-Item -Recurse "$env:APPDATA\IDE1.0"
Remove-Item -Recurse "$env:LOCALAPPDATA\IDE1.0"
```

#### Web 版

```bash
# プロセス停止
Stop-Process -Name "ide-shell-web" -Force

# バイナリ削除
Remove-Item ide-shell-web.exe
```

### DOC-21.D.1 環境復元

| 項目 | 確認 |
|---|---|
| WebView2 runtime | 残す (他アプリ利用) |
| PowerShell | 残す (Windows 標準) |
| Cargo / Rust | 残す (ユーザー判断) |
| Git | 残す (ユーザー判断) |
| ユーザープロジェクト | そのまま (影響なし) |

### DOC-21.D.2 残存物の確認

```bash
# レジストリ (Windows)
reg query "HKLM\SOFTWARE\IDE1.0"
reg query "HKCU\SOFTWARE\IDE1.0"

# ファイル
where.exe ide-shell-desktop
where.exe ide-shell-web

# サービス
Get-Service | Where-Object {$_.Name -like "*ide1*"}
```

## 5. 注記

### DOC-21.A.1.i 段階的廃止
v0.x 系の LTS サポート: 6 ヶ月. v1.0.0 リリース後は 12 ヶ月.

### DOC-21.B.1.i データ移行の重要性
ユーザーが作成したファイル (プロジェクト) は IDE 側で持たない. IDE はビューアのため, データ移行は不要.

## 6. 関連ドキュメント

- [DOC-19 保守計画](DOC-19_maintenance_plan.md) — 保守
- [DOC-20 運用手順](DOC-20_operations_manual.md) — 運用
- [DOC-15 構成管理](DOC-15_config_mgmt_plan.md) — リリース

**関連ファイル:**
- `README.md` (サポート情報)
- GitHub Releases (LTS 告知)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |