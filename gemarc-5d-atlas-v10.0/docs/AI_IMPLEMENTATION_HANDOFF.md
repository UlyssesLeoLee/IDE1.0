# AI Implementation Handoff — IDE1.0 5D 設計図集

> - 対象実装スタック: **IDE1.0 純 Rust カーネル**（`E:\IDE1.0` @ `dev` / `76c4e8f`）
> - 契約: 各タスクに objective / requirementIds / sourceEvidence / targetModules /
>   inputOutput / rules / failureModes / acceptanceTests / dependency / rollback を必ず書く。
> - 鉄則: 本書が列挙した ID 以外を新設 ID として書かない。既存 ID を拡張するときは
>   `models/architecture.json` と `models/causal-rules.json` に同時登録する
>   （未登録の ID は `.tooling/verify-refs.mjs` で失敗する）。

---

## TASK-01 · HTTP 文書 API の実装／拡張

- **objective**: `/api/doc/*` を `DocStore` の意味論と厳密に整合させ維持・拡張する。
- **requirementIds**: IR-02, FR-01, FR-03, CR-03
- **sourceEvidence**: EV-CODE-0020（ルート表）, EV-CODE-0016（DocStore 実メソッド群）, EV-DOC-0008（E001〜E004）
- **targetModules**: `crates/ide-shell-web/src/lib.rs`（ディスパッチ表）,
  `crates/ide-kernel/src/doc_store.rs`（状態）
- **inputOutput**:
  - in: `{ method, path, body }`（JSON）
  - out: `DocView`（`{id,name,path,lines,cursor,mode,dirty,language}`）または `{ error, code }`
- **rules**:
  1. ファイル系ルートは必ず `path_safety` を先に通す（ルート内・4MB・UTF-8）。
  2. 編集の語彙（バッファ操作・undo・redo・検索）を本層で再実装しない。常に `DocStore` へ委譲する。
  3. 未知エラーは `E006`、特定失敗は `E001`/`E002`/`E003`/`E004` に写像する。
  4. 新ルートを追加したら `models/architecture.json` の `ports` と、本図集の Case を更新する。
- **failureModes**:
  - `E001` ファイル未発見 / `E002` 4MB 超 / `E003` 非 UTF-8 / `E004` ルート外
  - `E405` 相当のメソッド不一致（既存実装の分岐漏れ）
- **acceptanceTests**:
  - `C-NORMAL-EDIT` が遮蔽 0 で完走する
  - `C-PATH-ESCAPE` / `C-SIZE-LIMIT` / `C-ENCODING-REJECT` / `C-FILE-NOT-FOUND` で
    `DocStore` へ**進入しない**（`runB.finalStates["N-KN-DOCSTORE"]` が無いこと）
  - `IT-WEB-API-01` / `UT-WEB-PATHSAFETY-01..04` が pass
- **dependency**: `ide-kernel`（DocStore/Document）, `editcore`（行ストア）, `aci-emitter`（イベント）
- **rollback / ops**: ディスパッチ表の該当分岐のみ revert。`DocStore` 側は変更しない。

---

## TASK-02 · ローカルサービスのトークン／来源検証

- **objective**: 任意ページからの直アクセスFaces EDS .Token と Host/Origin を必ず検証する。
- **requirementIds**: NFR-05, IR-02, CR-03
- **sourceEvidence**: EV-CODE-0025（脅威モデル）, EV-CODE-0026（検証ロジックと用例）, EV-CODE-0027（getrandom 依存）
- **targetModules**: `crates/ide-shell-web/src/auth.rs`
- **inputOutput**: in: `Host`, `Origin`, `x-ide-token` ヘッダ / out: `Result<(), String>`
- **rules**:
  1. トークンは OS CSPRNG（`getrandom`）由来。予測可能なシードを使わない。
  2. `Host` が `127.0.0.1` / `localhost` 以外なら拒否。`Origin` が本地以外なら拒否。
  3. 拒否時に内部パスや内部状態を漏らさない。
  4. `x-ide-token` が一致しない場合、**業務ルートへ到達させない**。
