//! 统一云端 HTTP 与本地 Stdio MCP 工具契约定义 (P1-05)。

use serde_json::{json, Value};

fn tool_def(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn core_edit_tools() -> Vec<Value> {
    let coedit_schema = json!({
        "type": "object",
        "properties": {
            "file_id": { "type": "string", "description": "Document identifier, e.g. 'ws:src/main.rs'" },
            "author": { "type": "string", "description": "Editing agent or provider ID" },
            "agent_id": { "type": "string", "description": "Alias for author" },
            "base_revision": { "type": "integer", "minimum": 1, "description": "Required baseline revision number observed by the agent" },
            "base_content_hash": { "type": "string", "description": "Optional SHA-256 hash at base_revision for strict CAS verification" },
            "fencing_token": { "type": "integer", "minimum": 1, "description": "Optional active lease fencing token" },
            "op_id": { "type": "string", "minLength": 1, "description": "Required scoped operation idempotency key" },
            "new_text": { "type": "string", "description": "Proposed full document text" },
            "mode": { "type": "string", "enum": ["git", "nema"] }
        },
        "required": ["file_id", "base_revision", "op_id", "new_text"]
    });

    let line_edit_schema = json!({
        "type": "object",
        "properties": {
            "file_id": { "type": "string" },
            "provider": { "type": "string" },
            "agent_id": { "type": "string", "description": "Required editing agent identifier" },
            "author": { "type": "string", "description": "Alias for agent_id" },
            "base_revision": { "type": "integer", "minimum": 1 },
            "base_content_hash": { "type": "string" },
            "fencing_token": { "type": "integer", "minimum": 1 },
            "start_line": { "type": "integer", "minimum": 1 },
            "end_line": { "type": "integer", "minimum": 1 },
            "replacement": { "type": "string" },
            "op_id": { "type": "string", "minLength": 1 },
            "summary": { "type": "string" }
        },
        "required": ["file_id", "base_revision", "op_id", "start_line", "end_line", "replacement"]
    });

    let replace_all_schema = json!({
        "type": "object",
        "properties": {
            "file_id": { "type": "string" },
            "provider": { "type": "string" },
            "agent_id": { "type": "string" },
            "author": { "type": "string" },
            "base_revision": { "type": "integer", "minimum": 1 },
            "base_content_hash": { "type": "string" },
            "fencing_token": { "type": "integer", "minimum": 1 },
            "find": { "type": "string", "minLength": 1 },
            "replace": { "type": "string" },
            "op_id": { "type": "string", "minLength": 1 }
        },
        "required": ["file_id", "base_revision", "op_id", "find", "replace"]
    });

    vec![
        tool_def(
            "headless_open",
            "Open or register a document without overwriting an existing HEAD or resetting its revision.",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "content": { "type": "string" },
                    "language": { "type": "string" },
                    "provider": { "type": "string" },
                    "agent_id": { "type": "string" },
                    "mode": { "type": "string", "enum": ["create_if_absent", "open_existing", "force_overwrite"] },
                    "confirm_force_overwrite": { "type": "boolean", "description": "Required explicit true confirmation when mode='force_overwrite'" },
                    "force_token": { "type": "string", "description": "Authorization token when IDE_MCP_FORCE_TOKEN is configured" }
                },
                "required": ["file_id"]
            }),
        ),
        tool_def(
            "ide_open_file",
            "Compatibility alias for headless_open (never resets an existing document unless mode='force_overwrite').",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "content": { "type": "string" },
                    "language": { "type": "string" },
                    "agent_id": { "type": "string" },
                    "mode": { "type": "string", "enum": ["create_if_absent", "open_existing", "force_overwrite"] },
                    "confirm_force_overwrite": { "type": "boolean", "description": "Required explicit true confirmation when mode='force_overwrite'" },
                    "force_token": { "type": "string", "description": "Authorization token when IDE_MCP_FORCE_TOKEN is configured" }
                },
                "required": ["file_id"]
            }),
        ),
        tool_def(
            "headless_read",
            "Read the current or historical document text, revision, and SHA-256 content_hash.",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "revision": { "type": "integer", "minimum": 1 },
                    "start_line": { "type": "integer", "minimum": 1 },
                    "end_line": { "type": "integer", "minimum": 1 }
                },
                "required": ["file_id"]
            }),
        ),
        tool_def(
            "headless_coedit",
            "Atomic CAS + 3-way auto-merge full-document collaborative edit. Conflicts never mutate HEAD and instead create a review proposal.",
            coedit_schema,
        ),
        tool_def(
            "headless_edit",
            "Atomic line-range edit with CAS base_revision, scoped op_id idempotency, and safe 3-way auto-merge.",
            line_edit_schema.clone(),
        ),
        tool_def(
            "ide_edit_lines",
            "Atomic compatibility alias for headless_edit (1-based line replacement under CAS + OCC).",
            line_edit_schema,
        ),
        tool_def(
            "ide_replace_all",
            "Atomic CAS find-and-replace across a document with base_revision and op_id protection.",
            replace_all_schema,
        ),
    ]
}

fn coordination_and_disk_tools() -> Vec<Value> {
    vec![
        tool_def(
            "headless_three_way_merge",
            "Pure algorithm 3-way merge preview without mutating any document.",
            json!({
                "type": "object",
                "properties": {
                    "base": { "type": "string" },
                    "ours": { "type": "string" },
                    "theirs": { "type": "string" }
                },
                "required": ["base", "ours", "theirs"]
            }),
        ),
        tool_def(
            "headless_history",
            "Query revision history, content_hash, op_id, and agent attributions.",
            json!({
                "type": "object",
                "properties": { "file_id": { "type": "string" } },
                "required": ["file_id"]
            }),
        ),
        tool_def(
            "headless_list_files",
            "List active collaborative files in the DocumentStore.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool_def(
            "headless_list_proposals",
            "List pending or resolved conflict proposals saved without polluting HEAD.",
            json!({
                "type": "object",
                "properties": { "file_id": { "type": "string" } }
            }),
        ),
        tool_def(
            "headless_resolve_proposal",
            "Commit reviewed conflict resolution content via atomic CAS against base_revision.",
            json!({
                "type": "object",
                "properties": {
                    "proposal_id": { "type": "string" },
                    "resolved_content": { "type": "string" },
                    "base_revision": { "type": "integer", "minimum": 1 },
                    "agent_id": { "type": "string" },
                    "op_id": { "type": "string", "minLength": 1 }
                },
                "required": ["proposal_id", "resolved_content", "base_revision", "agent_id", "op_id"]
            }),
        ),
        tool_def(
            "headless_acquire_lease",
            "Acquire a monotonically increasing fencing-token lease for exclusive file coordination.",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "agent_id": { "type": "string" },
                    "ttl_ms": { "type": "integer", "minimum": 100 }
                },
                "required": ["file_id", "agent_id"]
            }),
        ),
        tool_def(
            "diff_unified",
            "Compute Myers Unified Diff between two texts (accepts old_text/new_text or text_a/text_b).",
            json!({
                "type": "object",
                "properties": {
                    "old_text": { "type": "string" },
                    "new_text": { "type": "string" },
                    "text_a": { "type": "string" },
                    "text_b": { "type": "string" },
                    "context_lines": { "type": "integer", "default": 3 }
                }
            }),
        ),
        tool_def(
            "diff_revisions",
            "Compute Unified Diff between two revisions (accepts from_revision/to_revision or rev_a/rev_b).",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "from_revision": { "type": "integer" },
                    "to_revision": { "type": "integer" },
                    "rev_a": { "type": "integer" },
                    "rev_b": { "type": "integer" },
                    "context_lines": { "type": "integer", "default": 3 }
                },
                "required": ["file_id"]
            }),
        ),
        tool_def(
            "ide_rust_eval",
            "Execute Blender-style ide::context / ide::data / ide::ops Rust scripts to inspect and programmatically modify shared documents.",
            json!({
                "type": "object",
                "properties": {
                    "script": { "type": "string" },
                    "agent_id": { "type": "string" }
                },
                "required": ["script"]
            }),
        ),
        tool_def(
            "ide_sync_to_disk",
            "Atomically flush verified DocumentStore HEAD to a real disk file (detecting external modifications).",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "disk_path": { "type": "string" },
                    "force": { "type": "boolean", "default": false }
                },
                "required": ["file_id"]
            }),
        ),
        tool_def(
            "ide_reload_from_disk",
            "Detect external disk edits and merge them safely into DocumentStore history.",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "disk_path": { "type": "string" },
                    "agent_id": { "type": "string" },
                    "op_id": { "type": "string" }
                },
                "required": ["file_id", "agent_id", "op_id"]
            }),
        ),
        tool_def(
            "ide_git_diff",
            "Inspect real git status and unified diff in the local workspace.",
            json!({
                "type": "object",
                "properties": {
                    "workspace_root": { "type": "string" },
                    "file_path": { "type": "string" }
                },
                "required": ["workspace_root"]
            }),
        ),
        tool_def(
            "ide_rollback_revision",
            "Safely roll back a document to target_revision by committing a new monotonic revision.",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "target_revision": { "type": "integer", "minimum": 1 },
                    "agent_id": { "type": "string" },
                    "op_id": { "type": "string" },
                    "sync_disk": { "type": "boolean", "default": false }
                },
                "required": ["file_id", "target_revision", "agent_id", "op_id"]
            }),
        ),
        tool_def(
            "ide_launch_browser",
            "Get or open IDE1.0 Web Workbench URL.",
            json!({
                "type": "object",
                "properties": {
                    "file_id": { "type": "string" },
                    "open_local": { "type": "boolean", "default": false }
                }
            }),
        ),
    ]
}

pub fn build_tools_catalog() -> Vec<Value> {
    let mut tools = core_edit_tools();
    tools.extend(coordination_and_disk_tools());
    for t in editcore::agent_api::TOOLS {
        let schema: Value = serde_json::from_str(t.schema).unwrap_or(json!({ "type": "object" }));
        tools.push(tool_def(t.name, t.description, schema));
    }
    tools
}
