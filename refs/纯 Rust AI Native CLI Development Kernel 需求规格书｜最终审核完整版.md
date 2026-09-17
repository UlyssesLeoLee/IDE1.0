# 纯 Rust AI Native CLI Development Kernel
# 需求规格书
## Final Reviewed Edition

---

# 0. 文档定位

本文定义一套面向 AI Agent 时代的软件开发内核。

系统不是传统意义上的：

- Vim 替代品
- Sakura Editor CLI 版
- VS Code CLI 版
- 带聊天窗口的 IDE
- LangGraph 专用工具
- MCP Server
- AI Code Agent

而是一套：

> **纯 Rust 编写、CLI/TUI 原生、微内核化、原子解耦、插件热插拔、API First、Local AI First，并能够作为 LangGraph 等 Agent Framework 底层代码执行环境的 AI Native Development Kernel。**

系统可以独立作为 IDE 使用，也可以完全无界面运行，被 Agent、CI、自动化系统或其他 IDE 调用。

---

# 1. 一句话产品定义

> **人可以把它当 Vim 使用，工程师可以把它当 Sakura Editor 式文本工具使用，LangGraph 可以把它当 Tool Runtime 使用，AI Agent 可以把它当代码操作系统使用。**

---

# 2. 核心关键词

```text
Pure Rust

CLI Native
TUI Native

Microkernel

API First
Agent First
LangGraph First

Agent Agnostic

Atomic Architecture

Capability Driven

Event Driven

Plugin Hot Swap

Process Isolation

Local AI First

Token Optimized

Transaction Safe

Headless First

Topology Independent
```

---

# 3. 产品核心目标

系统需要同时满足四种身份。

```text
                ┌─────────────────────┐
                │ Development Kernel  │
                └──────────┬──────────┘
                           │
       ┌───────────────────┼───────────────────┐
       │                   │                   │
       ▼                   ▼                   ▼

     Editor             Agent Runtime      Automation
 CLI / TUI               Backend            Engine

       │                   │                   │
       └───────────────────┼───────────────────┘
                           │
                           ▼
                    Unified Kernel
```

第一身份：

```text
CLI / TUI IDE
```

第二身份：

```text
Agent Tool Runtime
```

第三身份：

```text
Local AI Compute Runtime
```

第四身份：

```text
Development Automation Kernel
```

四种身份必须共享同一内核。

不得维护四套逻辑。

---

# 4. 最高级架构原则

整个项目必须始终遵守以下原则。

## 4.1 Core 必须足够小

Core 只提供机制。

业务能力由 Plugin 提供。

---

## 4.2 API First

如果人能够通过 TUI 执行某项操作：

Agent 原则上也必须能够通过 API 执行。

---

## 4.3 Command First

人类、AI、Macro、CLI、Plugin、LangGraph 最终都调用：

```text
Command
```

不得为 AI 单独建立旁路。

---

## 4.4 Capability First

调用方调用的是：

```text
能力
```

而不是：

```text
某个具体插件
```

---

## 4.5 Event Driven

插件之间原则上不得直接调用内部对象。

主要通信方式：

```text
Command
Capability
Event
```

---

## 4.6 Local First

能够由：

```text
算法
AST
索引
图
Embedding
Rerank
本地小模型
```

完成的事情，不应首先发送给云端大模型。

---

## 4.7 Transaction First

所有修改性行为必须：

```text
可追踪
可预览
可提交
可取消
可回滚
```

---

## 4.8 Agent Framework Agnostic

首要适配：

```text
LangGraph
```

但 Core 不得依赖：

```text
LangGraph
LangChain
Python
```

---

## 4.9 Topology Independent

业务插件不得依赖系统部署拓扑。

调用方不应关心能力来自：

```text
当前进程
WASM
子进程
本机GPU
LAN服务器
远程服务
Cloud
```

---

## 4.10 Fail Isolated

任何：

```text
Plugin
Agent
AI Worker
Language Server
Model
```

崩溃都不应该使 IDE Kernel 一同崩溃。

---

# 5. 产品运行模式

必须支持四种模式。

---

# 5.1 Standalone IDE Mode

直接：

```bash
kernel .
```

或：

```bash
kernel edit .
```

启动：

```text
Terminal

   ↓

CLI/TUI

   ↓

Kernel
```

无需：

```text
Python
Node.js
LangGraph
Browser
Cloud
```

即可正常开发。

---

# 5.2 Headless Server Mode

```bash
kernel serve
```

启动：

```text
Kernel
 │
 ├ HTTP
 ├ SSE
 ├ WebSocket
 ├ JSON-RPC
 ├ MCP Adapter
 └ Agent Gateway
```

不启动 TUI。

主要供：

```text
LangGraph
Agent
CI
Web IDE
Remote Tool
Automation
```

使用。

---

# 5.3 Local IPC Mode

本地 Agent 不需要经过 HTTP。

支持：

Linux/macOS：

```text
Unix Domain Socket
stdio
```

Windows：

```text
Named Pipe
stdio
```

例如：

```text
LangGraph

   ↓ JSON-RPC

stdio

   ↓

Rust Kernel
```

---

# 5.4 Embedded Rust Mode

Rust 程序能够直接：

```rust
use kernel_core::Kernel;
```

将 Kernel 作为 Library 嵌入。

结构：

```text
Rust Application

      │

      ▼

Kernel API
```

无需：

```text
HTTP
IPC
JSON
```

---

# 6. 总体架构

