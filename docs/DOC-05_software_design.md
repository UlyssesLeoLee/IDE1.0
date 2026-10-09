# [DOC-05] ソフトウェア方式設計書 (SDD)

> **メタ情報:** DOC-05 / v0.1.0 / 2026-10-09 / ソフトウェア方式設計 (CF2013 §7) / Draft

## 1. 概要

ソフトウェアレベルの **方式設計**. モジュール構造・データフロー・状態モデル・並行性設計を定義.

## 2. プロセス定義

**プロセス ID:** DOC-05
**プロセス名:** ソフトウェア方式設計
**開始条件:** DOC-04 承認
**終了条件:** SDD レビュー

## 3. アクティビティ

### DOC-05.A モジュール構造設計
### DOC-05.B データフロー設計
### DOC-05.C 状態モデル設計
### DOC-05.D 並行性設計

## 4. タスク

### DOC-05.A.1 sakura-rs モジュール構成

```
sakura-rs/
├─ lib.rs              # 公開 API + re-exports
├─ buffer.rs           # テキストバッファ (Vec<Line>)
├─ cursor.rs           # カーソル位置 (row, col)
├─ undo.rs             # Undo/Redo スタック
├─ type_config.rs      # 言語別設定 (TypeConfig)
├─ types/              # 型定義
│   ├─ mod.rs
│   ├─ pos.rs          # 位置 (line, col, offset)
│   └─ outline.rs      # アウトライン項目
├─ grep.rs             # ファイル横断検索
└─ examples/bench.rs   # 性能ベンチ (56,971 ops/s)
```

### DOC-05.A.2 ide-shell-web 構成

```
ide-shell-web/
├─ src/
│   ├─ main.rs         # axum 起動
│   ├─ lib.rs          # 13 endpoint dispatcher
│   ├─ path_safety.rs  # サンドボックス検証
│   ├─ terminal.rs     # PowerShell + HTTP polling
│   ├─ wiki_data.rs    # ドキュメントテキスト
│   └─ editor.html     # 1348 行 (フル UI)
└─ tests/
    └─ path_safety.rs  # 12 tests
```

### DOC-05.A.3 ide-shell-desktop 構成

```
ide-shell-desktop/
├─ src/
│   ├─ main.rs         # Tauri 2 エントリ
│   ├─ lib.rs          # Tauri commands + wiki test
│   ├─ terminal.rs     # PowerShell 起動
│   ├─ wiki.rs         # ドキュメント
│   ├─ search.rs       # ファイル検索
│   ├─ scm.rs          # ソース管理
│   └─ outline.rs      # アウトライン
├─ dist/
│   └─ index.html      # 1348 行 (editor.html 同期)
└─ tauri.conf.json     # 0.1.9
```

### DOC-05.B.1 データフロー (ファイルオープン)

```
User → [doOpenFolder] → rfd ダイアログ
  → [abs_path] → path_safety 検証
  → [tree] → ファイルツリー表示
  → User → [ファイルクリック] → タブ開く
  → [Buffer::load] → sakura-rs Buffer
  → [Editor 表示]
```

### DOC-05.B.2 データフロー (ターミナル)

```
User → [+] → [terminal_create]
  → Rust: powershell.exe spawn (CREATE_NO_WINDOW)
  → stdin pipe + stdout/stderr thread
  → session_id 返却
  → User → 入力
  → [terminal_input] → stdin write
  → stdout reader thread → Buffer に追加
  → [terminal_output?id=X] → Buffer 取得
  → Terminal 表示
```

### DOC-05.C.1 状態モデル (三態键位)

```
       i / INSERT
   ┌──────────────────┐
   │                  ↓
[NORMAL] ←─── ESC ─── [INSERT]
   │                  ↑
   │    v (VISUAL)    │
   └──→ [VISUAL] ─────┘
         │
         ESC → NORMAL
```

| 状態 | 入力 | 動作 |
|---|---|---|
| NORMAL | h/j/k/l | カーソル移動 |
| NORMAL | i | INSERT に入る |
| NORMAL | v | VISUAL に入る |
| NORMAL | : | コマンドモード |
| INSERT | 文字 | バッファに挿入 |
| INSERT | ESC | NORMAL に戻る |
| VISUAL | h/j/k/l | 選択範囲拡張 |

### DOC-05.D.1 並行性モデル

| 場所 | 機構 | 用途 |
|---|---|---|
| Tauri runtime | tokio | async I/O |
| Terminal stdout | std::thread | pipe 読み込み |
| web polling | setInterval 100ms | Terminal 更新 |
| sakura-rs バッファ | 同期 (Mutex 不要) | 単一スレッド |

## 5. 注記

### DOC-05.A.1.i 0 重型依存
sakura-rs は tree-sitter WASM, regex crate 等を **一切使用しない**.
- 自作: line-based token scanner
- 自作: 単語境界判定 (Unicode カテゴリ)

### DOC-05.C.1.i 命名規則
- 状態名: 日本語 UI 用「NORMAL/INSERT/VISUAL」表示
- 内部 enum: `Mode::Normal | Insert | Visual`

## 6. 関連ドキュメント

- [DOC-04 ソフトウェア要件](DOC-04_software_requirements.md) — 上流
- [DOC-06 詳細設計](DOC-06_software_detail_design.md) — 詳細化
- [DOC-08 SIT](DOC-08_integration_test_spec.md) — 結合テスト
- [DOC-15 構成管理](DOC-15_config_mgmt_plan.md) — ブランチ/CI

**関連実装:**
- 全 crate の `src/lib.rs` ファイル
- `editor.html` 状態管理

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |