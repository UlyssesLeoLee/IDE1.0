use super::*;
use std::process::Command;

#[test]
fn test_three_way_merge_clean_non_overlapping() {
    let base = "line1\nline2\nline3\nline4\nline5\n";
    let ours = "line1_modified_by_claude\nline2\nline3\nline4\nline5\n";
    let theirs = "line1\nline2\nline3\nline4\nline5_modified_by_gpt\n";

    let m = three_way_merge(base, ours, theirs);
    assert!(m.clean);
    assert_eq!(
        m.merged_text,
        "line1_modified_by_claude\nline2\nline3\nline4\nline5_modified_by_gpt\n"
    );
    assert_eq!(m.conflicts.len(), 0);
}

#[test]
fn test_three_way_merge_multi_insert_delete_and_line_drift() {
    let base = "a\nb\nc\nd\ne\nf\n";
    let ours = "a\ninserted1\ninserted2\nb\nc\nd\ne\nf\n";
    let theirs = "a\nb\nc\ne\nf\nappended_tail\n";

    let m = three_way_merge(base, ours, theirs);
    assert!(
        m.clean,
        "Disjoint multi-insert + delete should merge cleanly"
    );
    assert_eq!(
        m.merged_text,
        "a\ninserted1\ninserted2\nb\nc\ne\nf\nappended_tail\n"
    );
}

#[test]
fn test_three_way_merge_partial_overlap_reports_conflict() {
    let base = "1\n2\n3\n4\n5\n";
    let ours = "1\n2_ours\n3_ours\n4\n5\n";
    let theirs = "1\n2\n3_theirs\n4_theirs\n5\n";

    let m = three_way_merge(base, ours, theirs);
    assert!(!m.clean);
    assert_eq!(m.conflicts.len(), 1);
    assert!(m.merged_text.contains("<<<<<<< HEAD (ours)"));
}

#[test]
fn test_three_way_merge_identical_changes_coalesce() {
    let base = "line1\nline2\n";
    let ours = "line1\nline2_new\n";
    let theirs = "line1\nline2_new\n";

    let m = three_way_merge(base, ours, theirs);
    assert!(m.clean);
    assert_eq!(m.merged_text, "line1\nline2_new\n");
}

#[test]
fn test_coedit_cross_provider_concurrent_auto_merge() {
    let store = DocumentStore::default();
    let file = "default:src/math.rs";

    let r1 = store
        .edit_doc(EditRequest {
            file_id: file.into(),
            provider: "anthropic".into(),
            agent_id: "claude".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: None,
            start_line: 1,
            end_line: 3,
            replacement: "pub fn add(a: i32, b: i32) -> i32 {\n    a.saturating_add(b)\n}".into(),
            op_id: Some("op-claude-1".into()),
            summary: Some("Use saturating_add".into()),
        })
        .unwrap();
    assert_eq!(r1["status"], "COMMITTED");
    assert_eq!(r1["new_revision"], 2);

    let r2 = store
        .edit_doc(EditRequest {
            file_id: file.into(),
            provider: "openai".into(),
            agent_id: "gpt-4o".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: None,
            start_line: 5,
            end_line: 7,
            replacement: "pub fn multiply(a: i32, b: i32) -> i32 {\n    a.saturating_mul(b)\n}"
                .into(),
            op_id: Some("op-gpt-1".into()),
            summary: Some("Use saturating_mul".into()),
        })
        .unwrap();
    assert_eq!(r2["status"], "COMMITTED");
    assert_eq!(r2["new_revision"], 3);
    assert_eq!(r2["auto_merged"], true);

    let read = store.read_doc(file, None, None).unwrap();
    let content = read["content"].as_str().unwrap();
    assert!(content.contains("saturating_add"));
    assert!(content.contains("saturating_mul"));
}

