# [DOC-16] 意思決定記録 (ADR)

> **メタ情報:**
> - **ドキュメント ID:** DOC-16
> - **ドキュメント名:** 意思決定記録 (ADR)
> - **バージョン:** v0.1.0
> - **作成日:** 2026-10-09
> - **最終更新:** 2026-10-09
> - **担当プロセス:** 意思決定管理プロセス (CF2013 §7)
> - **準拠標準:** IPA 共通フレーム2013 §3 / §5 / §6 / §7
> - **ステータス:** Draft

## 1. 概要

**Architecture Decision Records (ADR)**. 重要なアーキテクチャ決定の背景・選択肢・結果を記録.

## 2. プロセス定義

**プロセス ID:** DOC-16
**プロセス名:** 意思決定管理プロセス
**形式:** ADR-XXX 連番
**ステータス:** Proposed / Accepted / Superseded

## 3. アクティビティ

### DOC-16.A 意思決定識別
### DOC-16.B ADR 作成
### DOC-16.C レビュー

## 4. タスク

### DOC-16.A.1 ADR 一覧

| ID | タイトル | ステータス | 日付 |
|---|---|---|---|
| ADR-001 | Tauri 2 採用 | Accepted | 2026-09-22 |
| ADR-002 | sakura-rs 0 重型依存 | Accepted | 2026-09-25 |
| ADR-003 | 19 言語対応 | Accepted | 2026-09-30 |
| ADR-004 | ide-shell-protocol 22 メソッド | Accepted | 2026-10-02 |
| ADR-005 | Bottom Panel 単層 | Accepted | 2026-10-07 |
| ADR-006 | 命名規則 (vim/Cursor/VSCode 禁止) | Accepted | 2026-10-07 |
| ADR-007 | IPA 標準ドキュメント | Accepted | 2026-10-09 |
| ADR-008 | PowerShell Terminal 実装 | Accepted | 2026-10-05 |
| ADR-009 | PR #13 --merge 戦略 | Accepted | 2026-10-07 |
| ADR-010 | editor.html 完全重写 | Accepted | 2026-10-09 |
| ADR-011 | sakura 機能 100% カバレッジ | Accepted | 2026-10-09 |

### DOC-16.B.1 ADR 詳細

#### ADR-001: Tauri 2 採用

- **背景:** Electron の重さ回避, Rust コア再利用
- **選択肢:**
  - A. Electron + Node.js
  - B. Tauri 2 (採用)
  - C. egui/iced (TUI only)
  - D. Qt
- **決定:** B (Tauri 2)
- **理由:** 8.45 MB バイナリ, Rust コア共有, WebView2 活用
- **結果:** デスクトップ + Web で同じ UI 動作

#### ADR-002: sakura-rs 0 重型依存

- **背景:** ビルド時間・脆弱性最小化
- **決定:** tree-sitter WASM, regex crate 等を **使用しない**
- **実装:** 自作 line-based token scanner
- **結果:** sakura-rs ビルド 15s, 0 依存, 28 tests + 36 parity = 64 tests

#### ADR-003: 19 言語対応

- **背景:** ユーザー要望 (rust, python, js, ts, go, c, cpp, java, csharp, bash, yaml, toml, sql, plain, dockerfile, html, css, json, markdown)
- **決定:** 拡張子ベース判別, line-based lexer
- **結果:** 全 19 言語でハイライト動作 (UAT 10 case pass)

#### ADR-004: ide-shell-protocol 22 メソッド

- **背景:** AI agent 統合要件
- **決定:** JSON-RPC 2.0, 3 プロトコル (HTTP/WS/stdio), 22 メソッド
- **結果:** Python クライアント demo 動作, AI provider trait で拡張可

#### ADR-005: Bottom Panel 単層

- **背景:** ユーザー要望「下方不要双层」
- **決定:** Terminal/Output/Problems を **タブ切替** で表示 (multi-session 廃止)
- **結果:** UI シンプル化, `editor.html` 1348 行 (旧 7952)

#### ADR-006: 命名規則