- **failureModes**: トークン推測 / Origin 偽装 / Host 偽装 / リファラルの悪用
- **acceptanceTests**: `C-AUTH-REJECT` で鎖が認証辺で停止し下流（`DocStore`）へ到達しない。
  `UT-WEB-AUTH-01` pass。
- **dependency**: `getrandom`, `once_cell`
- **rollback / ops**: `auth.rs` のみ。検証を緩める変更は**禁止**（`R-02` リスク Branco）。

---

## TASK-03 · JSON-RPC メソッド面の拡張

- **objective**: `ide-shell-protocol` の JSON-RPC 2.0 を拡張し、エラー契約と 3 態 mode を維持する。
- **requirementIds**: IR-03, FR-04, FR-05
- **sourceEvidence**: EV-CODE-0011（実装済み 20 メソッドの完全リスト）, EV-CODE-0008（エラー符号）,
  EV-CODE-0009（AiProvider 契約）
- **targetModules**: `crates/ide-shell-protocol/src/lib.rs`
- **inputOutput**: in: `RpcRequest{jsonrpc:"2.0", id, method, params}` /
  out: `RpcResponse{result}` or `RpcResponse{error:{code,message,data}}`
- **rules**:
  1. 新メソッドは `handle_rpc` の分岐表と **`models/architecture.json` の `ports`** に同時に登録する。
     、片方だけの登録は「ルールが永久に発火しない」という無症状故障になる。
  2. 未知メソッドは `-32601` を返し**内核状態を変えない**。
  3. 存在しない buffer は `-32001`、読み取り専用操作は `-32002`。
  4. provider 未登録時は空の結果でなくエラーを返す。
  5. 3 態 mode（Normal/Insert/Command）と `set_mode(Insert)` 時の `push_undo` 挙動を維持する。
- **failureModes**: 未登録メソッド / 不正な params / provider 未登録 / undo スタック枯渇（上限 200）
- **acceptanceTests**: `C-RPC-UNKNOWN-METHOD` の**遷移数が 0**であること（＝仕様どおり）、
  `C-RPC-BUFFER-NOT-FOUND` が Failed 終端になること、`UT-PROTOCOL-RPC-01..03` pass、
  新メソッド追加時は `verify-cases.mjs` の `rules fired` に新ルールが現れること。
- **dependency**: `tokio`, `serde`, `serde_json`
- **rollback / ops**: 分岐表の該当 case のみ revert。エラー符号表は互換性のため追記のみ。

---

## TASK-04 · editcore の検索/置換・Grep の維持（0 重型依存）

- **objective**: `FR-06`/`FR-07` を CR-02（regex crate / tree-sitter WASM 禁止）を守って維持する。
- **requirementIds**: FR-06, FR-07, CR-02
- **sourceEvidence**: EV-CODE-0042（grep / grep_engine）, EV-CODE-0043（search_ops）,
  EV-CODE-0031（rg 優先 + フォールバック）, EV-DOC-0010（ADR-002）
- **targetModules**: `crates/editcore/src/search_ops.rs`, `grep.rs`, `grep_engine.rs`
- **inputOutput**: in: `FindOptions{方向, 大小文字, 正規表現}` + `Pos` /
  out: `FindResult` / `ReplaceOutcome{count}`
- **rules**:
  1. `regex` crate と tree-sitter WASM を**持ち込まない**。
  2. `rg` が無い環境では内蔵エンジンへフォールバックする（経路を一本化しない）。
  3. 正規表現検索は将来計画（DOC-02.A.2）。**今回実装しない**。
  4. 機能追加時は `crates/editcore/PARITY.md` の比較表を同時更新する。