```text
                       HUMAN
                         │
                    CLI / TUI
                         │
                         ▼
               ┌───────────────────┐
               │   FRONTEND API    │
               └─────────┬─────────┘
                         │
                         │
                         ▼
┌──────────────┐  ┌──────────────────────────┐
│  LangGraph   │─►│                          │
└──────────────┘  │                          │
                  │      AGENT GATEWAY       │
┌──────────────┐  │                          │
│ Other Agent  │─►│ REST / SSE / WS          │
└──────────────┘  │ JSON-RPC / MCP / A2A     │
                  └────────────┬─────────────┘
                               │
                               ▼
                  ┌──────────────────────────┐
                  │     UNIFIED API LAYER    │
                  │                          │
                  │ Command API              │
                  │ Event API                │
                  │ Capability API           │
                  │ Session API              │
                  │ Transaction API          │
                  │ Task API                 │
                  └────────────┬─────────────┘
                               │
                               ▼
                ┌─────────────────────────────┐
                │        MICROKERNEL          │
                │                             │
                │ Command Bus                 │
                │ Event Bus                   │
                │ Capability Registry         │
                │ Session Manager             │
                │ Transaction Manager         │
                │ Scheduler                   │
                │ Plugin Manager              │
                │ Security Manager            │
                └──────────────┬──────────────┘
                               │
                        Capability Router
                               │
          ┌────────────────────┼───────────────────┐
          │                    │                   │
          ▼                    ▼                   ▼
      Search Plugin        LSP Plugin          Git Plugin
          │                    │                   │
          ├────────────────────┼───────────────────┤
                               │
                               ▼
                 ┌──────────────────────────┐
                 │   LOCAL COMPUTE LAYER    │
                 │                          │
                 │ Parser                   │
                 │ AST                      │
                 │ Symbol Index             │
                 │ Dependency Graph         │
                 │ Embedding                │
                 │ Rerank                   │
                 │ Context Compiler         │
                 │ Local SLM                │
                 │ Cache                    │
                 └──────────────────────────┘
```

---

# 7. 微内核职责

Core 只允许包含系统必须具备的基础机制。

P0 Core：

```text
Buffer Engine

Command Bus

Event Bus

Capability Registry

Session Manager

Transaction Manager

Task Scheduler

Plugin Manager

Security / Permission

Workspace Abstraction

Configuration

Observability
```

Core 不直接实现：

```text
Git业务逻辑

LSP

具体语言支持

Debugger

LLM Provider

具体Embedding模型

Vector Database

Formatter

Compiler

Cloud AI

LangGraph

MCP业务逻辑
```

---

# 8. 统一标识模型

任何运行状态不得依赖全局变量。

系统必须明确以下 ID：

```text
WorkspaceId

SessionId

ExecutionId

TransactionId

CommandId

TaskId

PluginId

CapabilityId

BufferId

DocumentId

AgentId
```

例如：

```text
LangGraph Thread
       │
       ▼
Kernel Session

LangGraph Run
       │
       ▼
Kernel Execution
```

但两者属于映射关系。

不是同一个对象。

---

# 9. Session 模型

Session 是 Kernel 的一级概念。

Session 保存：

```text
session_id

workspace_id

actor

permissions

active_buffers

active_transaction

resource_budget

context_budget

worktree

metadata
```

Actor 可以是：

```text
Human

LangGraph

AI Agent

CI

Plugin

Automation
```

---

# 10. 禁止全局当前状态

禁止：

```text
GLOBAL_CURRENT_FILE

GLOBAL_CURSOR

GLOBAL_TRANSACTION

GLOBAL_WORKSPACE

GLOBAL_AGENT
```

必须：

```text
Session Scoped
```

因此可以同时：

```text
Human
Agent A
Agent B
Agent C
```

访问同一个 Kernel。

---

# 11. Vim 编辑思想

编辑模型吸收 Vim：

```text
Normal

Insert

Visual

Command
```

必须支持：

```text
Operator

Motion

Text Object

Register

Macro

Repeat

Command History

Search History
```

---

# 12. 扩展编辑 Mode

允许插件增加：

```text
AI Mode

Review Mode

Diff Mode

Debug Mode
```

但：

```text
Mode ≠ Core Business Logic
```

---

# 13. Sakura Editor 能力吸收

重点吸收 Sakura Editor 的：

```text
高性能文本处理

Grep

Regex

Grep Replace

多文件替换

编码识别

编码转换

换行符转换

列模式

矩形选择

Diff

大文件

外部命令

Macro

工程级搜索
```

最终形成：

```text
Vim Editing
+
Sakura Text Engineering
+
Agent Automation
```

---

# 14. Buffer Engine

Buffer Engine 属于 Kernel 基础能力。

必须支持：

```text
Open
Read
Edit
Insert
Delete
Replace
Patch
Undo
Redo
Snapshot
Save
Reload
```

Buffer 与文件系统必须解耦。

---

# 15. 文本编码

至少支持：

```text
UTF-8

UTF-16 LE

UTF-16 BE

Shift-JIS

EUC-JP

GBK

BOM
```

内部文本优先统一为：

```text
UTF-8
```

文件保存阶段进行编码转换。

---

# 16. 换行符

支持：

```text
LF

CRLF

CR
```

必须保留：

```text
原始文件格式信息
```

---

# 17. 大文件

系统必须支持超大文本文件。

原则：

> File Size 不应该直接等价于 Resident Memory Size。

需要支持：

```text
Lazy Read

Chunk Read

Memory Mapping

Partial Decode

Streaming Search

Streaming Replace

Incremental Index
```

不能因为打开 GB 级日志而要求整个文件一次性进入 Buffer。

---

# 18. Patch First

AI 和 Agent 默认不能通过“重写整个文件”进行修改。

优先使用：

```text
Patch
```

Patch 至少包含：

```text
document

range

before_hash

expected_version

change

after_hash
```

---

# 19. Optimistic Concurrency

Agent 读取文件后：

其他 Actor 可能已经修改文件。

因此 Patch 必须验证：

```text
expected_version
```

或：

```text
before_hash
```

不一致：

```text
CONFLICT
```

禁止静默覆盖。

---

# 20. Transaction

所有跨文件修改必须支持 Transaction。

状态：

```text
NEW

ACTIVE

VALIDATING

COMMITTED

ROLLING_BACK

ROLLED_BACK

FAILED
```

---

# 21. Transaction API

标准流程：

```text
transaction.begin

file.patch

file.patch

test.run

git.diff

transaction.commit
```

失败：

```text
transaction.rollback
```

---

# 22. AI Transaction

Agent 修改默认建议：

```text
Agent Run
   │
   ▼
Transaction
```

而不是直接写磁盘。

允许：

```text
preview

approve

commit

rollback
```

---

# 23. Crash Recovery

Workspace Mutation 必须考虑：

```text
Kernel Crash

OS Crash

Worker Crash
```

重要事务需要：

```text
Journal
```

恢复时能够识别：

```text
未完成 Transaction
```

---

# 24. Command 是唯一统一动作模型

所有动作统一为：

```text
Command
```

例如：

```text
file.open

file.read

file.patch

search.text

search.regex

symbol.search

language.definition

git.diff

test.run

context.build
```

---

