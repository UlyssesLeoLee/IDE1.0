# [DOC-06] ソフトウェア詳細設計書 (DDD)

> **メタ情報:**
> - **ドキュメント ID:** DOC-06
> - **ドキュメント名:** ソフトウェア詳細設計書 (DDD)
> - **バージョン:** v0.1.0
> - **作成日:** 2026-10-09
> - **最終更新:** 2026-10-09
> - **担当プロセス:** ソフトウェア詳細設計 (CF2013 §7)
> - **準拠標準:** IPA 共通フレーム2013 §3 / §5 / §6 / §7
> - **ステータス:** Draft

## 1. 概要

ソフトウェア方式 (DOC-05) を **実装レベル** で詳細化. クラス/関数単位の仕様・アルゴリズム・データ構造.

## 2. プロセス定義

**プロセス ID:** DOC-06
**プロセス名:** ソフトウェア詳細設計
**開始条件:** DOC-05 承認
**終了条件:** DDD レビュー

## 3. アクティビティ

### DOC-06.A データ構造詳細
### DOC-06.B 関数詳細
### DOC-06.C アルゴリズム詳細

## 4. タスク

### DOC-06.A.1 データ構造

```rust
// sakura-rs/src/buffer.rs
pub struct Buffer {
    lines: Vec<Vec<char>>,  // テキストを行ごとに保持
    cursor: Cursor,
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
    config: TypeConfig,
}

// cursor.rs
pub struct Cursor {
    pub row: usize,  // 0-indexed
    pub col: usize,  // 0-indexed (論理列)
}

// types/pos.rs
pub struct Pos {
    pub row: usize,
    pub col: usize,
}

// types/outline.rs
pub struct OutlineItem {
    pub kind: OutlineKind,  // Function | Class | Module | Variable
    pub name: String,
    pub range: (Pos, Pos),
}

// grep.rs
pub struct Match {
    pub path: PathBuf,
    pub line_no: usize,
    pub line: String,
    pub col_start: usize,
    pub col_end: usize,
}

// undo.rs
pub struct Edit {
    pub kind: EditKind,  // Insert | Delete | Replace
    pub pos: Pos,
    pub text: String,
}
```

### DOC-06.B.1 主要関数仕様

#### `Buffer::insert_char(c: char, pos: Pos) -> Result<(), BufferError>`
- **目的:** 位置に 1 文字挿入
- **アルゴリズム:**
  1. 行末判定: `pos.col >= self.lines[pos.row].len()`
  2. `Vec::insert` で挿入
  3. `Edit { Insert, pos, c.to_string() }` を undo_stack に push
  4. redo_stack クリア
- **計算量:** O(1) (行内 insert)
- **エラー:** 範囲外 `BufferError::OutOfRange`

#### `Cursor::move_word_fwd(buf: &Buffer) -> ()`
- **目的:** 単語単位で次へ進む
- **アルゴリズム:**
  1. 現在位置から文字種別判定 (alphanumeric / punctuation / space)
  2. 同種が続くまで進む
  3. 次の非同種 or 行末まで進む
  4. 空白スキップ
- **計算量:** O(line_length)
- **Unicode 対応:** `char::is_alphanumeric()` で OK

#### `Grep::walk_dir(root: &Path) -> Result<Vec<Match>, io::Error>`
- **目的:** ディレクトリを再帰的に走査し, ファイル一覧取得
- **アルゴリズム:**
  1. `walkdir` crate 相当 (sakura-rs は標準のみ)
  2. 拡張子フィルタ (`.rs .py .js .ts .go ...`)
  3. バイナリ判定: 初期 8KB に null バイトがあればスキップ
- **計算量:** O(n) where n = ファイル数

#### `Grep::grep_in_files(path: &Path, pattern: &str) -> Vec<Match>`
- **目的:** ファイル内検索
- **アルゴリズム:**
  1. UTF-8 検証
  2. 1 行ずつ走査
  3. 部分文字列検索 (sakura-rs は正規表現なし, `str::contains`)
  4. マッチ位置を行番号・列範囲として `Match` 生成
- **計算量:** O(file_size) per file

#### `Outline::dispatch(text: &str, lang: Lang) -> Vec<OutlineItem>`
- **目的:** 言語別アウトライン抽出
- **アルゴリズム (Rust 例):**
  ```
  regex: ^\s*(pub\s+)?(fn|struct|enum|impl|trait|mod)\s+(\w+)
  ```
  - `pub` 任意
  - キーワード 6 種
  - 識別子キャプチャ
- **対応言語:** rust / python / js / ts / go / c / cpp / java / csharp / bash / yaml / toml / sql / plain

#### `Terminal::spawn() -> Result<SessionId, io::Error>`
- **目的:** PowerShell セッション作成
- **アルゴリズム:**
  1. `Command::new("powershell.exe")`
  2. `CREATE_NO_WINDOW` フラグ
  3. `Stdio::piped()` で stdin/stdout/stderr
  4. spawn → child process handle
  5. 別 thread で stdout/stderr 読み込み → Buffer に書き込み
  6. セッション ID 返却