- **failureModes**: 巨大ファイルでの走査時間 / バイナリ誤検出 / フォールバック未到達
- **acceptanceTests**: `C-SEARCH-RG`（rg 主経路）, `C-SEARCH-FALLBACK`（内蔵へ回落）,
  `UT-EDITCORE-GREP-01`, `IT-DESKTOP-SEARCH-01/02`
- **dependency**: `editcore` 内部のみ（外部 crate 増やさぬこと）
- **rollback / ops**: エンジン差し替えは `grep_engine` と `search_ops` の 2 ファイルに限定。

---

## TASK-05 · AI Provider の実実装

- **objective**: `IR-04` を Designed から Implemented へTransition させる。
- **requirementIds**: IR-04, FR-04
- **sourceEvidence**: EV-CODE-0009（trait 定義）, EV-CODE-0036（PlaceholderAi 現状）,
  EV-DOC-0003（IR-04）
- **targetModules**: 新規 provider 実装（`crates/ide-kernel/src/…` 配下を推奨）+
  `crates/ide-shell-protocol/src/lib.rs` の登録経路
- **inputOutput**: in: `CompletionRequest{buffer_id,prefix,line_text,cursor,language,limit}` /
  out: `Vec<CompletionItem{label,insert_text,kind,detail,documentation}>`
- **rules**:
  1. trait のシグネチャは 1 文字も変えない（`complete` / `edit` / `chat`）。
  2. provider 未登録時は空配列でなくエラーを返す。
  3. 補完結果は現在の buffer の mode と `prefix()` 制約に従う。
  4. **機密と課金の取り決めが決まるまで実 supplier は接続しない**（GAP-BIZ-AI-PROVIDER）。
- **failureModes**: 応答遅延 / 幻覚を含む補完 / モード無視 / 登録漏れ
- **acceptanceTests**: `C-AI-COMPLETE` が AI Provider ノードへ入ること、
  `C-RPC-NORMAL`（未登録）では AI 経路へ入らないこと、`UT-PROTOCOL-AI-01` pass。
- **dependency**: `AiProvider` trait, `ide-shell-protocol`
- **rollback / ops**: provider は registry 経由で登録・解除できるため、trait 実装を丸ごと差し替え可能。

---

## TASK-06 · 図集への新ノード追加（図集維持）

- **objective**: IDE1.0 事実図にノードを 1 つ追加し、2D/3D/4D/5D の 4 層すべてで見えるようにする。
- **requirementIds**: IR-02
- **sourceEvidence**: EV-CODE-0004（editcore 設計方針）, EV-DOC-0015（テスト設計）
- **targetModules**: `models/architecture.json`, `models/spatial.json`, `models/causal-rules.json`, `models/cases.json`
- **inputOutput**: in: ノード定義（id/kind/name/qualifiedName/role/domainId/parentId/portIds/properties/verification/evidenceIds）/
  out: 4 層すべてでの可視化
- **rules**:
  1. `role` は 11 種の機能ロールのいずれか。違 Seminary すると 3D の形状が既定退化体になる。
  2. `spatial.nodeOffsets[id]` と `spatial.gemstoneMapping.byNode[id]` を**必ず**書く。
     欠けると 3D で「静かに消える」（エラーが出ない）。
  3. `verification` を実装 backed にし、`evidenceIds` に `path:startLine-endLine` を与える。
  4. 実行時状態遷移をamor させるなら 4D 規則と Case も追加する（さもなくば `deadEdges` になる）。
- **failureModes**: 役割名の誤記 / オフセット欠落 / 証拠なしの Implemented 断定 / 規則未登録
- **acceptanceTests**: `node .tooling/verify-refs.mjs .` が 0 violation、
  `node .tooling/verify-cases.mjs .` で新ノードに関わる規則が `rules fired` に現れる、
  `node tests/unit/atlas.test.mjs` が pass。
- **dependency**: `models/architecture.json`（正本）
- **rollback / ops**: モデル差分のみなので revert 容易。事実リポジトリは触らない。

---

## TASK-07 · 図集への Case 追加（4D 検証）