# 25. Command 数据结构

逻辑结构：

```text
Command

├ name
├ version
├ arguments
├ actor
├ session
├ permissions
├ timeout
├ metadata
└ cancellation_token
```

返回：

```text
CommandResult

├ status
├ data
├ diagnostics
├ metadata
├ side_effects
└ next_cursor
```

---

# 26. Command Side Effect

Command 必须声明 Side Effect 等级。

```text
READ_ONLY

LOCAL_STATE

WORKSPACE_MUTATION

PROCESS_EXECUTION

NETWORK

DESTRUCTIVE
```

例如：

```text
file.read
→ READ_ONLY

file.patch
→ WORKSPACE_MUTATION

test.run
→ PROCESS_EXECUTION

git.reset_hard
→ DESTRUCTIVE
```

---

# 27. Command Discovery

系统必须能够动态查询：

```text
command.list
```

返回：

```text
name

description

input_schema

output_schema

permissions

capabilities

side_effect

streaming

version
```

Agent 不需要预先硬编码 Kernel 所有能力。

---

# 28. Command Pipeline

CLI 支持组合：

```text
search TODO
|
filter
|
open
```

AI 场景：

```text
search "unsafe"
|
ai security-review
|
quickfix
```

Pipeline 的中间数据使用结构化对象。

而不是简单字符串拼接。

---

# 29. Capability 模型

Plugin 不应该被其他模块按名字直接调用。

禁止：

```text
call("rust-analyzer-plugin")
```

推荐：

```text
capability("language.definition")
```

---

# 30. Capability 示例

```text
text.search

text.regex

language.parse

language.symbol

language.definition

language.reference

language.completion

language.format

git.status

git.diff

debug.attach

test.run

ai.embed

ai.rerank

ai.complete

context.compress
```

---

# 31. Capability Router

Kernel 根据：

```text
Capability

Priority

Policy

Availability

Performance

Session

Resource
```

决定最终 Provider。

例如：

```text
ai.embed
```

可能来自：

```text
Local CPU Plugin

Local GPU Worker

LAN AI Server

Remote Provider
```

调用方无需知道。

---

# 32. Capability Provider 切换

允许：

```text
hot swap
```

例如：

```text
LocalEmbedderA
     ↓ unload

LocalEmbedderB
     ↓ activate
```

使用能力的 Agent 不需要修改。

---

# 33. Event Bus

统一事件总线。

事件示例：

```text
WorkspaceOpened

FileOpened

BufferChanged

FileSaved

TransactionStarted

TransactionCommitted

PluginLoaded

PluginFailed

CommandStarted

CommandCompleted

DiagnosticsUpdated

TestStarted

TestFinished

AgentConnected

AgentDisconnected

AIJobStarted

AIJobFinished
```

---

# 34. Event Envelope

事件统一包含：

```text
event_id

sequence

type

timestamp

workspace_id

session_id

execution_id

actor

payload
```

便于：

```text
Streaming
Tracing
Replay
Audit
```

---

# 35. Event 等级

事件区分：

```text
Transient Event

Durable Event
```

例如：

CursorMove：

```text
Transient
```

TransactionCommit：

```text
Durable
```

无需把所有 UI 高频事件永久落盘。

---

# 36. Plugin 总体设计

插件必须支持：

```text
discover

install

verify

enable

load

activate

suspend

reload

deactivate

unload

upgrade

rollback

remove
```

---

# 37. 不采用直接 Rust dylib 作为主要插件 ABI

禁止把：

```text
Rust dylib ABI
```

设计成唯一插件机制。

原因包括：

```text
Rust ABI不稳定

版本兼容困难

卸载复杂

panic隔离困难

资源释放困难

崩溃可能污染主进程
```

---

# 38. Plugin Runtime A：WASM/WASI Component

推荐用于：

```text
Formatter

Parser辅助

Text Transformer

Lint Rule

Small Tool

Context Filter

Automation
```

结构：

```text
Rust Plugin

   ↓ compile

WASM Component

   ↓

Plugin Host
```

优点：

```text
Sandbox

Stable Boundary

Hot Load

Hot Unload

Permission Control
```

---

# 39. Plugin Runtime B：Native Worker

重型能力使用独立进程。

适合：

```text
LSP

Debugger

Compiler

GPU

LLM

Embedding

Database

大型Indexer
```

结构：

```text
Kernel

   │ IPC

   ▼

Native Rust Worker
```

---

# 40. Worker Crash Isolation

必须保证：

```text
Worker Panic
Worker Crash
Worker OOM
Worker Timeout
```

不会直接导致：

```text
Kernel Crash
```

Kernel 需要能够：

```text
detect

terminate

cleanup

restart

disable
```

---

# 41. Plugin 生命周期

```text
DISCOVERED

INSTALLED

VERIFIED

LOADED

INITIALIZING

ACTIVE

SUSPENDED

DRAINING

UNLOADING

UNLOADED
```

异常：

```text
FAILED
QUARANTINED
```

---

# 42. Safe Unload

卸载插件必须：

```text
停止接受新任务

等待或取消旧任务

注销 Command

注销 Capability

取消 Event Subscription

释放 Resource Handle

停止 Worker

确认无活动引用

完成卸载
```

不得“标记关闭但实际仍常驻”。

---

# 43. Plugin Manifest

示例：

```toml
[plugin]

id = "language.rust"
name = "Rust Language Plugin"
version = "1.0.0"
api = "1"

[runtime]

type = "worker"

[provides]

capabilities = [
    "language.definition",
    "language.references",
    "language.diagnostics"
]

[requires]

capabilities = []

[permissions]

workspace_read = true
workspace_write = false
network = false

process = [
    "rust-analyzer"
]
```

---

# 44. 插件依赖规则

插件之间禁止直接持有：

```text
Plugin Object Reference
```

允许声明：

```text
requires capability
```

例如：

```text
code-review
```

依赖：

```text
language.symbol
```

而不是：

```text
rust-language-plugin
```

---

# 45. Plugin Protocol

外部协议必须版本化。

例如：

```text
plugin_api = 1
```

Kernel 内部可以持续重构。

Plugin Boundary 必须保持稳定。

---

# 46. Plugin SDK

提供官方 Rust SDK：

```rust
#[async_trait]
pub trait Plugin {
    async fn activate(
        &mut self,
        ctx: PluginContext,
    ) -> Result<()>;

    async fn deactivate(
        &mut self,
    ) -> Result<()>;
}
```