- **計算量:** O(1) 起動, その後 async I/O

#### `PathSafety::path_within(root: &Path, target: &Path) -> bool`
- **目的:** target が root 配下か検証
- **アルゴリズム:**
  1. 絶対パス化
  2. verbatim `\\?\` 剥除
  3. 8.3 短パス展開 (フォールバック)
  4. `target.starts_with(root)` 判定
- **計算量:** O(1) 文字列処理

### DOC-06.C.1 状態遷移表 (詳細)

| 現状態 | 入力 | 次状態 | 副作用 |
|---|---|---|---|
| NORMAL | `i` | INSERT | なし |
| NORMAL | `v` | VISUAL | 選択開始 = カーソル位置 |
| NORMAL | `h` | NORMAL | カーソル左 |
| NORMAL | `j` | NORMAL | カーソル下 |
| NORMAL | `k` | NORMAL | カーソル上 |
| NORMAL | `l` | NORMAL | カーソル右 |
| NORMAL | `w` | NORMAL | 単語前進 |
| NORMAL | `b` | NORMAL | 単語後退 |
| NORMAL | `0` | NORMAL | 行頭 |
| NORMAL | `$` | NORMAL | 行末 |
| NORMAL | `gg` | NORMAL | バッファ先頭 |
| NORMAL | `G` | NORMAL | バッファ末尾 |
| NORMAL | `dd` | NORMAL | 行削除 (undo 記録) |
| NORMAL | `yy` | NORMAL | 行コピー |
| NORMAL | `p` | NORMAL | 貼り付け |
| NORMAL | `u` | NORMAL | undo |
| NORMAL | `:` | COMMAND | コマンドライン |
| NORMAL | `zc` | NORMAL | 折り畳み |
| NORMAL | `zo` | NORMAL | 展開 |
| NORMAL | `:` `w` `q` Enter | NORMAL | 保存 |
| NORMAL | `:` `q` Enter | NORMAL | 終了 (確認) |
| INSERT | 任意 | INSERT | バッファに挿入 |
| INSERT | `ESC` | NORMAL | なし |
| VISUAL | `h/j/k/l` | VISUAL | 選択範囲拡張 |
| VISUAL | `y` | NORMAL | コピー |
| VISUAL | `d` | NORMAL | 削除 |
| VISUAL | `ESC` | NORMAL | 選択解除 |

### DOC-06.C.2 19 言語 lexer 仕様

**syntax.js 19 言語:**
1. rust — keyword (fn/let/struct/enum) + 文字リテラル + 数値
2. python — def/class/import + 文字列
3. javascript — const/let/var/function + テンプレート
4. typescript — interface/type/import
5. go — func/package/import
6. c — int/char/struct
7. cpp — class/template/namespace
8. java — public/private/class
9. csharp — using/namespace/class
10. bash — if/then/fi/echo
11. yaml — key:/list
12. toml — [section]/key =
13. sql — SELECT/FROM/WHERE
14. plain — ハイライトなし
15. dockerfile — FROM/RUN/COPY
16. html — `<tag>` + 属性
17. css — selector + property
18. json — key/value
19. markdown — #/**

**認識方法:** 拡張子で判別 → 対応言語指定

### DOC-06.C.3 Undo/Redo スタック

```rust
// undo.rs
pub struct UndoStack {
    stack: Vec<Edit>,
    current: usize,  // saturating_sub 安全
}

impl UndoStack {
    pub fn push(&mut self, edit: Edit) {
        self.stack.truncate(self.current);
        self.stack.push(edit);
        self.current = self.stack.len();
    }
    pub fn undo(&mut self) -> Option<Edit> {
        if self.current == 0 { return None; }
        self.current -= 1;
        self.stack.get(self.current).cloned()
    }
    pub fn redo(&mut self) -> Option<Edit> {
        if self.current >= self.stack.len() { return None; }
        let e = self.stack.get(self.current).cloned();
        self.current += 1;
        e
    }
}
```

## 5. 注記

### DOC-06.A.1.i 0 重型依存戦略
sakura-rs は外部 crate を 1 つも持たない.
- regex → 自作 line scanner
- tree-sitter → 自作 line-based トークナイザ
- memmap → 標準 `Vec<char>` のみ

### DOC-06.C.2.i 言語認識の限界
- 現状: 拡張子のみ
- 将来: shebang (`#!/usr/bin/env python`) 検出
- 将来: 内容ヒューリスティック

## 6. 関連ドキュメント

- [DOC-05 ソフトウェア SDD](DOC-05_software_design.md) — 上流
- [DOC-07 実装報告](DOC-07_implementation_report.md) — 実装
- [DOC-10 SUT](DOC-10_unit_test_spec.md) — テスト
- [PARITY.md](../crates/sakura-rs/PARITY.md) — 機能対比

**関連実装:**
- `crates/sakura-rs/src/` (lib/buffer/cursor/undo/grep/type_config)
- `crates/ide-shell-web/src/editor.html` (state machine)
- `crates/ide-shell-web/src/syntax.js` (言語 lexer)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |