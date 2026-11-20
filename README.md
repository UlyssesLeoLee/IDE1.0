# IDE1.0

> Pure safe-Rust AI-native editor kernel + high-performance native desktop app (`IDE1.0.exe`) + Vercel cloud MCP gateway.
> Supports concurrent AI agent workloads at scale: file-level routing, semantic-node-level parallelism, patch-level commit, CAS + fencing authority coordination.

---

## Tool Discovery Metadata

```json
{
  "name": "ide10_editor",
  "display_name": "IDE1.0 Native AI Development Environment",
  "version": "0.8.6",
  "description": "High-performance pure safe-Rust AI-native editor and multi-agent coordination platform. Sub-500ms cold start, O(1) buffer editing, up to 100,000 concurrent agent orchestration, atomic CAS transactional patching, PowerShell-fidelity terminal, MCP server (stdio + HTTP).",
  "keywords": [
    "ide", "code-editor", "rust-ide", "ai-native-editor", "multi-agent-orchestrator",
    "mcp-server", "cas-fencing", "headless-editor", "collaborative-coding",
    "zero-unsafe-code", "myers-diff", "3-way-merge"
  ],
  "capabilities": [
    "buffer.open", "buffer.read", "buffer.patch", "buffer.cas_commit",
    "buffer.diff_unified", "buffer.3way_merge",
    "terminal.execute", "terminal.persistent_session", "terminal.interrupt",
    "collab.dispatch", "collab.scale_roster", "collab.stream_events",
    "mcp.headless_coedit", "mcp.diff_unified", "mcp.ide_rust_eval"
  ],
  "mcp_endpoint": "https://ide1d0.vercel.app/mcp",
  "mcp_transport": "streamable-http",
  "license": "AGPL-3.0-only",
  "repository": "https://github.com/UlyssesLeoLee/IDE1.0"
}
```

---

## MCP Server — Import Guide

MCP endpoint: `https://ide1d0.vercel.app/mcp` (GET for discovery, POST for JSON-RPC 2.0)

### Available Tools (10)

| Tool | Description |
|------|-------------|
| `headless_coedit` | Multi-provider concurrent co-editing (CAS fast-path + 3-way auto-merge) |
| `headless_read` | Read current or historical revision of a headless document |
| `headless_history` | Query version history, author attributions, timestamps |
| `headless_list_files` | List all active documents in headless memory |
| `diff_unified` | Myers unified diff between any two texts |
| `diff_revisions` | Unified diff between two historical revisions of a file |
| `ide_rust_eval` | Blender bpy-style Rust automation (`ide::context::*`, `ide::data::*`, `ide::ops::*`) |
| `ide_open_file` | Open or initialize a document in memory |
| `ide_edit_lines` | Replace a line range in a document (1-based) |
| `ide_replace_all` | Find and replace all occurrences in a document |

### Antigravity (`~/.gemini/config/mcp_config.json` or `~/.gemini/settings.json`)

```json
{
  "mcpServers": {
    "ide1d0-mcp": {
      "url": "https://ide1d0.vercel.app/mcp"
    }
  }
}
```

Local stdio variant (requires built binary):
```json
{
  "mcpServers": {
    "ide1d0-mcp-local": {
      "command": "cargo",
      "args": ["run", "-p", "ide-mcp", "--", "--mcp"]
    }
  }
}
```

### Claude Desktop (`%APPDATA%\Claude\claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "ide1d0": { "url": "https://ide1d0.vercel.app/mcp" }
  }
}
```

### Cursor (`.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "ide1d0-mcp": { "url": "https://ide1d0.vercel.app/mcp" }
  }
}
```

### VS Code / Roo Code / Cline

```json
{
  "mcpServers": {
    "ide1d0-mcp": { "url": "https://ide1d0.vercel.app/mcp" }
  }
}
```

### Verify connectivity

```bash
# Discovery
curl https://ide1d0.vercel.app/mcp

# Tool call — Myers diff
curl -X POST https://ide1d0.vercel.app/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"diff_unified","arguments":{"text_a":"old\n","text_b":"new\n"}}}'
```

---

## REST API

Base URL: `https://ide1d0.vercel.app/api/v1`

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Liveness probe |
| `/mcp` | GET | MCP server discovery |
| `/mcp` | POST | MCP JSON-RPC 2.0 tool call |
| `/events` | GET | SSE event stream |
| `/jobs/dispatch` | POST | Dispatch agent job |
| `/tasks/claim` | POST | Claim a task (returns fencing token) |
| `/patches/commit` | POST | Commit CAS patch |

OpenAPI spec: [`docs/02_详细设计/Router-Worker-API.openapi.yaml`](docs/02_详细设计/Router-Worker-API.openapi.yaml)

---

## CLI Invocation

```bash
# Open files (IPC-activates existing window, or cold-starts in ≤500ms)
IDE1.0.exe src/main.rs src/lib.rs

# Force new instance
IDE1.0.exe --new src/feature.rs

# Launch 3D setup wizard
IDE1.0.exe --setup

# Maintenance/repair wizard
IDE1.0.exe --maintenance

# MCP stdio mode
cargo run -p ide-mcp -- --mcp

# List MCP tools
cargo run -p ide-mcp -- --tools

# Eval a Rust console expression
cargo run -p ide-mcp -- --eval 'ide::context::status()'
```

---

## Environment Variables

| Variable | Type | Default | Purpose |
|----------|------|---------|---------|
| `IDE10_TRACE` | absolute path | — | Append JSON perf telemetry (startup time, memory peak, IPC events) |
| `IDE10_SETTINGS` | absolute path | `%APPDATA%/IDE1.0/settings.json` | Override settings path for sandbox/CI isolation |
| `IDE10_OFFLINE` / `IDE10_NO_NETWORK` | `1`/`true` | `0` | Disable all outbound network (air-gapped environments) |
| `IDE10_MULTI` | `1`/`true` | `0` | Allow multiple concurrent instances (bypass single-instance mutex) |
| `IDE10_BACKDROP` | `none`/`acrylic`/`mica` | system | Window compositor material (`none` for deterministic E2E screenshots) |

---

## IPC Wire Protocol

Named message to activate existing window and navigate to file:

```
IDE1.0-OPEN
<file_absolute_path>
<line_number>:<column_number>
```

Cross-process response latency: ≤12ms. Auto-focuses and activates the matching tab.

---

## CAS Patch Protocol

```json
{
  "action": "apply_patch",
  "doc_id": 1,
  "expected_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "fencing_token": 1042,
  "patches": [
    {
      "start_line": 45,
      "end_line": 52,
      "replacement": "    let result = compute(input);\n"
    }
  ]
}
```

If `expected_hash` mismatches current document, returns `CAS_CONFLICT` with latest diff context.

---

## Rust Console API (`ide::*` DSL)

Available in both `[RUST CONSOLE]` native tab and `ide_rust_eval` MCP tool.

```rust
// Context
ide::context::status()          // workspace status summary
ide::context::active_file()     // current active document path
ide::context::theme()           // current theme name

// Data
ide::data::list_files()         // list all open documents
ide::data::read(doc_id)         // read document text
ide::data::history(doc_id)      // version history

// Ops
ide::ops::new_doc(path, content)              // create document
ide::ops::edit_lines(doc_id, start, end, text) // replace line range
ide::ops::replace_all(doc_id, from, to)        // find & replace
ide::ops::diff(doc_id_a, doc_id_b)            // unified diff
ide::ops::set_theme(name)                      // switch theme
ide::ops::collab(n_agents)                     // scale agent roster
```

---

## Multi-Agent Scale Guarantees

- **Roster capacity**: 1 to 100,000 concurrent agents
- **CAS serialization**: file-level authority, no concurrent overwrites
- **AST-level parallelism**: multiple agents can concurrently edit different functions in the same file (Rust, Python, TypeScript, JavaScript, C, PowerShell)
- **Fencing token TTL**: 60s monotonically increasing; expired or preempted commits rejected with 412
- **Op idempotency**: `op_id` guards against duplicate network retries

---

## Build from Source

