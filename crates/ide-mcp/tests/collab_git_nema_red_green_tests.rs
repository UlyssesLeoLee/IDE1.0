//! Git 式与 NEMA 式多 Agent 协同编辑：红绿对抗测试与数学不变量验证套件
//!
//! 1. 红绿测试做法 (Red-Green Adversarial Testing):
//!    - Red: 非法 NEMA 语法注入、格式破损、重叠行冲突拦截与冲突标记。
//!    - Green: 规范 NEMA 跨域/记录并行追加、Git 无交叉函数协同合入、AST 语法封闭性验证。
//!
//! 2. 数学测试做法 (Mathematical Invariants):
//!    - 幂等律 (Idempotence): f(f(S)) = f(S), op_id 重放版本无副作用。
//!    - 交换律/对称性 (Commutativity): 独立变更集 A 与 B 的合并 AST 状态在两种提交顺序下同构等价。
//!    - 单调性 (Monotonicity): revision 单调递增，Hash 状态单调演进。

use ide_mcp::merge::{CoEditWorkspace, CollabMode, FullCoEditRequest};

// ============================================================================
// 一、红绿对抗测试 (Red-Green Adversarial Testing)
// ============================================================================

/// [RED] 注入测试：在 NEMA 协作模式下，尝试注入包含非法未转义连续空格、TAB 污染或损坏域的文本，
/// 系统必须前置拦截并返回 REJECTED_NEMA_SYNTAX_ERROR，绝不允许脏数据污染协同文档。
#[test]
fn test_red_adversarial_nema_syntax_rejection() {
    let mut ws = CoEditWorkspace::default();
    let file = "workspace/spec.nema";

    let valid_base = " \\1 1\n\\%2\n\\# Key Val\nA B\n\n\n";
    ws.open_doc(file, valid_base, "nema", "system", "init");

    // 攻击性注入 1: 字段间未转义双空格 (NEMA 语法禁止)
    let bad_double_space = " \\1 1\n\\%2\n\\# Key Val\nA  B\n\n\n";
    let res1 = ws
        .coedit_full(FullCoEditRequest {
            file_id: file.into(),
            author: "malicious_agent".into(),
            new_text: bad_double_space.into(),
            base_revision: Some(1),
            op_id: Some("attack-001".into()),
            mode: Some(CollabMode::Nema),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(res1["ok"], false, "Red Check: 非法连续双空格必须被拒绝");
    assert_eq!(res1["status"], "REJECTED_NEMA_SYNTAX_ERROR");

    // 攻击性注入 2: 正文域注入 TAB 字符 (TAB 专供域控制，正文严格使用 ASCII 空格)
    let bad_tab = " \\1 1\n\\%2\n\\# Key Val\nA\tB\n\n\n";
    let res2 = ws
        .coedit_full(FullCoEditRequest {
            file_id: file.into(),
            author: "malicious_agent".into(),
            new_text: bad_tab.into(),
            base_revision: Some(1),
            op_id: Some("attack-002".into()),
            mode: Some(CollabMode::Nema),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(res2["ok"], false, "Red Check: TAB 字符注入必须被拒绝");
    assert_eq!(res2["status"], "REJECTED_NEMA_SYNTAX_ERROR");

    // 验证当前文档未被污染，依然保持在 Rev 1
    let read = ws.read_doc(file, None, None).unwrap();
    assert_eq!(read["current_revision"], 1);
}

/// [GREEN] 正向协作：两个 Agent 在 NEMA 模式下并行追加不同合法记录，
/// 系统自动 3-Way 合并并保证语法封闭性 (Syntax Closure: 合并产物依然通过 nema_codec 严格校验)。
#[test]
fn test_green_nema_concurrent_safe_coedit() {
    let mut ws = CoEditWorkspace::default();
    let file = "workspace/collab.nema";

    let valid_base = " \\1 1\n\\%2\n\\# Key Val\nAlpha 100\n\n\n";
    ws.open_doc(file, valid_base, "nema", "system", "init");

    // Agent A (Claude) 基于 Rev 1 在后面追加 Beta 记录
    let agent_a_text = " \\1 1\n\\%2\n\\# Key Val\nAlpha 100\nBeta 200\n\n\n";
    let res_a = ws
        .coedit_full(FullCoEditRequest {
            file_id: file.into(),
            author: "claude-3-7-sonnet".into(),
            new_text: agent_a_text.into(),
            base_revision: Some(1),
            op_id: Some("claude-nema-1".into()),
            mode: Some(CollabMode::Nema),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(res_a["ok"], true);
    assert_eq!(res_a["revision"], 2);
    assert_eq!(res_a["merged"], false); // Fast-Path CAS

    // Agent B (GPT-4o) 同时基于旧的 Rev 1 在后面追加 Gamma 记录 (未感知 Rev 2)
    let agent_b_text = " \\1 1\n\\%2\n\\# Key Val\nAlpha 100\nGamma 300\n\n\n";
    let res_b = ws
        .coedit_full(FullCoEditRequest {
            file_id: file.into(),
            author: "gpt-4o".into(),
            new_text: agent_b_text.into(),
            base_revision: Some(1), // 并发基准
            op_id: Some("gpt-nema-1".into()),
            mode: Some(CollabMode::Nema),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(res_b["ok"], true);
    assert_eq!(res_b["revision"], 3);
    assert_eq!(res_b["merged"], true); // 3-Way Auto-Merged
    assert_eq!(res_b["conflicts"], 0);

    // 验证合并产物的 NEMA 语法封闭性
    let final_text = res_b["content"].as_str().unwrap();
    let parse_res = nema_codec::parse_source(final_text);
    assert!(
        parse_res.is_ok(),
        "Green Check: 合并产物必须通过严格 NEMA AST 语法校验"
    );

    let doc = parse_res.unwrap();
    assert_eq!(doc.domains.len(), 1);
    // 包含 Alpha, Beta, Gamma 三条记录
    assert!(final_text.contains("Alpha 100"));
    assert!(final_text.contains("Beta 200"));
    assert!(final_text.contains("Gamma 300"));
}

/// [RED & GREEN] Git 模式：行冲突生成标准冲突标记与无冲突平滑合并
#[test]
fn test_git_mode_red_green_conflict_and_clean_merge() {
    let mut ws = CoEditWorkspace::default();
    let file = "workspace/calc.rs";

    let base = "fn step1() { 1 }\nfn step2() { 2 }\nfn step3() { 3 }\n";
    ws.open_doc(file, base, "rust", "system", "init");

    // Agent A 改 step1 (Rev 1 -> Rev 2)
    let res_a = ws
        .coedit_full(FullCoEditRequest {
            file_id: file.into(),
            author: "claude".into(),
            new_text: "fn step1() { 101 }\nfn step2() { 2 }\nfn step3() { 3 }\n".into(),
            base_revision: Some(1),
            op_id: None,
            mode: Some(CollabMode::Git),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(res_a["ok"], true);
    assert_eq!(res_a["revision"], 2);

    // Green Case: Agent B 改 step3 (无重叠区间，自动合入)
    let res_b_green = ws
        .coedit_full(FullCoEditRequest {
            file_id: file.into(),
            author: "gpt".into(),
            new_text: "fn step1() { 1 }\nfn step2() { 2 }\nfn step3() { 303 }\n".into(),
            base_revision: Some(1),
            op_id: None,
            mode: Some(CollabMode::Git),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(res_b_green["ok"], true);
    assert_eq!(res_b_green["revision"], 3);
    assert_eq!(res_b_green["merged"], true);
    assert_eq!(res_b_green["conflicts"], 0);
    let c3 = res_b_green["content"].as_str().unwrap();
    assert!(c3.contains("step1() { 101 }"));
    assert!(c3.contains("step3() { 303 }"));

    // Red Case: Agent C 基于旧的 Rev 1 也修改 step1，但改成 999 (P0-01: 冲突时不得提交至 HEAD，应创建独立提案)
    let res_c_red = ws
        .coedit_full(FullCoEditRequest {
            file_id: file.into(),
            author: "deepseek".into(),
            new_text: "fn step1() { 999 }\nfn step2() { 2 }\nfn step3() { 3 }\n".into(),
            base_revision: Some(1),
            op_id: None,
            mode: Some(CollabMode::Git),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        res_c_red["ok"], false,
        "Red Check: 冲突修改时 ok 必须为 false"
    );
    assert_eq!(res_c_red["status"], "CONFLICT");
    assert_eq!(res_c_red["head_revision"], 3);
    assert!(res_c_red["proposal_id"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    assert!(res_c_red["conflicts"].as_u64().unwrap() >= 1);
    let preview = res_c_red["conflict_preview"].as_str().unwrap();
    assert!(preview.contains("<<<<<<< HEAD (ours)"));
    assert!(preview.contains("======="));
    assert!(preview.contains(">>>>>>> INCOMING (theirs)"));
    // 验证 HEAD 正文未被污染
    let head_doc = ws.read_doc(file, None, None).unwrap();
    assert_eq!(head_doc["current_revision"], 3);
    assert!(!head_doc["content"].as_str().unwrap().contains("<<<<<<<"));
}

// ============================================================================
// 二、数学测试做法 (Mathematical Invariants: 代数性质与状态机)
// ============================================================================

/// 数学性质 1: 幂等律 (Idempotence)
/// $\forall S, op: \text{apply}(\text{apply}(S, op), op) = \text{apply}(S, op)$
#[test]
fn test_math_invariant_idempotence() {
    let mut ws = CoEditWorkspace::default();
    let file = "workspace/math_test.rs";
    ws.open_doc(file, "let x = 1;\n", "rust", "system", "init");

    let req = FullCoEditRequest {
        file_id: file.into(),
        author: "agent_alpha".into(),
        new_text: "let x = 42;\n".into(),
        base_revision: Some(1),
        op_id: Some("math-op-idempotent-001".into()),
        mode: Some(CollabMode::Git),
        ..Default::default()
    };

    let r1 = ws.coedit_full(req.clone()).unwrap();
    let r2 = ws.coedit_full(req).unwrap();

    assert_eq!(r1["revision"], r2["revision"], "幂等调用不得生成新的版本号");
    assert_eq!(
        r1["content_hash"], r2["content_hash"],
        "幂等调用内容哈希必须完全一致"
    );
    assert_eq!(r2["idempotent_replay"], true, "必须显式声明为幂等重放");
}

/// 数学性质 2: 状态与版本单调性 (Monotonicity)
/// $\text{Rev}_{k+1} > \text{Rev}_k \land \text{Hash}_{k+1} \neq \text{Hash}_k$ (当内容改变时)
#[test]
fn test_math_invariant_revision_monotonicity() {
    let mut ws = CoEditWorkspace::default();
    let file = "workspace/monotonic.txt";
    ws.open_doc(file, "0\n", "plain", "system", "init");

    let mut prev_rev = 1u64;
    let mut prev_hash = ws.get_doc(file).unwrap().content_hash.clone();

    for i in 1..=5 {
        let text = format!("{}\n", i);
        let res = ws
            .coedit_full(FullCoEditRequest {
                file_id: file.into(),
                author: format!("agent_{i}"),
                new_text: text,
                base_revision: Some(prev_rev),
                op_id: None,
                mode: Some(CollabMode::Git),
                ..Default::default()
            })
            .unwrap();

        let cur_rev = res["revision"].as_u64().unwrap();
        let cur_hash = res["content_hash"].as_str().unwrap().to_string();

        assert_eq!(cur_rev, prev_rev + 1, "版本号必须严格按步长 1 单调递增");
        assert_ne!(cur_hash, prev_hash, "每次修改后内容指纹必须严格不同");

        prev_rev = cur_rev;
        prev_hash = cur_hash;
    }
}

/// 数学性质 3: 对称性与交换律 (Commutativity on Independent Disjoint Edits)
/// 若变更集合 $A$ 与 $B$ 作用于非相交行空间，则：
/// $\text{State}(A \circ B) \cong \text{State}(B \circ A)$
#[test]
fn test_math_invariant_commutativity_disjoint_edits() {
    let base = "fn head() { 0 }\n\nfn middle() { 50 }\n\nfn tail() { 100 }\n";

    // 试验 1: Agent A 先改 head，Agent B 后并发改 tail
    let mut ws1 = CoEditWorkspace::default();
    ws1.open_doc("test.rs", base, "rust", "sys", "init");
    ws1.coedit_full(FullCoEditRequest {
        file_id: "test.rs".into(),
        author: "A".into(),
        new_text: "fn head() { 111 }\n\nfn middle() { 50 }\n\nfn tail() { 100 }\n".into(),
        base_revision: Some(1),
        op_id: None,
        mode: Some(CollabMode::Git),
        ..Default::default()
    })
    .unwrap();
    let res1 = ws1
        .coedit_full(FullCoEditRequest {
            file_id: "test.rs".into(),
            author: "B".into(),
            new_text: "fn head() { 0 }\n\nfn middle() { 50 }\n\nfn tail() { 999 }\n".into(),
            base_revision: Some(1), // 从 base 提交
            op_id: None,
            mode: Some(CollabMode::Git),
            ..Default::default()
        })
        .unwrap();

    // 试验 2: Agent B 先改 tail，Agent A 后并发改 head (逆序)
    let mut ws2 = CoEditWorkspace::default();
    ws2.open_doc("test.rs", base, "rust", "sys", "init");
    ws2.coedit_full(FullCoEditRequest {
        file_id: "test.rs".into(),
        author: "B".into(),
        new_text: "fn head() { 0 }\n\nfn middle() { 50 }\n\nfn tail() { 999 }\n".into(),
        base_revision: Some(1),
        op_id: None,
        mode: Some(CollabMode::Git),
        ..Default::default()
    })
    .unwrap();
    let res2 = ws2
        .coedit_full(FullCoEditRequest {
            file_id: "test.rs".into(),
            author: "A".into(),
            new_text: "fn head() { 111 }\n\nfn middle() { 50 }\n\nfn tail() { 100 }\n".into(),
            base_revision: Some(1), // 从 base 提交
            op_id: None,
            mode: Some(CollabMode::Git),
            ..Default::default()
        })
        .unwrap();

    let text1 = res1["content"].as_str().unwrap();
    let text2 = res2["content"].as_str().unwrap();

    assert_eq!(
        text1, text2,
        "数学对称性：独立无交叉改动在两种提交时序下最终合并文本完全同构"
    );
    assert!(text1.contains("fn head() { 111 }"));
    assert!(text1.contains("fn middle() { 50 }"));
    assert!(text1.contains("fn tail() { 999 }"));
}
