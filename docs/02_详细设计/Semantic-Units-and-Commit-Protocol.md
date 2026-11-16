# IDE1.0 Web — 语义单元划分与提交协调协议详细设计书

**文档编号**: DD-ROUTER-SEM-2026-V2.0  
**制定组织**: IDE1.0 内核与云架构委员会 / 日本 IPA 软件工程基准推进组  
**基准版本**: v2.0  
**状态**: 承认済 (Baseline Locked)  
**上游设计**: [Web-Agent-Router-Basic-Design.md](../01_基本设计/Web-Agent-Router-Basic-Design.md)

---

## 1. 语义单元定义与稳定身份哈希算法 (Stable Unit Identity)

### 1.1 为什么行号不是身份
在多 Agent 并行编辑场景中，如果 Agent 依赖绝对行号或字节偏移作为作业目标，上游函数哪怕只插入了一行注释，下游所有任务的坐标便全部失效。
因此，IDE1.0 确立：**行号和字节偏移仅作为显示与当前视图坐标，语义节点及其结构指纹才是永恒的作业身份**。

### 1.2 稳定身份字段构成
每个语义单元由 9 个关键元数据字段联合确立全局唯一且抗微小漂移的身份：

```rust
pub struct SemanticUnitIdentity {
    pub workspace_id: String,           // 逻辑工作区
    pub file_id: String,                // 规范化文件相对路径，如 "src/buffer/line.rs"
    pub language: String,               // "rust" | "python" | "typescript" | "javascript" | "c" | "powershell"
    pub qualified_symbol_path: String,  // 限定符号路径，如 "DocLineMgr::insert_str"
    pub node_kind: String,              // Tree-sitter 节点类型，如 "function_item", "method_definition"
    pub signature_hash: String,         // 符号签名哈希 (包含函数名、参数列表类型、返回类型)
    pub base_content_hash: String,      // 节点原始完整内容的 SHA-256
    pub parent_fingerprint: String,     // 父级容器（类、模块或 impl 块）的指纹
    pub node_fingerprint: String,       // 组合计算的唯一稳定指纹
}
```

### 1.3 指纹生成计算公式 (Formal Invariants)
```
signature_hash = SHA256(normalize_whitespace(node.signature_tokens()))
base_content_hash = SHA256(node.raw_text_utf8())
parent_fingerprint = SHA256(parent.qualified_symbol_path + ":" + parent.signature_hash)

node_fingerprint = SHA256(
    workspace_id + ":" +
    file_id + ":" +
    language + ":" +
    qualified_symbol_path + ":" +
    node_kind + ":" +
    signature_hash + ":" +
    parent_fingerprint
)
```

---

## 2. Tree-sitter 多语言解析器规则与提取映射

| 语言 | 顶级符号抽取规则 | 内部块抽取规则 | 签名特征提取 |
|:---|:---|:---|:---|
| **Rust** | `function_item`, `struct_item`, `enum_item`, `trait_item`, `impl_item` | `impl` 块内的 `function_item` | `fn name(args) -> Ret` |
| **Python** | `function_definition`, `class_definition` | 类中的成员 `function_definition` | `def name(args) -> Ret:` |
| **TypeScript** | `function_declaration`, `class_declaration`, `interface_declaration`, `type_alias_declaration` | 类内部的 `method_definition` | `function name(args): Ret` |
| **JavaScript** | `function_declaration`, `class_declaration` | 类方法与导出函数 | `function name(args)` |
| **C** | `function_definition`, `struct_specifier` | 结构体与函数体 | `Ret name(args)` |
| **PowerShell** | `function_statement` | 块函数与过滤器 | `function name { param(...) }` |
| **Fallback** | 任意不支持的语言或解析报错文件，整文件退化为单一单元 `file_root`，采用独占悲观锁。 |

---

## 3. 租约与 Fencing Token 协议 (Lease & Fencing Protocol)

为了杜绝因网络延迟、GC 停顿或 Worker 僵死导致的脑裂覆写，系统采用分布式系统标准的 **Fencing Token 租约机制**。

### 3.1 租约生命周期
1. **发放 (Claim)**:
   - Worker 通过 `POST /api/v1/tasks/claim` 申领就绪任务。
   - 数据库原子执行：
     ```sql
     UPDATE tasks 
     SET status = 'CLAIMED', 
         fencing_token = fencing_token + 1,
         lease_expires_at = NOW() + INTERVAL '60 seconds',
         worker_id = $1
     WHERE id = $2 AND status = 'READY'
     RETURNING fencing_token, lease_expires_at;
     ```
2. **心跳续约 (Heartbeat)**:
   - Worker 必须每 20 秒发送一次 `POST /api/v1/tasks/{task_id}/heartbeat`。
   - 携带分配到的 `fencing_token`。如果此时发现该任务的 `fencing_token` 已被更高值抢占，心跳立即返回失败，Worker 停止运行并自裁。