Plugin SDK 只暴露稳定 API。

不得允许插件依赖：

```text
Kernel Internal Modules
```

---

# 47. Plugin 安全模型

默认：

```text
Deny
```

Plugin 必须声明权限。

至少包括：

```text
workspace.read

workspace.write

filesystem.read

filesystem.write

process.spawn

network

git

ai.local

ai.remote

secrets
```

---

# 48. TUI

TUI 是 Kernel Client。

不是 Kernel 本身。

结构：

```text
TUI
 │
 ▼
Command API
```

---

# 49. TUI 基本布局

例如：

```text
┌─────────────────────────────────────────────┐
│ src/service.rs                              │
├──────────────────────────────┬──────────────┤
│                              │ Symbols      │
│                              │              │
│          EDITOR              │ Diagnostics  │
│                              │              │
│                              │ AI Context   │
├──────────────────────────────┴──────────────┤
│ Command / Task / Test / Agent               │
└─────────────────────────────────────────────┘
```

---

# 50. TUI Panel Plugin

右侧：

```text
Symbols

Git

Diagnostics

Tasks

AI Context
```

不应硬编码为 Core UI。

允许 Plugin View 注册。

---

# 51. Headless First

任何 TUI 可以完成的非纯显示功能：

原则上必须支持 Headless。

例如：

```bash
kernel search TODO

kernel symbol UserService

kernel test

kernel context build "auth bug"

kernel ai review

kernel git diff
```

---

# 52. Search Engine

搜索分为三层。

---

# 52.1 Text Search

```text
Exact

Case Sensitive

Case Insensitive

Regex

Glob

Grep
```

---

# 52.2 Structural Search

```text
AST

Symbol

Definition

Reference

Call Site
```

---

# 52.3 Semantic Search

```text
Embedding

Vector Similarity

Rerank

Code Intent
```

例如：

```text
find "用户身份验证逻辑"
```

即使源码没有出现相同文字：

仍可以找到：

```text
AuthMiddleware

TokenValidator

SessionService
```

---

# 53. Search 必须 Streaming

大型工程搜索：

不能等待全部完成后一次返回。

返回：

```text
SearchStarted

Match

Match

Match

Progress

SearchFinished
```

---

# 54. QuickFix

所有诊断统一进入：

```text
Diagnostic Model
```

来源：

```text
Compiler

LSP

Lint

Test

Security

AI Review

Git
```

统一显示：

```text
ERROR
WARNING
INFO
AI
```

---

# 55. Language 支持

Core 不知道具体：

```text
Rust

Python

Java

C#

C++

JavaScript
```

Language Plugin 注册：

```text
extension

grammar

parser

lsp

formatter

compiler

test

debug
```

---

# 56. Git

Git 不进入 Core。

通过：

```text
Git Plugin
```

提供：

```text
status

diff

commit

branch

worktree

merge

rebase

blame
```

---

# 57. Worktree

Agent 并行任务推荐使用：

```text
Git Worktree
```

例如：

```text
repository

├ main
├ worktree-human
├ worktree-agent-a
├ worktree-agent-b
└ worktree-review
```

---

# 58. Agent 与 Worktree

Session 可以绑定：

```text
worktree_id
```

例如：

```text
Session A
→ worktree-agent-a

Session B
→ worktree-agent-b
```

降低并行修改冲突。

---

# 59. API First

统一 API Layer 是项目最重要的长期资产之一。

必须覆盖：

```text
Workspace

Session

Command

Capability

Buffer

File

Search

Symbol

Diagnostics

Transaction

Task

Plugin

Context

Event
```

---

# 60. External API 数据格式

外部默认：

```text
JSON
```

Rust 内部：

```text
Strong Type
```

边界：

```text
Serde
```

---

# 61. Schema Single Source of Truth

Rust 类型应能够产生：

```text
JSON Schema
```

并进一步生成：

```text
REST Schema

MCP Tool Schema

LangGraph Tool Schema

OpenAI-compatible Tool

其他 Agent Tool
```

不得维护多套独立 Schema。

---

# 62. API Transport

P0：

```text
Rust Embedded API

stdio JSON-RPC

Unix Socket

Windows Named Pipe

HTTP JSON

SSE
```

P1：

```text
WebSocket

MCP
```

P2：

```text
A2A

gRPC Adapter
```

---

# 63. Transport 不进入 Core

必须：

```text
Transport

   ↓ Adapter

Unified API

   ↓

Kernel
```

因此：

```text
HTTP
MCP
JSON-RPC
```

只属于适配层。

---

# 64. REST API 基础结构

建议：

```text
/v1/system

/v1/workspaces

/v1/sessions

/v1/commands

/v1/capabilities

/v1/files

/v1/search

/v1/symbols

/v1/diagnostics

/v1/transactions

/v1/tasks

/v1/plugins

/v1/context

/v1/events
```

---

# 65. 错误格式

统一：

```json
{
  "error": {
    "code": "VERSION_CONFLICT",
    "message": "Document changed after read",
    "retryable": true,
    "details": {}
  }
}
```

不能只返回：

```text
Something went wrong
```

---

# 66. API Cancellation

长任务必须可取消。

例如：

```text
Search

Build

Test

Index

AI

Agent Execution
```

支持：

```text
task.cancel
```

---

# 67. API Pagination

大型结果必须支持：

```text
cursor
```

禁止一次性返回：

```text
数十万个Symbol
```

---

# 68. Correlation

所有调用携带：

```text
request_id

execution_id

session_id
```

使：

```text
Command

Event

Log

Trace

Metric
```

可以串联。

---

# 69. Agent Gateway

Agent Gateway 负责：

```text
Authentication

Session Mapping

Protocol Translation

Rate Limit

Tool Discovery

Streaming

Permission

Agent Metadata
```

不负责：

```text
代码分析
文件修改
AI推理
```

---

# 70. LangGraph 是首要参考实现

LangGraph 被定义为：

```text
Reference Agent Framework
```

即：

第一套完整 Agent Integration 必须使用 LangGraph 验证。

但：

```text
Kernel Core
```

不得 import 或依赖：

```text
langgraph
langchain
python
```

---

# 71. LangGraph Adapter

提供官方薄适配器：

```text
LangGraph Adapter
```

