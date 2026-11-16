//! 对抗式红绿测试套件 (Red-Green Adversarial Testing for .NEMA & Interpreter Plugin)
//!
//! 覆盖极限边界、注入攻击、协议跨度、热重载鲁棒性与严格断言

use nema_codec::{
    parse_strict, render_strict, parse_source,
    parse_strict_ast_json,
    EdgeDirection,
};

#[test]
fn red_green_adversarial_syntax_edge_cases() {
    // 1. 严格语法：空行处理、末尾无三连换行支持
    let src_clean = " \\1 1\n\\%2\n\\# Key Val\nA B";
    let res = parse_strict(src_clean);
    assert!(res.is_ok(), "Clean table without trailing 3 LF should parse in strict mode");

    // 2. 连续空格注入（字段内部禁止未转义的双空格）
    let src_double_space_in_row = " \\1 1\n\\%2\n\\# Col1 Col2\nA  B";
    assert!(parse_strict(src_double_space_in_row).is_err(), "Consecutive unescaped spaces must be rejected");

    // 3. 转义空格允许
    let src_escaped_space = " \\1 1\n\\%2\n\\# Col1 Col2\nA\\ B C";
    let doc_esc = render_strict(src_escaped_space).expect("Escaped space should be allowed");
    assert_eq!(doc_esc.domains[0].rows[0].cells[0], "A B");
    assert_eq!(doc_esc.domains[0].rows[0].cells[1], "C");

    // 4. 禁止跨模式重复域 ID
    let src_duplicate_domain_id = " \\1 1\n\\%1\n\\# C\nA\n \\?1 1\nB C D E F G\n";
    assert!(parse_strict(src_duplicate_domain_id).is_err(), "Duplicate domain ID across modes must fail");

    // 5. 非法转义符注入，例如 \q \a \z
    for bad_esc in ["\\q", "\\a", "\\z", "\\1", "\\0"] {
        let src = format!(" \\1 1\n\\%1\n\\# C\nTest{}", bad_esc);
        assert!(parse_strict(&src).is_err(), "Unknown escape {} must be rejected", bad_esc);
    }

    // 6. 独立 \e 与 \_ 不得与其他字符拼合
    let src_combine_e = " \\1 1\n\\%1\n\\# C\nprefix\\e";
    assert!(parse_strict(src_combine_e).is_err(), "Combining \\e with prefix must fail");
    let src_combine_missing = " \\1 1\n\\%1\n\\# C\n\\_suffix";
    assert!(parse_strict(src_combine_missing).is_err(), "Combining \\_ with suffix must fail");

    // 7. Graph 模式：边必须有合法操作符且两侧操作符一致
    let src_mismatch_op1 = " \\>1 1\nA NodeA\nB NodeB\nA>X<B Mismatch";
    assert!(parse_strict(src_mismatch_op1).is_err(), "Mismatched operators > and < must fail");
    let src_mismatch_op2 = " \\>1 1\nA NodeA\nB NodeB\nA=X>B Mismatch";
    assert!(parse_strict(src_mismatch_op2).is_err(), "Mismatched operators = and > must fail");

    // 8. Graph 模式：不能继续 X0 (列索引 0)
    let src_cont_x0 = " \\>1 1\nA NodeA\n 0 IllegalCont";
    assert!(parse_strict(src_cont_x0).is_err(), "Continuation on X0 must be rejected");

    // 9. 不能追加到 null (~) 或显式 missing (\_)
    let src_append_to_null = " \\1 1\n\\%2\n\\# C1 C2\nA ~\n 1 Appended";
    assert!(parse_strict(src_append_to_null).is_err(), "Cannot append continuation to null");
    let src_append_to_missing = " \\1 1\n\\%2\n\\# C1 C2\nA \\_\n 1 Appended";
    assert!(parse_strict(src_append_to_missing).is_err(), "Cannot append continuation to missing");

    // 10. 可以追加到空字符串 \e
    let src_append_to_empty = " \\1 1\n\\%2\n\\# C1 C2\nA \\e\n 1 Appended";
    let doc_app = render_strict(src_append_to_empty).expect("Appending to empty string should succeed");
    assert_eq!(doc_app.domains[0].rows[0].cells[1], "Appended");
}

#[test]
fn red_green_adversarial_ast_json_resilience() {
    // 1. 畸形 JSON 错误包裹
    let bad_json = "{ invalid json }";
    assert!(parse_strict_ast_json(bad_json).is_err());

    // 2. 携带 error 字段的 JSON
    let err_json = r#"{"error": "Simulated syntax failure at line 4"}"#;
    let res = parse_strict_ast_json(err_json);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Simulated syntax failure"));

    // 3. 各种层级包装兼容：{ "doc": ... }, { "domains": ... }, 以及原生 [ ... ]
    let ast_domains_only = r#"[
        {
            "mode": "table",
            "id": "100",
            "revision": 3,
            "columns": ["K", "V"],
            "records": [{"values": ["A", "B"]}]
        }
    ]"#;
    let doc1 = parse_strict_ast_json(ast_domains_only).expect("array AST parse");
    assert_eq!(doc1.domains[0].id, "100");

    let wrapped_doc = format!(r#"{{"doc": {}}}"#, serde_json::to_string(&doc1).unwrap());
    let doc2 = parse_strict_ast_json(&wrapped_doc).expect("wrapped doc parse");
    assert_eq!(doc2.domains[0].id, "100");

    let direct_rendered = serde_json::to_string(&doc1).unwrap();
    let doc3 = parse_strict_ast_json(&direct_rendered).expect("direct RenderedDocument parse");
    assert_eq!(doc3.domains[0].id, "100");
}

#[test]
fn red_green_adversarial_graph_cycles_and_forward_edges() {
    // 1. 先声明边，后声明节点（严格规范允许前向边定义，域末尾校验悬空）
    let src_forward_ref = " \\>1 1\nA>X>B Link\nB TargetNode\nA SourceNode";
    let doc = render_strict(src_forward_ref).expect("Forward edge reference should be accepted when nodes defined later");
    assert_eq!(doc.domains[0].rows.len(), 3);
    let edge = doc.domains[0].rows[0].edge.as_ref().unwrap();
    assert_eq!(edge.id, "X");
    assert_eq!(edge.from, "A");
    assert_eq!(edge.to, "B");
    assert_eq!(edge.direction, EdgeDirection::Directed);

    // 2. 自环与双向无向边
    let src_undirected = " \\>2 1\nA NodeA\nB NodeB\nA=Z=B PeerConnection";
    let doc2 = render_strict(src_undirected).expect("Undirected edge should parse");
    let e_undirected = doc2.domains[0].rows[2].edge.as_ref().unwrap();
    assert_eq!(e_undirected.direction, EdgeDirection::Undirected);
    assert_eq!(e_undirected.raw_operator, '=');

    // 3. 节点与边 ID 冲突攻击
    let src_id_overlap = " \\>3 1\nA NodeA\nB NodeB\nA>A>B EdgeWithNodeIdA";
    assert!(parse_strict(src_id_overlap).is_err(), "Edge ID cannot collide with Node ID");

    // 4. 双重节点声明攻击
    let src_dup_node = " \\>4 1\nA Node1\nA Node2";
    assert!(parse_strict(src_dup_node).is_err(), "Duplicate node declaration must fail");

    // 5. 跨域隔离性：Domain 1 的节点不能满足 Domain 2 的边引用
    let src_cross_domain_dangling = " \\>1 1\nA NodeA\n \\>2 1\nB NodeB\nA>X>B Link";
    assert!(parse_strict(src_cross_domain_dangling).is_err(), "Dangling edge in Domain 2 cannot reference Domain 1 node A");
}

#[test]
fn red_green_adversarial_compatibility_and_fallback_resilience() {
    // 验证旧版本兼容引擎与新严格引擎在各自合法输入下的互通性
    // 旧版本规范：\t 仅作为域头前缀，主体使用空格；若主体出现 \t 则触发 TabInBody 保护
    let legacy_tab = "\t\\1\nColA ColB\nValA ValB\n\n\n";
    let res_legacy = parse_source(legacy_tab);
    assert!(res_legacy.is_ok(), "parse_source must handle legacy \\t\\1 header");
    assert!(parse_strict(legacy_tab).is_err(), "parse_strict must reject raw Tab");

    // 严格模式下，表头必须用 \#，且不能是记录之后；\\ ColA ColB 中 `\\ ` 会被当做转义空格加上后续普通 token
    let slash_header = " \\1 1\n\\%2\n\\ ColA ColB\nValA ValB";
    let doc_slash = render_strict(slash_header).expect("Strict parses \\ ColA as data row");
    assert!(!doc_slash.domains[0].explicit_header, "\\ is not explicit header in strict mode");
    assert_eq!(doc_slash.domains[0].rows[0].cells[0], " ColA");
    assert_eq!(doc_slash.domains[0].rows[0].cells[1], "ColB");

    // 显式表头必须为 \#
    let hash_header = " \\1 1\n\\%2\n\\# ColA ColB\nValA ValB";
    let doc_hash = render_strict(hash_header).expect("Strict parses \\# as explicit header");
    assert!(doc_hash.domains[0].explicit_header, "\\# is explicit header in strict mode");
    assert_eq!(doc_hash.domains[0].columns, vec!["ColA", "ColB"]);

    // 双向渲染往返视图稳定性
    let canonical = " \\1 1\n\\%2\n\\# Key Value\nAlpha Beta";
    let r_strict = render_strict(canonical).expect("Strict render");
    assert_eq!(r_strict.domains[0].columns, vec!["Key", "Value"]);
    assert_eq!(r_strict.domains[0].rows[0].cells, vec!["Alpha", "Beta"]);
}

