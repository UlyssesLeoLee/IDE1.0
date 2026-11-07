//! `nema_pipeline_redgreen_tests` — 红绿测试套件 (TDD / Regression / Edge Cases)
//!
//! 针对多智能体协同流水线核心边界进行断言与证伪：
//! 1. 统筹合并输出的 .nema 源码必须能被标准 NemaCodec 无损二次解析（严格 3 LF 终止不变量）
//! 2. Graph 边合并必须保全原始说明文本与边方向（禁止硬编码 "Description" 与反向翻转）
//! 3. 词典归一化重排时，记录中引用的 ^N 必须精准重映射为新词典索引，杜绝语义错位
//! 4. Lowering 生成 Rust 代码时，对含数字开头、短横线、空格的标识符必须进行合法化清洗
//! 5. 验证闭环在遇到同指纹重复错误或预算耗尽时，必须严格触发熔断升级 (CircuitBreak)

use ide_agent_router::agent_profile::FeedbackDecision;
use ide_agent_router::nema_pipeline::{
    IterationFeedbackTicket, MultiAgentPipelineCoordinator,
    NemaSemanticMerger, NemaToRustLowering,
    TaskInputKind,
};
use nema_codec::{EdgeDirection, NemaCodec, StandardNemaCodec};

/// [RED-1] 统筹合并后的 .nema 源码必须能被标准 NemaCodec 成功解析
#[test]
fn test_red_merged_nema_source_must_be_losslessly_parseable_by_nema_codec() {
    let merger = NemaSemanticMerger;
    let codec = StandardNemaCodec;

    let base = "\t$1\nTokenZero\n\t\\1\n\\ Type Field\nTokenZero Core\n\n\n";
    let branch_a = "\t$2\nTokenZero\nTokenA\n\t\\1\n\\ Type Field\nTokenZero Core\nTokenA ModuleA\n\n\n";
    let branch_b = "\t$2\nTokenZero\nTokenB\n\t\\1\n\\ Type Field\nTokenZero Core\nTokenB ModuleB\n\n\n";

    let outcome = merger
        .merge("specs/File_A.nema", base, "feat/A1", branch_a, "feat/A2", branch_b)
        .expect("merge should succeed");

    // 关键断言：合并输出的字符串必须是合法的 NEMA 规范文档
    let parsed_doc = codec.parse(&outcome.rendered_nema_source);
    assert!(
        parsed_doc.is_ok(),
        "Merged .nema source failed validation! Error: {:?}\nSource was:\n{}",
        parsed_doc.err(),
        outcome.rendered_nema_source
    );
}

/// [RED-2] Graph 边合并必须保全说明文本与边方向，禁止反向与硬编码 "Description"
#[test]
fn test_red_merged_graph_edges_preserve_direction_and_descriptions() {
    let merger = NemaSemanticMerger;
    let codec = StandardNemaCodec;

    let base = "\t>1\nServer NodeA\nDatabase NodeB\n\n\n";
    // 分支 A: Database<Read<Server Client reads from database (注意这是反向操作符 <，Server -> Database)
    let branch_a = "\t>1\nServer NodeA\nDatabase NodeB\nDatabase<Read<Server Client reads from database\n\n\n";
    // 分支 B: Server>Write>Cache Cache writes
    let branch_b = "\t>1\nServer NodeA\nDatabase NodeB\nCache NodeC\nServer>Write>Cache Cache writes\n\n\n";

    let outcome = merger
        .merge("specs/Arch.nema", base, "feat/A1", branch_a, "feat/B1", branch_b)
        .expect("merge should succeed");

    let parsed_doc = codec
        .parse(&outcome.rendered_nema_source)
        .expect("merged graph source must be parseable");

    let graph_dom = parsed_doc
        .domains
        .iter()
        .find(|d| d.id == "1")
        .expect("Domain 1 must exist");

    // 必须包含原始说明文本，而不是被冲刷为 "Description"
    let read_edge_rec = graph_dom
        .records
        .iter()
        .find(|r| r.graph_edge.as_ref().map(|e| e.id.as_str()) == Some("Read"))
        .expect("Read edge must exist");

    let _write_edge_rec = graph_dom
        .records
        .iter()
        .find(|r| r.graph_edge.as_ref().map(|e| e.id.as_str()) == Some("Write"))
        .expect("Write edge must exist");

    // 验证方向：Read 边是从 Server 指向 Database
    let read_edge = read_edge_rec.graph_edge.as_ref().unwrap();
    assert_eq!(read_edge.from, "Server", "Read edge source must be Server");
    assert_eq!(read_edge.to, "Database", "Read edge target must be Database");
    assert_eq!(read_edge.direction, EdgeDirection::Directed);

    // 验证说明文本：必须包含原始业务语义
    let read_rendered_text = read_edge_rec
        .cells
        .iter()
        .flatten()
        .map(|c| c.raw.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        read_rendered_text.contains("Client reads from database")
            || outcome.rendered_nema_source.contains("Client reads from database"),
        "Original edge description was lost in merged source! Source:\n{}",
        outcome.rendered_nema_source
    );
}

