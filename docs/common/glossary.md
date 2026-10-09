# 用語集 (日英中对照)

> **準拠:** IPA 共通フレーム2013 §1 / §3
> **目的:** 「同じ言葉で話す」 (CF2013 核心目的)
> **対象:** IDE1.0 IDE Shell プロジェクト文档全部

## CF2013 主要术语 (プロセス体系)

| 日本語 (CF2013) | English | 中文 | 文档中的应用 |
|---|---|---|---|
| プロセス | Process | 过程 | DOC-01 ~ DOC-21 |
| アクティビティ | Activity | 活动 | 任务集合 |
| タスク | Task | 任务 | 个别作业 |
| 注記 | Note | 备注 | 参考・例示 |
| ワークフロー | Workflow | 工作流 | CF2013 取代 |
| ソフトウェアライフサイクルプロセス (SLCP) | Software Life Cycle Process | 软件生命周期过程 | §4 引用 |
| システムライフサイクルプロセス | System Life Cycle Process | 系统生命周期过程 | §4 引用 |

## CF2013 プロセス分类 (四大类)

| 日本語 | English | 中文 | 含义 |
|---|---|---|---|
| 合意プロセス | Agreement Process | 约定过程 | 取得/供给 |
| 組織のイネーブリングプロセス | Organizational Enabling Process | 组织赋能过程 | 基础设施/项目管理/质量管理 |
| テクニカルプロセス | Technical Process | 技术过程 | 企画/要件/設計/实现/验证/移行/保守/運用/废弃 |
| ソフトウェア実装プロセス | Software Implementation Process | 软件实现过程 | 软件详细生命周期 |

## 主要役割

| 日本語 | English | 中文 | 责任 |
|---|---|---|---|
| 発注者 | Acquirer | 采购方 | システム获得与要件定义 |
| 供給者 | Supplier | 供应方 | 系统供给 |
| 開発者 | Developer | 开发者 | 设计/实现/测试 |
| 運用者 | Operator | 运维者 | 运用/保守 |
| 保守者 | Maintainer | 维护者 | 软件保守 |
| プロジェクトマネージャ | Project Manager | 项目经理 | 进度・预算・风险管理 |
| アーキテクト | Architect | 架构师 | システム/软件方式设计 |

## 文档関連术语

| 日本語 | English | 中文 | 说明 |
|---|---|---|---|
| システム要件定義書 | System Requirements Specification (SRS) | 系统需求规格书 | DOC-02 |
| システム方式設計書 | System High-Level Design (HLD) | 系统概要设计书 | DOC-03 |
| ソフトウェア要件仕様書 | Software Requirements Specification | 软件需求规格书 | DOC-04 |
| ソフトウェア方式設計書 | Software Design Description (SDD) | 软件概要设计书 | DOC-05 |
| ソフトウェア詳細設計書 | Detailed Design Description (DDD) | 软件详细设计书 | DOC-06 |
| 結合テスト仕様書 | Integration Test Specification | 集成测试规格书 | DOC-08 |
| 単体テスト仕様書 | Unit Test Specification | 单元测试规格书 | DOC-10 |
| 統合テスト報告書 | System Test Report | 系统测试报告 | DOC-11 |
| プロジェクト計画書 | Project Plan | 项目计划书 | DOC-12 |
| リスク管理表 | Risk Register | 风险管理表 | DOC-14 |
| 構成管理計画書 | Configuration Management Plan | 配置管理计划 | DOC-15 |
| 意思決定記録 | Architecture Decision Record (ADR) | 架构决策记录 | DOC-16 |

## 工程/Phase (V 字モデル)