其职责仅为：

```text
Kernel Command
        ⇅
LangGraph Tool
```

以及：

```text
Kernel Event
        ⇅
LangGraph Streaming
```

---

# 72. LangGraph 映射

推荐：

```text
LangGraph Thread
      │
      ▼
Kernel Session Binding
```

```text
LangGraph Run
      │
      ▼
Kernel Execution
```

```text
LangGraph Tool Call
      │
      ▼
Kernel Command
```

```text
LangGraph Stream
      │
      ▼
Kernel Event Stream
```

---

# 73. LangGraph Checkpoint 与 Kernel Transaction 必须分离

LangGraph Checkpoint 保存：

```text
Agent Workflow State
```

Kernel Transaction 保存：

```text
Workspace Mutation State
```

两者绝对不能合并。

可以关联：

```text
checkpoint metadata

   ↓

transaction_id
```

---

# 74. LangGraph State 边界

LangGraph 管理：

```text
Messages

Agent State

Graph State

Planning

Workflow

Checkpoint

Human-in-the-loop
```

Kernel 管理：

```text
Workspace

Files

Buffers

AST

Symbols

Index

Git

Build

Test

Patch

Transaction

Context
```

---

# 75. LangGraph 与 Kernel 职责

核心原则：

```text
LangGraph

WHY
WHAT NEXT
```

Kernel：

```text
HOW
SAFE EXECUTION
LOCAL CONTEXT
```

---

# 76. LangGraph Agent 不应该默认直接操作 OS

推荐禁止 Agent 默认：

```python
open(...)

os.remove(...)

subprocess.run(...)

shutil...
```

而应：

```text
LangGraph

    ↓

Kernel Tool

    ↓

Permission

    ↓

Transaction

    ↓

Workspace
```

---

# 77. LangGraph Tool P0

必须提供：

```text
workspace.open

workspace.info

file.read

file.patch

search.text

search.regex

search.symbol

symbol.definition

symbol.references

diagnostics.list

context.build

test.run

build.run

git.status

git.diff

transaction.begin

transaction.commit

transaction.rollback
```

---

# 78. Tool Atomicity

禁止：

```text
solve_bug()
```

这种“大而全 Tool”。

应该拆为：

```text
search

read

analyze

patch

test

diff
```

由 LangGraph 自由编排。

---

# 79. Tool 返回结构

禁止大量返回无结构 Terminal 文本。

例如：

```json
{
  "matches": [
    {
      "file": "src/auth.rs",
      "line": 42,
      "symbol": "validate_token"
    }
  ],
  "truncated": false,
  "next_cursor": null
}
```

使 Agent 可以程序化判断。

---

# 80. Streaming

Streaming 是 P0。

必须可以实时返回：

```text
CommandStarted

TaskProgress

SearchMatch

BuildOutput

TestOutput

Diagnostic

PatchPrepared

ApprovalRequired

CommandCompleted
```

---

# 81. SSE

HTTP Agent 场景优先：

```text
SSE
```

适用于：

```text
Search

Build

Test

Index

AI

Agent Progress
```

---

# 82. Human in the Loop

Kernel 不实现 LangGraph 的业务 HITL。

Kernel 只输出：

```text
ApprovalRequired
```

例如高风险动作：

```text
git.reset_hard

delete directory

network upload

destructive patch
```

LangGraph Adapter 可以将该事件转换为自己的：

```text
interrupt / approval flow
```

---

# 83. Agent Permission

每个 Agent Session 独立权限。

例如 Reviewer：

```text
workspace.read = true
workspace.write = false
process = false
```

Coder：

```text
workspace.read = true
workspace.write = true
process = true
```

---

# 84. Agent Resource Budget

每个 Session 可以限制：

```text
CPU

RAM

Disk IO

Process Count

Execution Time

Network

AI Tokens

Context Tokens
```

---

# 85. 并发 Agent

Kernel 必须支持：

```text
Agent A

Agent B

Agent C
```

并发。

所有资源需要：

```text
Session Scoped
```

---

# 86. Local AI 的定位

AI 不是：

```text
聊天窗口
```

而是：

> Kernel 的一种可调度计算能力。

---

# 87. Local Compute Pipeline

默认优先级：

```text
Level 0
Deterministic

        ↓

Level 1
Index / Graph

        ↓

Level 2
Embedding / Rerank

        ↓

Level 3
Local SLM

        ↓

Level 4
Remote LLM
```

---

# 88. Level 0

优先使用：

```text
Hash

Diff

Regex

Parser

AST

Symbol

Call Graph

Dependency Graph
```

这些任务原则上不应该交给 LLM。

---

# 89. Level 1

使用：

```text
Index

Graph Query

Static Analysis

History

Cache
```

缩小上下文范围。

---

# 90. Level 2

使用：

```text
Embedding

Semantic Search

Rerank

Clustering

Deduplication
```

---

# 91. Level 3

本地小模型适合：

```text
Classification

Intent Routing

Context Summary

Code Ranking

Simple Explanation

Commit Classification

Compression
```

---

# 92. Level 4

云端大型模型处理：

```text
Complex Reasoning

Architecture

Large Refactor

Complex Generation

Cross-domain Decision
```

---

# 93. AI Router

每次任务进入：

```text
AI Router
```

判断：

```text
是否需要AI

是否能由算法完成

是否需要Embedding

是否需要SLM

是否允许Remote

预算多少

资源在哪里
```

---

# 94. Remote AI 必须可关闭

企业项目允许：

```text
remote_ai = false
```

此时：

任何源码不得发送给 Remote Provider。

---

# 95. AI Worker

模型不得直接常驻 Kernel 主进程。

采用：

```text
Kernel

   ↓ IPC

AI Worker

   ↓

Runtime
```

使：

```text
Model Crash
≠
Kernel Crash
```

---

# 96. Local AI Backend 热替换

模型 Provider 通过 Capability：

```text
ai.embed

ai.rerank

ai.complete
```

注册。

可以动态：

```text
load

switch

unload
```

---

# 97. Context Compiler

项目最重要的 AI 模块之一：

```text
Context Compiler
```

其工作：

```text
Repository

   ↓

Index

   ↓

Symbol

   ↓

Dependency Graph

   ↓

Relevance

   ↓

Diff

   ↓

Dedup

   ↓

Rerank

   ↓

Summary

   ↓

Token Budget

   ↓

Agent Context
```

