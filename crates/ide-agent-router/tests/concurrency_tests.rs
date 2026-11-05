use ide_agent_router::{
    CoordinatorError, FileCommitCoordinator, PatchError, PatchProposal,
    SemanticParser, SupportedLanguage,
};

#[test]
fn test_multi_language_parser_extraction() {
    // 1. Rust
    let rust_code = r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}
"#;
    let units = SemanticParser::parse("ws1", "main.rs", rust_code, SupportedLanguage::Rust);
    assert_eq!(units.len(), 2);
    assert_eq!(units[0].qualified_symbol_path, "add");
    assert_eq!(units[1].qualified_symbol_path, "subtract");

    // 2. Python
    let py_code = r#"
def calculate_tax(income):
    return income * 0.2

def calculate_net(income):
    return income * 0.8
"#;
    let py_units = SemanticParser::parse("ws1", "calc.py", py_code, SupportedLanguage::Python);
    assert_eq!(py_units.len(), 2);
    assert_eq!(py_units[0].qualified_symbol_path, "calculate_tax");
    assert_eq!(py_units[1].qualified_symbol_path, "calculate_net");

    // 3. TypeScript
    let ts_code = r#"
export function greet(name: string): string {
    return `Hello, ${name}`;
}
export function bye(name: string): string {
    return `Bye, ${name}`;
}
"#;
    let ts_units = SemanticParser::parse("ws1", "app.ts", ts_code, SupportedLanguage::TypeScript);
    assert_eq!(ts_units.len(), 2);
    assert_eq!(ts_units[0].qualified_symbol_path, "greet");
    assert_eq!(ts_units[1].qualified_symbol_path, "bye");

    // 4. PowerShell
    let ps_code = r#"
function Get-Greeting {
    Write-Output "Hello"
}
function Get-Farewell {
    Write-Output "Goodbye"
}
"#;
    let ps_units = SemanticParser::parse("ws1", "script.ps1", ps_code, SupportedLanguage::PowerShell);
    assert_eq!(ps_units.len(), 2);
    assert_eq!(ps_units[0].qualified_symbol_path, "Get-Greeting");
    assert_eq!(ps_units[1].qualified_symbol_path, "Get-Farewell");
}

#[test]
fn test_identity_hash_drift_resilience() {
    let original = "pub fn compute() -> i32 {\n    42\n}\n";
    let drifted = "// Added comment\n// Another comment\n\npub fn compute() -> i32 {\n    42\n}\n";

    let units_orig = SemanticParser::parse("ws1", "lib.rs", original, SupportedLanguage::Rust);
    let units_drift = SemanticParser::parse("ws1", "lib.rs", drifted, SupportedLanguage::Rust);

    // 即使绝对行号与起始字节改变，node_fingerprint 保持一致！
    assert_eq!(units_orig[0].node_fingerprint, units_drift[0].node_fingerprint);
}

#[test]
fn test_same_file_different_functions_100_patches_concurrent_merge() {
    let coordinator = FileCommitCoordinator::new();

    // 构建一个包含 100 个函数的源文件
    let mut initial_code = String::new();
    for i in 0..100 {
        initial_code.push_str(&format!("pub fn func_{}() -> i32 {{\n    {}\n}}\n\n", i, i));
    }

    let file = coordinator.register_file("ws_perf", "math.rs", &initial_code, SupportedLanguage::Rust);
    assert_eq!(file.current_revision, 1);

    let task_ids = coordinator.dispatch_job("ws_perf", "job_100", &[file.file_id.clone()], 100);
    assert_eq!(task_ids.len(), 100);

    // 100 个 Worker 并发认领任务并准备补丁 (基于 base_revision = 1)
    for i in 0..100 {
        let task = coordinator.claim_task(&format!("worker_{}", i), "ws_perf").expect("Should claim task");
        let symbol = task.symbol_path.clone();
        let replacement = format!("pub fn {}() -> i32 {{\n    /* updated */ {}\n}}", symbol, i * 10);

        let proposal = PatchProposal {
            task_id: task.task_id.clone(),
            worker_id: format!("worker_{}", i),
            fencing_token: task.fencing_token,
            file_id: file.file_id.clone(),
            base_revision: 1, // 所有 Worker 都以初版 rev=1 为基线
            symbol_path: symbol,
            expected_fingerprint: task.expected_fingerprint,
            replacement_text: replacement,
        };

        // 提交进入协调器
        let resp = coordinator.commit_patch(proposal, &format!("idemp-key-{}", i)).expect("Patch commit should succeed via AST relocate");
        assert_eq!(resp.new_revision, (i + 2) as u64);
        if i > 0 {
            assert!(resp.relocated, "Consecutive commits on same file must undergo AST relocation");
        }
    }

    // 验证发件箱记录完整 100 次提交
    let events = coordinator.get_events(0);
    assert_eq!(events.len(), 100);
}

