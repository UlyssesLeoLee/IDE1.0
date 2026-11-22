//! MCP 工具分发与 P0-05 强 CAS 参数校验层。

use crate::diff;
use crate::merge::{
    self, DocumentStore, EditRequest, FullCoEditRequest, OpenMode, ReplaceAllRequest,
};
use crate::rust_console::RustConsoleEvaluator;
use serde_json::{json, Value};
use std::path::Path;

fn req_non_empty_str<'a>(args: &'a Value, keys: &[&str], label: &str) -> Result<&'a str, String> {
    for k in keys {
        if let Some(s) = args.get(*k).and_then(Value::as_str) {
            if !s.trim().is_empty() {
                return Ok(s);
            }
        }
    }
    Err(format!(
        "MISSING_REQUIRED_CAS_FIELD: '{label}' is required for this operation"
    ))
}

fn req_base_revision(args: &Value) -> Result<u64, String> {
    args.get("base_revision")
        .and_then(Value::as_u64)
        .filter(|&r| r >= 1)
        .ok_or_else(|| {
            "MISSING_REQUIRED_CAS_FIELD: 'base_revision' (integer >= 1) is required for write operations"
                .to_string()
        })
}

fn parse_open_mode(args: &Value) -> Result<OpenMode, String> {
    match args.get("mode").and_then(Value::as_str) {
        None | Some("create_if_absent") => Ok(OpenMode::CreateIfAbsent),
        Some("open_existing") => Ok(OpenMode::OpenExisting),
        Some("force_overwrite") => Ok(OpenMode::ForceOverwrite),
        Some(other) => Err(format!("INVALID_ARGUMENT: unknown open mode '{other}'")),
    }
}

pub fn dispatch_tool(
    store: &DocumentStore,
    web_url: &str,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    match name {
        "headless_open" | "ide_open_file" => handle_open(store, args),
        "headless_read" => handle_read(store, args),
        "headless_coedit" => handle_coedit(store, args),
        "headless_edit" | "ide_edit_lines" => handle_line_edit(store, args),
        "ide_replace_all" => handle_replace_all(store, args),
        "headless_three_way_merge" => handle_three_way_preview(args),
        "headless_history" => store.doc_history(req_non_empty_str(args, &["file_id"], "file_id")?),
        "headless_list_files" => Ok(store.list_docs()),
        "headless_list_proposals" => {
            Ok(store.list_proposals(args.get("file_id").and_then(Value::as_str)))
        }
        "headless_resolve_proposal" => handle_resolve_proposal(store, args),
        "headless_acquire_lease" => handle_acquire_lease(store, args),
        "diff_unified" => handle_diff_unified(args),
        "diff_revisions" => handle_diff_revisions(store, args),
        "ide_rust_eval" => handle_rust_eval(store, args),
        "ide_sync_to_disk" => handle_sync_to_disk(store, args),
        "ide_reload_from_disk" => handle_reload_from_disk(store, args),
        "ide_git_diff" => handle_git_diff(args),
        "ide_rollback_revision" => handle_rollback(store, args),
        "ide_launch_browser" => handle_launch_browser(web_url, args),
        _ => editcore::agent_api::call(name, args),
    }
}

fn handle_open(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let content = args.get("content").and_then(Value::as_str).unwrap_or("");
    let language = args
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("plain");
    let provider = args
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("agent");
    let agent_id = args
        .get("agent_id")
        .or_else(|| args.get("author"))
        .and_then(Value::as_str)
        .unwrap_or("worker");
    let mode = parse_open_mode(args)?;
    if mode == OpenMode::ForceOverwrite {
        let confirmed = args
            .get("confirm_force_overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !confirmed {
            return Err(
                "UNAUTHORIZED_FORCE_OVERWRITE: mode='force_overwrite' requires confirm_force_overwrite=true"
                    .to_string(),
            );
        }
        if let Ok(expected_token) = std::env::var("IDE_MCP_FORCE_TOKEN") {
            if !expected_token.is_empty() {
                let provided = args
                    .get("force_token")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if provided != expected_token {
                    return Err(
                        "UNAUTHORIZED_FORCE_OVERWRITE: invalid or missing force_token".to_string(),
                    );
                }
            }
        }
    }
    if let Some(disk_path) = args.get("disk_path").and_then(Value::as_str) {
        return store.open_disk_file(file_id, Path::new(disk_path), language, provider, agent_id);
    }
    store.open_doc_with_mode(file_id, content, language, provider, agent_id, mode)
}