---

# 98. Context API

例如：

```text
context.build
```

输入：

```json
{
  "goal": "修复登录超时",
  "budget_tokens": 12000,
  "strategy": "balanced"
}
```

---

# 99. Context Strategy

支持：

```text
fast

balanced

deep

local_only
```

fast：

```text
Search
Symbol
AST
```

balanced：

```text
Search
AST
Graph
Rerank
```

deep：

```text
Search
AST
Graph
Embedding
Rerank
Local SLM
```

---

# 100. Context 输出必须可解释

必须告诉 Agent / Human：

为什么选这些内容。

例如：

```text
src/auth.rs
→ direct symbol reference

src/session.rs
→ caller dependency

tests/auth_test.rs
→ related test

src/config.rs
→ changed in current diff
```

---

# 101. Context Inspector

CLI：

```text
:context
```

可以查看：

```text
Context Budget: 12,000

auth.rs            2,400
session.rs         1,920
call graph           650
git diff             830
summary            1,200

TOTAL              7,000
```

---

# 102. Token Optimization

目标不是单纯：

```text
减少Prompt
```

而是：

> 使用本地计算将无价值信息在进入 LLM 前淘汰。

---

# 103. Incremental Index

文件发生改变：

```text
src/auth.rs
```

只更新：

```text
该文件

受影响Symbol

受影响Dependency

对应Embedding

相关Cache
```

不得默认重建整个 Repository。

---

# 104. Content Hash

缓存失效以：

```text
Content Hash
```

等可验证机制为基础。

未修改内容：

不得反复计算。

---

# 105. Workspace Graph

Kernel 应维护代码关系图。

Node：

```text
File

Module

Namespace

Struct

Trait

Class

Function

Method

Variable

Test
```

Edge：

```text
imports

calls

implements

depends

reads

writes

tests

references
```

---

# 106. 图数据库不是 Core 强依赖

Workspace Graph 是逻辑模型。

底层允许：

```text
Embedded Store

SQLite-like Store

Graph Engine

External DB
```

通过 Storage Provider 抽象。

Core 不绑定具体数据库。

---

# 107. Scheduler

统一 Scheduler 管理：

```text
INTERACTIVE

HIGH

NORMAL

BACKGROUND

IDLE
```

---

# 108. 编辑优先

任何：

```text
Embedding

Index

AI

Graph

Build
```

不得影响：

```text
Keyboard Input
Cursor
Screen Refresh
```

交互任务优先级最高。

---

# 109. Resource Control

支持：

```toml
[resources.ai]

cpu = "25%"
memory = "8GB"
gpu = "60%"

[resources.index]

cpu = "15%"
```

---

# 110. Background Computing

索引和 AI 预处理应尽可能：

```text
Idle Computing
```

利用开发机闲置资源。

---

# 111. 分布式本地算力

未来允许：

```text
Kernel

├ Local CPU
├ Local GPU
├ LAN Mac
├ LAN GPU Server
└ Cloud
```

Capability Router 决定执行位置。

---

# 112. Agent 不感知硬件拓扑

Agent 调用：

```text
context.build
```

不应该需要知道：

```text
Embedding在哪台机器

GPU型号

模型运行在哪

Cache存在哪里
```

---

# 113. AI Cost Inspector

显示：

```text
Local CPU Time

Local GPU Time

Input Token

Output Token

Context Saved

Cache Hit

Remote Cost
```

例如：

```text
Raw context       83,200 tokens

Local filtered    17,400 tokens

Final context      8,600 tokens

Saved             74,600 tokens
```

---

# 114. Macro

Macro 记录：

```text
Command
```

而不是只记录键盘。

例如：

```text
search
→ replace
→ format
→ test
→ git.diff
```

可以保存为 Macro。

---

# 115. Automation

支持：

```text
Event
   ↓
Rule
   ↓
Command
```

例如：

```text
FileSaved

    ↓

format

    ↓

affected tests

    ↓

index update
```

---

# 116. Agent 与 Macro 一致

Macro、Agent、Human 最终都调用：

```text
Command API
```

这是架构一致性的关键。

---

# 117. Security

安全模型采用：

```text
Zero Trust Plugin

Zero Trust Agent
```

---

# 118. Workspace Sandbox

Session 可以限制：

```text
allowed_paths

read_only_paths

write_paths

blocked_paths
```

---

# 119. Process Security

`process.spawn`：

默认禁止。

需要 Permission。

并支持：

```text
executable allowlist

working directory

environment filter

timeout

resource limit
```

---

# 120. Network Security

Network Permission：

可以限制：

```text
No Network

LAN Only

Domain Allowlist

Full Network
```

---

# 121. Secret

Plugin 不应该直接读取所有 Environment Variables。

提供：

```text
Secret API
```

按名称和权限访问。

---

# 122. Audit

重要行为记录：

```text
Actor

Time

Command

Target

Side Effect

Result

Transaction
```

Agent 修改必须可以追踪来源。

---

# 123. Observability

内置：

```text
status

tasks

plugins

sessions

memory

cpu

ai stats

context stats
```

---

# 124. Trace

支持：

```text
Execution

   ↓

Command

   ↓

Capability

   ↓

Plugin

   ↓

Task
```

全链路追踪。

---

# 125. Plugin Resource Inspector

例如：

```text
CORE               24 MB

rust-language      96 MB

git-worker         18 MB

indexer           210 MB

AI Worker        4.2 GB
```

---

# 126. Configuration

默认：

```text
TOML
```

位置：

```text
~/.kernel/config.toml

workspace/.kernel/config.toml
```

---

# 127. 配置层级

```text
Defaults

   ↓

Global

   ↓

Workspace

   ↓

Session
```

后者覆盖前者。

---

# 128. 配置热加载

大部分非安全关键配置：

允许：

```text
Hot Reload
```

无需重启。

---

# 129. Offline

必须可以完全离线使用。

无网络时仍支持：

```text
Edit

Search

Regex

Grep

Local LSP

Git

Local Index

Local Embedding

Local SLM

Local Agent
```

---

# 130. Pure Rust 定义

“纯 Rust”定义为：

Kernel：

```text
Rust
```

官方 TUI：

```text
Rust
```

官方基础 Plugin：

```text
Rust
```

官方 Worker：

```text
Rust
```

---

外部生态 Adapter 可以存在：

```text
Python SDK

TypeScript SDK
```

但它们只是 Client。

不能成为 Kernel 运行依赖。

因此：

```text
没有Python
```

IDE 仍然可以完整运行。

---

# 131. LangGraph Python SDK

允许提供：

```text
kernel-langgraph-python
```

但它只负责：

```text
Tool Wrapper

Session Binding

Streaming Binding
```

不能包含：

```text
IDE核心逻辑
```

---

# 132. MCP

MCP 是：

```text
Adapter
```

而不是：

```text
Kernel Architecture
```

结构：

```text
MCP Client

   ↓

MCP Adapter

   ↓

Command API
```

---

# 133. A2A

A2A 同样：

```text
Adapter
```

未来增加或更换 Agent Protocol：

不得修改 Core。

---

# 134. Performance 原则

首要目标：

```text
Typing must never wait for AI.
```

---

# 135. 初步性能目标

在参考开发机环境下：

冷启动到可编辑状态目标：

```text
≤ 150 ms
```

Core Idle Memory 目标：

```text
≤ 50 MiB
```

常规输入事件目标：

```text
P99 < 16 ms
```

本地 IPC 框架额外开销目标：

```text
P95 < 10 ms
```

以上属于工程目标。

后续通过 Benchmark 校准。

---

# 136. Lazy Startup

启动时禁止立即加载：

```text
AI Model

所有LSP

所有Plugin

所有Index
```

必须：

```text
Lazy Load
```

---

# 137. Startup Sequence

```text
Kernel Init

   ↓

Config

   ↓

Workspace

   ↓

Buffer

   ↓

TUI Ready

   ↓

Background Plugin Discovery

   ↓

Background Index / AI
```

---

# 138. 开发目录建议

```text
crates/

  kernel-core/

  buffer/

  command/

  event/

  capability/

  session/

  transaction/

  scheduler/

  security/

  workspace/

  plugin-runtime/

  plugin-sdk/

  worker-protocol/

  agent-gateway/

  api-schema/

  tui/

  context-engine/

  observability/
```

---

# 139. 官方插件建议

```text
plugins/

  search/

  grep/

  git/

  lsp/

  language-rust/

  formatter/

  test-runner/

  local-ai/

  remote-ai/

  mcp/

  langgraph-gateway/
```

---

# 140. 绝对禁止的架构

## 禁止一

```text
TUI
直接调用Git
```

必须：

```text
TUI
→ Command
→ Capability
→ Plugin
```

---

## 禁止二

```text
Agent
直接操作内部Buffer结构
```

---

## 禁止三

```text
Plugin A
持有
Plugin B Object
```

---

## 禁止四

```text
LangGraph State
进入Kernel Core
```

---

## 禁止五

```text
AI直接重写整个Repository
```

---

## 禁止六

```text
所有Agent共享一个Global Workspace State
```

---

## 禁止七

```text
任意Plugin拥有全部FS/Network权限
```

---

## 禁止八

```text
所有模型常驻主进程
```

---

## 禁止九

```text
Rust dylib成为唯一插件ABI
```

---

## 禁止十

```text
TUI拥有只有TUI才能使用的业务逻辑
```

---

# 141. MVP P0

第一阶段只证明底层架构成立。

必须完成：

```text
Microkernel

Buffer

CLI/TUI

Normal / Insert / Visual

File Open / Save

Undo / Redo

Text Search

Regex

Grep

Command Bus

Event Bus

Capability Registry

Session

Transaction

Plugin Runtime

WASM Plugin

Native Worker

Hot Load

Hot Unload

JSON-RPC

HTTP

SSE

LangGraph Adapter

Context Build

Local Index
```

---

# 142. MVP 不做

第一阶段明确不追求：

```text
完整Debugger

完整Git UI

大型Plugin Marketplace

几十种AI Provider

复杂GUI

自研大模型

所有语言

完整远程协作
```

---

# 143. MVP 验证一：Standalone

执行：

```bash
kernel .
```

必须可以：

```text
打开文件

Vim式编辑

搜索

Regex

Grep

Patch

Undo

Save
```

无需网络。

---

# 144. MVP 验证二：Plugin Hot Swap

IDE 运行期间：

```text
install

load

activate

use

deactivate

unload
```

Plugin。

过程中：

```text
Kernel 不重启
```

---

# 145. MVP 验证三：Plugin Crash

主动使 Native Plugin：

```text
panic
```

预期：

```text
Worker Crash

Kernel Alive

Plugin marked FAILED

Capability removed

可重新启动
```

---

# 146. MVP 验证四：LangGraph

完整场景：

```text
User

"修复这个Bug"

      ↓

LangGraph

      ↓

context.build

      ↓

symbol.search

      ↓

file.read

      ↓

transaction.begin

      ↓

file.patch

      ↓

test.run

      ↓

git.diff

      ↓

transaction.commit
```

整个过程：

```text
LangGraph
```

不得直接使用：

```text
open()

os.remove()

subprocess
```

修改项目。

---

# 147. MVP 验证五：Rollback

故意使：

```text
test.run
```

失败。

Agent 调用：

```text
transaction.rollback
```

Workspace 必须恢复到修改前状态。

---

# 148. MVP 验证六：并发 Agent

同时启动：

```text
Agent A
Agent B
```

分别绑定：

```text
Session A
Session B
```

不得互相污染：

```text
Cursor

Transaction

Context

Permissions
```

---

# 149. MVP 验证七：Local Context

Repository 原始上下文例如：

```text
100,000 tokens
```

调用：

```text
context.build
budget=10,000
```

必须输出：

```text
≤ 10,000 Token Budget
```

并显示：

```text
来源

筛选理由

Hash

Token统计
```

---

# 150. MVP 验证八：Streaming

LangGraph 调用：

```text
test.run
```

必须能够实时收到：

```text
started

stdout

progress

diagnostic

finished
```

而不是任务结束后才返回。

---

# 151. 第二阶段

P1：

```text
完整LSP

Symbol Graph

Semantic Search

Embedding

Rerank

Git

Worktree

QuickFix

Macro

Automation

Local SLM

Context Inspector

Cost Inspector

MCP
```

---

# 152. 第三阶段

P2：

```text
Debugger

Distributed Worker

LAN Compute

Remote Workspace

A2A

Multi-agent Coordination

Plugin Marketplace

Advanced Agent Policy

Graph-based Refactoring
```

