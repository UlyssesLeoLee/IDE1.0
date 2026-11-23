use ide_mcp::merge::{
    atomic_write_file, check_syntax_integrity, nema_merge, three_way_merge, CoEditWorkspace,
    DocumentStore, FullCoEditRequest, OpenMode,
};
use ide_mcp::rust_console::RustConsoleEvaluator;
use ide_mcp::McpServer;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

struct EnvResetGuard {
    key: &'static str,
    prev: Option<String>,
}

impl EnvResetGuard {
    fn set(key: &'static str, val: &str) -> Self {
        let prev = std::env::var(key).ok();
        std::env::set_var(key, val);
        Self { key, prev }
    }
}

impl Drop for EnvResetGuard {
    fn drop(&mut self) {
        if let Some(ref p) = self.prev {
            std::env::set_var(self.key, p);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("ide_mcp_cov_{tag}_{}_{ns}", std::process::id()));
    let _ = fs::create_dir_all(&p);
    p
}

#[test]
fn test_mcp_server_jsonrpc_and_dispatch_all_tools() {
    let mut server = McpServer::new("https://example.test");
    assert!(!server.list_tools().is_empty());

    // Notifications & protocol error paths
    assert!(server.handle_jsonrpc(&json!({"jsonrpc": "2.0"})).is_none());
    assert!(server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .is_none());
    assert!(server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "method": "cancelled"}))
        .is_none());

    let ping = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}))
        .unwrap();
    assert_eq!(ping["result"], json!({}));

    let prompts = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 2, "method": "prompts/list"}))
        .unwrap();
    assert_eq!(prompts["result"]["prompts"].as_array().unwrap().len(), 2);

    let unknown_method = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 3, "method": "foo/bar"}))
        .unwrap();
    assert_eq!(unknown_method["error"]["code"], -32601);

    let missing_params = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call"}))
        .unwrap();
    assert_eq!(missing_params["error"]["code"], -32602);

    let missing_tool = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {}}))
        .unwrap();
    assert_eq!(missing_tool["error"]["code"], -32602);

    let unknown_tool = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "no_such_tool", "arguments": {}}}))
        .unwrap();
    assert_eq!(unknown_tool["result"]["isError"], true);

    // resources/list & resources/read variants
    let rlist = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 7, "method": "resources/list"}))
        .unwrap();
    assert!(rlist["result"]["resources"].as_array().unwrap().len() >= 2);

    let r_files = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 8, "method": "resources/read", "params": {"uri": "ide10://files"}}))
        .unwrap();
    assert!(r_files["result"]["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("default:src/math.rs"));

    let r_one = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 9, "method": "resources/read", "params": {"uri": "file:///workspace/default:src/math.rs"}}))
        .unwrap();
    assert!(r_one["result"]["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("pub fn add"));

    let r_bad_uri = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 10, "method": "resources/read", "params": {"uri": "ftp://bad"}}))
        .unwrap();
    assert_eq!(r_bad_uri["error"]["code"], -32602);

    let r_missing_doc = server
        .handle_jsonrpc(&json!({"jsonrpc": "2.0", "id": 11, "method": "resources/read", "params": {"uri": "ide10://file/no_such.rs"}}))
        .unwrap();
    assert_eq!(r_missing_doc["error"]["code"], -32602);

    // Dispatch open modes & IDE_MCP_FORCE_TOKEN guard
    assert!(server
        .call_tool(
            "headless_open",
            &json!({"file_id": "missing.rs", "mode": "open_existing"})
        )
        .is_err());
    assert!(server
        .call_tool(
            "headless_open",
            &json!({"file_id": "x.rs", "mode": "bogus"})
        )
        .is_err());

    let opened = server
        .call_tool(
            "ide_open_file",
            &json!({"file_id": "ws:notes.md", "language": "markdown", "content": "# Title\nhello\n"}),
        )
        .unwrap();
    assert_eq!(opened["status"], "CREATED");

    {
        let _tok = EnvResetGuard::set("IDE_MCP_FORCE_TOKEN", "secret-tok");
        assert!(server
            .call_tool(
                "headless_open",
                &json!({
                    "file_id": "ws:notes.md",
                    "content": "x",
                    "mode": "force_overwrite",
                    "confirm_force_overwrite": true,
                    "force_token": "wrong"
                })
            )
            .is_err());
        let ok_force = server
            .call_tool(
                "headless_open",
                &json!({
                    "file_id": "ws:notes.md",
                    "content": "# H1\nline2\n",
                    "mode": "force_overwrite",
                    "confirm_force_overwrite": true,
                    "force_token": "secret-tok"
                }),
            )
            .unwrap();
        assert_eq!(ok_force["status"], "OVERWRITTEN");
    }

    // Read by revision, line slice & empty slice
    let r_rev = server
        .call_tool(
            "headless_read",
            &json!({"file_id": "ws:notes.md", "revision": 1}),
        )
        .unwrap();
    assert!(r_rev["content"].as_str().unwrap().contains("Title"));
    let r_empty = server
        .call_tool(
            "headless_read",
            &json!({"file_id": "ws:notes.md", "start_line": 10, "end_line": 2}),
        )
        .unwrap();
    assert_eq!(r_empty["content"], "");

    // Acquire lease -> stale vs valid fencing token -> edit / replace_all / conflict / resolve / history / diff
    let lease = server
        .call_tool(
            "headless_acquire_lease",
            &json!({"file_id": "ws:notes.md", "agent_id": "alice", "ttl_ms": 5000}),
        )
        .unwrap();
    let tok = lease["fencing_token"].as_u64().unwrap();
    assert!(server
        .call_tool(
            "ide_edit_lines",
            &json!({
                "file_id": "ws:notes.md",
                "agent_id": "alice",
                "base_revision": 2,
                "fencing_token": tok + 99,
                "op_id": "bad-tok",
                "start_line": 2,
                "end_line": 2,
                "replacement": "line2_a"
            })
        )
        .is_err());

    let e_ok = server
        .call_tool(
            "ide_edit_lines",
            &json!({
                "file_id": "ws:notes.md",
                "agent_id": "alice",
                "base_revision": 2,
                "fencing_token": tok,
                "op_id": "op-e1",
                "start_line": 2,
                "end_line": 2,
                "replacement": "line2_a"
            }),
        )
        .unwrap();
    assert_eq!(e_ok["ok"], true);

    // NO_OP replace_all & actual replace_all + idempotency mismatch check
    let noop_rep = server
        .call_tool(
            "ide_replace_all",
            &json!({
                "file_id": "ws:notes.md",
                "agent_id": "alice",
                "base_revision": 3,
                "op_id": "op-rep-0",
                "find": "NOT_FOUND",
                "replace": "X"
            }),
        )
        .unwrap();
    assert_eq!(noop_rep["status"], "NO_OP");
    assert_eq!(
        server
            .call_tool(
                "ide_replace_all",
                &json!({
                    "file_id": "ws:notes.md",
                    "agent_id": "alice",
                    "base_revision": 3,
                    "op_id": "op-rep-0",
                    "find": "NOT_FOUND",
                    "replace": "X"
                }),
            )
            .unwrap()["idempotent_replay"],
        true
    );
    assert!(server
        .call_tool(
            "ide_replace_all",
            &json!({
                "file_id": "ws:notes.md",
                "agent_id": "alice",
                "base_revision": 3,
                "op_id": "op-rep-0",
                "find": "DIFF",
                "replace": "Y"
            }),
        )
        .is_err());
    assert!(server
        .call_tool(
            "ide_replace_all",
            &json!({
                "file_id": "ws:notes.md",
                "agent_id": "alice",
                "base_revision": 3,
                "op_id": "op-rep-empty",
                "find": "",
                "replace": "Y"
            }),
        )
        .is_err());

    let rep_ok = server
        .call_tool(
            "ide_replace_all",
            &json!({
                "file_id": "ws:notes.md",
                "agent_id": "alice",
                "base_revision": 3,
                "op_id": "op-rep-1",
                "find": "line2_a",
                "replace": "line2_b"
            }),
        )
        .unwrap();
    assert_eq!(rep_ok["replacements"], 1);

    // Invalid mode on headless_coedit
    assert!(server
        .call_tool(
            "headless_coedit",
            &json!({
                "file_id": "ws:notes.md",
                "author": "bob",
                "base_revision": 4,
                "op_id": "c-bad-mode",
                "new_text": "# H1\nline2_c\n",
                "mode": "unknown"
            }),
        )
        .is_err());

    // Conflict + list_proposals + resolve_proposal + re-resolve error
    let conf = server
        .call_tool(
            "headless_coedit",
            &json!({
                "file_id": "ws:notes.md",
                "author": "bob",
                "base_revision": 2,
                "op_id": "c-conf",
                "new_text": "# H1\nline2_bob\n"
            }),
        )
        .unwrap();
    assert_eq!(conf["status"], "CONFLICT");
    let pid = conf["proposal_id"].as_str().unwrap().to_string();

    let props = server
        .call_tool(
            "headless_list_proposals",
            &json!({"file_id": "ws:notes.md"}),
        )
        .unwrap();
    assert_eq!(props["proposals"].as_array().unwrap().len(), 1);

    let resolved = server
        .call_tool(
            "headless_resolve_proposal",
            &json!({
                "proposal_id": pid,
                "resolved_content": "# H1\nline2_merged\n",
                "base_revision": 4,
                "agent_id": "reviewer",
                "op_id": "res-1"
            }),
        )
        .unwrap();
    assert_eq!(resolved["ok"], true);
    assert!(server
        .call_tool(
            "headless_resolve_proposal",
            &json!({
                "proposal_id": pid,
                "resolved_content": "x",
                "base_revision": 5,
                "agent_id": "reviewer",
                "op_id": "res-2"
            }),
        )
        .is_err());

    // Other stateless / query tools + editcore delegation + browser validation
    assert_eq!(
        server
            .call_tool(
                "headless_three_way_merge",
                &json!({"base": "a\n", "ours": "a1\n", "theirs": "a\nb\n"})
            )
            .unwrap()["clean"],
        true
    );
    assert!(
        server
            .call_tool("headless_history", &json!({"file_id": "ws:notes.md"}))
            .unwrap()["history"]
            .as_array()
            .unwrap()
            .len()
            >= 4
    );
    assert!(server.call_tool("headless_list_files", &json!({})).unwrap()["docs"].is_array());
    assert!(server
        .call_tool(
            "diff_unified",
            &json!({"text_a": "a\n", "text_b": "b\n", "context_lines": 1})
        )
        .unwrap()["unified_diff"]
        .as_str()
        .unwrap()
        .contains("-a"));
    assert!(server
        .call_tool(
            "diff_revisions",
            &json!({"file_id": "ws:notes.md", "rev_a": 1, "rev_b": 4})
        )
        .unwrap()["unified_diff"]
        .as_str()
        .is_some());
    assert_eq!(
        server
            .call_tool("md_outline", &json!({"text": "# H1\n## H2\n"}))
            .unwrap()["outline"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let br_ok = server
        .call_tool(
            "ide_launch_browser",
            &json!({"file_id": "ws:notes.md", "open_local": false}),
        )
        .unwrap();
    assert!(br_ok["web_url"].as_str().unwrap().contains("ws%3Anotes.md"));
    assert!(server
        .call_tool("ide_launch_browser", &json!({"open_local": false}))
        .is_ok());

    let mut bad_url_srv = McpServer::new("ftp://invalid");
    assert!(bad_url_srv
        .call_tool("ide_launch_browser", &json!({}))
        .is_err());
    let mut ctrl_url_srv = McpServer::new("https://ok.test\n");
    assert!(ctrl_url_srv
        .call_tool("ide_launch_browser", &json!({}))
        .is_err());
}

#[test]
fn test_disk_sync_reload_rollback_git_diff_and_compat_workspace() {
    let dir = temp_dir("disk_git");
    let file_path = dir.join("sub").join("doc.rs");
    atomic_write_file(&file_path, "pub fn a() {\n    let x = 1;\n}\n").unwrap();

    // Workspace root restriction check
    {
        let _root = EnvResetGuard::set("IDE_MCP_WORKSPACE_ROOT", &dir.to_string_lossy());
        let mut server = McpServer::default();
        assert!(server
            .call_tool(
                "ide_open_file",
                &json!({"file_id": "d:outside", "disk_path": "C:\\outside_forbidden.txt"})
            )
            .is_err());
        assert!(server
            .call_tool(
                "ide_open_file",
                &json!({"file_id": "d:trav", "disk_path": "../escape.txt"})
            )
            .is_err());
    }

    // Initialize real temp git repo for `ide_git_diff`
    let git_init = Command::new("git").arg("init").current_dir(&dir).output();
    if let Ok(out) = git_init {
        if out.status.success() {
            let _ = Command::new("git")
                .args(["config", "user.email", "test@example.com"])
                .current_dir(&dir)
                .output();
            let _ = Command::new("git")
                .args(["config", "user.name", "Test"])
                .current_dir(&dir)
                .output();
            let _ = Command::new("git")
                .args(["config", "core.fsmonitor", "false"])
                .current_dir(&dir)
                .output();
            let _ = Command::new("git")
                .args(["config", "commit.gpgsign", "false"])
                .current_dir(&dir)
                .output();
            let _ = Command::new("git")
                .args(["add", "."])
                .current_dir(&dir)
                .output();
            let _ = Command::new("git")
                .args([
                    "-c",
                    "core.fsmonitor=false",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-m",
                    "init",
                ])
                .current_dir(&dir)
                .output();
        }
    }

    let mut server = McpServer::default();
    let opened = server
        .call_tool(
            "headless_open",
            &json!({
                "file_id": "disk:doc.rs",
                "disk_path": file_path.to_string_lossy(),
                "language": "rust"
            }),
        )
        .unwrap();
    assert!(opened["disk_path"].as_str().is_some());

    // Reload when up-to-date
    let up = server
        .call_tool(
            "ide_reload_from_disk",
            &json!({"file_id": "disk:doc.rs", "agent_id": "watcher", "op_id": "rel-0"}),
        )
        .unwrap();
    assert_eq!(up["status"], "DISK_UP_TO_DATE");

    // In-memory coedit -> sync_to_disk -> external disk tamper -> reject without force -> force sync
    server
        .call_tool(
            "headless_coedit",
            &json!({
                "file_id": "disk:doc.rs",
                "author": "agent",
                "base_revision": 1,
                "op_id": "mem-1",
                "new_text": "pub fn a() {\n    let x = 2;\n}\n"
            }),
        )
        .unwrap();
    let synced = server
        .call_tool(
            "ide_sync_to_disk",
            &json!({"file_id": "disk:doc.rs", "force": false}),
        )
        .unwrap();
    assert_eq!(synced["status"], "SYNCED_TO_DISK");

    // Simulate external disk edit (append comment at end)
    fs::write(
        &file_path,
        "pub fn a() {\n    let x = 2;\n}\n// disk tail\n",
    )
    .unwrap();
    let blocked = server.call_tool(
        "ide_sync_to_disk",
        &json!({"file_id": "disk:doc.rs", "force": false}),
    );
    assert!(blocked.unwrap_err().contains("EXTERNAL_DISK_MODIFICATION"));

    // Reload external disk edit into memory, then edit & rollback with sync_disk=true
    let reloaded = server
        .call_tool(
            "ide_reload_from_disk",
            &json!({"file_id": "disk:doc.rs", "agent_id": "watcher", "op_id": "rel-1"}),
        )
        .unwrap();
    assert_eq!(reloaded["ok"], true);

    let rolled = server
        .call_tool(
            "ide_rollback_revision",
            &json!({
                "file_id": "disk:doc.rs",
                "target_revision": 1,
                "agent_id": "ops",
                "op_id": "rb-1",
                "sync_disk": true
            }),
        )
        .unwrap();
    assert_eq!(rolled["rolled_back_to"], 1);
    assert_eq!(rolled["disk_sync"]["status"], "SYNCED_TO_DISK");
    assert!(server
        .call_tool(
            "ide_rollback_revision",
            &json!({"file_id": "disk:doc.rs", "target_revision": 999, "agent_id": "ops", "op_id": "rb-bad"})
        )
        .is_err());

    // Create pending conflict proposal -> sync_to_disk must block with PENDING_CONFLICT_PROPOSALS
    let conf = server
        .call_tool(
            "headless_coedit",
            &json!({
                "file_id": "disk:doc.rs",
                "author": "stale",
                "base_revision": 2,
                "op_id": "disk-conf",
                "new_text": "pub fn a() {\n    let x = 999;\n}\n"
            }),
        )
        .unwrap();
    assert_eq!(conf["status"], "CONFLICT");
    assert!(server
        .call_tool("ide_sync_to_disk", &json!({"file_id": "disk:doc.rs"}))
        .unwrap_err()
        .contains("PENDING_CONFLICT_PROPOSALS"));

    // Git diff against temp repo + non-existent dir error
    let _ = server.call_tool(
        "ide_git_diff",
        &json!({"workspace_root": dir.to_string_lossy(), "file_path": "sub/doc.rs"}),
    );
    let empty_non_git = temp_dir("nongit");
    let _ = fs::remove_dir_all(&empty_non_git);
    let _ = fs::remove_dir_all(&dir);

    // CoEditWorkspace compat wrapper
    let shared = Arc::new(DocumentStore::new());
    let mut compat = CoEditWorkspace::from_store(Arc::clone(&shared));
    assert!(Arc::ptr_eq(compat.store(), &shared));
    assert!(compat.get_doc("default:src/math.rs").is_some());
    assert_eq!(
        compat.open_doc("c:a.txt", "a\n", "plain", "p", "u")["status"],
        "CREATED"
    );
    assert!(compat
        .open_doc_with_mode("c:a.txt", "b\n", "plain", "p", "u", OpenMode::OpenExisting)
        .is_ok());
    assert!(compat.read_doc("c:a.txt", None, None).is_ok());
    assert!(compat
        .edit_doc(ide_mcp::merge::EditRequest {
            file_id: "c:a.txt".into(),
            provider: "p".into(),
            agent_id: "u".into(),
            base_revision: Some(1),
            start_line: 1,
            end_line: 1,
            replacement: "a1".into(),
            op_id: Some("ce1".into()),
            ..Default::default()
        })
        .is_ok());
    assert!(compat
        .coedit_full(FullCoEditRequest {
            file_id: "c:a.txt".into(),
            author: "u".into(),
            base_revision: Some(2),
            new_text: "a2\n".into(),
            op_id: Some("ce2".into()),
            ..Default::default()
        })
        .is_ok());
    assert!(compat.diff_revisions("c:a.txt", 1, 3, 2).is_ok());
    assert!(compat.list_docs()["docs"].is_array());
    assert!(compat.doc_history("c:a.txt").is_ok());
}

#[test]
fn test_algo_syntax_nema_merge_and_rust_console_all_paths() {
    // Algo: fast path where theirs is strictly before ours (b1 <= a0), multi-hunk cluster, identical overlap
    let base = "1\n2\n3\n4\n5\n6\n";
    let ours = "1\n2\n3\n4\n5_o\n6\n";
    let theirs = "1_t\n2\n3\n4\n5\n6\n";
    let fast = three_way_merge(base, ours, theirs);
    assert!(fast.clean);
    assert!(fast.merged_text.contains("1_t") && fast.merged_text.contains("5_o"));

    let ident = three_way_merge("a\nb\nc\n", "a\nb1\nc1\n", "a\nb1\nc1\n");
    assert!(ident.clean);
    let cluster = three_way_merge(
        "1\n2\n3\n4\n5\n6\n7\n8\n",
        "1\n2o\n3\n4o\n5\n6o\n7\n8\n",
        "1\n2\n3t\n4t\n5\n6t\n7t\n8\n",
    );
    assert!(!cluster.clean);

    // Syntax integrity branches
    assert!(check_syntax_integrity("rust", "<<<<<<< HEAD").is_err());
    assert!(check_syntax_integrity("rust", ">>>>>>> B").is_err());
    assert!(check_syntax_integrity("json", "{\"k\":1}").is_ok());
    assert!(check_syntax_integrity("json", "{bad").is_err());
    assert!(check_syntax_integrity("js", "const s = `hi`; // ok\n").is_ok());
    assert!(check_syntax_integrity("js", "fn(}").is_err());
    assert!(check_syntax_integrity("js", "fun())").is_err());
    assert!(check_syntax_integrity("js", "const s = \"unclosed").is_err());
    assert!(check_syntax_integrity(
        "rust",
        "/* a /* b\n */ c */ fn f<'a>() { let c = '\\''; let r = r#\"x\"#; }"
    )
    .is_ok());
    assert!(check_syntax_integrity("rust", "/* unclosed").is_err());
    assert!(check_syntax_integrity("rust", "fn x() { let s = r#\"unclosed").is_err());
    assert!(check_syntax_integrity("python", "def f(): pass").is_ok());

    // NEMA merge: dict edits, header edits, domain add/delete, Graph conflicts, fallback post-merge check
    let n_base = "\t$1\nd0\n\t|T1 1\n\\%2\n\\# K V\nA 1\nB 2\n\t>G1 1\nN1 x\nN1>e1>N1 rel\n\n\n";
    let n_ours = "\t$1\nd0\n\t|T1 1\n\\%2\n\\# K V\nA 1\n\t>G1 1\nN1 x_o\nN2 y\nN1>e1>N1 rel\n\n\n";
    let n_theirs =
        "\t$1\nd1\n\t|T1 2\n\\%2\n\\# K V\nA 1\nB 2\n\t>G1 1\nN1 x\nN3 z\nN1>e1>N1 rel2\n\n\n";
    let m_nema = nema_merge(n_base, n_ours, n_theirs).unwrap();
    assert!(m_nema.merged_text.contains("T1"));

    let n_clean_base = "\t|T1 1\n\\%2\n\\# K V\nA 1\nB 2\n\n\n";
    let n_clean_o = "\t|T1 1\n\\%2\n\\# K V\nA 1_o\nB 2\n\n\n";
    let n_clean_t = "\t|T1 1\n\\%2\n\\# K V\nA 1\nB 2_t\n\n\n";
    let m_clean = nema_merge(n_clean_base, n_clean_o, n_clean_t).unwrap();
    assert!(m_clean.clean);

    assert!(nema_merge(n_clean_base, "bad", n_clean_t).is_err());
    assert!(nema_merge(n_clean_base, n_clean_o, "bad").is_err());
    assert!(nema_merge("broken base\n", n_clean_o, n_clean_t).is_ok());

    // RustConsoleEvaluator: every error guard, empty script, comments, quotes, outline, no-trailing-newline insert
    let store = DocumentStore::new();
    let empty_res = RustConsoleEvaluator::eval_with_agent(&store, "  // comment only\n", "  ");
    assert!(empty_res.ok && empty_res.summary.contains("Empty"));

    let script_sweep = r##"
        ide::context::help();
        ide::context::status();
        ide::data::files();
        ide::data::read();
        ide::data::read("missing.rs");
        ide::data::lines("missing.rs");
        ide::data::history();
        ide::data::history("default:src/math.rs");
        ide::ops::open();
        ide::ops::open("r:notrail.md", "markdown", "# Title");
        ide::ops::edit();
        ide::ops::edit("r:notrail.md", bad, 1, "x");
        ide::ops::edit("r:notrail.md", 1, bad, "x");
        ide::ops::insert();
        ide::ops::insert("r:notrail.md", bad, "x");
        ide::ops::insert("r:notrail.md", 99, "tail");
        ide::ops::append();
        ide::ops::append("r:notrail.md", "more\nline");
        ide::ops::delete_lines();
        ide::ops::delete_lines("r:notrail.md", bad, 1);
        ide::ops::delete_lines("r:notrail.md", 1, bad);
        ide::ops::delete_lines("r:notrail.md", 3, 1);
        ide::ops::delete_lines("r:notrail.md", 2, 2);
        ide::ops::coedit();
        ide::ops::coedit("r:notrail.md", bad_rev, "x");
        ide::ops::coedit("r:notrail.md", 1, "# Conf\n");
        ide::ops::coedit("r:notrail.md", "# CleanCoedit\nlineA\n");
        ide::ops::replace_all();
        ide::ops::replace_all("r:notrail.md", "lineA", "lineB");
        ide::ops::diff();
        ide::ops::diff("r:notrail.md", 1, 2);
        ide::ops::search();
        ide::ops::search("r:notrail.md", "CleanCoedit");
        ide::ops::outline("r:notrail.md");
        ide::ops::outline("missing.md");
        bad::stmt();
    "##;
    let res = RustConsoleEvaluator::eval_with_agent(&store, script_sweep, "sweeper");
    assert!(!res.ok);
    assert!(res.steps.iter().any(|s| s.output.contains("CONFLICT")));
    assert!(res.steps.iter().any(|s| s.output.contains("Unrecognized")));
}

#[test]
fn test_ide_mcp_binary_cli_and_stdio_jsonrpc() {
    let bin = env!("CARGO_BIN_EXE_ide-mcp");

    let ver = Command::new(bin).arg("--version").output().unwrap();
    assert!(ver.status.success());
    assert!(String::from_utf8_lossy(&ver.stdout).contains("ide10-mcp"));

    let tools = Command::new(bin).arg("--tools").output().unwrap();
    assert!(tools.status.success());
    let parsed_tools: Value = serde_json::from_slice(&tools.stdout).unwrap();
    assert!(parsed_tools.is_array());

    let ev_ok = Command::new(bin)
        .args(["--eval", "ide::context::status()"])
        .output()
        .unwrap();
    assert!(ev_ok.status.success());

    let ev_fail = Command::new(bin)
        .args(["--eval", "unknown::fn()"])
        .output()
        .unwrap();
    assert!(!ev_fail.status.success());

    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin);
        let _ = writeln!(stdin, "{{not json");
        let _ = writeln!(
            stdin,
            "{}",
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        );
        let _ = writeln!(
            stdin,
            "{}",
            json!({"jsonrpc": "2.0", "id": 42, "method": "ping"})
        );
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("-32700"));
    assert!(stdout.contains("\"id\":42"));
}