#[test]
fn test_p0_01_conflict_does_not_commit_and_creates_proposal() {
    let store = DocumentStore::default();
    let file = "default:src/math.rs";

    store
        .edit_doc(EditRequest {
            file_id: file.into(),
            provider: "deepseek".into(),
            agent_id: "deepseek-r1".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: None,
            start_line: 2,
            end_line: 2,
            replacement: "    a + b + 0".into(),
            op_id: Some("op-ds".into()),
            summary: None,
        })
        .unwrap();

    let head_before = store.read_doc(file, None, None).unwrap();
    let res = store
        .coedit_concurrent(FullCoEditRequest {
            file_id: file.into(),
            author: "gemini".into(),
            new_text: "pub fn add(a: i32, b: i32) -> i32 {\n    a + b + 42\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: None,
            op_id: Some("op-gem".into()),
            mode: Some(CollabMode::Git),
        })
        .unwrap();

    assert_eq!(res["ok"], false);
    assert_eq!(res["status"], "CONFLICT");
    let pid = res["proposal_id"].as_str().unwrap().to_string();
    assert!(!pid.is_empty());

    let head_after = store.read_doc(file, None, None).unwrap();
    assert_eq!(head_after["current_revision"], 2);
    assert_eq!(head_after["content_hash"], head_before["content_hash"]);
    assert!(!head_after["content"].as_str().unwrap().contains("<<<<<<<"));

    let resolved = store
        .resolve_proposal(
            &pid,
            "pub fn add(a: i32, b: i32) -> i32 {\n    a + b + 99\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n",
            2,
            "reviewer",
            "op-resolve-1",
        )
        .unwrap();
    assert_eq!(resolved["ok"], true);
    assert_eq!(resolved["new_revision"], 3);
}

#[test]
fn test_p0_04_reopen_existing_preserves_revisions() {
    let store = DocumentStore::default();
    let f = "ws1:file.rs";
    let c1 = store
        .open_doc_with_mode(
            f,
            "fn main() {}\n",
            "rust",
            "sys",
            "a",
            OpenMode::CreateIfAbsent,
        )
        .unwrap();
    assert_eq!(c1["status"], "CREATED");
    assert_eq!(c1["current_revision"], 1);

    store
        .coedit_concurrent(FullCoEditRequest {
            file_id: f.into(),
            author: "agent1".into(),
            new_text: "fn main() { println!(\"hi\"); }\n".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: None,
            op_id: Some("op1".into()),
            mode: Some(CollabMode::Git),
        })
        .unwrap();

    let reopen = store
        .open_doc_with_mode(
            f,
            "RESET CONTENT",
            "rust",
            "sys",
            "b",
            OpenMode::CreateIfAbsent,
        )
        .unwrap();
    assert_eq!(reopen["status"], "ALREADY_EXISTS");
    assert_eq!(reopen["created"], false);
    assert_eq!(reopen["current_revision"], 2);
    assert!(reopen["content"].as_str().unwrap().contains("println!"));
}

#[test]
fn test_p0_05_scoped_idempotency_mismatch_and_fencing_token() {
    let store = DocumentStore::default();
    let f1 = "ws:a.rs";
    let f2 = "ws:b.rs";
    store.open_doc(f1, "fn a() {}\n", "rust", "sys", "init");
    store.open_doc(f2, "fn b() {}\n", "rust", "sys", "init");

    let lease = store.acquire_lease(f1, "agent-lease", 60_000).unwrap();
    let tok = lease["fencing_token"].as_u64().unwrap();

    let bad_tok = store.coedit_concurrent(FullCoEditRequest {
        file_id: f1.into(),
        author: "agent".into(),
        new_text: "fn a() { 1 }\n".into(),
        base_revision: Some(1),
        base_content_hash: None,
        fencing_token: Some(tok + 99),
        op_id: Some("op-shared".into()),
        mode: Some(CollabMode::Git),
    });
    assert!(bad_tok.is_err());

    let ok1 = store
        .coedit_concurrent(FullCoEditRequest {
            file_id: f1.into(),
            author: "agent".into(),
            new_text: "fn a() { 1 }\n".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: Some(tok),
            op_id: Some("op-shared".into()),
            mode: Some(CollabMode::Git),
        })
        .unwrap();
    assert_eq!(ok1["revision"], 2);

    // Same op_id + different payload on SAME file must be rejected
    let mismatch = store.coedit_concurrent(FullCoEditRequest {
        file_id: f1.into(),
        author: "agent".into(),
        new_text: "fn a() { 999 }\n".into(),
        base_revision: Some(1),
        base_content_hash: None,
        fencing_token: Some(tok),
        op_id: Some("op-shared".into()),
        mode: Some(CollabMode::Git),
    });
    assert!(mismatch.is_err());

    // Same op_id on DIFFERENT file must succeed cleanly
    let ok2 = store
        .coedit_concurrent(FullCoEditRequest {
            file_id: f2.into(),
            author: "agent".into(),
            new_text: "fn b() { 2 }\n".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: None,
            op_id: Some("op-shared".into()),
            mode: Some(CollabMode::Git),
        })
        .unwrap();
    assert_eq!(ok2["revision"], 2);
}

