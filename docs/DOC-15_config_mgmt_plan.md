# [DOC-15] 構成管理計画書

> **メタ情報:**
> - **ドキュメント ID:** DOC-15
> - **ドキュメント名:** 構成管理計画書
> - **バージョン:** v0.1.0
> - **作成日:** 2026-10-09
> - **最終更新:** 2026-10-09
> - **担当プロセス:** 構成管理プロセス (CF2013 §7)
> - **準拠標準:** IPA 共通フレーム2013 §3 / §5 / §6 / §7
> - **ステータス:** Draft

## 1. 概要

**Git ブランチ戦略, タグ, CI/CD, ビルド成果物** を含む構成管理計画.

## 2. プロセス定義

**プロセス ID:** DOC-15
**プロセス名:** 構成管理プロセス
**開始条件:** プロジェクト開始
**終了条件:** プロジェクト終了

## 3. アクティビティ

### DOC-15.A ブランチ戦略
### DOC-15.B タグ管理
### DOC-15.C CI/CD
### DOC-15.D ビルド成果物

## 4. タスク

### DOC-15.A.1 ブランチ戦略

| ブランチ | 用途 | 保護 |
|---|---|---|
| `main` | 本番リリース | ✅ Protected (no force push) |
| `dev` | 開発統合 | ✅ Protected |
| `dev-3` | 並行開発 (legacy) | なし |
| `agent/<model>/ulys-NNN-hash` | エージェント作業 | なし |
| `test/ulys-NNN-name` | テスト・実験 | なし |
| `hotfix/<issue>` | 緊急修正 | なし |

**命名規則:**
- `ulys-NNN`: 課題追跡 ID
- `agent/<model>`: 自動化エージェント
- `test/`: テスト専用
- `hotfix/`: 緊急対応

### DOC-15.A.2 コミットメッセージ規約

```
<type>(<scope>): <subject>

type: feat | fix | docs | refactor | test | chore
scope: ulys-191-N | 範囲
subject: 簡潔な説明

例:
feat(ulys-191-30): sakura 機能対比 + UI リファクタ
fix(ulys-191-31): wiki テキストから Vim 削除
docs(ulys-191-34): README + IPA ドキュメント
```

### DOC-15.B.1 タグ管理

| タグ | 意味 |
|---|---|
| `v0.1.0` | 初回リリース |
| `v0.1.1` | マイナー修正 |
| `v0.1.5` | 機能追加 |
| `v0.1.7` | 安定版 |
| `v0.1.9` | 本日 |
| `v0.2.0` | 計画 (sakura 100% 達成済 — 2026-10-09) |
| `v0.3.0` | 計画 (プラグイン) |

### DOC-15.C.1 CI/CD (9 jobs)

| # | Job | トリガ | 内容 |
|---|---|---|---|
| 1 | fmt | push, PR | `cargo fmt --all -- --check` |
| 2 | clippy | push, PR | `cargo clippy --all -- -D warnings` |
| 3 | build (Linux) | push, PR | workspace build |
| 4 | test (Linux) | push, PR | `cargo test --workspace` |
| 5 | coverage | push, PR | 66.25% 計測 |
| 6 | integration (Linux) | push, PR | SIT 実行 |
| 7 | web build (Linux) | push, PR | ide-shell-web ビルド |
| 8 | tauri desktop (Linux) | push, PR | MSI ビルド試行 |
| 9 | UAT (Linux) | push, PR | Playwright e2e |

### DOC-15.C.2 CI 環境

| 項目 | 値 |
|---|---|
| Runner | GitHub-hosted (ubuntu-22.04) |
| Rust | stable |
| キャッシュ | `~/.cargo` + `target/` |
| 並列度 | 9 jobs 並列 |
| 平均時間 | 5-8 分 |

### DOC-15.D.1 ビルド成果物

| 名前 | 場所 | リリース |
|---|---|---|
| MSI installer | `target/release/bundle/msi/IDE1.0_*.msi` | GitHub Release |
| desktop exe | `target/release/ide-shell-desktop.exe` | (内部) |
| web exe | `target/release/ide-shell-web.exe` | (内部) |
| protocol server | `target/release/ide-shell-protocol-server.exe` | (内部) |

### DOC-15.D.2 リリース手順

1. `dev` ブランチで開発
2. PR 作成 → CI 9 jobs pass
3. `dev` → `main` へマージ
4. タグ `v0.1.N` 作成
5. GitHub Release 公開 + MSI 添付
6. `origin/main` 更新

## 5. 注記

### DOC-15.A.1.i worktree 利用
複数 agent 開発時は `git worktree` 活用 (例: `E:/IDE1.0` + `D:/orcaWork/IDE1.0/dev-3`).

### DOC-15.C.1.i Linux 制限
powershell.exe がないため Terminal 系テストは skip. UI テストは chromium で実行.

## 6. 関連ドキュメント

- [DOC-14 リスク管理](DOC-14_risk_register.md) — リスク
- [DOC-12 プロジェクト計画](DOC-12_project_plan.md) — 計画
- [DOC-16 意思決定記録](DOC-16_decision_log.md) — 決定

**関連ファイル:**
- `.github/workflows/ci.yml`
- `Cargo.toml` (workspace)
- `crates/ide-shell-desktop/tauri.conf.json`

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |