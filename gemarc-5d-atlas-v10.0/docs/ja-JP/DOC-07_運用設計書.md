# [ID1.0-JP-07] 運用設計書

> - 正本: `docs/DOC-03_system_hld.md` / `docs/DOC-20_operations_manual.md` / `docs/DOC-19_maintenance_plan.md` / `docs/DOC-21_disposal_plan.md`
> - 本図集では運用測定を**実施していない**。数値は出典文書からの引用であり、本図集での再測定ではない。

## 1. 配布形態

| 形態 | 成果物 | サイズ | 起動方法 | 配布先 | 根拠 |
|---|---|---|---|---|---|
| MSI | `IDE1.0_0.1.9_x64_en-US.msi` | 2.88 MiB | `ide-shell-desktop.exe` | GitHub Releases | EV-DOC-0018 |
| Web 単体 | `ide-shell-web.exe` | 846 KB | `127.0.0.1:8123` | Cargo / セルフesia | EV-DOC-0018 |
| ソース | 8 crates | — | `cargo build --release` | GitHub main | EV-CODE-0050 |

## 2. 実行環境

| 項目 | 最小 | 推奨 | 根拠 |
|---|---|---|---|
| OS | Windows 10 1909 | Windows 11 | EV-DOC-0019 |
| WebView2 runtime | 100.0+ | 120.0+ | EV-DOC-0019 |
| PowerShell | 5.1 | 7.x | EV-DOC-0019 |
| メモリ | 4 GB | 8 GB | EV-DOC-0019 |
| ディスク | 50 MB | 100 MB | EV-DOC-0019 |

## 3. ネットワーク

| 項目 | 実態 | 根拠 |
|---|---|---|
| 待ち受け | ループバックのみ（`127.0.0.1:8123`） | EV-DOC-0018 |
| 外部通信 | なし（オフライン動作が前提） | EV-DOC-0018 |
| 認証 | 起動毎の CSPRNG トークン（`x-ide-token`）+ Host/Origin 検証 | EV-CODE-0025, EV-CODE-0027 |
| 危険 | 任意 Web ページから `fetch("http://127.0.0.1:8123/api/...")` が可能 → トークンと Origin 検証が必須 | EV-CODE-0025 |

図: `DG-NETWORK-LOCAL`。

## 4. ファイル管理と制約

| 項目 | 値 | 根拠 |
|---|---|---|
| アクセス範囲 | プロジェクトルート内のみ | EV-DOC-0004 |
| サイズ上限 | 4 MB | EV-DOC-0008 |
| エンコード | UTF-8 のみ | EV-CODE-0044 |
| 自動バックアップ | 5 分周期・10 世代 | EV-CODE-0045 |
| ロック | `editcore::file_lock` による排他 | EV-CODE-0005 |

## 5. 監視指標（文書記載）

| 指標 | 目的 | 出典 |
|---|---|---|
| CI 成功率 | 回帰の早期検出（記載 88.9%） | EV-DOC-0002 |
| テスト数 / 被覆率 | 品質の下限期（253 件 / 66.25%） | EV-DOC-0002 |
| bench ops/s | 編集性能（記載 56,971） | EV-DOC-0002 |
| 起動時間 | 体感（記載 1.5s） | EV-DOC-0013 |
| MSI サイズ | 配布（記載 2.88 MiB） | EV-DOC-0018 |

## 6. 障害時の挙動（図集の 4D で確認済みのもの）

| 事象 | 挙動 | Case |
|---|---|---|
| トークン不正 | 認証辺で阻断、下流へ進まない | C-AUTH-REJECT |
| ルート外パス | 沙箱と保管辺の二重阻断 | C-PATH-ESCAPE |
| 4MB 超 | 保管辺で阻断 | C-SIZE-LIMIT |
| 非 UTF-8 | 保管辺で阻断 | C-ENCODING-REJECT |
| powershell 不在 | 端末経路が進まない | C-TERMINAL-MISSING |
| rg 不在 | 内蔵エンジンへ回落 | C-SEARCH-FALLBACK |
| Git .non リーポジトリ | SCM が Failed 終端 | C-GIT-NOT-REPO |
| プラグイン実行超過 | Failed → 退避再試行 → Recovered | C-PLUGIN-TIMEOUT |
| WebView2 不在 | desktop の invoke が到達しない | C-E2E-CLICK-WEBVIEW2 |

## 7. 保守・廃棄

保守計画（`docs/DOC-19_maintenance_plan.md`）と廃棄計画（`docs/DOC-21_disposal_plan.md`）が
既存に存在する。本図集がこれらを更新することはしない。
廃棄の観点でDiagram 上的注意を 1 点: `editcore::auto_backup` が 10 世代を保持するため、
廃棄手順は世代数分の退避領域を考慮する必要がある（EV-CODE-0045）。

## 8. 本図集自身の運用

| 項目 | 方針 |
|---|---|
| ビルド | `node .tooling/build.mjs .`（構文門禁 + モデル鍵名門禁を通過しないと成果物を出さない） |
| 参照整合 | `node .tooling/verify-refs.mjs .`（悬空 ID があれば非 0 終了） |
| 動的検証 | `node .tooling/verify-cases.mjs .`（24 ケース実走 + 被覆率の実測回写） |
| 単体 | `node tests/unit/atlas.test.mjs` |
| E2E | `node .tooling/e2e-journeys.mjs`（file:// + システム Chrome） |
| 視覚 | `node .tooling/screenshots.mjs`（12 ショット + レイアウト観測） |
| 成果物の扱い | `index.html` / `dist/` / `qa/*.json` は**ビルド生成物**であり、既定では commit しない |
| 対象リポジトリへの影響 | **なし**。IDE1.0 の作業ツリーを変更・commit・push しない |