#[test]
fn test_p1_06_nema_official_tab_syntax_table_5w1h_graph_merge() {
    let base = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 100\n\t?W1 1\nBuild auth flow user today edge\n\t>G1 1\nLoginAPI node Entrypoint\nAuthSvc node Backend\nLoginAPI>Call1>AuthSvc edge Auth\n\n\n";
    let ours = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 100\nBeta 200\n\t?W1 1\nBuild auth flow user today edge\n\t>G1 1\nLoginAPI node Entrypoint\nAuthSvc node Backend\nLoginAPI>Call1>AuthSvc edge Auth\n\n\n";
    let theirs = "\t|T1 1\n\\%2\n\\# Key Val\nAlpha 100\n\t?W1 1\nBuild auth flow user today edge\n\t>G1 1\nLoginAPI node Entrypoint\nAuthSvc node Backend\nTokenSvc node Cache\nLoginAPI>Call1>AuthSvc edge Auth\n\n\n";

    let m = nema_merge(base, ours, theirs).unwrap();
    assert!(
        m.clean,
        "Domain-aware merge must succeed: {:?}",
        m.conflicts
    );
    assert!(nema_codec::parse_source(&m.merged_text).is_ok());
    assert!(m.merged_text.contains("Beta 200"));
    assert!(m.merged_text.contains("TokenSvc node Cache"));
}

#[test]
fn test_p1_04_real_disk_sync_external_mod_and_git_diff() {
    let temp = std::env::temp_dir().join(format!("ide10_mcp_disk_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();

    Command::new("git")
        .current_dir(&temp)
        .arg("init")
        .output()
        .unwrap();
    let file_path = temp.join("sample.rs");
    std::fs::write(&file_path, "pub fn one() -> i32 { 1 }\n").unwrap();
    Command::new("git")
        .current_dir(&temp)
        .args(["add", "sample.rs"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(&temp)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "init",
        ])
        .output()
        .unwrap();

    let store = DocumentStore::default();
    let fid = "audit:sample.rs";
    store
        .open_disk_file(fid, &file_path, "rust", "sys", "init")
        .unwrap();

    store
        .coedit_concurrent(FullCoEditRequest {
            file_id: fid.into(),
            author: "agent-disk".into(),
            new_text: "pub fn one() -> i32 { 42 }\n".into(),
            base_revision: Some(1),
            base_content_hash: None,
            fencing_token: None,
            op_id: Some("op-disk-1".into()),
            mode: Some(CollabMode::Git),
        })
        .unwrap();

    let sync_ok = store.sync_to_disk(fid, None, false).unwrap();
    assert_eq!(sync_ok["status"], "SYNCED_TO_DISK");
    let backup_dir = temp.join(".ide10-backup");
    assert!(backup_dir.exists(), "Pre-save backup dir must be created");
    assert!(
        std::fs::read_dir(&backup_dir).unwrap().count() >= 1,
        "Pre-save backup file must exist before overwrite"
    );

    assert!(disk::validate_disk_path(std::path::Path::new("../escape.rs")).is_err());
    assert!(disk::validate_disk_path(std::path::Path::new("bad\0name.rs")).is_err());

    let diff_res = git_diff_workspace(&temp, Some("sample.rs")).unwrap();
    assert_eq!(diff_res["has_changes"], true);
    assert!(diff_res["unified_diff"]
        .as_str()
        .unwrap()
        .contains("+pub fn one() -> i32 { 42 }"));

    // Simulate external edit on disk -> sync without force must detect and reject!
    std::fs::write(&file_path, "pub fn one() -> i32 { 777 }\n").unwrap();
    let conflict_err = store.sync_to_disk(fid, None, false);
    assert!(conflict_err.is_err());

    let rb = store
        .rollback_revision(fid, 1, "reviewer", "op-rb-1", true)
        .unwrap();
    assert_eq!(rb["new_revision"], 3);
    assert_eq!(
        std::fs::read_to_string(&file_path).unwrap(),
        "pub fn one() -> i32 { 1 }\n"
    );

    let _ = std::fs::remove_dir_all(&temp);
}
