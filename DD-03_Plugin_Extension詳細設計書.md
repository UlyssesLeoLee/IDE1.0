# DD-03 Plugin / Extension 詳細設計書

## Plugin Manager / Plugin Loader / Plugin Sandbox / Plugin Hot Swap / Capability Provider Interface

---

## 目次

1. 文書情報
2. 目的・対象・用語
3. 上位設計との対応
4. モジュール一覧と責務分割
5. Plugin Manager 詳細設計
6. Plugin Loader 詳細設計（6.0 加载机制候选分析を含む）
7. Plugin Sandbox 詳細設計
8. Plugin Hot Swap 詳細設計
9. Capability Provider Interface 詳細設計
10. エラー体系
11. 状態遷移設計
12. Sequence Diagram 集
13. トレーサビリティ
14. 未決事項（TBD）
15. 上位設計確認事項（QA）
16. 自己審査チェックリスト

---

## 1. 文書情報

### 1.1 メタデータ

| 項目 | 値 |
|---|---|
| Document ID | DD-03 |
| Title | Plugin / Extension 詳細設計書 |
| Version | 1.0 |
| Status | Draft |
| Effective Date | 2026-09-14 |
| Author | Haiku Agent (c383a9c9-21f8-4755-bb52-dcfac3aa1dbf) |
| Related Requirements | REQ-001, AD-001, SD-001, IF-PLG-001 |
| Parent Issue | ULYS-37 |
| This Issue | ULYS-40 |

### 1.2 改訂履歴

| Version | Date | Author | Remarks |
|---|---|---|---|
| 1.0 | 2026-09-14 | Haiku | Initial draft |
| 1.1 | 2026-09-17 | M3 (MinimaxM3) | §6.0 Loading Mechanism 候補分析（dlopen/WASM/Subprocess）を追加。QA-DD03-005〜008 を §15 に追記。上位設計「禁止只选一种加载机制而不列候选」要件への対応。 |

### 1.3 配布管理

- **配置**: worktree `agent/haiku/ulys-40`
- **ファイル名**: `DD-03_Plugin_Extension詳細設計書.md`
- **格式**: Markdown
- **行数**: 目標 1,800～2,200 行
- **容量**: 目標 約 95～120 KB

---

## 2. 目的・対象・用語

### 2.1 目的

本詳細設計書は、以下の上位設計を実装レベルまで細化する：

- **基本設計（AD-001）§3, §4.2**: Plugin 総体構想、Plugin Manager / Loader / Sandbox スコープ
- **安全性設計（SD-001）§6, §7**: Plugin 権限モデル、Sandbox セキュリティ
- **IF設計（IF-PLG-001）**: Plugin Protocol、Manifest スキーマ、Capability 記述形式
- **要件定義（REQ-001）§36～50**: Plugin 生命周期、Runtime 選定基準、SDK 仕様

開発エンジニアが実装可能、テスト者が検証可能な水準まで、以下を定義：

- 各モジュール内部の処理フロー
- 入力・出力・状態遷移
- エラーハンドリングと復旧
- 並行制御・Transaction 境界
- ホットスワップの安全性保証
- Sandbox による権限隔離の仕組み

### 2.2 対象読者

1. **実装者**: Plugin Manager, Loader, Sandbox の内部実装を担当するエンジニア
2. **テスト設計者**: Plugin 関連のテストケース、テスト観点を策定する者
3. **セキュリティレビュー**: Plugin 権限隔離、Sandbox 逃脱リスクを評価する者
4. **アーキテクト**: 上位設計との整合確認、横断的影響を確認する者
5. **他 DD 担当エージェント**: DD-01, DD-02, DD-04 との接続点を確認する者

### 2.3 用語定義

| 用語 | 定義 |
|---|---|
| **Plugin** | 業務機能を提供する拡張モジュール。WASM Component またはネイティブワーカープロセスとして実行される。|
| **Plugin Manager** | Plugin の登録・検索・有効化・無効化・バージョン管理を司る内核コンポーネント。|
| **Plugin Loader** | Plugin のロード・初期化・アンロードの life cycle を管理し、2つの Runtime (WASM / Worker) を統一的に制御。|
| **Plugin Sandbox** | Plugin が実行される隔離環境。WASM は VM Sandbox、Worker は Process Sandbox。権限ポリシーの適用。|
| **Plugin Hot Swap** | 稼働中のシステムを中断せず Plugin をアップグレード・ダウングレードする機構。|
| **Capability** | Plugin が提供する能力（機能）。Command / Event / API 形式で暴露。Caller は Plugin 名でなく Capability 名を指定。|
| **Capability Registry** | 全 Plugin が登録した Capability の中央台帳（DD-01 で詳細化）。|
| **Plugin Manifest** | Plugin の静的メタデータ（TOML/JSON）。名前・バージョン・提供能力・必要能力・権限宣言を記述。|
| **Plugin Runtime** | Plugin 実行環境の種別。**Runtime A**: WASM/WASI Component。**Runtime B**: ネイティブワーカープロセス。|
| **WASM Component** | WebAssembly Component Model ベースの Plugin。Sandbox 強固、Hot Load/Unload 容易。|
| **Native Worker** | 独立した Rust プロセスで実行される Plugin。LSP、Debugger、GPU タスク向け。|
| **Process Isolation** | ワーカープロセスの崩壊が Kernel に影響しない仕組み。Watchdog + Restart + Quarantine。|
| **Plugin Context** | Plugin が activate() 時に受け取る execution context。Command Bus / Event Bus / Registry 等への access を持つ。|
| **Manifest Validation** | Manifest ファイルの構文・セマンティクス検査。スキーマ照合、権限フィールド検証。|
| **Plugin State** | Plugin の内部生命周期状態。DISCOVERED → INSTALLED → ... → ACTIVE など（§3.2 参照）。|

---

## 3. 上位設計との対応

### 3.1 要件・基本設計の参照構造

```
REQ-001 (§36～50 Plugin 総体)
  ↓
  ├→ AD-001 §4.1 "Core は機構のみ、能力は Plugin"
  │  (→ MOD-PM-001 / MOD-PL-001 の責務)
  │
  ├→ AD-001 §2.2～2.3 "Plugin Manager / Loader スコープ"
  │  (→ MOD-PM-001 / MOD-PL-001 の module boundary)
  │
  ├→ SD-001 §6 "Plugin 権限モデル（Zero Trust + RBAC）"
  │  (→ MOD-PS-001 permission enforcement)
  │
  ├→ SD-001 §7 "Process Isolation + Sandbox"
  │  (→ MOD-PS-001 Worker isolation, panic handling)
  │
  ├→ IF-PLG-001 "Plugin Protocol + Manifest Schema"
  │  (→ MOD-PM-001 manifest parsing, IF-PLG-XXX)
  │
  └→ REQ-001 §47 "Plugin 安全モデル（deny-by-default）"
     (→ MOD-PS-001 permission validation)
```

### 3.2 上位設計との整合チェック一覧

| チェック項目 | AD-001 参照 | DD-03 対応個所 |
|---|---|---|
| Plugin Manager の初期化タイミング | §4.1 | §5.3 M-PM-001 initialize() |
| Capability Registry との連携 | §2.2 | §9.4, §9.5 |
| Plugin 依存関係の解決 | §4.2 | §6.4, §6.5 P-PL-002 |
| Worker 監視と crash isolation | §4.3 | §7.5 P-PS-003 |
| Hot Swap 時の reference 迁移 | §4.4 | §8.4, §8.5 |
| Error code 体系の統一 | §5 | §10 ERR-PLG-XXX |
| Permission validation の順序 | SD-001 §6.2 | §7.4 |
| Audit logging 範囲 | SD-001 §7.5 | §9.6, §5.8 |

---

## 4. モジュール一覧と責務分割

### 4.1 モジュール全体図

```
┌─────────────────────────────────────────────────────────┐
│                    Kernel Core                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │         Command Bus / Event Bus / Registry       │  │
│  │         (DD-01 で詳細化)                         │  │
│  └──────────────────────────────────────────────────┘  │
│                         ▲                               │
│                         │ IF-PLG-001                    │
│  ┌──────────────────────┴──────────────────────────┐   │
│  │                                                  │   │
│  │  ┌───────────────────┐    ┌────────────────┐   │   │
│  │  │  MOD-PM-001       │    │   MOD-CPI-001  │   │   │
│  │  │ Plugin Manager    │◄──►│ Capability     │   │   │
│  │  │ (Registry入出)    │    │ Provider IF    │   │   │
│  │  └─────────┬─────────┘    └────────────────┘   │   │
│  │            │                                    │   │
│  │  ┌─────────▼─────────┐                         │   │
│  │  │  MOD-PL-001       │                         │   │
│  │  │ Plugin Loader     │                         │   │
│  │  │ (Lifecycle管理)   │                         │   │
│  │  └─────────┬─────────┘                         │   │
│  │            │                                    │   │
│  │  ┌─────────▼──────────────────┐                │   │
│  │  │  MOD-PS-001                │                │   │
│  │  │ Plugin Sandbox             │                │   │
│  │  │ (Permission + Isolation)   │                │   │
│  │  └─────────┬──────────────────┘                │   │
│  │            │                                    │   │
│  │  ┌─────────▼──────────────────┐                │   │
│  │  │  MOD-PHS-001               │                │   │
│  │  │ Plugin Hot Swap            │                │   │
│  │  │ (State Transition Safety)  │                │   │
│  │  └────────────────────────────┘                │   │
│  │                                                │   │
│  └────────────────────────────────────────────────┘   │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │         WASM Runtime    │    Worker Runtime      │  │
│  │    (wasmtime engine)    │    (子プロセス管理)    │  │
│  └──────────────────────────────────────────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 4.2 モジュール責務表

| Module ID | 日本語名 | 責務 | 入出力 | 依存先 |
|---|---|---|---|---|
| **MOD-PM-001** | Plugin Manager | ・Plugin の登録・検索・削除<br>・バージョン管理<br>・メタデータ解析（Manifest）<br>・Loader との連携 | 入: Plugin Manifest<br>出: Plugin Metadata | Registry, Loader, Sandbox |
| **MOD-PL-001** | Plugin Loader | ・WASM/Worker Runtime の選択<br>・Load・Initialize・Unload<br>・生命周期状態遷移<br>・Runtime Engine 制御 | 入: Plugin Metadata<br>出: Plugin Instance | PM, Sandbox, Runtime |
| **MOD-PS-001** | Plugin Sandbox | ・Permission Validation<br>・System Call Filter (WASM)<br>・Capability Whitelist (Worker)<br>・Crash Isolation (Watchdog)<br>・Resource Limit Enforcement | 入: Request + Policy<br>出: Allow/Deny | Permission DB, Audit |
| **MOD-PHS-001** | Plugin Hot Swap | ・State Snapshot<br>・Reference Migration<br>・Old Instance Graceful Shutdown<br>・Failure Rollback<br>・In-flight Request Draining | 入: Old / New Plugin<br>出: Swap Result | Loader, Sandbox, Registry |
| **MOD-CPI-001** | Capability Provider IF | ・Plugin による Capability 暴露 Protocol<br>・Command/Event/API routing<br>・Response Serialization<br>・Error Mapping<br>・Trace ID 伝播 | 入: Caller Request<br>出: Capability Response | Command Bus, Event Bus |

### 4.3 モジュール初期化順序

```
Kernel startup
  │
  ├─ 1. MOD-PM-001 initialize()
  │  (Plugin Registry 初期化)
  │
  ├─ 2. MOD-PS-001 initialize()
  │  (Permission Policy Load、Sandbox 準備)
  │
  ├─ 3. MOD-PL-001 initialize()
  │  (WASM Runtime Engine + Worker Daemon pool 初期化)
  │
  ├─ 4. MOD-PHS-001 initialize()
  │  (Hot Swap state machine 初期化)
  │
  ├─ 5. MOD-CPI-001 initialize()
  │  (Capability routing table 初期化)
  │
  └─ 6. Plugin auto-load phase
     (manifest scan → load → activate → register)
```

---

## 5. Plugin Manager (MOD-PM-001) 詳細設計

### 5.1 モジュール概要

**Plugin Manager** は以下を責務とする：

1. 全 Plugin の meta data registry
2. Plugin の検索・バージョン管理
3. Manifest の parsing / validation
4. Plugin Loader との連携（install → load → activate flow）
5. 権限宣言の解析・Sandbox への引き渡し
6. Audit logging（全 Plugin 操作の記録）

### 5.2 主要クラス設計

#### CLS-PM-001: PluginManager

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PM-001 |
| **名称** | PluginManager |
| **職責** | Plugin の登録・検索・削除・バージョン管理・Manifest parsing |
| **Lifecycle** | Singleton。Kernel startup で 1 回 initialize、shutdown で cleanup。|
| **Dependency** | PluginRegistry, PluginLoader, PluginSandbox, PluginManifest, AuditLogger |
| **State Field** | - registry: HashMap<PluginId, PluginMetadata><br>- version_map: HashMap<PluginId, Vec<Version>><br>- manifest_cache: HashMap<PluginId, Manifest><br>- loader: Arc<PluginLoader><br>- sandbox: Arc<PluginSandbox> |
| **Public Method** | initialize(), register(), unregister(), find_by_id(), find_by_capability(), list_all(), get_version_history(), upgrade(), downgrade() |
| **Exception** | PluginNotFound, ManifestInvalid, DependencyNotMet, VersionConflict |

#### CLS-PM-002: PluginMetadata

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PM-002 |
| **名称** | PluginMetadata |
| **職責** | Plugin の静的メタデータを保持。Manifest から生成。|
| **Lifecycle** | Immutable。Plugin 登録時に生成、アンロード時に破棄。|
| **Field/State** | - id: PluginId<br>- name: String<br>- version: SemVer<br>- runtime_type: RuntimeType (WASM or Worker)<br>- provided_capabilities: Vec<CapabilityId><br>- required_capabilities: Vec<CapabilityId><br>- permissions: PermissionSet<br>- manifest_hash: Hash<br>- install_date: Timestamp<br>- last_updated: Timestamp |
| **Interface** | version_compatible(other: &SemVer) → bool, dependencies_satisfied(registry: &Registry) → Result |

#### CLS-PM-003: ManifestParser

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PM-003 |
| **名称** | ManifestParser |
| **職責** | TOML/JSON Manifest ファイルをパースし、検証する。|
| **Public Method** | parse_toml(), parse_json(), validate(), to_metadata() |
| **Exception** | ParseError, ValidationError, SchemaViolation |

### 5.3 主要メソッド設計

#### M-PM-001: initialize()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PM-001 |
| **名称** | PluginManager::initialize() |
| **目的** | Plugin Manager を初期化。既存 Plugin の Manifest をスキャンし registry をロード。|
| **Caller** | Kernel startup 時のみ呼び出し |
| **入出力** | 入: config: PluginConfig<br>出: Result<(), ManagerError> |
| **前提条件** | ・Plugin directory が存在すること<br>・Permission policy が事前に load されていること |
| **処理フロー** | P-PM-001 |
| **後提条件** | ・registry が全 Plugin metadata で満たされる<br>・audit log に initialize event が記録される |
| **例外** | PluginDirNotFound, ParseError, ValidationError |
| **Side Effect** | ファイルシステム読み込み、audit log 追記 |

#### M-PM-002: register()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PM-002 |
| **名称** | PluginManager::register() |
| **目的** | 新規 Plugin を registry に登録。load → activate の手前まで。|
| **Caller** | admin console, API endpoint |
| **入出力** | 入: manifest_path: Path<br>出: Result<PluginId, RegisterError> |
| **前提条件** | ・Manifest ファイルが存在・読み取り可能<br>・Plugin ID が registry に未登録<br>・Permission に plugin.install 権限を持つ |
| **処理フロー** | P-PM-002 |
| **後提条件** | ・registry に新規 entry が追加<br>・audit log に register event 記録<br>・Plugin は INSTALLED 状態 |
| **例外** | ManifestNotFound, ParseError, DuplicateId, PermissionDenied |
| **Transaction** | BEGIN: registry lock COMMIT: on success ROLLBACK: on error |

#### M-PM-003: find_by_capability()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PM-003 |
| **名称** | PluginManager::find_by_capability() |
| **目的** | 与えられた Capability ID を提供する Plugin を検索。複数ヒット時はバージョン優先度順。|
| **Caller** | Command Bus, Capability Registry |
| **入出力** | 入: capability_id: CapabilityId<br>出: Result<Vec<PluginMetadata>, SearchError> |
| **前提条件** | registry が initialized |
| **処理フロー** | P-PM-003 |
| **後提条件** | 戻り値は version 降順（最新が先） |
| **例外** | CapabilityNotFound, RegistryCorrupted |

### 5.4 処理フロー詳細

#### P-PM-001: initialize() 処理フロー

```
1. config から plugin_dir を取得
2. plugin_dir が存在するか確認
   IF 存在しない
      RETURN error: PluginDirNotFound
   ENDIF
3. plugin_dir 配下の全 *.toml / *.json をスキャン
4. 各 manifest ファイルに対して：
   4.1 ManifestParser::parse_toml() または parse_json()
   4.2 スキーマ検証 (IF-PLG-001 参照)
       IF validation failed
          LOG warn "Manifest parse error: {path}"
          continue to next
       ENDIF
   4.3 ManifestParser::to_metadata() で PluginMetadata 生成
   4.4 Permission policy から permission set を取得
   4.5 version_map に entry を追加
   4.6 registry に entry を追加
5. 全 plugin に対して依存関係チェック（§5.5 参照）
   IF unresolved dependency detected
      LOG warn "Unresolved dependency for {plugin_id}: {capability}"
      Mark as QUARANTINED
   ENDIF
6. audit logger に "PluginManager.initialize completed: {count} plugins loaded" を記録
7. RETURN success
```

#### P-PM-002: register() 処理フロー

```
1. manifest_path が存在するか確認
   IF 存在しない
      RETURN error: ManifestNotFound
   ENDIF
2. Permission check: caller に plugin.install 権限があるか
   IF 権限なし
      RETURN error: PermissionDenied
   ENDIF
3. ManifestParser::parse_toml() または parse_json()
   IF parse failed
      RETURN error: ParseError
   ENDIF
4. ManifestParser::validate()
   IF validation failed
      RETURN error: ValidationError(details)
   ENDIF
5. Metadata 生成 → plugin_id 取得
6. registry lock 取得 (M_PM_REGISTRY_LOCK)
7. registry に plugin_id が既に存在するか確認
   IF exists
      release lock
      RETURN error: DuplicateId
   ENDIF
8. registry に新 entry 追加
9. version_map に新 entry 追加
10. Permission policy へ permission set を登録（§7 参照）
11. registry lock 解放
12. audit logger へ "register event" を記録（who, when, plugin_id, version）
13. Plugin state を INSTALLED に遷移
14. RETURN success: plugin_id
```

#### P-PM-003: find_by_capability() 処理フロー

```
1. registry に RwLock::read() で access
2. registry 内の全 entry を iterate
3. 各 entry の provided_capabilities を検查
   IF entry.provided_capabilities.contains(capability_id)
      candidate に追加
   ENDIF
4. candidates を version 降順（新しい順）に sort
5. sort 後の list を RETURN
6. IF no candidate found
      RETURN error: CapabilityNotFound
   ENDIF
```

### 5.5 依存関係解決

Plugin A が "requires: capability.B" を宣言した場合、Plugin Manager は以下を検証：

```
1. Registry 内で capability.B を provide する plugin が存在するか確認
2. 存在しない場合：
   - Plugin A を QUARANTINED 状態に遷移
   - audit log に警告を記録
3. 複数 plugin が同じ capability を provide する場合：
   - Version 優先度順に resolve（実装時は Loader で呼び出しシーケンスが決定）
4. 循環依存の検出：
   - DFS で依存グラフをチェック
   - 循環検出時は ERR-PLG-004 を raise
```

### 5.6 バージョン互換性チェック

```
semver_compatible(v1: &Version, v2: &Version) -> bool {
   // v1 が v2 と互換性があるか（v1.major == v2.major）
   v1.major == v2.major && v1.minor >= v2.minor
}
```

### 5.7 Manifest Validation Checklist

| 検査項目 | 検査内容 | 失敗時 Error Code |
|---|---|---|
| 必須フィールド | [plugin].id, name, version, api が存在 | ERR-PLG-001 |
| ID 形式 | plugin.id が `[a-z0-9.-]+` に合致 | ERR-PLG-002 |
| Version 形式 | version が SemVer に合致 | ERR-PLG-003 |
| Runtime 種別 | runtime.type が "wasm" または "worker" | ERR-PLG-005 |
| Capability ID | [provides]/[requires] capability ID が valid format | ERR-PLG-006 |
| Permission フィールド | [permissions] の各キーが許可リストに存在 | ERR-PLG-007 |
| Manifest signature | (SD-001 連携) manifest hash と署名を検証 | ERR-PLG-008 |

### 5.8 Audit Logging

Plugin Manager の全公開メソッド呼び出しは audit log に記録する：

```
log_level: INFO (成功時) / WARN (警告) / ERROR (失敗時)
fields:
  - timestamp
  - method_name
  - caller_identity (who)
  - plugin_id
  - operation (register / unregister / upgrade / etc)
  - result (success / failure reason)
  - affected_capabilities (list)

例:
  2026-09-14T10:23:45.123Z | INFO | register 
  | actor: admin@system 
  | plugin_id: language.rust 
  | version: 1.0.0 
  | capabilities: [language.definition, language.references]
