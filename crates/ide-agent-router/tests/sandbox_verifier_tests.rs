use ide_agent_router::{
    PatchProposal, PatchVerifier, SandboxError, SupportedLanguage, VerificationStatus,
    WorkerSandbox,
};
use std::path::Path;

#[test]
fn test_sandbox_create_and_file_isolation() {
    let mut sandbox = WorkerSandbox::new("worker-alpha", "task-001")
        .expect("Failed to create worker sandbox");

    // 禁止自动清理以在测试中验证文件存在
    sandbox.set_auto_cleanup(false);
    let root = sandbox.sandbox_root.clone();
    assert!(root.exists(), "Sandbox root should exist");

    // 写入文件
    let rel = "src/modules/core.rs";
    let content = b"pub fn compute() -> i32 { 42 }";
    let staged_path = sandbox
        .write_file(rel, content)
        .expect("Failed to write file in sandbox");

    assert!(staged_path.exists());
    assert!(staged_path.starts_with(&root));

    // 读取验证
    let read_back = sandbox
        .read_to_string(rel)
        .expect("Failed to read file from sandbox");
    assert_eq!(read_back, "pub fn compute() -> i32 { 42 }");

    // 删除文件
    sandbox
        .remove_file(rel)
        .expect("Failed to remove file from sandbox");
    assert!(!staged_path.exists());

    // 手动清理
    sandbox.cleanup().expect("Failed to cleanup sandbox");
    assert!(!root.exists(), "Sandbox root should be deleted after cleanup");
}

#[test]
fn test_path_traversal_attack_prevention() {
    let sandbox = WorkerSandbox::new("worker-beta", "task-traversal")
        .expect("Failed to create worker sandbox");

    let traversal_paths = vec![
        "../outside.txt",
        "..\\outside.txt",
        "sub/../../outside.txt",
        "/etc/passwd",
        "\\Windows\\System32\\cmd.exe",
        "C:\\autoexec.bat",
        "src/evil\0file.rs",
        "C:relative_drive.txt",
    ];

    for bad_path in traversal_paths {
        let write_result = sandbox.write_file(bad_path, b"malicious content");
        assert!(
            matches!(write_result, Err(SandboxError::PathTraversalDetected(_))),
            "Expected PathTraversalDetected for '{}', got {:?}",
            bad_path,
            write_result
        );

        let read_result = sandbox.read_file(bad_path);
        assert!(
            matches!(read_result, Err(SandboxError::PathTraversalDetected(_))),
            "Expected PathTraversalDetected on read for '{}', got {:?}",
            bad_path,
            read_result
        );
    }
}

#[test]
fn test_readonly_mount_protection() {
    let mut sandbox = WorkerSandbox::new("worker-gamma", "task-ro")
        .expect("Failed to create worker sandbox");

    // 创建测试源文件
    let temp_dir = std::env::temp_dir().join("ide_test_source_ro");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let source_file = temp_dir.join("readonly_config.json");
    std::fs::write(&source_file, b"{\"immutable\": true}").unwrap();

    // 挂载只读
    let dest_rel = Path::new("config/readonly_config.json");
    let mounted_path = sandbox
        .mount_readonly(&source_file, dest_rel)
        .expect("Failed to mount readonly file");
    assert!(mounted_path.exists());

    // 读取测试
    let read_data = sandbox
        .read_to_string(dest_rel)
        .expect("Should be able to read readonly mount");
    assert_eq!(read_data, "{\"immutable\": true}");

    // 尝试修改只读文件 -> 应该被拦截
    let write_attempt = sandbox.write_file(dest_rel, b"{\"hacked\": true}");
    assert!(
        matches!(write_attempt, Err(SandboxError::ReadOnlyViolation(_))),
        "Expected ReadOnlyViolation, got {:?}",
        write_attempt
    );

    // 尝试删除只读文件 -> 应该被拦截
    let remove_attempt = sandbox.remove_file(dest_rel);
    assert!(
        matches!(remove_attempt, Err(SandboxError::ReadOnlyViolation(_))),
        "Expected ReadOnlyViolation, got {:?}",
        remove_attempt
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_sandbox_environment_stage_and_simulate() {
    let sandbox = WorkerSandbox::new("worker-delta", "task-sim")
        .expect("Failed to create worker sandbox");

    let mut env = sandbox.create_environment();
    env.set_env("AGENT_ENV", "sandbox_test");

    let file_path = env
        .stage_file(&sandbox, "output.txt", b"Build succeeded")
        .expect("Failed to stage file in environment");
    assert!(file_path.exists());

    // 模拟运行内置 mock 命令
    let result = env
        .simulate_run("mock", &[])
        .expect("Simulation should execute");
    assert_eq!(result.exit_code, 0);
    assert!(result.success);
    assert!(result.stdout.contains("Simulation passed for 1 staged files"));
}

#[test]
fn test_valid_patch_verification_passed() {
    let initial_code = r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;

    let patch_proposal = PatchProposal {
        task_id: "task-001".to_string(),
        worker_id: "worker-1".to_string(),
        fencing_token: 1,
        file_id: "math.rs".to_string(),
        base_revision: 1,
        symbol_path: "add".to_string(),
        expected_fingerprint: None,
        replacement_text: r#"pub fn add(a: i32, b: i32) -> i32 {
    // Optimized add implementation
    a.saturating_add(b)
}"#
        .to_string(),
    };

    let report = PatchVerifier::verify(
        "ws_test",
        "math.rs",
        initial_code,
        1,
        SupportedLanguage::Rust,
        &patch_proposal,
    );

    assert_eq!(report.status, VerificationStatus::Passed);
    assert!(report.is_valid);
    assert!(report.ast_integrity_preserved);
    assert_eq!(report.previous_revision, 1);
    assert_eq!(report.new_revision, Some(2));
    assert!(report.patched_content.is_some());

    let patched_text = report.patched_content.unwrap();
    assert!(patched_text.contains("a.saturating_add(b)"));
    assert!(patched_text.contains("pub fn subtract"));
}

#[test]
fn test_ast_relocation_and_sibling_integrity_preservation() {
    // 模拟并发情况下，其他 Agent 在文件上方插入了新函数或修改，导致 revision 升至 2 且字符偏移漂移
    let concurrently_modified_code = r#"
// Concurrently inserted by Worker-0
pub fn calculate_negate(x: i32) -> i32 {
    -x
}

pub fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}

pub fn calculate_product(a: i32, b: i32) -> i32 {
    a * b
}
"#;

    let patch_proposal = PatchProposal {
        task_id: "task-002".to_string(),
        worker_id: "worker-2".to_string(),
        fencing_token: 2,
        file_id: "calc.rs".to_string(),
        base_revision: 1, // 基于 rev 1 生成
        symbol_path: "calculate_product".to_string(),
        expected_fingerprint: None,
        replacement_text: r#"pub fn calculate_product(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}"#
        .to_string(),
    };

    let report = PatchVerifier::verify(
        "ws_test",
        "calc.rs",
        concurrently_modified_code,
        2, // 当前文件版本已是 rev 2
        SupportedLanguage::Rust,
        &patch_proposal,
    );

    assert_eq!(report.status, VerificationStatus::Passed);
    assert!(report.is_valid);
    assert!(report.relocation_applied, "Should trigger AST relocation");
    assert!(
        report.ast_integrity_preserved,
        "All siblings should remain intact"
    );

    let patched = report.patched_content.expect("Patched content present");
    assert!(patched.contains("calculate_negate"));
    assert!(patched.contains("calculate_sum"));
    assert!(patched.contains("a.wrapping_mul(b)"));
}

#[test]
fn test_corrupted_and_out_of_bounds_patch_intercepted() {
    let base_code = r#"
pub fn validate_order(id: u64) -> bool {
    id > 0
}
"#;

    // 1. 尝试修改不存在的符号路径 -> Conflict
    let ghost_proposal = PatchProposal {
        task_id: "task-ghost".to_string(),
        worker_id: "worker-3".to_string(),
        fencing_token: 1,
        file_id: "orders.rs".to_string(),
        base_revision: 1,
        symbol_path: "non_existent_function".to_string(),
        expected_fingerprint: None,
        replacement_text: "fn foo() {}".to_string(),
    };
    let ghost_report = PatchVerifier::verify(
        "ws_test",
        "orders.rs",
        base_code,
        1,
        SupportedLanguage::Rust,
        &ghost_proposal,
    );
    assert_eq!(ghost_report.status, VerificationStatus::Conflict);
    assert!(!ghost_report.is_valid);

    // 2. 路径穿透文件路径 -> SecurityBlocked
    let traversal_proposal = PatchProposal {
        task_id: "task-hack".to_string(),
        worker_id: "worker-hack".to_string(),
        fencing_token: 1,
        file_id: "../../etc/shadow".to_string(),
        base_revision: 1,
        symbol_path: "validate_order".to_string(),
        expected_fingerprint: None,
        replacement_text: "fn validate_order() {}".to_string(),
    };
    let sec_report = PatchVerifier::verify(
        "ws_test",
        "../../etc/shadow",
        base_code,
        1,
        SupportedLanguage::Rust,
        &traversal_proposal,
    );
    assert_eq!(sec_report.status, VerificationStatus::SecurityBlocked);
    assert!(!sec_report.is_valid);

    // 3. 包含破坏性载荷 (rm -rf /) -> SecurityBlocked
    let destructive_proposal = PatchProposal {
        task_id: "task-destruct".to_string(),
        worker_id: "worker-evil".to_string(),
        fencing_token: 1,
        file_id: "orders.rs".to_string(),
        base_revision: 1,
        symbol_path: "validate_order".to_string(),
        expected_fingerprint: None,
        replacement_text: r#"pub fn validate_order(id: u64) -> bool {
    std::process::Command::new("sh").arg("-c").arg("rm -rf /").spawn();
    false
}"#
        .to_string(),
    };
    let dest_report = PatchVerifier::verify(
        "ws_test",
        "orders.rs",
        base_code,
        1,
        SupportedLanguage::Rust,
        &destructive_proposal,
    );
    assert_eq!(dest_report.status, VerificationStatus::SecurityBlocked);
    assert!(!dest_report.is_valid);

    // 4. 空字节注入 -> SecurityBlocked
    let null_byte_proposal = PatchProposal {
        task_id: "task-null".to_string(),
        worker_id: "worker-evil".to_string(),
        fencing_token: 1,
        file_id: "orders.rs".to_string(),
        base_revision: 1,
        symbol_path: "validate_order".to_string(),
        expected_fingerprint: None,
        replacement_text: "pub fn validate_order() { /* \0 null byte */ }".to_string(),
    };
    let null_report = PatchVerifier::verify(
        "ws_test",
        "orders.rs",
        base_code,
        1,
        SupportedLanguage::Rust,
        &null_byte_proposal,
    );
    assert_eq!(null_report.status, VerificationStatus::SecurityBlocked);
}

