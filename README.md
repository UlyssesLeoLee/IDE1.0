# IDE1.0 — Pure Rust AI Native CLI Development Kernel

`Cargo Workspace` の Rust crate 群で構築する、AI Native CLI カーネル。

## Workspace 構成 (MVP-1)

| Tier | Crate | 責務 |
|------|-------|------|
| 0 | `kernel-error` | 統一エラー型・コード体系 (INC-07) |
| 1 | `kernel-config` | 設定読込・環境変数統合 |
| 1 | `kernel-tracing` | 構造化ログ・Trace Context |
| 2 | `kernel-eventbus` | Event 購読・配信・永続化・Replay |
| 2 | `kernel-session` | Session ライフサイクル・権限 |
| 3 | `kernel-capability-registry` | Capability 登録・解決 |
| 3 | `kernel-cmd-bus` | Command 実行・Capability Routing |
| 4 | `kernel-core` | Microkernel 基盤集約 |
| 7 | `cli` | CLI エントリポイント |

## ビルド

```bash
cargo check --workspace        # 型検査
cargo test --workspace         # テスト
cargo build --release          # リリースビルド
```

## 詳細設計

各 DD (詳細設計書) との対応は `kernel-error/README.md` および各 crate 冒頭のコメント参照。

- DD-01: Microkernel Core (Command Bus / Event Bus / Capability Registry)
- DD-02: Session / Transaction / Buffer Engine
- DD-04: Internal API + 横断関心
- DD-13: Cursor 機能対比・極簡版方案