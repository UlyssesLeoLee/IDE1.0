# [ARCH-01] フロントエンドを純 Rust へ移行する — 移行方針

> **メタ情報:**
>
> - **ドキュメント ID:** ARCH-01
> - **ドキュメント名:** 純 Rust 化 移行方針 (ADR)
> - **バージョン:** v1.1.0
> - **決定日:** 2026-10-18
> - **基線コミット:** `6bcffbc`
> - **ステータス:** ✅ 承認済み (ユーザー決定 2026-10-18)
> - **関連:** [WBS-01](WBS-01_feature-parity.md) §5 U-04 / [SPA-02](SPA-02_coverage.md)

---

## 0. 決定の要約

**2026-10-18、ユーザーは「このプロジェクトはこれから逐步的に純 Rust にリファクタリングする」と決定した。**

本 ADR はその決定を ADR として記録し、**移行の目標状態・移行順序・禁止事項・各 WBS への影響**を定める。

| 項目 | 内容 |
|---|---|
| **目標** | 編集ロジックを 100% Rust に寄せる |
| **ホスト** | `ide-kernel` (Rust) |
| **方向** | **Rust 優先** — 新機能は Rust に書く |
| **禁止** | 新しい JS 製品ロジックの追加 |
| **移行手段** | 逐次 (strangler fig パターン) |

---

## 1. なぜこの決定が必要だったか (現状の測定)

移行計画の前提として、**2026-10-18 時点の分離を実測した**。

### 1.1 コード配置 (JS と Rust の内訳)

```text
crates/
  editcore            10,093 行  Rust  緩衝・検索・マクロ・型・Grep エンジン
  ide-kernel           1,764 行  Rust  文書ストア・セッション ( DocLineMgr / DocStore )
  ide-shell-web        2,749 行  Rust  HTTP サーバー (lib.rs 1,356 行)
    └─ editor.html     7,679 行  JS    ★ 実ロジックはこの中
  ide-shell-desktop    2,044 行  Rust  Tauri 2 製
  ide-kernel-core         66 行  Rust  (ほぼ空)
```text

**結合度:** `editor.html` は **自己完結した JS** で、HTTP API は 4 つだけ使う
(`/api/pick_folder`, `/api/search`, `/api/terminal_input`, `/api/terminal_output`)。
**Rust 側は JS から一切呼ばれていない** (`window.__ideReady` はダミー)。

→ **Rust と JS の間に橋が存在しない。** これが本 ADR の出発点。

### 1.2 JS 側の規模 (実測)

| 項目 | 実測値 |
|---|---|
| `<script>` ブロック | 6,174 行 |
| トップレベル `function` | 280 個 |
| `dispatchCommand` の command case | 130 個 |
| `window.*` エクスポート | 252 個 |

**これが Rust に移すべきロジックの総量**。

---

## 2. 目標アーキテクチャ

```text
[Browser / WebView]
      │  fetch (JSON)  ← 今生じている唯一の通信パラダイム
      ▼
[ide-shell-web  (Rust)]
      │  in-process call
      ▼
[ide-kernel  (Rust)]  ← 文書・セッション・状態管理
      │
      ├── [editcore (Rust)]  ← 緩衝・検索・Grep・マクロ・型・変換
      └── [ide-shell-desktop (Rust)] ← Tauri 2 command
```text

