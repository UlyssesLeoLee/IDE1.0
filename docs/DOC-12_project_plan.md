# [DOC-12] プロジェクト計画書

> **メタ情報:** DOC-12 / v0.1.0 / 2026-10-09 / プロジェクト計画 (CF2013 §7) / Draft

## 1. 概要

IDE1.0 IDE Shell の **プロジェクト全体計画**. スコープ, WBS, マイルストーン, リソース, スケジュールを定義.

## 2. プロセス定義

**プロセス ID:** DOC-12
**プロセス名:** プロジェクト計画プロセス
**開始条件:** DOC-01 Charter 承認
**終了条件:** 計画書承認

## 3. アクティビティ

### DOC-12.A スコープ定義
### DOC-12.B WBS 作成
### DOC-12.C スケジュール計画
### DOC-12.D リソース計画

## 4. タスク

### DOC-12.A.1 スコープ

| IN (含む) | OUT (含まない) |
|---|---|
| Rust コア (sakura-rs) | 言語サーバ (LSP) 完全対応 |
| Tauri 2 デスクトップ | 完全なデバッガ |
| Web エディタ | マーケットプレース |
| PowerShell ターミナル | プラグイン API 完全公開 |
| 19 言語シンタックス | 多言語 UI (英中のみ) |
| IPA ドキュメント | フル機能マクロ (将来) |

### DOC-12.B.1 WBS (Work Breakdown Structure)

```
1. 企画・要件 (Stage 1)
   1.1 企画書 (DOC-01)
   1.2 システム要件 (DOC-02)
   1.3 ソフトウェア要件 (DOC-04)

2. 設計 (Stage 2)
   2.1 システム HLD (DOC-03)
   2.2 ソフトウェア SDD (DOC-05)
   2.3 詳細設計 (DOC-06)

3. 実装 (Stage 3)
   3.1 sakura-rs コア
   3.2 ide-shell-web
   3.3 ide-shell-desktop
   3.4 ide-shell-protocol
   3.5 Terminal

4. 検証 (Stage 4)
   4.1 単体テスト (DOC-10)
   4.2 結合テスト (DOC-08)
   4.3 システムテスト (DOC-09)
   4.4 UAT (DOC-11)

5. 配布 (Stage 5)
   5.1 MSI パッケージ
   5.2 Web リリース
   5.3 GitHub Release

6. 拡張 (Stage 6)
   6.1 sakura 機能カバレッジ (76% → 100%)
   6.2 IPA ドキュメント (21 件)
   6.3 ベンチマーク公開
```

### DOC-12.C.1 スケジュール・マイルストーン

| Stage | 期間 | 開始 | 完了 | コミット | 状態 |
|---|---|---|---|---|---|
| 1. 企画 | 1 週 | 2026-09-15 | 2026-09-22 | (ulys-191-1, 3) | ✅ |
| 2. 設計 | 1 週 | 2026-09-22 | 2026-09-29 | ulys-191-4, 5 | ✅ |
| 3. 実装 | 2 週 | 2026-09-29 | 2026-10-09 | ulys-191-6 ~ 30 | ✅ |
| 4. 検証 | 並行 | 2026-10-01 | 2026-10-09 | ulys-191-21, 22, 31 | ✅ |
| 5. 配布 | 1 日 | 2026-10-09 | 2026-10-09 | PR #13 → main | ✅ |
| 6. 拡張 | 進行中 | 2026-10-09 | - | ulys-191-34 | 🟡 |

**総期間:** 約 4 週 (Stage 1-5) + Stage 6 進行中

### DOC-12.D.1 リソース

| ロール | 担当 | 工数 |
|---|---|---|
| プロジェクトマネージャ | Ulysses | 5h/週 |
| アーキテクト | Ulysses + Hermes | 10h/週 |
| 開発者 | Hermes (主), Ulysses (review) | 30h/週 |
| テスター | Playwright (自動) + Ulysses (手動) | 5h/週 |
| DevOps | GitHub Actions | 自動化 |

### DOC-12.D.2 ツール

| 用途 | ツール |
|---|---|
| コード管理 | Git + GitHub |
| タスク管理 | Issue + Project |
| CI | GitHub Actions (9 jobs) |
| テスト | cargo test + Playwright |
| ドキュメント | Markdown (本ファイル) |
| IDE | **IDE1.0** (dogfooding) |

### DOC-12.D.3 コスト概算

| 項目 | 値 |
|---|---|
| 工数 (人時) | ~200 h |
| CI 分数 (GitHub) | ~50 分/run × 20 run = 1,000 分 |
| 開発ツール | $0 (OSS) |
| 配布 | $0 (GitHub Releases) |
| **合計** | **$0** |

## 5. 注記

### DOC-12.C.1.i 並行開発
Stage 3-4 は並行 (TDD サイクル).

### DOC-12.D.3.i OSS 戦略
全て MIT ライセンス (予定). 外部依存 crate は MIT / Apache 2.0 のみ.

## 6. 関連ドキュメント

- [DOC-01 企画書](DOC-01_project_charter.md) — 上位
- [DOC-13 進捗管理](DOC-13_progress_report.md) — 進捗
- [DOC-14 リスク管理](DOC-14_risk_register.md) — リスク

**関連コミット:**
- 多数 (ulys-191-1 ~ 34)
- PR #11, #12, #13 (PR #13 merged → 5334142)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |