//! .NEMA 全语法变化矩阵 · 4³ = 64 态组合验证与边界注入自动化测试套件
//! 对齐《.NEMA 全语法变化矩阵 · 七列完整审查版》规范要求

use nema_codec::{
    adaptive_render_view, parse_source, probe_interpreter, discover_candidates,
    InterpreterProtocol,
};

#[test]
fn test_table_64_state_combinations_exhaustion() {
    // 4 种基本状态：Value ("A"), Null ("~"), Empty ("\e"), Missing ("\_")
    let states = [
        ("val", "A"),
        ("null", "~"),
        ("empty", "\\e"),
        ("missing", "\\_"),
    ];

    let mut count = 0;
    for (name0, tok0) in &states {
        for (name1, tok1) in &states {
            for (name2, tok2) in &states {
                count += 1;
                let sample = format!(
                    " \\1 1\n\\# column1 column2 column3\n{} {} {}\n\n\n",
                    tok0, tok1, tok2
                );

                // 1. 自适应渲染视图断言
                let (ast, _latency, _engine, _is_custom) = adaptive_render_view(&sample);
                assert!(ast.is_array(), "Combination #{} ({} {} {}) failed: {}", count, name0, name1, name2, ast);
                let domains = ast.as_array().unwrap();
                assert_eq!(domains.len(), 1, "Must have exactly 1 domain in #{}", count);

                // 2. 原生解析器断言
                let doc = parse_source(&sample);
                assert!(doc.is_ok(), "Native parse failed for #{} ({}, {}, {}): {:?}", count, name0, name1, name2, doc.err());
                let d = &doc.unwrap().domains[0];
                assert_eq!(d.columns.len(), 3, "Schema must have 3 columns in #{}", count);
                assert_eq!(d.records.len(), 1, "Must have 1 record in #{}", count);
            }
        }
    }

    assert_eq!(count, 64, "Must exhaustively verify all 4^3 = 64 combinations");
}

#[test]
fn test_syntax_matrix_boundary_and_error_injection() {
    // 1. 非法零编号拒绝
    let bad_zero_id = " \\0 1\nA B\n\n\n";
    assert!(parse_source(bad_zero_id).is_err(), "Zero domain ID must be rejected");

    // 2. 5W1H 超过 6 列
    let too_many_5w1h = " \\?1 1\nA B C D E F G\n\n\n";
    assert!(parse_source(too_many_5w1h).is_err(), "5W1H > 6 fields must be rejected");

    // 3. Graph 畸形边缺少 Edge ID
    let malformed_edge = " \\>1 1\nA NodeA\nB NodeB\nA>>B\n\n\n";
    assert!(parse_source(malformed_edge).is_err(), "Malformed edge A>>B must be rejected");

    // 4. 悬空引用检测：只定义 A，不定义 B
    let dangling_edge = " \\>1 1\nA NodeA\nA>X>B Reads\n\n\n";
    assert!(parse_source(dangling_edge).is_err(), "Dangling endpoint B must be rejected");
}

#[test]
fn test_candidates_and_adaptive_probe_flow() {
    // 测试自动候选发现
    let cands = discover_candidates(None);
    assert!(!cands.is_empty(), "Should discover candidates");

    // 探针合法性
    let rep = probe_interpreter("E:\\.NEMA\\codec\\native.py", Some("python"));
    if std::path::Path::new("E:\\.NEMA\\codec\\native.py").exists() {
        assert!(rep.success, "Official native.py probe should succeed");
        assert_eq!(rep.protocol, InterpreterProtocol::PythonNativeView);
        assert!(rep.sample_ast_nodes >= 2);
    }
}