**目標**: 上図の「in-process call」を JS から呼び出せるようにする。
選択肢は [§5](#5- technologically-選択肢の比較) のとおり。

---

## 3. 移行順序

**依存が下流向きの順に、大きい塊から。**

### Phase 1 — 基盤 (最重要)

| タスク | 内容 | 前提 |
|---|---|---|
| **1-A** | **JS ↔ Rust 橋の設計** | — |
| **1-B** | `ide-kernel` に編集コマンド層を追加 (130 commands のインターフェース) | 1-A |
| **1-C** | `editor.html` の `dispatchCommand` を Rust に委譲 | 1-B |
| **1-D** | 文書状態 (`DocLineMgr` 系) を Rust に一本化 | 1-C |

**Phase 1 完了の定義:** `dispatchCommand` が Rust を呼ぶ形になり、JS 側に残るのは描画だけ。

### Phase 2 — 編集プリミティブ

| タスク | 内容 |
|---|---|
| 2-A | 矩形選択 (`set_selection_box` / `paste_box`) を Rust に実装 (PARITY.md の虚偽 ✅) |
| 2-B | 行/単語操作 (WBS-01 WBS-01 §参照) |
| 2-C | 入力メソッド (かな/漢字変換) のインターフェース |

### Phase 3 — 検索/ Grep

| タスク | 内容 |
|---|---|
| 3-A | Grep の 9 残項目を**Rust 側 dialog ロジック含めて**実装 (WBS-02 を Rust 版に置き換える) |
| 3-B | 複数ファイル置換を Rust に |
| 3-C | 検索マーク / 位置表示 |

### Phase 4 — 構文/ アウトライン

| タスク | 内容 |
|---|---|
| 4-A | `default_registry` を 20 → 22+ に拡充 |
| 4-B | 言語別コメント規則/識別子規則を Rust に統一 |
| 4-C | ctags 相当を Rust 化 (外部 ctags 依存の解消) |

### Phase 5 — マクロ/プラグイン

| タスク | 内容 |
|---|---|
| 5-A | JS マクロを本物のエンジン化 (rquickjs 等) — Rust 内 |
| 5-B | PPA マクロを Rust に実装 |
| 5-C | プラグイン ABI を Rust に移設 + wasm feature を有効化 |

### Phase 6 — 設定/ウィンドウ

| タスク | 内容 |
|---|---|
| 6-A | 共通設定/タイプ別属性を Rust に |
| 6-B | 鍵割当データを Rust に (設定永続化と一括) |
| 6-C | タブ/分割状態を Rust に |

---

## 4. 各 WBS (WBS-01) への影響

本決定により [WBS-01](WBS-01_feature-parity.md) の §5 **U-04 は解決済み**。

| WBS | 変更 | 備考 |
|---|---|---|
| WBS-02 (Grep, P1) | **実装先を Rust に変更** | 「全部 `editor.html`」という制約が外れた |
| WBS-03 (移動/選択) | M-19 (矩形 Rust 実装) が Phase 2 に入った | 工期は据え置き |
| WBS-04 (検索/置換) | Rust に統一 | 工期は据え置き |
| WBS-05 (構文/アウトライン) | Phase 4 と統合 | 工期は据え置き |
| WBS-06 (設定) | Rust 実装を前提 | 工期は据え置き |
| WBS-07 (窓/タブ) | Rust 実装を前提 | 工期は据え置き |
| WBS-08 (符号) | Rust 実装を前提 | 工期は据え置き |
| WBS-09 (マクロ) | rquickjs 等の Rust crate を使う | 工期は据え置き |
| WBS-10 (プラグイン) | wasm feature を **default 有効化** | 工期は据え置き |
| WBS-11 (依存) | Rust crate で優先 (既存 crate が使える場合) | 工期は据え置き |

**WBS-01 §1 の工期表は変更なし** — 総工数 37-48 人日のまま。
**変わるのは「どこで書くか」だけ** (JS → Rust)。

**追加工程:** Phase 1 の**橋の設計と 130 command の Rust 化**。
これは WBS-01 に含まれていなかった**新規工程**で、
**約 8-12 人日**と見込む (§7)。

---

## 5. 技術的選択肢の比較

### 5.1 JS と Rust の接続方式

| 方式 | 遅延 | 実装工数 | 制限 |
|---|---|---|---|
| **HTTP API (現状方式)** | 高 (1 リクエスト ~数 ms) | 少 | 状態管理が難しい、非同期 |
| **wasm-bindgen** | 低 | 中 (~4 人日) | Rust→JS 方向のみ。JS→Rust は wasm 呼び出し |
| **WebSocket / ストリーム** | 中 | 中 | サーバ側に長寿命接続的处理が必要 |
| **Tauri command** | **最低** | 中 | Web 版 (非 Tauri) では使えない |

**✅ 決定 (2026-10-18): Phase 1 は HTTP API を拡張し、Web 版と Tauri 版の両方で使える形にする。**

**Web 版 (`ide-shell-web`) は維持する** (ユーザー決定 2026-10-18)。
Desktop 版 (`ide-shell-desktop`, Tauri 2) との**両方**が起動可能な状態が要件。

このため wasm や Tauri 固有 API に依存できない。HTTP が唯一の共通点。
Phase 2 以降で Desktop 側は Tauri ネイティブに寄せ、Web 版は strangler で縮小する
(縮めるのは Web 版の実装範囲だけで、**Web 版そのものは維持**する)。

→ **Phase 1 の工数は 8-12 人日のまま** (Web 版維持のため 4 人日への短縮は不可)。

### 5.2 段階移行パターン

```text
Phase 0: 全部 JS (現状)
   ↓
Phase 1: 編集コマンドを Rust に (JS は UI として生存)
   ↓
Phase 2: 編集プリミティブを Rust に
   ↓
Phase 3-6: 各機能を順次 Rust に
   ↓
Phase 7: JS は描画のみに縮小
```text

**strangler fig パターン** を採用。
一度に書き換えない ( 数千行規模のコードベースでも一度の書き換えは困難)。

---

## 6. 禁止事項と必須条件

### 6.1 禁止 (Rust 優先の例外)

| 禁止事項 | 理由 |
|---|---|
| **新しい編集ロジックを JS に書かない** | 二重実装と移行工数の増加 |
| **Rust 側を「後でリファクタ」として後回しにする** | 技術的負債の再生産 |
| **`#[allow(dead_code)]` で未接続の Rust を放置しない** | Phase 1 完了の定義に違反 |

### 6.2 必須条件

| 条件 | 詳細 |
|---|---|
| **全 Rust コードに `#![forbid(unsafe_code)]` を維持** | 既に workspace 全部に適用済み |
| **clippy -D warnings を CI で強制** | 既に Jenkinsfile で実施済み |
| **UAT は 0 failed を維持** | 本 ADR の移行中も Green 必須 |
| **純粋なロジックは `editcore` に置く** | `ide-shell-*` は転送層のみ。業務ロジックは `editcore` / `ide-kernel` |
| **新機能には Rust のテストを伴う** | TDD。JS テスト (Playwright) は Rust へ |

---

## 7. 前提となる不確実性

| ID | 内容 | 影響 |
|---|---|---|
| **U-04** | ~~フロントエンド移行時期~~ | **✅ 解決 (本 ADR)** |
| **U-01** | PPA マクロ互換性の目標レベル | MC-02/04 の工数が 2 倍 |
| **U-02** | プラグイン ABI を 3 系そのまま再現するか | PL-02 の工数 |
| ~~NEW-01~~ | ~~Phase 1 の橋の設計 (8-12 人日) が妥当か~~ | **✅ 解決** — HTTP 拡張で確定 (§5.1) |
| ~~NEW-02~~ | ~~Web 版 (非 Tauri) を存続させるか~~ | **✅ 解決** — **存続させる** (2026-10-18 ユーザー決定) |

**NEW-01/NEW-02 は 2026-10-18 に解決。** Web 版を維持する方針が確定したため、
橋は HTTP 拡張 (Web/Tauri 共通) となり、Phase 1 は **8-12 人日で確定**。

残るのは U-01 (PPA 互換性) と U-02 (プラグイン ABI) の 2 件のみ。

---

## 8. 関連ドキュメント

- [WBS-01 機能カバレッジ残作業 WBS](WBS-01_feature-parity.md) — 本 ADR  szerint WBS の実装先が変わる
- [SPA-02 ギャップ分析](SPA-02_coverage.md) — 上流規模
- [SPA-01 機能対比仕様](SPA-01_parity.md) — 受入契約
- [DOC-18 アーキテクチャ決定記録](DOC-18_architecture_decision_log.md) — 関連 ADR

## 9. 改訂履歴

| 版 | 日付 | 担当 | 変更内容 |
|---|---|---|---|
| v1.0.0 | 2026-10-18 | Hermes Agent | 初版 — 純 Rust 化決定を記録、Phase 1-6 を定義、WBS-01 の U-04 を解決 |
| v1.1.0 | 2026-10-18 | Hermes Agent | NEW-01/02 解決 — **Web 版維持** → 橋は HTTP 拡張、Phase 1 = 8-12 人日で確定 |