| 日本語 | English | 中文 | IDE1.0 实际 |
|---|---|---|---|
| 構想 | Inception | 构思 | ulys-191-1, 3 |
| 企画 | Planning | 策划 | ulys-191-4 (tauri PoC) |
| 要件定義 | Requirements | 需求定义 | ulys-191-5 |
| システム方式設計 | System Design | 系统设计 | ulys-191-7 (ide-shell-protocol) |
| ソフトウェア方式設計 | Software Design | 软件设计 | ulys-191-15 (Outline) |
| 詳細設計 | Detailed Design | 详细设计 | ulys-191-30 (sakura-rs PARITY) |
| 実装 | Implementation | 实现 | ulys-191-9, 10, 30 |
| 単体テスト | Unit Test | 单元测试 | 253/253 cargo test pass |
| 結合テスト | Integration Test | 集成测试 | ulys-191-21, 22 |
| システムテスト | System Test | 系统测试 | ulys-191-31, 32 |
| 受入テスト | Acceptance Test (UAT) | 验收测试 | 56/72 Playwright e2e |
| 運用 | Operation | 运维 | tauri msi 0.1.9 / web release |
| 保守 | Maintenance | 维护 | issue tracker / patch |

## IDE1.0 固有术语

| 日本語 | English | 中文 | 说明 |
|---|---|---|---|
| プロジェクトルート | Project Root | 项目根 | `IDE_SHELL_WEB_TEST_ROOT` 环境变量 |
| 三態键位 | Tri-state keymap | 三态键位模式 | NORMAL / INSERT / VISUAL |
| 単層ペイン | Single-layer Panel | 单层面板 | Bottom Panel (Terminal/Output/Problems) |
| ファイルツリー | File Tree | 文件树 | Explorer side bar |
| アウトラインパネル | Outline Panel | 大纲面板 | sakura-rs `outline_dispatch` |
| グリッドパネル | Grid Panel | 多面板栅格 | (未实现 - 计划 DOC-19) |
| カスタムメニュー | Custom Menu | 自定义菜单 | (未实现 - 计划 DOC-15) |

## 测试层级 (4 层矩阵)

| 日本語 | English | 中文 | 数量 |
|---|---|---|---|
| 単体テスト (UT) | Unit Test | 单元测试 | 253 (Rust) |
| 結合テスト (IT) | Integration Test | 集成测试 | 31 (Rust) |
| システムテスト (ST) | System Test | 系统测试 | 26 (Python mock) + 4 (cli_smoke) |
| 受入テスト (UAT) | Acceptance Test | 验收测试 | 56/72 (Playwright) |

## コード関連术语

| 日本語 | English | 中文 | 代码位置 |
|---|---|---|---|
| ワークスペース | Workspace | 工作空间 | IDE1.0/ (Cargo.toml) |
| クレート | Crate | crate | 9 个 (ide-shell 等) |
| モジュール | Module | 模块 | ide-cli 4 个 + ide-kernel-core 4 个 |
| プラグイン | Plugin | 插件 | cluster_switch, module_switch |
| コア | Core | 核心 | ide-kernel-core |
| シェル | Shell | shell | ide-shell crate |
| プロトコル | Protocol | 协议 | ide-shell-protocol |
| Web版 | Web Version | Web 版 | ide-shell-web |
| デスクトップ版 | Desktop Version | 桌面版 | ide-shell-desktop |

## 质量相关术语

| 日本語 | English | 中文 | 实际 |
|---|---|---|---|
| カバレッジ | Coverage | 覆盖率 | 66.25% (≥90% 4 files) |
| RUSTFLAGS=-D warnings | RUSTFLAGS=-D warnings | 严格警告 | CI 强制 |
| Clippy lint | Clippy lint | Clippy 检查 | CI 强制 |
| コミットメッセージ規約 | Conventional Commits | 约定式提交 | ulys-NNN-M |
| 受け入れ基準 | Acceptance Criteria | 验收标准 | 文档中明确 |

---

**作成:** v1.0.0 (2026-10-09)
**準拠:** IPA 共通フレーム2013 §1 / §3
**参照:** [doc-template.md](doc-template.md) | [numbering-system.md](numbering-system.md)