- **背景:** ユーザー要望「不要出现 vim/Cursor/VSCode 命名」
- **決定:** 製品名・参考エディタ名 使用禁止
- **実装:**
  - "Vim 键位" → "三态键位"
  - "Cursor 风格" → 削除
  - "VSCode" → 削除
- **結果:** 全 UI, doc, code で 0 件確認 (commit c61682e)

#### ADR-007: IPA 標準ドキュメント

- **背景:** ユーザー要求「符合日本 IPA 标准」
- **決定:** 21 ドキュメント + 3 共通 = 24 ファイル
- **準拠:** 共通フレーム2013 §3-7
- **結果:** 本ドキュメント体系完成 (本日)

#### ADR-008: PowerShell Terminal

- **背景:** ユーザー要求「Terminal 能实际打开 PowerShell, 不是空壳」
- **決定:** `Command::new("powershell.exe")` + CREATE_NO_WINDOW + pipe I/O
- **検証:** `Write-Host`, `Get-Date`, `Get-Location` 全て実動作
- **結果:** PowerShell 完全動作

#### ADR-009: PR #13 --merge 戦略

- **背景:** PR #11 (失敗) → #12 (squash で 28 commits 圧縮) → #13 必要
- **選択肢:**
  - A. squash (履歴圧縮)
  - B. --merge (履歴保持) ← 採用
  - C. rebase
- **決定:** B
- **結果:** 5334142 merge commit, 28 commits 保持, レビュー可

#### ADR-010: editor.html 完全重写

- **背景:** JS 構文エラー (innerHTML 二重引用符, 注释 `/* */`, CRLF, brace 不平衡)
- **決定:** 1348 行へ完全リライト
- **戦略:** wiki base64 埋め込み, template literal escape 回避
- **結果:** 全機能動作, 1348 行 (旧 7952)

#### ADR-011: sakura 機能 100% カバレッジ

- **背景:** ユーザー要求「sakura editor 全機能を実装」 (ulys-191-36/37/38 計画)
- **選択肢:**
  - A. 76.9% で停止 (当初の計画)
  - B. 90% で停止 (Phase 2 完了時点)
  - C. 100% 達成 (Phase 3-6 含む全実装) ← 採用
- **決定:** C (100% 達成)
- **理由:** ユーザー明示要求, 機能カバレッジ最大の価値提供
- **コスト:** 重型依存追加 (encoding_rs ~500KB, wasmtime ~10MB), js_macro は mini interpreter で代替 (rquickjs/boa 依存回避)
- **結果:** 76.9% → 83.7% (Phase 1) → 90.4% (Phase 2) → **100.0% (Phase 3-6)** 🎉
- **内訳:**
  - Phase 1: 7 件 (高優先) — `1d1e338`
  - Phase 2: 7 件 (IO + Migemo) — `ce0b9d8`
  - Phase 3-6: 10 件 (表示/ウィンドウ/マクロ/プラグイン) — `9ee2c4a`

## 5. 注記

### DOC-16.A.1.i ADR 命名規則
`ADR-NNN` (3 桁数字). 連番, 削除しない (Superseded で残す).

### DOC-16.B.1.i ADR フォーマット
各 ADR は Context / Decision / Consequences を含む (Michael Nygard 形式).

## 6. 関連ドキュメント

- [DOC-15 構成管理](DOC-15_config_mgmt_plan.md) — ブランチ戦略
- [DOC-14 リスク管理](DOC-14_risk_register.md) — リスク
- [PARITY.md](../crates/sakura-rs/PARITY.md) — sakura 機能対比

**関連コミット:**
- `c61682e` ulys-191-34 — editor.html 重写
- `e3adc9c` ulys-191-30 — UI リファクタ + 命名規則
- `5334142` PR #13 --merge
- `1d1e338` ulys-191-36 — Phase 1 sakura 補完
- `ce0b9d8` ulys-191-37 — Phase 2 sakura 補完
- `9ee2c4a` ulys-191-38 — Phase 3-6 sakura 補完 (100% 達成)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 (10 ADR 記録) |