#[test]
fn test_multilang_static_syntax_checking() {
    // 1. Rust: 括号未闭合
    let rust_unclosed = r#"pub fn broken(a: i32) -> i32 {
    let x = (a + 1;
    x
}"#;
    let issues = PatchVerifier::verify_syntax(rust_unclosed, SupportedLanguage::Rust);
    assert!(
        issues
            .iter()
            .any(|i| i.rule == "syntax_unbalanced_delimiter"),
        "Should detect unclosed parenthesis in Rust"
    );

    // 2. Python: 缺失冒号 ':'
    let py_missing_colon = r#"def calculate_discount(price)
    return price * 0.9"#;
    let py_issues = PatchVerifier::verify_syntax(py_missing_colon, SupportedLanguage::Python);
    assert!(
        py_issues.iter().any(|i| i.rule == "python_missing_colon"),
        "Should detect missing colon in Python def"
    );

    // 3. Python: 空格与 Tab 混合缩进
    let py_mixed_indent = "def foo():\n \treturn 1\n";
    let indent_issues = PatchVerifier::verify_syntax(py_mixed_indent, SupportedLanguage::Python);
    assert!(
        indent_issues
            .iter()
            .any(|i| i.rule == "python_mixed_indentation"),
        "Should detect mixed tabs and spaces"
    );

    // 4. TypeScript: 缺失参数括号
    let ts_missing_parens = "function computeSum { return 42; }";
    let ts_issues = PatchVerifier::verify_syntax(ts_missing_parens, SupportedLanguage::TypeScript);
    assert!(
        ts_issues
            .iter()
            .any(|i| i.rule == "js_missing_parameter_list"),
        "Should detect missing parameter list in TS"
    );

    // 5. 格式告警检查: 尾随空格与超长行
    let code_with_warnings = format!(
        "pub fn format_test() {{\n    let a = 1;   \n    let very_long = \"{}\";\n}}",
        "a".repeat(130)
    );
    let format_issues = PatchVerifier::verify_format(&code_with_warnings);
    assert!(
        format_issues
            .iter()
            .any(|i| i.rule == "format_trailing_whitespace"),
        "Should detect trailing whitespace"
    );
    assert!(
        format_issues
            .iter()
            .any(|i| i.rule == "format_line_length_exceeded"),
        "Should detect line length exceeding 120"
    );
}

#[test]
fn test_verifier_in_sandbox_integration() {
    let sandbox = WorkerSandbox::new("worker-verify", "task-sbx-int")
        .expect("Failed to create worker sandbox");

    let initial_code = r#"
pub fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}
"#;

    let proposal = PatchProposal {
        task_id: "task-sbx-int".to_string(),
        worker_id: "worker-verify".to_string(),
        fencing_token: 1,
        file_id: "greeting.rs".to_string(),
        base_revision: 1,
        symbol_path: "greet".to_string(),
        expected_fingerprint: None,
        replacement_text: r#"pub fn greet(name: &str) -> String {
    format!("Hi, {}!", name)
}"#
        .to_string(),
    };

    let (report, staged_path) = PatchVerifier::verify_in_sandbox(
        &sandbox,
        "ws_int",
        "greeting.rs",
        initial_code,
        1,
        SupportedLanguage::Rust,
        &proposal,
    )
    .expect("Sandbox verification should not fail");

    assert_eq!(report.status, VerificationStatus::Passed);
    assert!(staged_path.is_some());
    let path = staged_path.unwrap();
    assert!(path.exists());

    let staged_content = sandbox
        .read_to_string("greeting.rs")
        .expect("Staged file should be readable in sandbox");
    assert!(staged_content.contains("Hi, {}!"));
}
