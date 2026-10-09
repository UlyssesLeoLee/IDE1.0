# IDE1.0 IDE Shell — IPA 準拠ドキュメント体系

> **準拠:** IPA 共通フレーム2013 (Software Engineering Center, 2013/03/04)
> **適用範囲:** IDE1.0 IDE Shell 全プロジェクト (V 字モデル)
> **現状:** 2026-10-09 (Stage 6 完成 — sakura-rs bench + CI benchmark)

---

## V 字モデル図 (CF2013 §3 適用)

```
                    要件定義      詳細設計      実装      検証
                        ↓              ↓           ↓           ↓
                       DOC-02          DOC-06      DOC-07    DOC-09
要件 ───→ システム方式 ──→ ソフトウェア方式 ─→ コード ──→ システム
  ↑              ↑              ↑            ↑           ↑
[DOC-12]    [DOC-04]        [DOC-05]                  DOC-08 結合
プロジェクト   SRS-soft        SDD                          SIT
計画                                                           ↑
                                                          DOC-11
                                                          UAT
```

## 📚 ドキュメント一覧 (21 文档 + 3 共通)

### 共通ドキュメント (Task 1 完成)

| ファイル | 説明 |
|---|---|
| [doc-template.md](common/doc-template.md) | IPA 4層構造テンプレート |
| [numbering-system.md](common/numbering-system.md) | CF2013 準拠 编号規則 |
| [glossary.md](common/glossary.md) | 日英中 用語集 |

### V字模型 左辺 (要件・設計) — Task 2

| ID | 名称 | CF2013 主プロセス |
|---|---|---|
| [DOC-01](DOC-01_project_charter.md) | 企画書 (Project Charter) | 企画プロセス |
| [DOC-02](DOC-02_system_requirements.md) | システム要件定義書 (SRS) | 要件定義プロセス |
| [DOC-03](DOC-03_system_hld.md) | システム方式設計書 (HLD) | システム方式設計 |
| [DOC-04](DOC-04_software_requirements.md) | ソフトウェア要件仕様書 | ソフトウェア要件分析 |
| [DOC-05](DOC-05_software_design.md) | ソフトウェア方式設計書 (SDD) | ソフトウェア方式設計 |
| [DOC-06](DOC-06_software_detail_design.md) | ソフトウェア詳細設計書 (DDD) | ソフトウェア詳細設計 |

### V字模型 底部 (実装・結合) — Task 3

| ID | 名称 | CF2013 主プロセス |
|---|---|---|
| [DOC-07](DOC-07_implementation_report.md) | 実装報告書 | 実装プロセス |
| [DOC-08](DOC-08_integration_test_spec.md) | 結合テスト仕様書 (SIT) | システム結合 |

### V字模型 顶部・右辺 (テスト・検証) — Task 4

| ID | 名称 | CF2013 主プロセス |
|---|---|---|
| [DOC-09](DOC-09_system_test_report.md) | システムテスト報告書 | システム適格性確認テスト |
| [DOC-10](DOC-10_unit_test_spec.md) | 単体テスト仕様書 (SUT) | ソフトウェア結合 |
| [DOC-11](DOC-11_uat_report.md) | 統合テスト報告書 (UAT) | ソフトウェア適格性確認テスト |

### プロセス系 (計画・管理) — Task 5

| ID | 名称 | CF2013 主プロセス |
|---|---|---|
| [DOC-12](DOC-12_project_plan.md) | プロジェクト計画書 | プロジェクト計画 |
| [DOC-13](DOC-13_progress_report.md) | 進捗管理報告書 | プロジェクトアセスメント及び制御 |
| [DOC-14](DOC-14_risk_register.md) | リスク管理表 | リスク管理 |
| [DOC-15](DOC-15_config_mgmt_plan.md) | 構成管理計画書 | 構成管理 |
| [DOC-16](DOC-16_decision_log.md) | 意思決定記録 (ADR) | 意思決定管理 |
| [DOC-17](DOC-17_doc_index.md) | ドキュメント管理表 | 情報管理 |
| [DOC-18](DOC-18_metrics_report.md) | 測定・分析報告 | 測定 |

### ライフサイクル後段 (保守・運用・廃棄) — Task 6

| ID | 名称 | CF2013 主プロセス |
|---|---|---|
| [DOC-19](DOC-19_maintenance_plan.md) | 保守計画書 | 保守プロセス |
| [DOC-20](DOC-20_operations_manual.md) | 運用手順書 | 運用プロセス |
| [DOC-21](DOC-21_disposal_plan.md) | 廃棄計画書 | 廃棄プロセス |

## 📊 現状 (2026-10-09)

| 指标 | 値 |
|---|---|
| Crates | 9 (ide-cli, ide-kernel-core, ide-shell, ide-shell-web, ide-shell-desktop, ide-shell-protocol, sakura-rs, aci-emitter, ide-cli) |
| Cargo tests | **253/253** pass (under `RUSTFLAGS=-D warnings`) |
| CI jobs | **8/9** PASS (UAT 在 Linux 失败 — programmatic click 限制) |
| Coverage | **66.25%** (4 个 file ≥ 90%) |
| Latest commit | `c61682e feat(ulys-191-34): 重写 editor.html` |
| origin/main HEAD | `c61682e` |

## 🎯 CF2013 適用プロセス体系

- **合意プロセス** (取得/供给): DOC-01, DOC-02
- **テクニカルプロセス** (技术): DOC-03 ~ DOC-11, DOC-19 ~ DOC-21
- **プロジェクトプロセス** (项目): DOC-12, DOC-13
- **組織のイネーブリングプロセス** (组织赋能): DOC-14 ~ DOC-18

## 🔗 関連リンク

- [README.md](../README.md) — 项目主 README
- [PARITY.md](../crates/sakura-rs/PARITY.md) — sakura editor 機能对比
- [CI workflow](../.github/workflows/ci.yml) — CI 9 jobs 详细
- [IPA 共通フレーム2013](https://www.ipa.go.jp/archive/files/000027415.pdf) — 标准原文

---

**作成:** Hermes Agent (Stage 6 完了時点)
**準拠:** IPA 共通フレーム2013 §3 / §5 / §6 / §7
**最終更新:** 2026-10-09