fn handle_read(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let s = args
        .get("start_line")
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let e = args
        .get("end_line")
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    if let Some(rev) = args.get("revision").and_then(Value::as_u64) {
        return store.read_doc_revision(file_id, rev);
    }
    store.read_doc(file_id, s, e)
}

fn handle_coedit(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?.to_string();
    let author = req_non_empty_str(args, &["author", "agent_id"], "author/agent_id")?.to_string();
    let base_rev = req_base_revision(args)?;
    let op_id = req_non_empty_str(args, &["op_id"], "op_id")?.to_string();
    let new_text = args
        .get("new_text")
        .and_then(Value::as_str)
        .ok_or("Missing new_text")?
        .to_string();
    let mode = args
        .get("mode")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| format!("Invalid mode: {e}"))?;

    store.coedit_concurrent(FullCoEditRequest {
        file_id,
        author,
        new_text,
        base_revision: Some(base_rev),
        base_content_hash: args
            .get("base_content_hash")
            .and_then(Value::as_str)
            .map(str::to_string),
        fencing_token: args.get("fencing_token").and_then(Value::as_u64),
        op_id: Some(op_id),
        mode,
    })
}

fn handle_line_edit(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?.to_string();
    let agent_id = req_non_empty_str(args, &["agent_id", "author"], "agent_id/author")?.to_string();
    let base_rev = req_base_revision(args)?;
    let op_id = req_non_empty_str(args, &["op_id"], "op_id")?.to_string();
    let start_line = args
        .get("start_line")
        .and_then(Value::as_u64)
        .ok_or("Missing start_line")? as usize;
    let end_line = args
        .get("end_line")
        .and_then(Value::as_u64)
        .ok_or("Missing end_line")? as usize;
    let replacement = args
        .get("replacement")
        .and_then(Value::as_str)
        .ok_or("Missing replacement")?
        .to_string();

    store.edit_doc(EditRequest {
        file_id,
        provider: args
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("mcp")
            .to_string(),
        agent_id,
        base_revision: Some(base_rev),
        base_content_hash: args
            .get("base_content_hash")
            .and_then(Value::as_str)
            .map(str::to_string),
        fencing_token: args.get("fencing_token").and_then(Value::as_u64),
        start_line,
        end_line,
        replacement,
        op_id: Some(op_id),
        summary: args
            .get("summary")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn handle_replace_all(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let agent_id = req_non_empty_str(args, &["agent_id", "author"], "agent_id/author")?;
    let base_rev = req_base_revision(args)?;
    let op_id = req_non_empty_str(args, &["op_id"], "op_id")?;
    let find = args
        .get("find")
        .and_then(Value::as_str)
        .ok_or("Missing find")?;
    let replace = args
        .get("replace")
        .and_then(Value::as_str)
        .ok_or("Missing replace")?;
    let provider = args
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("mcp");

    store.replace_all_atomic(ReplaceAllRequest {
        file_id: file_id.to_string(),
        provider: provider.to_string(),
        agent_id: agent_id.to_string(),
        base_revision: Some(base_rev),
        base_content_hash: args
            .get("base_content_hash")
            .and_then(Value::as_str)
            .map(str::to_string),
        fencing_token: args.get("fencing_token").and_then(Value::as_u64),
        find: find.to_string(),
        replace: replace.to_string(),
        op_id: Some(op_id.to_string()),
    })
}

fn handle_three_way_preview(args: &Value) -> Result<Value, String> {
    let base = args
        .get("base")
        .and_then(Value::as_str)
        .ok_or("Missing base")?;
    let ours = args
        .get("ours")
        .and_then(Value::as_str)
        .ok_or("Missing ours")?;
    let theirs = args
        .get("theirs")
        .and_then(Value::as_str)
        .ok_or("Missing theirs")?;
    Ok(json!(merge::three_way_merge(base, ours, theirs)))
}

fn handle_resolve_proposal(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let pid = req_non_empty_str(args, &["proposal_id"], "proposal_id")?;
    let content = args
        .get("resolved_content")
        .and_then(Value::as_str)
        .ok_or("Missing resolved_content")?;
    let base_rev = req_base_revision(args)?;
    let agent_id = req_non_empty_str(args, &["agent_id", "author"], "agent_id/author")?;
    let op_id = req_non_empty_str(args, &["op_id"], "op_id")?;
    store.resolve_proposal(pid, content, base_rev, agent_id, op_id)
}

fn handle_acquire_lease(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let agent_id = req_non_empty_str(args, &["agent_id", "author"], "agent_id")?;
    let ttl_ms = args.get("ttl_ms").and_then(Value::as_u64).unwrap_or(60_000);
    store.acquire_lease(file_id, agent_id, ttl_ms)
}

fn handle_diff_unified(args: &Value) -> Result<Value, String> {
    let old_t = args
        .get("old_text")
        .or_else(|| args.get("text_a"))
        .and_then(Value::as_str)
        .ok_or("Missing old_text / text_a")?;
    let new_t = args
        .get("new_text")
        .or_else(|| args.get("text_b"))
        .and_then(Value::as_str)
        .ok_or("Missing new_text / text_b")?;
    let ctx = args
        .get("context_lines")
        .and_then(Value::as_u64)
        .unwrap_or(3) as usize;
    let d = diff::diff_text(old_t, new_t, ctx);
    let u = diff::unified(&d, "original", "modified");
    Ok(json!({
        "identical": d.identical,
        "added": d.added,
        "removed": d.removed,
        "coarse": d.coarse,
        "unified_diff": u
    }))
}

fn handle_diff_revisions(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let from_r = args
        .get("from_revision")
        .or_else(|| args.get("rev_a"))
        .and_then(Value::as_u64)
        .ok_or("Missing from_revision / rev_a")?;
    let to_r = args
        .get("to_revision")
        .or_else(|| args.get("rev_b"))
        .and_then(Value::as_u64)
        .ok_or("Missing to_revision / rev_b")?;
    let ctx = args
        .get("context_lines")
        .and_then(Value::as_u64)
        .unwrap_or(3) as usize;
    store.diff_revisions(file_id, from_r, to_r, ctx)
}

fn handle_rust_eval(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let script = args
        .get("script")
        .and_then(Value::as_str)
        .ok_or("Missing script")?;
    let res = RustConsoleEvaluator::eval(store, script);
    let steps_json: Vec<Value> = res
        .steps
        .iter()
        .map(|s| json!({ "expr": s.expr, "ok": s.ok, "output": s.output }))
        .collect();
    Ok(json!({
        "ok": res.ok,
        "summary": res.summary,
        "steps": steps_json
    }))
}

fn handle_sync_to_disk(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let disk_path = args.get("disk_path").and_then(Value::as_str).map(Path::new);
    let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
    store.sync_to_disk(file_id, disk_path, force)
}

fn handle_reload_from_disk(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let agent_id = req_non_empty_str(args, &["agent_id", "author"], "agent_id")?;
    let op_id = req_non_empty_str(args, &["op_id"], "op_id")?;
    let disk_path = args.get("disk_path").and_then(Value::as_str).map(Path::new);
    store.reload_from_disk(file_id, disk_path, agent_id, op_id)
}

fn handle_git_diff(args: &Value) -> Result<Value, String> {
    let root = req_non_empty_str(args, &["workspace_root"], "workspace_root")?;
    let file_path = args.get("file_path").and_then(Value::as_str);
    merge::git_diff_workspace(Path::new(root), file_path)
}

fn handle_rollback(store: &DocumentStore, args: &Value) -> Result<Value, String> {
    let file_id = req_non_empty_str(args, &["file_id"], "file_id")?;
    let target_rev = args
        .get("target_revision")
        .and_then(Value::as_u64)
        .ok_or("Missing target_revision")?;
    let agent_id = req_non_empty_str(args, &["agent_id", "author"], "agent_id")?;
    let op_id = req_non_empty_str(args, &["op_id"], "op_id")?;
    let sync_disk = args
        .get("sync_disk")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    store.rollback_revision(file_id, target_rev, agent_id, op_id, sync_disk)
}

fn handle_launch_browser(web_url: &str, args: &Value) -> Result<Value, String> {
    let file_id = args.get("file_id").and_then(Value::as_str);
    let open_local = args
        .get("open_local")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target_url = if let Some(fid) = file_id {
        format!("{web_url}/#file={}", url_encode(fid))
    } else {
        web_url.to_string()
    };

    if !(target_url.starts_with("https://") || target_url.starts_with("http://"))
        || target_url.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return Err(
            "INVALID_ARGUMENT: web_url must be a valid http(s) URL without control characters"
                .to_string(),
        );
    }

    if open_local {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("powershell")
                .creation_flags(CREATE_NO_WINDOW)
                .env("IDE_LAUNCH_URL", &target_url)
                .args([
                    "-NoProfile",
                    "-Command",
                    "Start-Process $env:IDE_LAUNCH_URL",
                ])
                .spawn();
        }
    }

    Ok(json!({
        "web_url": target_url,
        "launched_locally": open_local,
        "status": "READY"
    }))
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}