---

# 153. 最终 Agent 模型

未来的 Agent 不应该：

```text
自己理解文件系统

自己解析所有代码

自己管理Git

自己实现Patch

自己遍历Repository

自己控制Rollback
```

这些属于：

```text
Development Kernel
```

---

Agent 应主要负责：

```text
Reason

Plan

Decide

Orchestrate
```

---

# 154. Kernel 最终责任

Kernel 最终负责：

```text
Read

Search

Parse

Index

Graph

Context

Patch

Build

Test

Git

Transaction

Rollback

Security

Resource Scheduling
```

---

# 155. AI Agent 与 Kernel 的理想关系

```text
                  AGENT
                    │
                    │
             "What should I do?"
                    │
                    ▼
             Decision / Plan
                    │
                    │ Commands
                    ▼
       ┌──────────────────────────┐
       │    DEVELOPMENT KERNEL    │
       │                          │
       │ Context                  │
       │ Search                   │
       │ Code                     │
       │ Patch                    │
       │ Test                     │
       │ Git                      │
       │ Transaction              │
       │ Security                 │
       └────────────┬─────────────┘
                    │
                    ▼
                 PROJECT
```

---

# 156. 产品核心竞争力

真正需要建立壁垒的不是：

```text
TUI皮肤
```

也不是：

```text
聊天窗口
```

而是以下六个核心能力。

```text
1. Microkernel

2. Capability / Plugin Runtime

3. Agent-safe Command API

4. Transactional Workspace

5. Context Compiler

6. Local Compute Scheduler
```

---

# 157. 终极架构

```text
                            USERS
                              │
            ┌─────────────────┼─────────────────┐
            │                 │                 │
            ▼                 ▼                 ▼

         CLI/TUI          LangGraph         Other Agent

            │                 │                 │
            └─────────────┬───┴─────────────────┘
                          │
                          ▼
               ┌────────────────────┐
               │   AGENT GATEWAY    │
               │                    │
               │ JSON-RPC           │
               │ REST               │
               │ SSE                │
               │ WebSocket          │
               │ MCP                │
               │ A2A                │
               └─────────┬──────────┘
                         │
                         ▼
               ┌────────────────────┐
               │    UNIFIED API     │
               │                    │
               │ Command            │
               │ Event              │
               │ Capability         │
               │ Session            │
               │ Transaction        │
               └─────────┬──────────┘
                         │
                         ▼
             ┌──────────────────────────┐
             │       MICROKERNEL        │
             │                          │
             │ Command Bus              │
             │ Event Bus                │
             │ Capability Registry      │
             │ Plugin Manager           │
             │ Session Manager          │
             │ Transaction Manager      │
             │ Scheduler                │
             │ Security                 │
             └────────────┬─────────────┘
                          │
                    Capability
                          │
       ┌──────────────────┼───────────────────┐
       │                  │                   │
       ▼                  ▼                   ▼

     Search              LSP                 Git

       │                  │                   │
       └──────────────────┼───────────────────┘
                          │
                          ▼
             ┌─────────────────────────┐
             │     COMPUTE FABRIC      │
             │                         │
             │ Parser                  │
             │ AST                     │
             │ Symbol                  │
             │ Graph                   │
             │ Index                   │
             │ Embedding               │
             │ Rerank                  │
             │ Local SLM               │
             │ Context Compiler        │
             │ Cache                   │
             └────────────┬────────────┘
                          │
            ┌─────────────┼─────────────┐
            │             │             │
            ▼             ▼             ▼

         CPU/GPU       LAN Worker      Cloud
```

---

# 158. 最终架构判断标准

以后加入任何新功能之前，都必须问：

### Question 1

这是：

```text
机制
```

还是：

```text
业务能力
```

如果是业务能力：

优先 Plugin。

---

### Question 2

其他模块需要它时：

是否应该调用：

```text
Capability
```

如果是：

不得直接形成模块依赖。

---

### Question 3

Agent 是否能够：

```text
通过Command API使用
```

如果不能：

需要重新检查设计。

---

### Question 4

这个动作能否：

```text
Audit
Cancel
Rollback
```

如果修改 Workspace 却不能：

设计不合格。

---

### Question 5

Plugin 崩溃：

```text
Kernel 会不会死？
```

如果会：

隔离设计不合格。

---

### Question 6

替换 LangGraph：

```text
Core 是否需要修改？
```

如果需要：

Agent Framework 解耦不合格。

---

### Question 7

替换 AI Model：

```text
业务层是否需要修改？
```

如果需要：

Capability 抽象不合格。

---

### Question 8

本地算法可以解决：

为什么调用 LLM？

Local First 原则要求首先证明：

```text
LLM确实必要。
```

---

# 159. 最终产品定义

正式建议定义为：

> **AI Native Development Kernel**

详细定义：

> 一套以 Rust 实现的 CLI/TUI 原生开发内核，通过微内核、Command、Event、Capability、Session 与 Transaction 构成稳定基础层，通过 WASM Component 与隔离 Native Worker 实现真正可控的插件热插拔，并通过统一 API 向人类、LangGraph、Agent、CI 和其他工具开放代码操作能力。同时利用本地 CPU/GPU 完成代码解析、索引、图分析、Embedding、Rerank、上下文压缩和小模型推理，从而降低远程大模型的上下文规模、Token 成本与算力压力。

---

# 160. 最终一句话

> **一个像 Vim 一样快、像 Sakura Editor 一样擅长文本工程处理、像微内核操作系统一样原子化、可以独立运行，也可以成为 LangGraph 与其他 Agent 底层执行环境，并把尽可能多的 AI 工作留在本地完成的纯 Rust 开发内核。**

---

# 161. 项目最核心的六层

最终可以将整个项目压缩成：

```text
Human / Agent
      │
      ▼
    API
      │
      ▼
   Command
      │
      ▼
 Microkernel
      │
      ▼
 Capability
      │
      ▼
Plugin / Compute
```

而 AI 时代新增的最重要一层是：

```text
Repository
    │
    ▼
Local Compute
    │
    ▼
Context Compiler
    │
    ▼
Agent
```

即：

> **不是让大模型承担 IDE 的工作，而是让 IDE Kernel 先完成自己能够完成的所有确定性工作，再把真正需要推理的问题交给 Agent。**