#[test]
fn test_same_function_dual_worker_write_conflict() {
    let coordinator = FileCommitCoordinator::new();
    let initial_code = "pub fn execute() -> &'static str {\n    \"initial\"\n}\n";
    let file = coordinator.register_file("ws_conflict", "exec.rs", initial_code, SupportedLanguage::Rust);

    let _task_ids = coordinator.dispatch_job("ws_conflict", "job_conflict", &[file.file_id.clone()], 100);

    // Worker A 认领并提交
    let task_a = coordinator.claim_task("worker_a", "ws_conflict").unwrap();
    let proposal_a = PatchProposal {
        task_id: task_a.task_id.clone(),
        worker_id: "worker_a".to_string(),
        fencing_token: task_a.fencing_token,
        file_id: file.file_id.clone(),
        base_revision: 1,
        symbol_path: "execute".to_string(),
        expected_fingerprint: task_a.expected_fingerprint.clone(),
        replacement_text: "pub fn execute() -> &'static str {\n    \"from_worker_a\"\n}".to_string(),
    };
    coordinator.commit_patch(proposal_a, "idemp-a").unwrap();

    // 模拟 Worker B 带着旧的 expected_fingerprint 尝试提交相同的函数
    let proposal_b = PatchProposal {
        task_id: task_a.task_id.clone(),
        worker_id: "worker_b".to_string(),
        fencing_token: task_a.fencing_token,
        file_id: file.file_id.clone(),
        base_revision: 1,
        symbol_path: "execute".to_string(),
        expected_fingerprint: task_a.expected_fingerprint, // 旧指纹
        replacement_text: "pub fn execute() -> &'static str {\n    \"from_worker_b\"\n}".to_string(),
    };

    let err = coordinator.commit_patch(proposal_b, "idemp-b").unwrap_err();
    match err {
        CoordinatorError::PatchFailure(PatchError::SemanticWriteConflict { symbol, .. }) => {
            assert_eq!(symbol, "execute");
        }
        other => panic!("Expected SemanticWriteConflict, got {:?}", other),
    }
}

#[test]
fn test_stale_fencing_token_rejected() {
    let coordinator = FileCommitCoordinator::new();
    let code = "pub fn ping() -> bool { true }\n";
    let file = coordinator.register_file("ws_fencing", "net.rs", code, SupportedLanguage::Rust);
    coordinator.dispatch_job("ws_fencing", "job_net", &[file.file_id.clone()], 100);

    // 首次认领，获得 token = 1
    let task = coordinator.claim_task("worker_1", "ws_fencing").unwrap();
    assert_eq!(task.fencing_token, 1);

    // 模拟此 Worker 携带伪造或过期的 fencing_token = 0 发起提交
    let proposal = PatchProposal {
        task_id: task.task_id.clone(),
        worker_id: "worker_1".to_string(),
        fencing_token: 0, // 过期
        file_id: file.file_id.clone(),
        base_revision: 1,
        symbol_path: "ping".to_string(),
        expected_fingerprint: task.expected_fingerprint,
        replacement_text: "pub fn ping() -> bool { false }".to_string(),
    };

    let err = coordinator.commit_patch(proposal, "idemp-stale").unwrap_err();
    assert!(matches!(err, CoordinatorError::FencingTokenStale { .. }));
}

#[test]
fn test_idempotency_key_deduplication() {
    let coordinator = FileCommitCoordinator::new();
    let code = "pub fn version() -> u32 { 1 }\n";
    let file = coordinator.register_file("ws_idemp", "ver.rs", code, SupportedLanguage::Rust);
    coordinator.dispatch_job("ws_idemp", "job_ver", &[file.file_id.clone()], 100);

    let task = coordinator.claim_task("worker_1", "ws_idemp").unwrap();
    let proposal = PatchProposal {
        task_id: task.task_id.clone(),
        worker_id: "worker_1".to_string(),
        fencing_token: task.fencing_token,
        file_id: file.file_id.clone(),
        base_revision: 1,
        symbol_path: "version".to_string(),
        expected_fingerprint: task.expected_fingerprint,
        replacement_text: "pub fn version() -> u32 { 2 }".to_string(),
    };

    let resp1 = coordinator.commit_patch(proposal.clone(), "idemp-key-unique").unwrap();
    assert_eq!(resp1.new_revision, 2);

    // 模拟网络重复投递，再次提交相同的 proposal
    let resp2 = coordinator.commit_patch(proposal, "idemp-key-unique").unwrap();
    assert_eq!(resp2.new_revision, 2);
    assert_eq!(resp1.commit_id, resp2.commit_id);

    // 验证发件箱只生成了 1 条事件，杜绝重复事件风暴
    let events = coordinator.get_events(0);
    assert_eq!(events.len(), 1);
}

#[test]
fn test_1000_tasks_queue_and_dispatch() {
    let coordinator = FileCommitCoordinator::new();

    // 注册 10 个文件，每个文件 100 个函数，共计 1000 个独立语义单元
    let mut file_ids = Vec::new();
    for f_idx in 0..10 {
        let mut code = String::new();
        for fn_idx in 0..100 {
            code.push_str(&format!("pub fn fn_{}_{}() -> i32 {{\n    {}\n}}\n\n", f_idx, fn_idx, fn_idx));
        }
        let file = coordinator.register_file("ws_1000", &format!("mod_{}.rs", f_idx), &code, SupportedLanguage::Rust);
        file_ids.push(file.file_id);
    }

    // 分发大任务
    let task_ids = coordinator.dispatch_job("ws_1000", "job_massive_1000", &file_ids, 50);
    assert_eq!(task_ids.len(), 1000, "Must successfully partition into 1000 semantic tasks");

    // 认领所有任务
    let mut claimed_count = 0;
    while let Some(task) = coordinator.claim_task("worker_swarm", "ws_1000") {
        claimed_count += 1;
        assert_eq!(task.fencing_token, 1);
        if claimed_count == 1000 {
            break;
        }
    }
    assert_eq!(claimed_count, 1000, "All 1000 tasks claimed without starvation");

    // 队列耗尽时再 claim 应返回 None
    assert!(coordinator.claim_task("worker_swarm", "ws_1000").is_none());
}