- **objective**: 異常・境界挙動を 1 つ、4D で再現可能にする。
- **requirementIds**: NFR-05
- **sourceEvidence**: EV-DOC-0015（合格基準）, EV-CODE-0008（エラー符号）
- **targetModules**: `models/cases.json`, `models/causal-rules.json`
- **inputOutput**: in: `{ id, name, origin, scenario, expected, entryNodeId, entryState, inputs, seed }` /
  out: 事件トレースと終端状態
- **rules**:
  1. `origin` は `ObservedReplay` / `DesignedSimulation` / `HypotheticalExperiment` のいずれか。
     証拠の無いのに ObservedReplay を使わない。
  2. **遷移 0 のケースは「なぜ 0 が仕様か」を必ず書く**（被覆欠落と区別する）。
  3. `nodeIds` / `edgeIds` / `evidenceIds` は実在 ID のみ。
  4. 同じ `inputs` と `seed` なら同一トレースになること（決定性）。
- **failureModes**: 期待と実挙動の乖離 / ガード未設定による無意味通過 / 非決定性
- **acceptanceTests**: `verify-cases.mjs` の「遷移 0 の未説明数」が 0 のまま、
  新ケースが `rules fired` の増加として現れること。
- **dependency**: `models/causal-rules.json` の規則
- **rollback / ops**: モデル差分のみ。

---

## TASK-08 · 文書ドリフトの解消（DOC-04 / ADR ↔ 実装）

- **objective**: `GAP-SRC-API-DRIFT` と `GAP-SRC-AXUM-MISMATCH` を解消する。
- **requirementIds**: IR-02, IR-03
- **sourceEvidence**: EV-DOC-0006（22 method 記載）, EV-DOC-0007（13 endpoint 記載）,
  EV-DOC-0020（Axum 記載）, EV-CODE-0011, EV-CODE-0019, EV-CODE-0027
- **targetModules**: `docs/DOC-04_software_requirements.md`, `docs/DOC-16_decision_log.md`（ADR-004 等）
- **inputOutput**: in: 実装の現実 / out: 実装と一致する文書記述
- **rules**:
  1. **実装を正本とする**。コードを変えるのではなく文書を実装に合わせる。
  2. 文書と実装の両方を記録し、片方を黙って捨てない。
  3. Historical ADR は上書きせず、Accepted 状態のまま改訂履歴を追記するか、
     Superseded として新しい ADR を追加する（ADR 命名規則: 連番、削除しない）。
  4. 解決後、`models/evidence.json` の gap を閉じ、対応する ADR ID を記録する。
- **failureModes**: 文書だけ先に直す / 実装がifo に戻る / 履歴の改竄
- **acceptanceTests**: `verify-refs.mjs` が pass、gap リストと実際の未解消事項が一致、
  `docs/DOC-04` の method 数と `crates/ide-shell-protocol/src/lib.rs` の分岐数が一致。
- **dependency**: 実装コード（読み取りのみ）
- **rollback / ops: ** 文档 commit のみ。実装には影響しない。

---

## 共通契約（すべてのタスク）

1. **事実の出典**:`path:startLine-endLine` と revision を必ず添える。依存宣言・接口定義・DTO・Mock・TODO は実装の証拠にならない。
2. **ID 登録**:新規 ID は `models/architecture.json`（nodes/edges/ports）と、必要に応じ `models/causal-rules.json`（rules/stateMachines）へ同時登録する。
3. **門禁**:`.tooling/verify-refs.mjs` → `.tooling/verify-cases.mjs` → `node --test tests/unit` → `.tooling/e2e-journeys.mjs` の順に全緑。
4. **生成物**:`index.html` / `dist/` / `qa/*.json` は commit しない（`git_governance.allow_local_commit = false`）。
5. **対象リポジトリの改変禁止**:本図集は IDE1.0 のコードを変更しない。push / merge / deploy / 設定変更は行わない。
