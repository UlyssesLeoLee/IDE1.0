# sakura-rs — 100% 機能カバレッジ計画 (24 件 補完)

> **For Hermes:** 用户选择 C 方案 — 补完全部 24 件未实现的 sakura 功能
> **目標:** 76.9% → 100% カバレッジ
> **期間:** 3 週 (約 210 工時)
> **コミット策略:** 4-5 件 / コミット, 段階的に push

## Goal

sakura editor 全 104 機能を sakura-rs で実装. PARITY.md の ❌ (8件) + 🚧 (16件) = 24 件を全完了.

## 现状

- 総項目: 104
- 完了: 80 (✅ 76 + 🟡 4)
- 未完了: 24 (❌ 8 + 🚧 16)
- カバレッジ: **76.9%**

## 補完計画 — 7 批次 (4 PR / 1 週)

### Phase 1: 高優 7 件 (Week 1) — 24h

| ID | 機能 | テスト | 状態 | 工時 |
|---|---|---|---|---|
| 7.7 | 自動バックアップ | test_auto_backup_periodic | ✅ 計画 | 3h |
| 7.8 | カーソル位置保持 | test_cursor_persistence_*.rs | ✅ 計画 | 2h |
| 2.11 | 对应括弧跳转 | test_jump_to_matching_paren | ✅ 計画 | 2h |
| 4.4 | インクリメンタルサーチ | test_incremental_search | ✅ 計画 | 5h |
| 10.2 | 全角⇔半角 | test_zenkaku_hankaku | ✅ 計画 | 4h |
| 6.3 | 文字色/背景/太字/下線 | test_syntax_styling | ✅ 計画 | 5h |
| 7.5 | コントロールコード表示 | test_control_code_visualize | ✅ 計画 | 3h |
| **Week 1 合計** | **7 件** | | | **24h** |

### Phase 2: 中優 + IO 4 件 (Week 2 前半) — 28h

| ID | 機能 | 状態 | 工時 |
|---|---|---|---|
| 7.2 | Shift_JIS 読み書き | 🚧 → ✅ | 6h |
| 7.3 | JIS/EUC/UTF-16 | 🚧 → ✅ | 8h |
| 7.6 | ファイル排他制御 | 🚧 → ✅ | 4h |
| 4.11 | 検索文字列強調表示 | 🚧 → ✅ | 3h |
| 4.5 | Migemo 検索 | ❌ → ✅ | 7h (基本実装) |
| **Week 2 前半 合計** | **5 件** | | **28h** |

### Phase 3: 表示 + 強調 5 件 (Week 2 後半) — 22h

| ID | 機能 | 状態 | 工時 |
|---|---|---|---|
| 12.2 | ルーラー (桁ルーラー) | 🚧 → ✅ | 4h |
| 12.5 | フォント変更 | 🚧 → ✅ | 3h |
| 6.2 | 強調キーワード 10 セット | 🚧 → ✅ | 6h |
| 6.6 | 行頭数字/記号ツリー | 🚧 → ✅ | 4h |
| 4.5 続き | Migemo 完全実装 | 計画 → ✅ | 5h |
| **Week 2 後半 合計** | **5 件** | | **22h** |

### Phase 4: ウィンドウ + ナビ 3 件 (Week 3 前半) — 24h

| ID | 機能 | 状態 | 工時 |
|---|---|---|---|
| 11.1 | SDI (文書毎ウィンドウ) | 🚧 → ✅ | 8h |
| 11.5 | 縦横分割 (四方) | 🚧 → ✅ | 10h |
| 8.4 | ダイレクトタグジャンプ | 🚧 → ✅ | 6h |
| **Week 3 前半 合計** | **3 件** | | **24h** |

### Phase 5: マクロ 3 件 (Week 3 後半) — 53h

| ID | 機能 | 状態 | 工時 |
|---|---|---|---|
| 9.1 | キーマクロ記録/再生 | ❌ → ✅ | 8h |
| 9.2 | PPA マクロ (basic) | ❌ → ✅ (basic のみ) | 20h |
| 9.3 | WSH/JScript マクロ | ❌ → ✅ (JS のみ) | 25h |
| **Week 3 後半 合計** | **3 件** | | **53h** |

### Phase 6: プラグイン + 常駐 2 件 (Week 3 末) — 36h

| ID | 機能 | 状態 | 工時 |
|---|---|---|---|
| 14.1 | 常駐機能 | ❌ → ✅ | 6h |
| 14.2 | プラグイン API | ❌ → ✅ (basic) | 30h |
| **Week 3 末 合計** | **2 件** | | **36h** |

### Phase 7: 最終 テスト + ドキュメント (Week 3 末) — 8h

- 全 104 機能の sakura_parity.rs テスト追加
- PARITY.md 100% 更新
- ベンチマーク再実行

## Tech Stack