/// [RED-3] 词典重排归一化时，记录中的 ^N 索引必须精准映射到新词典
#[test]
fn test_red_dictionary_index_remapping_for_caret_tokens() {
    let merger = NemaSemanticMerger;
    let codec = StandardNemaCodec;

    // 分支 A: 词典中有 ["ZetaToken", "AlphaToken"]
    // 在 A 中：^0 = "ZetaToken", ^1 = "AlphaToken"
    let branch_a = "\t$2\nZetaToken\nAlphaToken\n\t\\1\n\\ Col1 Col2\n^1 FirstRow\n\n\n";

    // 分支 B: 词典中有 ["BetaToken", "ZetaToken"]
    // 在 B 中：^0 = "BetaToken", ^1 = "ZetaToken"
    let branch_b = "\t$2\nBetaToken\nZetaToken\n\t\\1\n\\ Col1 Col2\n^0 SecondRow\n\n\n";

    let outcome = merger
        .merge("specs/Tokens.nema", "", "feat/A1", branch_a, "feat/B1", branch_b)
        .expect("merge ok");

    // 词典合并后字典字母序应该是: ["AlphaToken", "BetaToken", "ZetaToken"]
    // 此时新索引:
    // "AlphaToken" -> ^0
    // "BetaToken"  -> ^1
    // "ZetaToken"  -> ^2
    let parsed_doc = codec
        .parse(&outcome.rendered_nema_source)
        .expect("parsed merged doc ok");

    // 第一行来自 A 的 ^1 (原本是 AlphaToken)，现在必须映射为新词典中的 ^0 或解析得到 "AlphaToken"
    let rec1 = &parsed_doc.domains[0].records[0];
    let rec1_col1_atom = &rec1.cells[0][0].atoms[0];
    assert_eq!(
        rec1_col1_atom.text, "AlphaToken",
        "Branch A token remapping mismatch! Expected AlphaToken, got {}",
        rec1_col1_atom.text
    );

    // 第二行来自 B 的 ^0 (原本是 BetaToken)，现在必须映射为新词典中的 ^1 或解析得到 "BetaToken"
    let rec2 = &parsed_doc.domains[0].records[1];
    let rec2_col1_atom = &rec2.cells[0][0].atoms[0];
    assert_eq!(
        rec2_col1_atom.text, "BetaToken",
        "Branch B token remapping mismatch! Expected BetaToken, got {}",
        rec2_col1_atom.text
    );
}

/// [RED-4] Lowering 代码生成必须对非法 Rust 标识符进行合法化清洗
#[test]
fn test_red_lowering_sanitizes_illegal_rust_identifiers() {
    let lowering = NemaToRustLowering;
    let codec = StandardNemaCodec;

    // 包含数字开头、短横线、空格、点号的复杂标识符
    let source = "\t\\1\n\\ Type Field\n123-Payment-Gateway field1\nuser.session_manager field2\n\n\n";
    let doc = codec.parse(source).expect("valid nema source");

    let outcome = lowering.lower("specs/Types.nema", &doc, "src/types.rs", "feat/A1");

    // 生成的代码中不能出现 "pub struct 123-Payment-Gateway" 或 "pub struct user.session_manager"
    assert!(
        !outcome.rust_code.contains("pub struct 123-Payment-Gateway"),
        "Generated illegal identifier with dash and leading digit!"
    );
    assert!(
        !outcome.rust_code.contains("pub struct user.session_manager"),
        "Generated illegal identifier with dot!"
    );
    // 必须清洗为合法 Rust 标识符
    assert!(
        outcome.rust_code.contains("pub struct Entity_123_Payment_Gateway")
            || outcome.rust_code.contains("pub struct PaymentGateway123")
            || outcome.rust_code.contains("pub struct _123_Payment_Gateway"),
        "Expected sanitized struct identifier in generated code:\n{}",
        outcome.rust_code
    );
}

/// [RED-5] 闭环守卫必须在达到重试上限或遇到重复错误指纹时坚决熔断 (CircuitBreak)
#[test]
fn test_red_circuit_breaker_terminates_infinite_defect_repair_loop() {
    let mut coordinator = MultiAgentPipelineCoordinator::default();
    coordinator.loop_guard.max_rounds = 3;

    let ticket = IterationFeedbackTicket {
        failed_gate: "UnitTest".to_string(),
        rust_file: "src/service.rs".to_string(),
        rust_line: 42,
        mapped_nema_file: "specs/Service.nema".to_string(),
        mapped_domain_id: "1".to_string(),
        mapped_record_key: "ServiceConfig".to_string(),
        responsible_branch: "feat/A1".to_string(),
        responsible_agent: "Agent-A1".to_string(),
        diagnostic_summary: "Assertion failed: expected 200 OK got 500".to_string(),
        suggested_task_kind: TaskInputKind::DefectRepair,
    };

    // 第 1 次：正常进入重试轮次
    let d1 = coordinator.process_iteration_feedback(&ticket, 5000);
    assert!(matches!(d1, FeedbackDecision::ContinueIteration { next_round: 1 }));

    // 第 2 次：报告完全相同的错误指纹 -> 必须直接熔断，避免无限循环！
    let d2 = coordinator.process_iteration_feedback(&ticket, 5000);
    match d2 {
        FeedbackDecision::CircuitBreak { reason } => {
            assert!(
                reason.contains("连续重复") && reason.contains("错误指纹"),
                "Expected fingerprint circuit break, got: {}",
                reason
            );
        }
        other => panic!("Expected CircuitBreak on identical error, got: {:?}", other),
    }
}