3. **过期与回收 (Reaping)**:
   - 调度器后台每 5 秒扫描 `lease_expires_at < NOW() AND status = 'CLAIMED'` 的任务，直接将其重置为 `READY`，重新排队。

---

## 4. CAS 提交协调与 AST-aware 重定位算法

### 4.1 核心提交算法流程
当 Worker 发起 `POST /api/v1/patches/commit` 时，Coordinator 执行如下严格校验序列：

```
                [Receive Patch Commit Request]
                              │
            ┌─────────────────▼─────────────────┐
            │ Check 1: Fencing Token Valid?     │
            │   request.fencing_token == DB?    │
            └─────────────────┬─────────────────┘
                     NO │           │ YES
             ┌──────────▼───┐       │
             │ Reject: 412  │       │
             │ FENCING_STALE│       │
             └──────────────┘       │
            ┌───────────────────────▼───────────┐
            │ Check 2: Idempotency Key Exists?  │
            │   already committed this key?     │
            └─────────────────┬─────────────────┘
                    YES │           │ NO
             ┌──────────▼───┐       │
             │ Return 200   │       │
             │ cached commit│       │
             └──────────────┘       │
            ┌───────────────────────▼───────────┐
            │ Check 3: Base Revision Match?     │
            │  request.base_rev == file.rev?    │
            └─────────┬───────────────────────┬─┘
                  YES │                   NO  │ (Concurrent commits occurred!)
                      │                       ▼
                      │             ┌─────────────────────────────┐
                      │             │ Run Rust AST Relocation:    │
                      │             │ 1. Parse latest file text   │
                      │             │ 2. Match symbol_path & kind │
                      │             │ 3. Check target footprint   │
                      │             │    modified by intervening? │
                      │             └──────────────┬──────────────┘
                      │                 NO │              │ YES
                      │        ┌───────────▼──┐   ┌───────▼─────────┐
                      │        │Relocate Pass:│   │ Reject: 409     │
                      │        │Update Offsets│   │ WRITE_CONFLICT  │
                      │        └──────┬───────┘   └─────────────────┘
                      │               │
            ┌─────────▼───────────────▼─────────┐
            │ Execute CAS Commit:               │
            │ 1. Apply replacement to buffer    │
            │ 2. Atomically increment rev       │
            │ 3. Write immutable object store   │
            │ 4. Insert outbox_events           │
            └─────────────────┬─────────────────┘
                              │
                    [Return 200 SUCCESS]
```

### 4.2 冲突矩阵与判定规格 (Conflict Matrix)

| 场景 | Worker 目标符号 | 最新文件状态 | 判定结果 | 响应码与动作 |
|:---|:---|:---|:---|:---|
| **严格无并发** | `fn foo` | Revision 未变 | **通过 (FAST_PATH)** | 200 OK，直接替换并递增 revision。 |
| **并发修改不同函数** | `fn foo` | Revision 已变，但仅 `fn bar` 被修改 | **通过 (RELOCATED)** | 200 OK，重新计算 `fn foo` 的新行偏移，无损替换并递增 revision。 |
| **并发修改同一函数** | `fn foo` | Revision 已变，且 `fn foo` 内容已被修改 | **拦截 (CONFLICT)** | 409 Conflict，返回 `SEMANTIC_WRITE_CONFLICT` 及最新符号 diff。 |
| **目标函数被删除/重命名** | `fn foo` | 符号在最新 AST 中丢失 | **拦截 (NOT_FOUND)** | 409 Conflict，返回 `SYMBOL_NOT_FOUND`，终止任务。 |
| **Fencing Token 过期** | 任何 | 租约已被回收或由新 Worker 认领 | **拦截 (STALE_LEASE)**| 412 Precondition Failed，拒绝写入。 |

---

## 5. 跨文件 ChangeSet 原子元数据事务

当高层任务需要同时修改多个相互关联的文件时（例如重构函数接口并更新所有调用点）：
1. 所有文件的 Patch Proposals 先在只读沙箱内准备并校验完成。
2. 调用 `POST /api/v1/changesets` 传入所有文件的 `(file_id, base_revision, patch)`。
3. Coordinator 开启单次 Postgres 事务：
   - 对所有目标文件加行级更新锁：`SELECT id FROM files WHERE id IN (...) FOR UPDATE;`
   - 对每个文件逐一执行 CAS 与 AST 重定位。
   - 任何一个文件发生冲突或重定位失败，**整个 ChangeSet 立即回滚**，绝不残留半同步脏状态。
   - 全部通过后，原子生成 `changeset_commit` 记录并批量写入 `outbox_events`。

---
**确认人**: 
- 内核架构师: Mavis (Rust Core Lead)
- 分布式事务审查: Ulysses (Staff Engineer)