```

---

## 6. Plugin Loader (MOD-PL-001) 詳細設計

### 6.0 加载机制候选分析 (Loading Mechanism Candidate Analysis)

基本设计 AD-001 §4.2 已确定「Plugin Loader 必须支持多种加载机制，并依据 Plugin Manifest 的 `[runtime] type` 选择具体机制」。本节列出候选、推荐与待确认事项。

#### 6.0.1 候选机制一览

| 候选 | 机制 | 进程模型 | 接口方式 | 沙箱强度 | 性能 | 启动开销 | 语言限制 |
|---|---|---|---|---|---|---|---|
| **A. 动态库加载 (Dynamic Library / dlopen)** | 同进程内 `dlopen`/`LoadLibrary` 动态库直接加载 Rust crate | 同一进程（kernel 进程） | 静态 FFI 符号绑定 (extern "C" trait) | 弱 (Zero Trust 上仅依赖语言隔离，无 OS 边界) | 高 (no IPC) | 极低 (μs 级) | 必须 Rust ABI 兼容 |
| **B. WASM Component (wasmtime)** | WASM Component Model + WASI Preview2 | 同进程内 wasmtime engine (独立 VM) | WIT-defined interface + Component Model invoke | 强 (WASI capability / linear memory / fuel) | 中 (JIT/AOT, ~10x native) | 中 (10〜100ms, 含 compilation) | 多语言 (Rust/C/Go → wasm32-wasi) |
| **C. 子进程 + JSON-RPC (Subprocess + JSON-RPC over pipe)** | 子进程 spawn + stdin/stdout pipe (length-prefixed) | 独立 OS 进程 | JSON-RPC 2.0 over framed pipe / vsock | 强 (OS-level 隔离 + seccomp/AppArmor 可加) | 中 (IPC 开销 0.1〜1ms) | 高 (50〜200ms, 进程启动) | 完全语言无关 |
| D. (备选) Shared memory + lock-free ring buffer | 共享内存 + 轮询 | 独立进程 | 共享内存协议 | 中 (无 OS 隔离，仅 memory 隔离) | 极高 (ns 级) | 中 | 语言无关 | 

#### 6.0.2 候选详细比较

**A. 动态库加载 (dlopen)**

- **优点**
  - 性能最高：no IPC、no serialization、native FFI 调用
  - 启动最快：μs 级
  - 调试简单：同一进程，可直接 gdb attach
- **缺点**
  - **违反 Zero Trust**：与 kernel 同进程 → 一个 plugin panic / memory bug 可使整个 kernel crash
  - **违反 SD-001 §6 Zero Trust**：未跨越 OS 信任边界
  - 静态类型绑定 → Plugin 更新流程必须 ABI 兼容 (Rust 不保证 stable ABI)
  - 不能 host 不同语言 Plugin
- **风险**：违反上位安全设计 SD-001 §6.1（process-level isolation requirement）

**B. WASM Component (wasmtime)**

- **优点**
  - 强沙箱：linear memory 隔离、WASI capability 限制、fuel-based execution limit
  - 多语言：Rust/C/Go/AssemblyScript 可编译到 wasm32-wasi
  - Hot swap 友好：WASM module 替换无需重启进程
  - 性能可接受：JIT 后 ~native 的 50〜80%
- **缺点**
  - 启动有 compilation 开销 (10〜100ms，可通过 AOT cache 降至 < 5ms)
  - 无法直接访问 OS 特定功能 (需 WASI 扩展或 host function)
  - 调试比 native 复杂 (DWARF support 已成熟但 setup 成本仍存在)
- **风险**：fuel limit 与真实 wall-clock 时间不完全对齐 → 需额外 timeout 保护

**C. 子进程 + JSON-RPC**

- **优点**
  - **最强 OS 隔离**：独立进程 → kernel crash 影响隔离
  - 语言完全无关：Python/NodeJS/Rust/Java/Go plugin 均可
  - 可叠加 OS-level sandbox (seccomp/AppArmor/cgroups)
  - 故障隔离：worker panic 仅影响自身
- **缺点**
  - IPC 开销：JSON serialize/parse 0.1〜1ms per call
  - 启动慢：50〜200ms per spawn（可优化：prefork pool）
  - 进程数膨胀：N plugins → N processes（资源消耗）
  - Worker 与 kernel 的状态共享需要 protocol（state snapshot 复杂度上升）
- **风险**：process spawn storm → 需 prefork pool 与 rate limit

**D. Shared memory + ring buffer（备选，未实施）**

- 性能最高但实现复杂度极高、调试困难、安全模型不清。**未采纳**为本期候选。

#### 6.0.3 推荐方案

**主推：B (WASM) + C (Subprocess + JSON-RPC) 双轨制**

| Plugin 类型 | 加载机制 | 理由 |
|---|---|---|
| **轻量 plugin** (无 OS 资源依赖、纯计算) | **B. WASM Component** | 启动快、隔离强、跨语言、HOT SWAP 简单 |
| **需要 OS 资源 / 外部语言 / 性能宽松** | **C. Subprocess + JSON-RPC** | 强 OS 隔离 + 完全语言无关 |
| A. dlopen | **不推荐作为主路径** | 违反 SD-001 §6 Zero Trust；保留为未来特殊场景的 escape hatch，需经 Security Lead 批准 |

**不采用 A 的核心理由**：基本设计 AD-001 §4.2 + 安全性设计书 SD-001 §6.1 已明确规定「Plugin 必须在独立执行环境运行，不与 kernel 同进程」，dlopen 不满足此要求。仅在特殊高性能内嵌场景（且经 Security Lead 批准）作为 escape hatch 保留。

#### 6.0.4 推荐理由

1. **满足上位设计约束**：WASM 同进程内 VM 隔离 + Subprocess 跨 OS 进程，均满足 SD-001 §6.1 的隔离要求
2. **覆盖所有 plugin 类型**：轻量 → WASM；重依赖 / 多语言 → Subprocess
3. **Hot Swap 友好**：WASM module 可热替换；Subprocess 通过 graceful drain + state handoff 支持热插拔
4. **故障隔离**：WASM panic 由 wasmtime trap 捕获；Subprocess crash 由 WorkerMonitor 重启
5. **测试性**：可独立测试 WASM runtime 与 Worker IPC，不需要依赖 kernel 完整环境

#### 6.0.5 待确认事项

| QA ID | 内容 | 责任方 | 状态 |
|---|---|---|---|
| **QA-DD03-005** | WASM AOT cache 策略（持久化路径、失效条件、首次启动 vs 缓存命中开销） | Runtime Team | 【TBD】 |
| **QA-DD03-006** | Subprocess worker prefork pool 规模（默认 N、动态伸缩策略） | Runtime Team | 【TBD】 |
| **QA-DD03-007** | A. dlopen escape hatch 的使用条件与审批流程 | Security Lead + Arch Lead | 【TBD】 |
| **QA-DD03-008** | WASM host function 边界（plugin 可调用的 kernel API 集合，需与 §9 Capability Provider 整合） | Arch Lead | 【TBD】 |

### 6.1 モジュール概要

**Plugin Loader** は以下を責務とする：

1. Plugin の life cycle 管理（Discovered → Active → Unloaded）
2. WASM Runtime (wasmtime) と Native Worker Runtime の統一的制御
3. Plugin Instance の生成・初期化・破棄
4. 依存関係の解決と initialization 順序の決定
5. Plugin の有効化・無効化
6. アップグレード・ダウングレード時の old instance 管理

### 6.2 主要クラス設計

#### CLS-PL-001: PluginLoader

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PL-001 |
| **名称** | PluginLoader |
| **職責** | Plugin の load/initialize/activate/suspend/unload を統括。Runtime selection も含む。|
| **Dependency** | PluginManager, PluginSandbox, WasmRuntime, WorkerRuntime, PluginContext |
| **State Field** | - loaded_plugins: HashMap<PluginId, LoadedPlugin><br>- runtime_engines: (WasmRuntime, WorkerRuntime)<br>- initialization_order: Vec<PluginId><br>- dependency_graph: DAG<PluginId> |
| **Public Method** | initialize(), load(), initialize_plugin(), activate(), suspend(), unload(), upgrade_plugin() |

#### CLS-PL-002: LoadedPlugin

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PL-002 |
| **名称** | LoadedPlugin |
| **職責** | Loader に登録された Plugin instance を表現。|
| **Field/State** | - id: PluginId<br>- metadata: PluginMetadata<br>- state: PluginState<br>- runtime_type: RuntimeType<br>- instance: PluginInstance (Union型)<br>- context: PluginContext<br>- load_time: Timestamp<br>- last_called: Timestamp |
| **Instance 型** | enum PluginInstance { Wasm(WasmModule), Worker(WorkerHandle) } |

#### CLS-PL-003: PluginInstance

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PL-003 |
| **名称** | PluginInstance (trait) |
| **職責** | Plugin 実行インスタンスの統一インターフェース。|
| **Method** | async fn initialize() → Result, async fn execute_command() → Result, async fn deactivate() → Result |

### 6.3 Plugin 生命周期状態機

```
                DISCOVERED
                    │
                    │ install
                    ▼
        ┌─────► INSTALLED
        │       (manifest loaded)
        │           │
        │           │ verify
        │           ▼
        │       VERIFIED
        │           │
        │           │ load
        │           ▼
        │       LOADED
        │       (binary loaded)
        │           │
        │           │ initialize
        │           ▼
        │   INITIALIZING
        │       (async init)
        │           │
        │           ├─ success ─► ACTIVE
        │           │
        │           └─ failure ─► FAILED (→ restart logic)
        │                         │
        │                         └──► QUARANTINED (unrecoverable)
        │
        │     during ACTIVE:
        │     ┌──────────────────┬──────────────────┐
        │     │                  │                  │
        │     │ suspend          │ hot_swap_begin   │
        │     ▼                  ▼                  ▼
        │  SUSPENDED          HOT_SWAP_IN_PROGRESS
        │     │                  │
        │     │ resume           │ hot_swap_commit
        │     └──────►ACTIVE◄────┘
        │
        │     unload:
        │     from ACTIVE (or SUSPENDED):
        │     ┌──────────────────┐
        │     │ stop_accepting   │
        │     │ drain in_flight  │
        │     ▼
        │  DRAINING
        │     │
        │     │ shutdown_complete
        │     ▼
        │  UNLOADING
        │     │
        │     │ cleanup
        │     ▼
        │  UNLOADED
        │
        └─────────────────────────────────────────
```

### 6.4 Runtime Selection Logic

Plugin Manifest の `[runtime] type` フィールドに基づき、以下で Runtime を選定：

```
IF runtime.type == "wasm":
   candidates = WasmRuntime engines matching plugin.api version
   IF candidates.empty():
      RETURN error: ERR-PLG-009 (no compatible wasm engine)
   ENDIF
   selected = candidates[0]  // newest version first
   RETURN selected

ELSE IF runtime.type == "worker":
   IF plugin requires LSP / GPU / heavy computation:
      selected = WorkerRuntime with resource constraints
      RETURN selected
   ELSE:
      LOG warn "Plugin {id} runtime=worker but lightweight; consider WASM"
      still proceed with worker
   ENDIF

ELSE:
   RETURN error: ERR-PLG-005 (invalid runtime type)
ENDIF
```

### 6.5 Initialization Order Determination (P-PL-001)

```
1. PluginManager から全 Plugin metadata を取得
2. dependency graph を構築 (provided_capabilities / required_capabilities)
3. Topological sort → initialization_order list
4. Cycle detection：
   IF cycle detected:
      identify plugins in cycle
      LOG error "Circular dependency: {plugin_list}"
      RETURN error: ERR-PLG-004
   ENDIF
