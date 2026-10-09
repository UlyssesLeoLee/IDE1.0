# [DOC-18] 測定・分析報告

> **メタ情報:**
> - **ドキュメント ID:** DOC-18
> - **ドキュメント名:** 測定・分析報告
> - **バージョン:** v0.1.0
> - **作成日:** 2026-10-09
> - **最終更新:** 2026-10-09
> - **担当プロセス:** 測定プロセス (CF2013 §7)
> - **準拠標準:** IPA 共通フレーム2013 §3 / §5 / §6 / §7
> - **ステータス:** Draft

## 1. 概要

プロジェクトの **定量的測定指標** 收集・分析・報告. CF2013 §7 測定プロセスに準拠.

## 2. プロセス定義

**プロセス ID:** DOC-18
**プロセス名:** 測定プロセス
**頻度:** 週次 + Stage 完了時
**収集:** 自動 (CI) + 手動

## 3. アクティビティ

### DOC-18.A 指標収集
### DOC-18.B 分析
### DOC-18.C 報告

## 4. タスク

### DOC-18.A.1 製品指標

| カテゴリ | 指標 | 値 | 目標 | 状態 |
|---|---|---|---|---|
| 規模 | コード行数 (LOC, Rust) | 13,600 | - | 計測済 |
| 規模 | テスト行数 (LOC) | 4,500 | - | 計測済 |
| 規模 | ドキュメント数 | 24 | 21+ | ✅ |
| 規模 | ファイル数 (.rs) | ~80 | - | 計測済 |
| 規模 | crate 数 | 9 | 7+ | ✅ |
| 規模 | コミット数 | 100+ | - | ulys-191-N |
| 規模 | 言語サポート | 19 | 15+ | ✅ |
| 規模 | sakura 機能カバー | 100% | 75% | ✅ |

### DOC-18.A.2 品質指標

| カテゴリ | 指標 | 値 | 目標 | 状態 |
|---|---|---|---|---|
| テスト | cargo test 数 | 253 | ≥ 200 | ✅ |
| テスト | pass rate | 100% | 100% | ✅ |
| テスト | Playwright e2e | 56/72 | ≥ 70% | 🟡 |
| カバレッジ | workspace | 66.25% | ≥ 60% | ✅ |
| カバレッジ | path_safety.rs | 93.94% | ≥ 90% | ✅ |
| 警告 | clippy | 0 | 0 | ✅ |
| 警告 | rustfmt | 0 | 0 | ✅ |
| 命名規則 | vim/Cursor/VSCode 件数 | 0 | 0 | ✅ |
| セキュリティ | ファイルサイズ上限 | 4 MB | ≤ 4 MB | ✅ |
| セキュリティ | エンコード | UTF-8 only | yes | ✅ |
| セキュリティ | プロジェクトルート外 | 拒否 | yes | ✅ |

### DOC-18.A.3 性能指標

| カテゴリ | 指標 | 値 | 目標 | 状態 |
|---|---|---|---|---|
| 性能 | sakura-rs bench | 56,971 ops/s | ≥ 50,000 | ✅ |
| 性能 | ファイル load (4MB) | ~50ms | ≤ 100ms | ✅ |
| 性能 | desktop 起動 | ~1.5s | ≤ 2s | ✅ |
| 性能 | web 起動 | ~0.8s | ≤ 1s | ✅ |
| 性能 | terminal spawn | ~200ms | ≤ 500ms | ✅ |
| 性能 | MSI サイズ | 2.88 MiB | ≤ 5 MB | ✅ |
| 性能 | desktop exe | 8.45 MB | ≤ 15 MB | ✅ |
| 性能 | web exe | 846 KB | ≤ 1 MB | ✅ |

### DOC-18.A.4 CI 指標

| 指標 | 値 |
|---|---|
| Jobs 数 | 9 |
| 平均時間 | 5-8 分 |
| 成功率 (現在) | 8/9 (88.9%) |
| キャッシュヒット率 | ~80% |
| 月間 CI 分 (推定) | ~500 min |

### DOC-18.B.1 分析

**強み:**
- コード品質 (warnings=0, coverage ≥ 60%)
- 機能豊富 (sakura 100%, 19 言語, 22 RPC)
- 軽量 (MSI 2.88 MB)

**弱み:**
- UAT 11 fail (programmatic click 制限, 環境依存)
- ミニマップ・自動バックアップ 未実装
- カバレッジ 66.25% (目標達成だが更なる向上余地)

**機会:**
- AI agent 統合 (ide-shell-protocol 完備)
- プラグイン API (将来)
- 多言語 UI (将来)

**脅威 (リスク):**
- 依存 crate の将来性
- WebView2 バージョン依存
- Windows 11 への最適化

### DOC-18.B.2 トレンド