- **言語:** Rust (sakura-rs crate)
- **テスト:** cargo test (既存 64 + 24 = 88 tests)
- **外部 crate (新增, 仅必要):**
  - `encoding_rs` — Shift_JIS / EUC-JP / JIS 変換
  - `notify` — ファイル排他制御 + 監視
  - `rune` or `rquickjs` — JS マクロ 実行
- **既存依存:** 0 (維持)

## Step-by-Step Plan

### Task 1: Phase 1 — 高優先 7 件 (Week 1)

**Files to create/modify:**
- `crates/sakura-rs/src/auto_backup.rs` (新)
- `crates/sakura-rs/src/cursor_persistence.rs` (新)
- `crates/sakura-rs/src/matching_paren.rs` (新)
- `crates/sakura-rs/src/incremental_search.rs` (新)
- `crates/sakura-rs/src/zenkaku_hankaku.rs` (新)
- `crates/sakura-rs/src/syntax_styling.rs` (新)
- `crates/sakura-rs/src/control_code.rs` (新)
- `crates/sakura-rs/src/lib.rs` (re-exports)
- `crates/sakura-rs/tests/sakura_parity.rs` (新 7 テスト)
- `crates/sakura-rs/PARITY.md` (更新)

**Step 1:** 7 模块实现 (24h 集中)
**Step 2:** 7 tests 追加
**Step 3:** commit + push (ulys-191-36)

### Task 2: Phase 2 — IO + Migemo (Week 2 前)

**Files:**
- `crates/sakura-rs/src/encoding_io.rs` (新, encoding_rs 使用)
- `crates/sakura-rs/src/file_lock.rs` (新)
- `crates/sakura-rs/src/search_highlight.rs` (新)
- `crates/sakura-rs/src/migemo.rs` (新, basic 実装)

**Step 1:** 4 模块
**Step 2:** 4 tests
**Step 3:** commit (ulys-191-37)

### Task 3: Phase 3 — 表示 + 強調 (Week 2 後)

**Files:**
- `crates/sakura-rs/src/ruler.rs`
- `crates/sakura-rs/src/font_manager.rs`
- `crates/sakura-rs/src/keyword_set.rs` (10 セット)
- `crates/sakura-rs/src/outline_extended.rs` (行頭数字/記号)
- Migemo 完全版

### Task 4: Phase 4 — ウィンドウ + ナビ (Week 3 前)

**Files:**
- `crates/sakura-rs/src/sdi.rs`
- `crates/sakura-rs/src/split_quad.rs`
- `crates/sakura-rs/src/tag_jump.rs`

### Task 5: Phase 5 — マクロ (Week 3 後)

**Files:**
- `crates/sakura-rs/src/key_macro.rs`
- `crates/sakura-rs/src/ppa_macro.rs` (basic)
- `crates/sakura-rs/src/js_macro.rs` (rquickjs)

### Task 6: Phase 6 — プラグイン (Week 3 末)

**Files:**
- `crates/sakura-rs/src/daemon.rs` (常駐)
- `crates/sakura-rs/src/plugin.rs` (basic API)

### Task 7: Phase 7 — 最終検証

**Files:**
- `crates/sakura-rs/tests/sakura_parity.rs` (88 tests)
- `crates/sakura-rs/PARITY.md` (100% ✅)
- `crates/sakura-rs/examples/bench.rs` (再実行)

## Tests / Validation

- 既存 cargo test 253 → 277 (24 追加)
- 既存 sakura_parity 36 → 60 (24 追加)
- cargo test -p sakura-rs: 88/88 pass
- bench: ≥ 50,000 ops/s 維持
- Cargo.lock sync
- `RUSTFLAGS=-D warnings` 厳守

## Risks, Tradeoffs, and Open Questions

| Risk | Mitigation |
|---|---|
| 210h 集中作業 — ユーザー レビュー 必要 | 7 批次に分け, 各 Phase 完了時に ユーザー確認 |
| マクロ系 (PPA/WSH) は複雑な実装 | basic 機能のみ (完全互換は不可, 将来拡張) |
| プラグイン API 設計 | minimal viable API のみ (D-Boy/extension points) |
| 依存 crate 追加 (encoding_rs, notify, rquickjs) | 必要最小限, 0 重型依存 方針は維持 |
| 既存 80 件 機能の regression | 全 Phase で cargo test 実行 + UAT |

## Definition of Done

- [ ] PARITY.md 100% ✅
- [ ] sakura_parity.rs 60 tests pass
- [ ] cargo test -p sakura-rs: 88/88
- [ ] bench: ≥ 50,000 ops/s
- [ ] No warnings (`-D warnings`)
- [ ] 7 commits pushed to origin/main
- [ ] 最終 commit メッセージ: `feat(ulys-191-43): 100% sakura 機能カバレッジ達成`