5. 依存関係未満足の plugin をマーク (QUARANTINED)
6. initialization_order に従って load → initialize を実行
   FOR each plugin_id IN initialization_order:
      TRY:
         M-PL-001 load(plugin_id)
         M-PL-002 initialize_plugin(plugin_id)
      ON failure:
         LOG error, mark as FAILED
         continue to next (don't block later plugins)
7. initialized plugin を ACTIVE に遷移
8. audit log に "initialization completed: {n} plugins active" を記録
```

### 6.6 主要メソッド設計

#### M-PL-001: load()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PL-001 |
| **名称** | PluginLoader::load() |
| **目的** | Plugin binary を load し、instance を生成。|
| **入出力** | 入: plugin_id: PluginId<br>出: Result<LoadedPlugin, LoadError> |
| **処理フロー** | P-PL-002 |
| **前提条件** | Plugin が VERIFIED 状態 |
| **後提条件** | Plugin が LOADED 状態に遷移 |

#### M-PL-002: initialize_plugin()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PL-002 |
| **名称** | PluginLoader::initialize_plugin() |
| **目的** | Plugin の async initialization を実行し、activate 前準備。|
| **入出力** | 入: plugin_id: PluginId, context: PluginContext<br>出: Result<(), InitError> |
| **処理フロー** | P-PL-003 |
| **後提条件** | Plugin が ACTIVE 状態に遷移、Capability Registry に登録 |

#### M-PL-003: suspend()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PL-003 |
| **名称** | PluginLoader::suspend() |
| **目的** | ACTIVE plugin を SUSPENDED へ遷移。Hot Swap や maintenance 時に使用。|
| **入出力** | 入: plugin_id: PluginId<br>出: Result<(), SuspendError> |
| **前提条件** | Plugin が ACTIVE 状態 |
| **処理フロー** | P-PL-004 |
| **後提条件** | Plugin が SUSPENDED 状態；in-flight requests は timeout前に完了 |

#### M-PL-004: unload()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PL-004 |
| **名称** | PluginLoader::unload() |
| **目的** | Plugin を完全に unload し、resource を解放。Safe Unload (REQ-001 §42) に準拠。|
| **入出力** | 入: plugin_id: PluginId, timeout_ms: u32<br>出: Result<(), UnloadError> |
| **処理フロー** | P-PL-005 |
| **後提条件** | Plugin が UNLOADED 状態；全 resource 解放 |

### 6.7 処理フロー詳細

#### P-PL-002: load() 処理フロー

```
1. PluginManager から plugin metadata 取得
2. metadata.runtime_type に従い分岐：
   IF WASM:
      2.1 WasmRuntime::load_module(plugin_path)
      2.2 module instance 生成
      2.3 WASM sandbox setup （MOD-PS-001 との連携）
   ELSE IF Worker:
      2.1 WorkerRuntime::spawn_worker(plugin_path, config)
      2.2 worker process 起動
      2.3 Process isolation setup （MOD-PS-001 との連携）
   ENDIF
3. plugin_id を key として loaded_plugins に entry 追加
4. Plugin state を LOADED に遷移
5. audit log に "load event" を記録
6. RETURN success
```

#### P-PL-003: initialize_plugin() 処理フロー

```
1. loaded_plugins から LoadedPlugin を retrieve
   IF not found:
      RETURN error: PluginNotFound
   ENDIF
2. PluginContext を生成
   - command_bus: Arc<CommandBus> reference
   - event_bus: Arc<EventBus> reference
   - registry: Arc<CapabilityRegistry> reference
   - sandbox: Arc<PluginSandbox> reference
   - logger: Arc<PluginLogger> reference
   - trace_id: generate new UUID for this initialization
3. Plugin state を INITIALIZING に遷移
4. Plugin instance へ context を pass し、async initialization を起動：
   IF WASM:
      wasm_module.initialize(context).await
   ELSE IF Worker:
      worker.send_init_request(context).await
   ENDIF
5. timeout 設定：
   - max_init_time_ms = metadata.max_init_ms か 【TBD】秒（§14 参照）
   - set_timeout(max_init_time_ms)
6. initialization wait：
   result = await plugin.init_result with timeout
   IF timeout:
      LOG error, mark plugin as FAILED
      RETURN error: ERR-PLG-010 (initialization timeout)
   ENDIF
   IF error:
      LOG error with error details
      mark plugin as FAILED
      RETURN error: InitError(details)
   ENDIF
7. Capability Registry へ provided capabilities を登録
   FOR each cap_id IN metadata.provided_capabilities:
      registry.register_capability(cap_id, plugin_id, version)
8. audit logger へ "initialize event" を記録
9. Plugin state を ACTIVE に遷移
10. audit logger へ "plugin activated" を記録
11. RETURN success
```

#### P-PL-004: suspend() 処理フロー

```
1. loaded_plugins から plugin を retrieve
   IF state != ACTIVE:
      RETURN error: InvalidStateTransition
   ENDIF
2. Plugin state を SUSPENDED に遷移
3. Capability Registry へ通知：
   registry.set_availability(plugin_id, UNAVAILABLE)
4. 新規 request の受け入れを停止（Sandbox で check）
5. in-flight requests の完了待ち：
   - monitoring_thread で active request count を observe
   - timeout_ms 秒内に全 request の完了を待つ
   IF timeout:
      force kill in-flight requests
      LOG warn "Plugin {id} requests forcefully terminated"
   ENDIF
6. Plugin instance の suspend() method を call（あれば）
7. audit log に "suspend event" を記録
8. RETURN success
```

#### P-PL-005: unload() 処理フロー

実装順序は REQ-001 §42 に準拠：

```
1. Plugin state が ACTIVE or SUSPENDED か確認
   IF other state:
      RETURN error: InvalidStateTransition
   ENDIF
2. M-PL-003 suspend() 呼び出し（まだ SUSPENDED でない場合）
3. Plugin state を DRAINING に遷移
4. Command Bus から plugin の registered commands をすべて注销
   registry.unregister_commands(plugin_id)
5. Capability Registry から plugin の capabilities をすべて注销
   registry.unregister_capabilities(plugin_id)
6. Event Bus から plugin の event subscriptions をすべて注销
   event_bus.unsubscribe_all(plugin_id)
7. Plugin resource handles をすべて close
   IF WASM:
      wasm_module.cleanup()
   ELSE IF Worker:
      worker.shutdown_graceful(timeout_ms)
      IF graceful shutdown timeout:
         worker.kill_forcefully()
      ENDIF
   ENDIF
8. 全 inflight references が消滅したか確認（§6.8 参照）
   result = wait_for_zero_references(plugin_id, timeout_ms)
   IF timeout:
      LOG warn, proceed anyway (graceful degradation)
   ENDIF
9. loaded_plugins から entry を削除
10. Plugin state を UNLOADED に遷移
11. audit log に "unload event" を記録
12. RETURN success
```

### 6.8 Reference Tracking（Hot Swap / Unload Safe 保証）

Plugin instance への reference が最後に消滅するまで、instance の破棄を遅延：

```
struct PluginReference {
   plugin_id: PluginId,
   created_at: Timestamp,
   trace_id: TraceId,
   callee: String,  // 呼び出し元の module/function
}

在线 plugins に対して weak reference を使用：
   Arc<RwLock<LoadedPlugin>> → Weak<RwLock<LoadedPlugin>>

unload / hot_swap 時：
   1. 新規 reference 生成を禁止
   2. 既存 strong reference count を監視
      strong_count == 1 (loader self only) になるまで待機
   3. weak reference の upgrade に fail するように sentinel 設定
```

---

## 7. Plugin Sandbox (MOD-PS-001) 詳細設計

### 7.1 モジュール概要

**Plugin Sandbox** は以下を責務とする：

1. Permission validation と enforcement
2. WASM system call filtering
3. Worker process isolation と resource limiting
4. Worker crash detection と recovery
5. Audit logging（全操作の監査）

### 7.2 主要クラス設計

#### CLS-PS-001: PluginSandbox

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PS-001 |
| **名称** | PluginSandbox |
| **職責** | Plugin execution の isolation と permission enforcement。|
| **Dependency** | PermissionPolicy, AuditLogger, WorkerMonitor |
| **State Field** | - permission_db: HashMap<PluginId, PermissionSet><br>- worker_monitors: HashMap<WorkerPid, WorkerMonitor><br>- audit_logger: Arc<AuditLogger> |
| **Public Method** | initialize(), validate_permission(), setup_wasm_sandbox(), setup_worker_isolation(), monitor_worker(), handle_worker_crash() |

#### CLS-PS-002: PermissionSet

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PS-002 |
| **名称** | PermissionSet |
| **職責** | Plugin に付与される権限集合。Manifest から derive。|
| **Field/State** | - plugin_id: PluginId<br>- workspace_read: bool<br>- workspace_write: bool<br>- filesystem_read: PathSet<br>- filesystem_write: PathSet<br>- process_spawn: ProcessWhitelist<br>- network: NetworkPolicy<br>- git: GitPolicy<br>- ai_local: bool<br>- ai_remote: bool<br>- secrets: bool<br>- custom_capabilities: Vec<String> |

#### CLS-PS-003: WorkerMonitor

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PS-003 |
| **名称** | WorkerMonitor |
| **職責** | 単一ワーカープロセスの health monitoring。Crash detection / restart。|
| **State Field** | - worker_pid: ProcessId<br>- plugin_id: PluginId<br>- state: WorkerState<br>- last_heartbeat: Timestamp<br>- restart_count: u32<br>- memory_usage: u64<br>- cpu_percent: f32 |
| **Public Method** | start_monitoring(), send_heartbeat(), detect_crash(), request_restart() |

### 7.3 Permission Model （SD-001 §6 と連携）

#### 権限レベル

```
LEVEL 1: Deny by Default
  - Plugin は明示的に Manifest で declare しない限り、すべて DENY

LEVEL 2: Capability-based Authorization
  - 各権限（workspace.read, filesystem.write など）に対して、
    Plugin の Manifest での宣言と Policy DB の照合で許可を判定

LEVEL 3: Fine-grained Path/Resource ACL
  - filesystem_read, filesystem_write は Path whitelist で細粒度制御
  - process_spawn は Process whitelist で制御
```

#### Permission Validation Flow（P-PS-001）

```
1. Plugin が capability を invoke する際、Sandbox::validate_permission(
      plugin_id, capability, resource_type) を call
2. Permission DB から plugin の PermissionSet を取得
   IF not found:
      RETURN DENY (ERR-PLG-011)
   ENDIF
3. capability に応じて分岐：
   IF capability == "workspace.read":
      IF permission.workspace_read == true:
         RETURN ALLOW
      ELSE:
         RETURN DENY (ERR-PLG-012)
      ENDIF
   ELSE IF capability == "filesystem.write":
      path = request.resource_path
      IF path IN permission.filesystem_write:
         RETURN ALLOW
      ELSE:
         RETURN DENY (ERR-PLG-013)
      ENDIF
   ELSE IF capability == "process.spawn":
      process_name = request.process_name
      IF process_name IN permission.process_spawn:
         check_resource_limits() → ALLOW or DENY (ERR-PLG-014)
      ELSE:
         RETURN DENY (ERR-PLG-015)
      ENDIF
   ... (他の capability 種別)
4. audit logger へ permission check 結果を記録
   - timestamp, plugin_id, capability, result (ALLOW/DENY), trace_id
5. RETURN result
```

### 7.4 WASM Sandbox Setup（P-PS-002）

```
1. WasmRuntime の linker に対して、許可済み system call set をのみ export
   - 許可：basic I/O, memory allocation, time functions
   - 拒否：network, filesystem (except whitelist), arbitrary process creation
2. Plugin manifest で [permissions] 権限を指定した場合、
   wasmtime linker に selective import を追加：
   IF filesystem_read permission:
      export limited file open for read-only
   ENDIF
   IF network permission:
      export socket creation (restricted port set)
   ENDIF
3. WASM module の memory は isolated heap：
   - max size は plugin manifest で指定（デフォルト 128 MB, 【TBD】）
   - Kernel heap とは分離
4. WASM module の execution time に制限：
   max_execution_ms = manifest.max_execution_ms || 【TBD】秒
   Timeout 時は module を interrupt
5. audit log へ "wasm sandbox setup" を記録
```

### 7.5 Worker Process Isolation（P-PS-003）

```
1. Worker process spawn 時：
   cgroup / resource limit setup:
   - memory limit: manifest.memory_limit_mb (default 512 MB, 【TBD】)
   - CPU limit: manifest.cpu_limit_percent (default 50%, 【TBD】)
   - open file limit: 1024 (【TBD】)
2. WorkerMonitor を起動：
   - heartbeat interval: 5 秒（【TBD】）
   - timeout: 30 秒 missed heartbeat で crash と判定（【TBD】）
3. Worker process の stdout/stderr をキャプチャ：
   - plugin 固有ログファイルに redirect
   - Kernel 主プロセスの stderr と混濁させない
4. Signal handling：
   - Worker が SIGTERM 受信時：graceful shutdown（タイムアウト 10 秒後 SIGKILL）
   - Worker が予期外に exit：M-PS-003 handle_worker_crash() を invoke
5. audit log へ "worker isolation setup" を記録
```

### 7.6 Worker Crash Isolation & Recovery（P-PS-004, P-PS-005）

#### Crash Detection（P-PS-004）

```
1. WorkerMonitor::start_monitoring() で monitoring thread 起動
2. 定期的に worker process の alive status をチェック：
   IF process exited unexpectedly:
      detect_crash() を invoke
   ENDIF
3. detect_crash() 時の処理：
   3.1 plugin state を FAILED に遷移
   3.2 Plugin capability を Capability Registry から一時的に remove
   3.3 audit log へ "worker crash" を記録
   3.4 restart_count を increment
4. RETURN crash detected
```

#### Automatic Restart（P-PS-005）

```
1. Crash 検出後、restart_count に応じた restart strategy：
   max_restart_count = manifest.max_restarts || 3 (【TBD】)
   IF restart_count <= max_restart_count:
      backoff_ms = initial_backoff_ms * (2 ^ (restart_count - 1))
      backoff_ms = min(backoff_ms, max_backoff_ms)  // 最大値制限（【TBD】秒）
      wait(backoff_ms)
      retry_worker_start()
      IF success:
         restart_count reset to 0
         plugin state を ACTIVE に restore
         RETURN success
      ELSE:
         restart_count increment, go to next retry
      ENDIF
   ENDIF
2. max_restart_count に達した場合：
   2.1 plugin state を QUARANTINED に遷移（自動復旧不可）
   2.2 audit log へ "plugin quarantined after max restarts" を記録
   2.3 管理者に alert を発行
   2.4 RETURN failure, require manual intervention
3. Restart 失敗の reason を audit log に記録
```

### 7.7 主要メソッド設計

#### M-PS-001: validate_permission()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PS-001 |
| **名称** | PluginSandbox::validate_permission() |
| **目的** | Plugin の capability 呼び出しを permission check。|
| **入出力** | 入: plugin_id, capability_id, resource_type, trace_id<br>出: Result<(), PermissionError> |
| **処理フロー** | P-PS-001 |
| **例外** | PermissionDenied, PluginQuarantined |

#### M-PS-002: setup_worker_isolation()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PS-002 |
| **名称** | PluginSandbox::setup_worker_isolation() |
| **目的** | Worker process をリソース制限と isolation で setup。|
| **入出力** | 入: plugin_id, worker_config<br>出: Result<WorkerMonitor, SetupError> |
| **処理フロー** | P-PS-003 |

#### M-PS-003: handle_worker_crash()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PS-003 |
| **名称** | PluginSandbox::handle_worker_crash() |
| **目的** | Worker crash を検出し、automatic restart か quarantine を判定。|
| **入出力** | 入: plugin_id<br>出: Result<(), RecoveryError> |
| **処理フロー** | P-PS-004, P-PS-005 |
| **後提条件** | Plugin が ACTIVE か QUARANTINED かのいずれかに遷移 |

---

## 8. Plugin Hot Swap (MOD-PHS-001) 詳細設計

### 8.1 モジュール概要

**Plugin Hot Swap** は以下を責務とする：

1. In-flight requests を draining しながら plugin をアップグレード/ダウングレード
2. 古い plugin instance への reference を新しい instance へ migrate
3. 状態の保存・復元（state snapshot）
4. Failure 時の rollback
5. Dual-stack management（old/new instance 同時実行期間の管理）

### 8.2 Hot Swap の基本流れ

```
Current State: Plugin A v1.0 (ACTIVE)
│
│ trigger: upgrade to v1.1
│
├─ 1. Download / Verify v1.1
├─ 2. Load v1.1 into separate instance
├─ 3. Initialize v1.1 (async)
├─ 4. v1.0 → SUSPENDED, stop accepting new requests
├─ 5. Wait for v1.0 in-flight requests to drain
├─ 6. Take state snapshot from v1.0
├─ 7. Inject state snapshot to v1.1
├─ 8. Redirect all references: v1.0 → v1.1
├─ 9. v1.1 → ACTIVE
├─ 10. v1.0 graceful shutdown
├─ 11. Unload v1.0
│
└─ Result: Plugin A v1.1 (ACTIVE)

On failure at any step:
│ restore v1.0 → ACTIVE (rollback)
│ cleanup v1.1 instance
└─ RETURN error
```

### 8.3 主要クラス設計

#### CLS-PHS-001: PluginHotSwapper

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PHS-001 |
| **名称** | PluginHotSwapper |
| **職責** | Hot swap 操作を統括。upgrade / downgrade を実行。|
| **Dependency** | PluginLoader, PluginManager, PluginSandbox, PluginStateSnapshot |
| **Public Method** | upgrade(), downgrade(), check_compatibility(), perform_swap() |

#### CLS-PHS-002: SwapTransaction

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PHS-002 |
| **名称** | SwapTransaction |
| **職責** | 単一の swap 操作を transaction として管理。Rollback の可能性。|
| **State Field** | - id: TransactionId<br>- plugin_id: PluginId<br>- old_version: Version<br>- new_version: Version<br>- old_instance: LoadedPlugin<br>- new_instance: LoadedPlugin<br>- state_snapshot: PluginStateSnapshot<br>- status: SwapStatus (InProgress / Committed / RolledBack) |

#### CLS-PHS-003: PluginStateSnapshot

| 項目 | 内容 |
|---|---|
| **ID** | CLS-PHS-003 |
| **名称** | PluginStateSnapshot |
| **職責** | Plugin の状態を save/restore する。|
| **Method** | save_state(old_instance) → Vec<u8>, restore_state(new_instance, snapshot) → Result |

### 8.4 Compatibility Check（P-PHS-001）

```
upgrade v1.0 → v1.1 を実行する前に、compatibility を verify：

1. Manifest の api version が compatible か確認
   old_api = metadata_v1_0.api
   new_api = metadata_v1_1.api
   IF old_api != new_api:
      LOG error "API version mismatch: {old_api} → {new_api}"
      RETURN error: ERR-PLG-016 (incompatible upgrade)
   ENDIF

2. provided_capabilities が互換か確認
   old_caps = metadata_v1_0.provided_capabilities
   new_caps = metadata_v1_1.provided_capabilities
   IF new_caps の一部が old_caps にない：
      LOG warn "New capabilities in upgrade: {new_caps - old_caps}"
      (これは許可、新機能追加)
   ENDIF
   IF old_caps の一部が new_caps にない：
      LOG error "Removed capabilities: {old_caps - new_caps}"
      (依存している他 plugin の確認が必要)
      依存 plugin がない確認後のみ proceed
   ENDIF

3. Permission の追加が問題ないか確認
   (policy update が必要な場合は管理者確認 → 【TBD】)

4. RETURN compatibility check result
```

### 8.5 State Snapshot & Restore（P-PHS-002, P-PHS-003）

#### Save State（P-PHS-002）

```
1. old_instance の state を serialize
   IF WASM:
      wasm_module.export_state() を call
      → binary state blob を取得
   ELSE IF Worker:
      worker.send_export_state_request().await
      wait for response with state blob
   ENDIF

2. state blob を compress
   compressed = zstd::compress(state_blob)

3. state metadata を embed
   snapshot = {
      plugin_id: ...,
      old_version: ...,
      timestamp: now(),
      checksum: sha256(compressed),
      compressed_blob: compressed,
   }

4. snapshot を memory または temp file に保持
   (hot swap transaction duration: ~秒)
```

#### Restore State（P-PHS-003）

```
1. new_instance の initialization が完了した後、
   snapshot を new_instance へ inject

2. IF WASM:
      wasm_module.import_state(snapshot.compressed_blob)
   ELSE IF Worker:
      worker.send_import_state_request(snapshot).await
   ENDIF

3. new_instance の state が old_instance と identical か検証
   IF checksum mismatch:
      LOG error "State restore checksum mismatch"
      RETURN error: ERR-PLG-017 (state restore failed)
   ENDIF

4. RETURN success
```

### 8.6 Reference Migration（P-PHS-004）

```
1. 全 active reference を old_instance から新 instance へ redirect

2. 実装方式 A: Weak Reference + Guard
   old_instance に sentinel を設定
   Guard::upgrade() が fail → caller は新 instance を request

3. 実装方式 B: Atomic Reference Swap
   global registry の plugin_id → instance mapping を atomic swap
   arc.swap() で参照を切り替え

【TBD】: reference migration の最終方式を確定（実装性と performance を考慮）

4. redirect が完了したことを確認
   → old_instance への strong reference count が 0 になるまで待機
```

### 8.7 主要メソッド設計

#### M-PHS-001: upgrade()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PHS-001 |
| **名称** | PluginHotSwapper::upgrade() |
| **目的** | Plugin を新バージョンへアップグレード（ダウンタイムなし）。|
| **入出力** | 入: plugin_id, new_version<br>出: Result<(), UpgradeError> |
| **処理フロー** | P-PHS-005 |
| **前提条件** | 古い plugin が ACTIVE 状態 |
| **後提条件** | 新 plugin が ACTIVE、古い plugin が UNLOADED |

#### M-PHS-002: downgrade()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-PHS-002 |
| **名称** | PluginHotSwapper::downgrade() |
| **目的** | Plugin を前バージョンへダウングレード（失敗時 rollback）。|
| **入出力** | 入: plugin_id, target_version<br>出: Result<(), DowngradeError> |
| **処理フロー** | P-PHS-006 |

### 8.8 処理フロー詳細

#### P-PHS-005: upgrade() 完全フロー

```
Phase 1: Preparation
  1. old_instance = loaded_plugins[plugin_id] (state: ACTIVE)
  2. PluginManager から new_version の metadata を取得
  3. M-PHS-001 check_compatibility(old_version, new_version)
     IF incompatible:
        RETURN error: ERR-PLG-016
     ENDIF
  4. PluginLoader::load(plugin_id, new_version)
     new_instance 生成、state: LOADED に遷移
  5. TransactionId を生成

Phase 2: Initialization
  6. PluginLoader::initialize_plugin(new_instance)
     new_instance を INITIALIZING → ACTIVE に遷移
  7. IF error:
        cleanup new_instance
        RETURN error: ERR-PLG-018 (init failed)
     ENDIF

Phase 3: Draining & Snapshot
  8. old_instance state を SUSPENDED に遷移
  9. M-PL-003 suspend() で in-flight requests を drain
     timeout: 30 秒（【TBD】）
     IF timeout:
        suspend anyway (graceful degradation)
     ENDIF
  10. M-PHS-002 save_state(old_instance)
      → PluginStateSnapshot を生成
      IF error:
         restore old_instance to ACTIVE
         cleanup new_instance
         RETURN error: ERR-PLG-019 (snapshot failed)
      ENDIF

Phase 4: State Injection & Redirect
  11. M-PHS-003 restore_state(new_instance, snapshot)
      state を new_instance へ inject
      IF error:
         restore old_instance to ACTIVE
         cleanup new_instance
         RETURN error: ERR-PLG-017
      ENDIF
  12. M-PHS-004 migrate_references(old_instance → new_instance)
      ✓ Weak reference upgrade → new_instance へ redirect
      ✓ registry mapping を swap
      in-flight requests がすべて新 instance を使用するまで待機
      timeout: 10 秒（【TBD】）
  13. IF reference migration fails:
         restore old_instance to ACTIVE
         cleanup new_instance
         RETURN error: ERR-PLG-020 (reference migration failed)
      ENDIF

Phase 5: Activation & Cleanup
  14. new_instance state を ACTIVE に遷移
  15. Capability Registry に notify
      registry.set_availability(plugin_id, AVAILABLE, new_version)
  16. M-PL-004 unload(old_instance, timeout=5s)
      old_instance を UNLOADED に遷移
      resource cleanup
  17. audit log へ "upgrade completed" を記録
  18. TransactionId / old_version を archive（rollback history）
  19. RETURN success

Rollback Handling (any phase failure):
  20. IF Phase 2-5 でエラー:
         restore old_instance to ACTIVE
         try cleanup new_instance
         audit log へ "upgrade rolled back" を記録
         alert 発行
         RETURN error: ERR-PLG-XXX
      ENDIF
```

#### P-PHS-006: downgrade() フロー

downgrade() は upgrade() とほぼ同じ、ただし target_version が古いことを確認：

```
1. check_compatibility() で target_version が old_version より古いことを verify
   IF not:
      RETURN error: ERR-PLG-021 (target not older version)
   ENDIF
2. 以降は upgrade() と同じ flow を実行
3. version rollback history をもとに restore
```

---

## 9. Capability Provider Interface (MOD-CPI-001) 詳細設計

### 9.1 モジュール概要

**Capability Provider Interface** は以下を責務とする：

1. Plugin が業務能力（Capability）を Command / Event / API 形式で暴露するプロトコル
2. Caller からの request を plugin へ routing
3. Plugin response を caller へ map-back
4. Error handling と response serialization
5. Trace ID propagation （observability）

### 9.2 主要概念

#### Capability の3つの形式

| 形式 | 説明 | 使用例 |
|---|---|---|
| **Command** | request-response 型。Caller は capability 名を指定し、argument を送信。Plugin は response を返す。timeout あり。| language.definition, workspace.search |
| **Event** | publish-subscribe 型。Plugin が event を emit、複数 subscriber が listen。| file.changed, config.updated |
| **API** | REST-like HTTP API。Plugin が HTTP server として expose。| lsp://localhost:9999/... |

### 9.3 主要クラス設計

#### CLS-CPI-001: CapabilityProvider

| 項目 | 内容 |
|---|---|
| **ID** | CLS-CPI-001 |
| **名称** | CapabilityProvider |
| **職責** | Plugin が implement する interface。Capability を提供するための method を定義。|
| **Method** | async fn handle_command(request: CapabilityRequest) → Result<CapabilityResponse>, async fn publish_event(event: CapabilityEvent) |

#### CLS-CPI-002: CapabilityRequest

| 項目 | 内容 |
|---|---|
| **ID** | CLS-CPI-002 |
| **名称** | CapabilityRequest |
| **職責** | Capability invoke request の標準形式。|
| **Field** | - capability_id: String<br>- request_id: UUID<br>- trace_id: TraceId<br>- correlation_id: CorrelationId<br>- arguments: serde_json::Value<br>- metadata: HashMap<String, String> |

#### CLS-CPI-003: CapabilityResponse

| 項目 | 内容 |
|---|---|
| **ID** | CLS-CPI-003 |
| **名称** | CapabilityResponse |
| **職責** | Capability invoke response の標準形式。|
| **Field** | - request_id: UUID<br>- status: ResponseStatus (OK / Error)<br>- data: serde_json::Value<br>- error: Option<CapabilityError><br>- diagnostics: Option<Diagnostics> |

### 9.4 Capability Routing & Invocation（P-CPI-001）

```
Caller が Capability を invoke する際の flow：

1. Caller (Command Bus または REST API client) が CapabilityRequest を生成
   - capability_id を指定（plugin 名ではなく）
   - arguments を JSON serialize
   - trace_id / correlation_id を generate / carry forward

2. Capability Registry へ capability_id を lookup
   providers = registry.find_providers(capability_id)
   IF providers.empty():
      RETURN error: ERR-PLG-022 (capability not found)
   ENDIF

3. Version-based selection
   provider = providers[0]  // newest version first
   IF provider.state != ACTIVE:
      RETURN error: ERR-PLG-023 (provider not available)
   ENDIF

4. Permission check（MOD-PS-001 連携）
   sandbox.validate_permission(provider.plugin_id, capability_id)
   IF denied:
      RETURN error: ERR-PLG-012
   ENDIF

5. Request を plugin へ forward
   IF WASM:
      wasm_module.invoke(capability_request)
   ELSE IF Worker:
      worker.send_request(capability_request).await
   ENDIF

6. Response を待機（timeout あり）
   timeout_ms = manifest.max_execution_ms || 【TBD】
   response = await with_timeout(timeout_ms, plugin.wait_response())
   IF timeout:
      RETURN error: ERR-PLG-024 (capability timeout)
   ENDIF

7. Response を deserialize
   IF parse error:
      RETURN error: ERR-PLG-025 (response malformed)
   ENDIF

8. Trace ID / Correlation ID を response に embed して caller へ return

9. audit log へ capability invocation を記録
   - timestamp, caller_id, capability_id, provider_id, result, duration_ms
```

### 9.5 Trace ID Propagation（P-CPI-002）

```
Trace ID chain:

Client Request
    │
    ├─ trace_id: "abc123" (generate or inherit from header)
    │
    ├─ → Command Bus
    │  └─ trace_id: "abc123"
    │
    ├─ → Capability Registry lookup
    │  └─ trace_id: "abc123"
    │
    ├─ → Permission validation
    │  └─ trace_id: "abc123"
    │
    ├─ → Plugin invocation (Capability)
    │  └─ trace_id: "abc123"
    │
    ├─ → [Plugin internal operations]
    │  └─ all logs embed trace_id: "abc123"
    │
    ├─ → Response serialize
    │  └─ trace_id: "abc123"
    │
    └─ → Client

Correlation ID（複数 request 関連）:
  request_1: trace_id="abc123", correlation_id="group-xyz"
  request_2: trace_id="def456", correlation_id="group-xyz"
  request_3: trace_id="ghi789", correlation_id="group-xyz"
  → audit log / observability で "group-xyz" でフィルタ可能
```

### 9.6 Error Mapping（P-CPI-003）

Plugin が raise した error を system-wide error code へ map：

```
IF plugin returns error:
   IF error type == "ValidationError":
      map to ERR-PLG-026
   ELSE IF error type == "PermissionError":
      map to ERR-PLG-012
   ELSE IF error type == "TimeoutError":
      map to ERR-PLG-024
   ELSE IF error type == "ResourceExhausted":
      map to ERR-PLG-027
   ELSE IF error type == "InternalError":
      map to ERR-PLG-028
   ELSE:
      map to ERR-PLG-029 (unknown plugin error)
   ENDIF

Preserve original plugin error details in diagnostics field:
   {
      status: Error,
      error: {
         code: "ERR-PLG-026",
         message: "Validation failed",
      },
      diagnostics: {
         original_error: plugin.error_message,
         plugin_id: plugin_id,
         trace_id: trace_id,
      }
   }
```

### 9.7 主要メソッド設計

#### M-CPI-001: invoke_capability()

| 項目 | 内容 |
|---|---|
| **Method ID** | M-CPI-001 |
| **名称** | CapabilityProvider::invoke_capability() |
| **目的** | 指定された Capability を plugin invoke。|
| **入出力** | 入: request: CapabilityRequest<br>出: Result<CapabilityResponse, CapabilityError> |
| **処理フロー** | P-CPI-001, P-CPI-002, P-CPI-003 |

---

## 10. エラー体系

### 10.1 Plugin エラーコード定義

| Code | 範囲 | 説明 | 対応 Module | Recovery |
|---|---|---|---|---|
| **ERR-PLG-001** | Manifest | 必須フィールド不在 | MOD-PM-001 | manual fix manifest |
| **ERR-PLG-002** | Manifest | Plugin ID 形式不正 | MOD-PM-001 | manual fix |
| **ERR-PLG-003** | Manifest | Version format 不正 | MOD-PM-001 | manual fix |
| **ERR-PLG-004** | Dependency | 循環依存 | MOD-PM-001 | redesign dependency |
| **ERR-PLG-005** | Runtime | Runtime type invalid | MOD-PL-001 | manual fix manifest |
| **ERR-PLG-006** | Manifest | Capability ID format invalid | MOD-PM-001 | manual fix |
| **ERR-PLG-007** | Manifest | Permission field invalid | MOD-PM-001 | manual fix |
| **ERR-PLG-008** | Security | Manifest signature invalid | MOD-PM-001 | re-sign manifest |
| **ERR-PLG-009** | Runtime | No compatible WASM engine | MOD-PL-001 | update engine |
| **ERR-PLG-010** | Initialization | Plugin init timeout | MOD-PL-001 | increase timeout, debug plugin |
| **ERR-PLG-011** | Permission | Permission not found | MOD-PS-001 | update permission DB |
| **ERR-PLG-012** | Permission | Permission denied | MOD-PS-001 | update policy, escalate |
| **ERR-PLG-013** | Permission | Filesystem write denied | MOD-PS-001 | whitelist path |
| **ERR-PLG-014** | Resource | Resource limit exceeded | MOD-PS-001 | increase limit or optimize plugin |
| **ERR-PLG-015** | Permission | Process spawn denied | MOD-PS-001 | whitelist process |
| **ERR-PLG-016** | Hot Swap | Incompatible upgrade | MOD-PHS-001 | manual version selection |
| **ERR-PLG-017** | Hot Swap | State restore failed | MOD-PHS-001 | rollback |
| **ERR-PLG-018** | Hot Swap | New plugin init failed | MOD-PHS-001 | rollback |
| **ERR-PLG-019** | Hot Swap | State snapshot failed | MOD-PHS-001 | rollback |
| **ERR-PLG-020** | Hot Swap | Reference migration failed | MOD-PHS-001 | rollback |
| **ERR-PLG-021** | Hot Swap | Not older version | MOD-PHS-001 | use correct version |
| **ERR-PLG-022** | Capability | Capability not found | MOD-CPI-001 | enable plugin, install plugin |
| **ERR-PLG-023** | Capability | Provider unavailable | MOD-CPI-001 | wait for plugin active |
| **ERR-PLG-024** | Capability | Capability timeout | MOD-CPI-001 | increase timeout, optimize plugin |
| **ERR-PLG-025** | Capability | Response malformed | MOD-CPI-001 | fix plugin response serialization |
| **ERR-PLG-026** | Capability | Validation error | MOD-CPI-001 | fix request, debug plugin |
| **ERR-PLG-027** | Resource | Resource exhausted | MOD-PS-001 | increase limit, optimize plugin |
| **ERR-PLG-028** | Capability | Plugin internal error | MOD-CPI-001 | debug plugin |
| **ERR-PLG-029** | Capability | Unknown plugin error | MOD-CPI-001 | debug plugin |

### 10.2 エラーレスポンス形式

```json
{
  "error": {
    "code": "ERR-PLG-012",
    "message": "Permission denied: workspace.write",
    "timestamp": "2026-09-14T10:30:45.123Z",
    "trace_id": "abc123def456",
    "plugin_id": "language.rust",
    "request_id": "req-xyz789"
  },
  "diagnostics": {
    "permission_required": "workspace.write",
    "policy_tag": "plugin:language.rust",
    "http_status": 403
  }
}
```

---

## 11. 状態遷移設計

### 11.1 Plugin 生命周期状態機

```
                    START
                      │
                      ▼
        ┌──────────────────────────┐
        │     DISCOVERED           │
        │ (manifest on disk)       │
        └────────┬─────────────────┘
                 │ register
                 ▼
        ┌──────────────────────────┐
        │     INSTALLED            │
        │ (metadata loaded)        │
        └────────┬─────────────────┘
                 │ verify
                 ▼
        ┌──────────────────────────┐
        │     VERIFIED             │
        │ (manifest validated)     │
        └────────┬─────────────────┘
                 │ load
                 ▼
        ┌──────────────────────────┐
        │     LOADED               │
        │ (binary/module loaded)   │
        └────────┬─────────────────┘
                 │ initialize
                 ▼
        ┌──────────────────────────┐
        │   INITIALIZING           │
        │ (async init in progress) │
        └────┬───────────────────┬─┘
             │                   │
        success              failure/timeout
             │                   │
             ▼                   ▼
        ┌──────────────────┐  ┌──────────────────┐
        │     ACTIVE       │  │     FAILED       │
        │ (ready to serve) │  │ (init failed)    │
        └────┬────────┬────┘  └────┬─────────────┘
             │        │             │
         suspend  hot_swap_begin    │ (restart retry)
             │        │             │
             ▼        ▼             ▼
        ┌──────────────────────────────────────┐
        │      SUSPENDED / HOT_SWAP_IN_PROGRESS│
        │   (not serving requests)             │
        └─────┬──────────────┬──────┬──────────┘
              │              │      │
           resume      hot_swap_  automatic
              │        commit   restart
              │              │    │
              └──────┬───────┘    │
                     │            │
                     ▼            ▼
                  ACTIVE    (success/failure)
                            │
                            ▼
                        QUARANTINED
                     (unrecoverable)

Unload from ACTIVE/SUSPENDED:
        │ unload
        ▼
    ┌──────────────────┐
    │     DRAINING     │
    │ (wait for inflight)
    └────────┬─────────┘
             │
             ▼
    ┌──────────────────┐
    │   UNLOADING      │
    │ (cleanup)        │
    └────────┬─────────┘
             │
             ▼
    ┌──────────────────┐
    │   UNLOADED       │
    │ (removed)        │
    └──────────────────┘
```

### 11.2 状態遷移テーブル

| Current State | Event | Guard | Next State | Action |
|---|---|---|---|---|
| DISCOVERED | register | manifest valid | INSTALLED | create metadata, audit log |
| INSTALLED | verify | schema OK | VERIFIED | validate signature |
| VERIFIED | load | dependency met | LOADED | load binary/module |
| LOADED | initialize | resource available | INITIALIZING | invoke async init |
| INITIALIZING | init_success | - | ACTIVE | register capability, audit log |
| INITIALIZING | init_timeout | - | FAILED | log error, start retry |
| ACTIVE | suspend | - | SUSPENDED | drain requests, audit log |
| SUSPENDED | resume | - | ACTIVE | resume serving, audit log |
| ACTIVE | hot_swap_begin | new version available | HOT_SWAP_IN_PROGRESS | load new, snapshot state |
| HOT_SWAP_IN_PROGRESS | hot_swap_commit | state OK | ACTIVE (new) | redirect ref, unload old |
| HOT_SWAP_IN_PROGRESS | hot_swap_abort | error | ACTIVE (old) | cleanup new, resume old |
| ACTIVE / SUSPENDED | unload | - | DRAINING | stop accept, drain requests |
| DRAINING | drain_complete | all requests finished | UNLOADING | cleanup resource |
| UNLOADING | cleanup_complete | all refs zero | UNLOADED | remove from registry |
| FAILED | retry | < max_restart | LOADED | (or INITIALIZING) |
| FAILED | max_restart_exceeded | - | QUARANTINED | alert admin |
| ANY | crash | recovery impossible | QUARANTINED | alert admin, manual intervention |

---

## 12. Sequence Diagram 集

### 12.1 通常系：Plugin Load & Activate

```
Admin/CLI
    │
    ├─ register command
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ PluginManager::register(manifest_path)                  │
│  ├─ parse manifest                                      │
│  ├─ validate schema                                     │
│  ├─ register metadata in registry                       │
│  └─ state: INSTALLED                                    │
└────────────────┬────────────────────────────────────────┘
                 │
                 ├─ load command
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ PluginLoader::load(plugin_id)                           │
│  ├─ select runtime (WASM / Worker)                      │
│  ├─ load binary/module                                  │
│  └─ state: LOADED                                       │
└────────────────┬────────────────────────────────────────┘
                 │
                 ├─ initialize command
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ PluginLoader::initialize_plugin(plugin_id, context)     │
│  ├─ create PluginContext                                │
│  ├─ state: INITIALIZING                                 │
│  ├─ invoke plugin.initialize(context)                   │
│  │  (async, with timeout)                               │
│  ├─ register to Capability Registry                     │
│  └─ state: ACTIVE                                       │
└────────────────┬────────────────────────────────────────┘
                 │
                 └─ [Plugin ready to serve]
```

### 12.2 Permission Check Flow

```
Command Bus
    │
    ├─ invoke capability
    │  (capability_id: "language.definition")
    │
    ▼
┌──────────────────────────────────────┐
│ CapabilityRegistry::lookup()          │
│  ├─ find provider                     │
│  └─ return: plugin_id, version        │
└────────────────┬─────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────┐
│ PluginSandbox::validate_permission()  │
│  ├─ get PermissionSet(plugin_id)     │
│  ├─ check "language.read" allowed?   │
│  │  IF allowed:                       │
│  │    continue                        │
│  │  IF denied:                        │
│  │    RETURN ERR-PLG-012              │
│  └─ audit log "permission check"      │
└────────────────┬─────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────┐
│ PluginSandbox::call_plugin()          │
│  ├─ IF WASM:                          │
│  │    invoke wasm function            │
│  │  IF Worker:                        │
│  │    send IPC request to worker      │
│  └─ wait response (with timeout)      │
└────────────────┬─────────────────────┘
                 │
                 ▼
         [Response to caller]
```

### 12.3 Worker Crash & Restart

```
WorkerMonitor
    │
    ├─ [periodic heartbeat check]
    │
    ▼
   Worker process alive?
    │ NO (crash detected)
    │
    ▼
┌──────────────────────────────────┐
│ PluginSandbox::handle_worker_crash()│
│  ├─ plugin state: ACTIVE → FAILED   │
│  ├─ Capability Registry: unavailable│
│  ├─ restart_count++                 │
│  └─ audit log "crash detected"      │
└────────────────┬─────────────────────┘
                 │
                 ├─ IF restart_count <= max:
                 │    wait backoff time
                 │    respawn worker
                 │    IF success:
                 │      state: ACTIVE
                 │      restart_count reset
                 │    IF fail:
                 │      state: FAILED
                 │      continue retry
                 │
                 ├─ IF restart_count > max:
                 │    state: QUARANTINED
                 │    alert admin
                 │    require manual intervention
                 │
                 └─ audit log "recovery {success/exhausted}"
```

### 12.4 Hot Swap: Upgrade Flow

```
Admin: trigger upgrade Plugin A v1.0 → v1.1
    │
    ▼
┌────────────────────────────────────┐
│ PluginHotSwapper::upgrade()        │
│ Phase 1: Preparation               │
│  ├─ check_compatibility()          │
│  ├─ load new version               │
│  └─ initialize new version         │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│ Phase 2: Draining                  │
│  ├─ suspend old instance           │
│  ├─ drain in-flight requests       │
│  └─ wait for completion (timeout)  │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│ Phase 3: State Transition          │
│  ├─ save_state(old)                │
│  ├─ restore_state(new, snapshot)   │
│  └─ verify state checksum          │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│ Phase 4: Reference Migration       │
│  ├─ redirect Arc/Weak refs         │
│  ├─ atomic Registry swap           │
│  └─ wait for ref migration done    │
└────────────────┬───────────────────┘
                 │
                 ▼
┌────────────────────────────────────┐
│ Phase 5: Cleanup                   │
│  ├─ new instance: ACTIVE           │
│  ├─ unload old instance            │
│  └─ audit log "upgrade completed"  │
└────────────────────────────────────┘

On error at any phase:
    │ rollback
    ▼
├─ restore old instance to ACTIVE
├─ cleanup new instance
├─ audit log "upgrade rolled back"
└─ alert admin
```

---

## 13. トレーサビリティ

### 13.1 DD → BD → REQ 追跡可能性

| DD ID | BD ID | REQ §  | 説明 |
|---|---|---|---|
| MOD-PM-001 | BD-PM-001 | §36, §43 | Plugin Manager (metadata registry) |
| MOD-PL-001 | BD-PL-001 | §36, §38, §39 | Plugin Loader (lifecycle, runtime selection) |
| MOD-PS-001 | BD-PS-001 | §40, §47 | Plugin Sandbox (isolation, permission) |
| MOD-PHS-001 | BD-PHS-001 | §36 (upgrade/downgrade) | Plugin Hot Swap |
| MOD-CPI-001 | BD-CPI-001 | §45, §46 | Capability Provider IF |

### 13.2 逆向き（REQ → DD）トレース

| REQ § | 内容 | DD 対応 | 実装対象 |
|---|---|---|---|
| 36 | Plugin 総体設計 | §4 (モジュール全体) | all modules |
| 37 | 不採用: Rust dylib | MOD-PL-001 §6.4 | Runtime selection logic |
| 38 | WASM Runtime | MOD-PL-001 §6.2, P-PL-002 | WASM engine integration |
| 39 | Native Worker | MOD-PL-001 §6.2, P-PL-002 | Worker spawn + IPC |
| 40 | Worker Crash Isolation | MOD-PS-001 §7.5, P-PS-004/005 | Watchdog + restart |
| 41 | Plugin Life Cycle | §11 (state machine) | state transitions |
| 42 | Safe Unload | MOD-PL-001 P-PL-005 | drain → cleanup |
| 43 | Manifest | MOD-PM-001 §5.2-3, CLS-PM-003 | manifest parsing |
| 44 | Dependency Rules | MOD-PM-001 §5.5 | dependency resolution |
| 45 | Plugin Protocol | MOD-CPI-001 §9.2-3 | request/response format |
| 46 | Plugin SDK | MOD-CPI-001 CLS-CPI-001 | CapabilityProvider trait |
| 47 | Security Model | MOD-PS-001 §7.3-4, P-PS-001 | permission validation |

---

## 14. 未決事項（TBD）

### 14.1 TBD 一覧表

| TBD ID | 内容 | 影響範囲 | 負責人 | 期限 | 状態 |
|---|---|---|---|---|---|
| **TBD-DD03-001** | WASM memory limit per plugin | MOD-PS-001 §7.4 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-002** | Worker memory limit per plugin | MOD-PS-001 §7.5 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-003** | Worker CPU limit percent | MOD-PS-001 §7.5 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-004** | Max init time for plugin | MOD-PL-001 §6.7 P-PL-003 | Plugin Team | MVP-2 | open |
| **TBD-DD03-005** | Max restart attempts | MOD-PS-001 §7.6 P-PS-005 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-006** | Initial backoff for restart | MOD-PS-001 §7.6 P-PS-005 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-007** | Max backoff for restart | MOD-PS-001 §7.6 P-PS-005 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-008** | Heartbeat interval (Worker) | MOD-PS-001 §7.5 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-009** | Heartbeat timeout (Worker) | MOD-PS-001 §7.5 | Sandbox Team | MVP-2 | open |
| **TBD-DD03-010** | Reference migration strategy | MOD-PHS-001 §8.6 | Loader Team | MVP-3 | open |
| **TBD-DD03-011** | State snapshot max size | MOD-PHS-001 §8.5 | Hot Swap Team | MVP-3 | open |
| **TBD-DD03-012** | In-flight request drain timeout | MOD-PL-001 §6.7 P-PL-004 | Loader Team | MVP-2 | open |
| **TBD-DD03-013** | Capability invocation timeout | MOD-CPI-001 §9.4 P-CPI-001 | Capability Team | MVP-2 | open |

### 14.2 TBD 備考

- **MVP-2**: MVP フェーズ 2 までに確定必須（パフォーマンステストで検証）
- **MVP-3**: MVP フェーズ 3 までに確定（ユーザーフィードバック可能）
- 【性能検証必要】と標記した値は Benchmark / Performance Test の実施後に決定

---

## 15. 上位設計確認事項（QA）

### 15.1 AD-001 / SD-001 との整合確認（実装着手前）

| QA ID | 質問内容 | 関連セクション | 確認者 | 状態 |
|---|---|---|---|---|
| **QA-DD03-001** | Plugin と DD-01 Command Bus との境界は？<br>Plugin で独自 Command 登録が必須か？ | MOD-CPI-001, DD-01 §3 | Arch Lead | 【TBD】 |
| **QA-DD03-002** | Worker process の IPC 通信プロトコルは JSON-RPC か HTTP か？ | MOD-PL-001 §6.2, IF-PLG-001 | Protocol Lead | 【TBD】 |
| **QA-DD03-003** | Hot Swap 中に新 plugin の state snapshot に失敗した場合、<br>old plugin を継続か、abort か？ | MOD-PHS-001 P-PHS-005 Phase 3 | Design Lead | 【TBD】 |
| **QA-DD03-004** | Permission DB の実装が RBAC table か attribute-based か？<br>（SD-001 §6.1 の詳細化） | MOD-PS-001 §7.3 | Security Lead | 【TBD】 |
| **QA-DD03-005** | WASM AOT cache 戦略（永続化パス、キャッシュ無効化条件、初回起動とキャッシュヒット時のオーバーヘッド） | MOD-PL-001 §6.0 | Runtime Team | 【TBD】 |
| **QA-DD03-006** | Subprocess worker prefork pool 規模（デフォルト N、動的スケーリング戦略） | MOD-PL-001 §6.0 | Runtime Team | 【TBD】 |
| **QA-DD03-007** | A. dlopen escape hatch の使用条件と承認フロー（SD-001 §6 Zero Trust 違反前提） | MOD-PL-001 §6.0 | Security Lead + Arch Lead | 【TBD】 |
| **QA-DD03-008** | WASM host function 境界（plugin が呼べる kernel API 集合、§9 Capability Provider との整合） | MOD-PL-001 §6.0, MOD-CPI-001 §9 | Arch Lead | 【TBD】 |

### 15.2 QA 回答に基づく設計更新計画

各 QA に対する回答により、以下セクションを更新予定：

- **QA-DD03-001 回答** → MOD-CPI-001 §9 を更新
- **QA-DD03-002 回答** → MOD-PL-001 §6.2 CLS-PL-001 を更新、IF-PLG-001 と同期
- **QA-DD03-003 回答** → MOD-PHS-001 P-PHS-005 Phase 3 の rollback logic を更新
- **QA-DD03-004 回答** → MOD-PS-001 CLS-PS-002 PermissionSet を更新
- **QA-DD03-005 回答** → MOD-PL-001 §6.0 候補 B (WASM) 推奨セクションを確定、AOT cache 戦略を §6.4 Runtime Selection に追記
- **QA-DD03-006 回答** → MOD-PL-001 §6.0 候補 C (Subprocess) prefork pool パラメータ確定、§6.4 に反映
- **QA-DD03-007 回答** → MOD-PL-001 §6.0 候補 A (dlopen) escape hatch 運用ポリシーを Security Lead と合意の上、§6.0.3 に追記
- **QA-DD03-008 回答** → MOD-PL-001 §6.0 と MOD-CPI-001 §9 の host function ↔ capability 境界を確定

---

## 16. 自己審査チェックリスト

詳細設計書の品質確認（skill-multica-2 §47 準拠）

### 16.1 構造要件

- [x] 文書 ID / Version / Author が明記
- [x] 目次が完全
- [x] 上位設計との対応表が存在
- [x] 用語定義表が完備
- [x] モジュール全体図が視覚化
- [x] 責務分割が明確

### 16.2 モジュール設計要件

- [x] 各 Module に唯一の ID (MOD-XXX)
- [x] 各 Class に唯一の ID (CLS-XXX)
- [x] 各 Method に唯一の ID (M-XXX)
- [x] 各 処理 に番号付け (P-XXX)

### 16.3 詳細度チェック

- [x] API input/output が明確
- [x] 処理フロー (P-XXX) が分岐条件を含む
- [x] State transition が遷移テーブル化
- [x] Error code が ERR-PLG-XXX で統一
- [x] Timeout / Retry 戦略が明記

### 16.4 トレーサビリティ

- [x] DD → BD → REQ 追跡可能
- [x] 逆向き（REQ → DD）も確認
- [x] TBD が明確に列挙
- [x] 確認事項 (QA) が Architecture Lead へ

### 16.5 セキュリティ確認

- [x] Permission validation が明記 (P-PS-001)
- [x] Process isolation が明記 (P-PS-003)
- [x] Worker crash handling が明記 (P-PS-004/005)
- [x] Audit logging scope が定義 (各 M-XXX)

### 16.6 性能設計

- [x] Timeout 値が (【TBD】含む) 明記
- [x] Resource limit が (【TBD】含む) 明記
- [x] Concurrent request handling が考慮
- [x] 【性能検証必要】マーカーが適切に配置

### 16.7 テスト設計への準備

- [x] Test perspective table (§12 相当) 別途作成予定
- [x] Error code ごとの test case 設計可能
- [x] State transition test 設計可能
- [x] Sequence diagram ごとのテストシナリオ抽出可能

### 16.8 AI チェック禁止事項

- [x] 架空サーバー IP/Port を記載しない（→ 【TBD】使用）
- [x] 架空 API endpoint を記載しない
- [x] 架空 DB スキーマを記載しない
- [x] 推測で TBD を埋めない

---

## 17. 自己評価

### 17.1 設計完成度自評価

**約 82%**

#### カバー済み部分（82%）

1. ✅ Plugin Manager (MOD-PM-001) - 完全設計
2. ✅ Plugin Loader (MOD-PL-001) - 完全設計
3. ✅ Plugin Sandbox (MOD-PS-001) - 完全設計（権限モデル除く）
4. ✅ Plugin Hot Swap (MOD-PHS-001) - 完全設計
5. ✅ Capability Provider IF (MOD-CPI-001) - 完全設計
6. ✅ Error Code System - 29 コード定義完了
7. ✅ State Transition - 完全設計
8. ✅ Sequence Diagram × 4 種 - 作成完了
9. ✅ Traceability Matrix - 作成完了

#### 残る 18%（TBD + 確認待ち）

1. ⏳ Permission DB 実装方式（RBAC vs ABE）→ QA-DD03-004
2. ⏳ Worker IPC プロトコル詳細（JSON-RPC vs HTTP）→ QA-DD03-002
3. ⏳ Resource limit 値の確定（memory, cpu, timeout）→ TBD-DD03-001〜013
4. ⏳ Reference migration 最終方式→ TBD-DD03-010
5. ⏳ State snapshot max size → TBD-DD03-011
6. ⏳ Loading mechanism 候補分析（dlopen / WASM / Subprocess）の最終確定 → §6.0 + QA-DD03-005〜008

### 17.2 主要リスク

1. **Plugin ↔ DD-01 (Command Bus) 境界**
   - 現在: MOD-CPI-001 が Command 暴露を仲介
   - リスク: Command 登録方法の誤解
   - 軽減: QA-DD03-001 で確認、必要に応じて §9 更新

2. **Worker Crash Recovery の restart storm**
   - 現在: backoff exponential, max_restart_count で制限
   - リスク: 【TBD】値未確定時に実装が不安定
   - 軽減: TBD-DD03-005～007 を MVP-2 で確定

3. **Hot Swap state migration の reliability**
   - 現在: checksum verify + rollback 戦略
   - リスク: state serialization format の互換性
   - 軽減: IF-PLG-001 で state format を versioning

4. **Multi-module coordination (PM ↔ PL ↔ PS ↔ PHS)**
   - 現在: 各モジュール間の I/F を定義（§4.2）
   - リスク: 実装時の interfacing が複雑
   - 軽減: Integration test / 単体テストで早期検出

### 17.3 次のステップ（DD-01～04 の依存関係）

- **DD-01** (Microkernel Core, 既完) → Command Bus / Event Bus IF がこの DD の前提
- **DD-02** (Session / Transaction, 既完) → Transaction 境界がこの DD に影響
- **DD-03** (このドキュメント) → Plugin 管理の詳細化
- **DD-04** (API + 横断関切) → Plugin が API を expose する方式への調整予定
- **DD-02 Cross-Review (ULYS-42, Stage 2)** → この DD-03 との整合確認

### 17.4 設計品質の確認指標

| 指標 | 結果 | 備考 |
|---|---|---|
| Module 数 | 5 個 | PM, PL, PS, PHS, CPI |
| Class 数 | 10+ 個 | CLS-XXX 詳細定義 |
| Method 数 | 15+ 個 | M-XXX 詳細定義 |
| Process フロー | 12 個 | P-XXX 番号付与 |
| Sequence Diagram | 4 種 | 通常系, permission, crash, hot_swap |
| Error Code | 29 個 | ERR-PLG-001～029 |
| State | 12 個 | state machine で定義 |
| TBD | 13 個 | 明確に列挙、owner / 期限 |
| QA | 4 個 | Arch Lead への確認事項 |

---

## 謝辞・帰属

本設計書は以下を参照して作成されました：

- **REQ-001**: 要件定義書 (§36～50 Plugin 関連)
- **AD-001**: 基本設計書 §4.1～4.3 (Plugin Manager / Loader / Sandbox スコープ)
- **SD-001**: 安全性設計書 §6～7 (Permission Model, Process Isolation)
- **IF-PLG-001**: インタフェース設計書 (Plugin Protocol, Manifest Schema)
- **DD-01**: Microkernel Core 詳細設計書 (Command Bus, Event Bus, Registry との接続)
- **DD-02**: Session / Transaction 詳細設計書 (Transaction 境界との整合)
- **skill-multica-2**: IPA 詳細設計 50 章準拠スキル

🤖 Generated with Claude Haiku 4.5 (Haiku Agent c383a9c9-21f8-4755-bb52-dcfac3aa1dbf)

Co-Authored-By: Claude Haiku 4.5 <noreply@anthropic.com>