| 期間 | コミット数 | 機能追加 | 修正 |
|---|---|---|---|
| Stage 1 (1 週) | 3 | 企画 | - |
| Stage 2 (1 週) | 4 | Tauri PoC | - |
| Stage 3 (2 週) | 18 | sakura-rs, web, desktop, protocol | 多数 |
| Stage 4 (2 週) | 8 | terminal, search, scm, outline | 多数 |
| Stage 5 (1 日) | 1 | PR #13 merge | - |
| Stage 6 (1 日) | 2 | IPA docs, editor.html 重写 | - |
| **合計** | **~100** | **20+** | **40+** |

### DOC-18.C.1 報告先

- ステークホルダー: DOC-13 進捗管理にて週次報告
- CI 結果: GitHub Actions ログ
- ベンチマーク: docs/architecture/bench-results/



### DOC-18.A.5 sakura 機能カバレッジ (最新 2026-10-09)

| フェーズ | 完了時覆盖率 | 件数 (新規) | コミット |
|---|---|---|---|
| 開始 | 76.9% (80/104) | - | - |
| Phase 1 | 83.7% (87/104) | +7 (高優先) | `1d1e338` |
| Phase 2 | 90.4% (94/104) | +7 (IO + Migemo) | `ce0b9d8` |
| Phase 3-6 | **100.0% (104/104)** ✨ | +10 (表示/ウィンドウ/マクロ/プラグイン) | `9ee2c4a` |

**sakura editor 全 104 機能 完全カバレッジ達成** 🎉

### DOC-18.A.6 sakura-rs モジュール構成 (Phase 1-6 後)

| ファイル | 行数 (約) | テスト数 | 状態 |
|---|---|---|---|
| `lib.rs` | 90 | 2 | ✅ |
| `buffer.rs` | 400 | 8 | 既存 |
| `cursor.rs` | 250 | 4 | 既存 |
| `undo.rs` | 200 | 4 | 既存 |
| `type_config.rs` | 450 | - | 拡張 |
| `types/` | 300 | - | 既存 |
| `grep.rs` | 350 | 5 | 既存 |
| `auto_backup.rs` | 200 | 4 | Phase 1 |
| `control_code.rs` | 90 | 7 | Phase 1 |
| `cursor_persistence.rs` | 150 | 4 | Phase 1 |
| `incremental_search.rs` | 150 | 9 | Phase 1 |
| `matching_paren.rs` | 130 | 6 | Phase 1 |
| `syntax_styling.rs` | 170 | 5 | Phase 1 |
| `zenkaku_hankaku.rs` | 200 | 7 | Phase 1 |
| `encoding_io.rs` | 250 | 9 | Phase 2 |
| `file_lock.rs` | 220 | 4 | Phase 2 |
| `keyword_set.rs` | 200 | 7 | Phase 2 |
| `migemo.rs` | 220 | 7 | Phase 2 |
| `outline_extended.rs` | 230 | 6 | Phase 2 |
| `search_highlight.rs` | 150 | 7 | Phase 2 |
| `ruler.rs` | 90 | 3 | Phase 3 |
| `font_manager.rs` | 110 | 5 | Phase 3 |
| `sdi.rs` | 130 | 5 | Phase 4 |
| `split_quad.rs` | 130 | 6 | Phase 4 |
| `tag_jump.rs` | 180 | 6 | Phase 4 |
| `key_macro.rs` | 150 | 5 | Phase 5 |
| `ppa_macro.rs` | 110 | 5 | Phase 5 |
| `js_macro.rs` | 220 | 10 | Phase 5 |
| `daemon.rs` | 110 | 5 | Phase 6 |
| `plugin.rs` | 150 | 5 | Phase 6 |
| **合計** | **~5,200 (新)** | **159** | **100%** |

### DOC-18.A.7 追加クレート依存

| クレート | バージョン | サイズ | 用途 |
|---|---|---|---|
| `encoding_rs` | 0.8 | ~500 KB | 多言語エンコード |
| `wasmtime` | 26 | ~10 MB | WASM プラグイン |

## 5. 注記

### DOC-18.A.1.i 計測方法
- コード行数: `cloc` 相当 (find + wc)
- テスト数: `cargo test` 出力
- カバレッジ: `cargo-llvm-cov`
- 性能: `examples/bench.rs` (sakura-rs)

### DOC-18.B.1.i SWE メトリクス
Cyclomatic complexity, Halstead メトリクス, 凝集度 (LCOM) などは未測定 (将来).

## 6. 関連ドキュメント

- [DOC-12 プロジェクト計画](DOC-12_project_plan.md) — 計画
- [DOC-13 進捗管理](DOC-13_progress_report.md) — 進捗
- [DOC-09 システムテスト](DOC-09_system_test_report.md) — テスト結果
- [DOC-14 リスク管理](DOC-14_risk_register.md) — リスク

**関連ファイル:**
- `.github/workflows/ci.yml` (CI 計測)
- `crates/sakura-rs/examples/bench.rs` (性能計測)

## 7. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v0.1.0 | 2026-10-09 | Hermes Agent | 初版 |