```powershell
# Requirements: Rust 1.77+, Windows (primary target)
$env:CARGO_TARGET_DIR = 'E:\DevCache\cargo\target_ide1'  # recommended

# Native desktop app
cargo build --release -p ide-fast

# MCP stdio server
cargo build --release -p ide-mcp

# Run all tests
cargo test --workspace

# Build Windows installer (requires WiX v3)
powershell ide-fast/packaging/build-msi.ps1
# Output: dist-fast/IDE1.0_0.8.6_x64.msi
#         dist-fast/IDE1.0_Setup_0.8.6_x64.exe
#         dist-fast/IDE1.0-0.8.6-windows-x64.exe
#         dist-fast/SHA256SUMS.txt
```

---

## Architecture

```
Agent / LLM / MCP Client / CLI
              │
   ┌──────────┴──────────┐
   │  MCP HTTP Gateway   │  https://ide1d0.vercel.app/mcp
   │  (Vercel Serverless)│  GET discovery · POST JSON-RPC 2.0
   └──────────┬──────────┘
              │
   ┌──────────┴──────────┐
   │  REST Agent API     │  /api/v1 (jobs · tasks · patches · events)
   └──────────┬──────────┘
              │
   ┌──────────┴──────────┐
   │  editcore (Rust)    │  Buffer · CAS · Myers diff · 3-way merge
   │  ide-mcp (Rust)     │  Stdio MCP server · CoEditWorkspace
   └──────────┬──────────┘
              │
   ┌──────────┴──────────┐
   │  ide-fast (Rust)    │  Native egui desktop · [RUST CONSOLE] tab
   └─────────────────────┘
```

**Four non-negotiable invariants:**
1. **File = consistency + routing boundary** — authority CAS-serialized by coordinator
2. **Semantic unit = work + lock boundary** — AST-level (function/type) concurrency
3. **Patch = delivery + verification boundary** — agents submit proposals, never overwrite
4. **Fencing token = lease protection** — monotonic, TTL-bounded, 412 on expiry

---

## License

AGPL-3.0-only. See [LICENSE](LICENSE).

Code signing: provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org). *(Pending approval — current release binaries are unsigned.)*

Privacy: IDE1.0 runs locally with no telemetry. Network calls occur only when you explicitly trigger them (GitHub update check on click; AI chat sends only your selected text to your own configured endpoint; API keys stored locally only).

---

## For Humans — Feature Summary

**Desktop app** (`IDE1.0.exe`, Windows x64):
- Native egui UI, sub-500ms cold start, 28MB single binary
- Full ANSI TrueColor terminal (16/256/24-bit RGB), PowerShell CLIXML error parsing
- Multi-cursor, minimap, breadcrumb, symbol outline, file tree
- AI chat panel with multi-provider support (DeepSeek, OpenAI, Anthropic, Ollama, custom)
- Multi-agent collaboration workbench (up to 100,000 agents)
- `[RUST CONSOLE]` tab with live `ide::*` DSL execution
- 3D cel-shaded diamond logo setup wizard
- Sticky notes, Markdown preview (Obsidian-style), i18n (zh/en/ja)
- Windows installer (MSI + Setup.exe) with Start Menu shortcut, silent install support

**Cloud / MCP**:
- Vercel serverless MCP endpoint (`https://ide1d0.vercel.app/mcp`)
- 10 MCP tools: headless co-editing, Myers diff, 3-way merge, Rust eval, file ops
- Cross-provider concurrent editing (Claude, GPT, DeepSeek, Gemini simultaneously)
- Auto-deploy on `git push` via Vercel Git integration

**Downloads** (v0.8.6):
- [GitHub Releases](https://github.com/UlyssesLeoLee/IDE1.0/releases/tag/v0.8.6)
  - `IDE1.0_Setup_0.8.6_x64.exe` — Installer with 3D wizard
  - `IDE1.0_0.8.6_x64.msi` — Silent enterprise MSI
  - `IDE1.0-0.8.6-windows-x64.exe` — Portable single-file
  - `SHA256SUMS.txt` — Checksums
