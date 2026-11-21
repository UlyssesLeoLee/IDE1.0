//! 严格数学测试：实测 2 个代理共同编辑与单个代理编辑的速度差与并行效率
//!
//! 数学度量体系:
//! 1. 耗时与吞吐量: T_1 (单代理耗时), T_2 (双代理耗时), Throughput (ops/sec)
//! 2. 加速比 (Speedup): S = T_1 / T_2
//! 3. 并行效率 (Parallel Efficiency): E = S / 2 = T_1 / (2 * T_2)
//! 4. 阿姆达尔定律拟合 (Amdahl's Law): S = 1 / ((1 - f) + f / 2) => f = 2 * (1 - 1/S)
//! 5. 延迟统计分布: P50 (中位数), P90, P95, P99, Mean (mu), StdDev (sigma), CV
//! 6. 数学不变量检验:
//!    - 修订号严格单调性: delta(rev) == N
//!    - 内容无损性: 双方所有提交的函数/记录必须全部保存在最终版本中 (Losslessness)
//!    - NEMA 模式语法封闭性: parse_source(final_text) == Ok

#![forbid(unsafe_code)]

use ide_mcp::merge::{CoEditWorkspace, CollabMode, FullCoEditRequest};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 统计分布计算辅助结构
#[derive(Debug, Clone)]
pub struct MathDistribution {
    pub count: usize,
    pub total_ms: f64,
    pub mean_ms: f64,
    pub stddev_ms: f64,
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub throughput_ops_sec: f64,
}

impl MathDistribution {
    pub fn compute(mut latencies: Vec<f64>) -> Self {
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let count = latencies.len();
        if count == 0 {
            return Self {
                count: 0,
                total_ms: 0.0,
                mean_ms: 0.0,
                stddev_ms: 0.0,
                p50_ms: 0.0,
                p90_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                min_ms: 0.0,
                max_ms: 0.0,
                throughput_ops_sec: 0.0,
            };
        }

        let total_ms: f64 = latencies.iter().sum();
        let mean_ms = total_ms / count as f64;

        let variance: f64 = latencies.iter().map(|&x| (x - mean_ms).powi(2)).sum::<f64>()
            / if count > 1 { (count - 1) as f64 } else { 1.0 };
        let stddev_ms = variance.sqrt();

        let percentile = |p: f64| -> f64 {
            let idx = (p * (count - 1) as f64).round() as usize;
            latencies[idx.min(count - 1)]
        };

        let p50_ms = percentile(0.50);
        let p90_ms = percentile(0.90);
        let p95_ms = percentile(0.95);
        let p99_ms = percentile(0.99);
        let min_ms = latencies[0];
        let max_ms = latencies[count - 1];
        let throughput_ops_sec = if total_ms > 0.0 {
            (count as f64) / (total_ms / 1000.0)
        } else {
            0.0
        };

        Self {
            count,
            total_ms,
            mean_ms,
            stddev_ms,
            p50_ms,
            p90_ms,
            p95_ms,
            p99_ms,
            min_ms,
            max_ms,
            throughput_ops_sec,
        }
    }
}

// ============================================================================
// 测试 1: Git 模式下单代理 vs 双代理共同编辑基准压测
// ============================================================================
#[test]
fn test_math_bench_git_mode_single_vs_dual_speed() {
    println!("\n================================================================================");
    println!(" [Math Benchmark 1] Git-mode Co-Editing Speed: Single Agent vs Dual Agents");
    println!("================================================================================");

    let base_code = r#"// Shared Architecture Service
pub struct MathEngine {
    pub version: u32,
}

impl MathEngine {
    pub fn new() -> Self {
        Self { version: 1 }
    }
}
"#;

    const TOTAL_EDITS: usize = 100;

    // ------------------------------------------------------------------------
    // 1. 单代理顺序编辑基线 (Single Agent Baseline)
    // ------------------------------------------------------------------------
    let mut ws_single = CoEditWorkspace::default();
    let file_id = "project/src/math_service.rs";
    ws_single.open_doc(file_id, base_code, "rust", "system", "init");

    let mut single_latencies = Vec::with_capacity(TOTAL_EDITS);
    let t0 = Instant::now();

    for i in 1..=TOTAL_EDITS {
        let op_start = Instant::now();
        let cur_rev = ws_single.docs.get(file_id).unwrap().current_revision;
        let cur_content = ws_single.docs.get(file_id).unwrap().content.clone();

        // 单代理追加新函数
        let new_text = format!(
            "{cur_content}\n// Agent-A function #{i}\npub fn op_a_{i}(x: i32) -> i32 {{ x + {i} }}\n"
        );

        let res = ws_single
            .coedit_full(FullCoEditRequest {
                file_id: file_id.into(),
                author: "agent-a".into(),
                new_text,
                base_revision: Some(cur_rev),
                op_id: Some(format!("single-op-{i}")),
                mode: Some(CollabMode::Git),
            })
            .expect("Single agent coedit must succeed");

        assert_eq!(res["ok"], true);
        single_latencies.push(op_start.elapsed().as_secs_f64() * 1000.0);
    }

    let t1_total_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let single_dist = MathDistribution::compute(single_latencies);

    // 验证单代理数学不变量
    let final_single_doc = ws_single.docs.get(file_id).unwrap();
    assert_eq!(
        final_single_doc.current_revision,
        1 + TOTAL_EDITS as u64,
        "单代理版本号必须单调递增恰好 N"
    );

    // ------------------------------------------------------------------------
    // 2. 双代理并发协同编辑 (Dual Agents Concurrent Co-Editing)
    // ------------------------------------------------------------------------
    let ws_dual = Arc::new(Mutex::new(CoEditWorkspace::default()));
    {
        let mut ws = ws_dual.lock().unwrap();
        ws.open_doc(file_id, base_code, "rust", "system", "init");
    }

    let edits_per_agent = TOTAL_EDITS / 2;
    let dual_t0 = Instant::now();

    let ws_a = Arc::clone(&ws_dual);
    let ws_b = Arc::clone(&ws_dual);

    // Agent A 负责顶部模块函数插入，Agent B 负责底部模块函数插入
    let handle_a = std::thread::spawn(move || {
        let mut lats = Vec::with_capacity(edits_per_agent);
        for i in 1..=edits_per_agent {
            let start = Instant::now();
            let (cur_rev, cur_content) = {
                let ws = ws_a.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };

            // 在头部之后插入独立函数区段
            let new_text = format!(
                "// [Module A Head]\npub fn mod_a_{i}() -> &'static str {{ \"mod_a_{i}\" }}\n\n{cur_content}"
            );

            let res = {
                let mut ws = ws_a.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-a".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("dual-agent-a-{i}")),
                    mode: Some(CollabMode::Git),
                })
                .expect("Agent A coedit must succeed")
            };

            assert_eq!(res["ok"], true);
            lats.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        lats
    });

    let handle_b = std::thread::spawn(move || {
        let mut lats = Vec::with_capacity(edits_per_agent);
        for j in 1..=edits_per_agent {
            let start = Instant::now();
            let (cur_rev, cur_content) = {
                let ws = ws_b.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };

            // 在尾部追加独立函数区段
            let new_text = format!(
                "{cur_content}\n// [Module B Tail]\npub fn mod_b_{j}() -> &'static str {{ \"mod_b_{j}\" }}\n"
            );

            let res = {
                let mut ws = ws_b.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-b".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("dual-agent-b-{j}")),
                    mode: Some(CollabMode::Git),
                })
                .expect("Agent B coedit must succeed")
            };

            assert_eq!(res["ok"], true);
            lats.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        lats
    });

    let lats_a = handle_a.join().unwrap();
    let lats_b = handle_b.join().unwrap();
    let t2_total_ms = dual_t0.elapsed().as_secs_f64() * 1000.0;

    let mut all_dual_lats = lats_a;
    all_dual_lats.extend(lats_b);
    let dual_dist = MathDistribution::compute(all_dual_lats);

    // ------------------------------------------------------------------------
    // 3. 数学模型计算 (Speedup, Efficiency, Amdahl fraction)
    // ------------------------------------------------------------------------
    let speedup = t1_total_ms / t2_total_ms;
    let parallel_efficiency = (speedup / 2.0) * 100.0;
    let amdahl_f = 2.0 * (1.0 - (1.0 / speedup));

    println!("--------------------------------------------------------------------------------");
    println!(" [实测结果统计汇总 (Git 模式)]");
    println!("  - 总编辑事务数量 (N)         : {} 次", TOTAL_EDITS);
    println!("  - 单代理基准总耗时 (T_1)     : {:.3} ms (Throughput: {:.1} ops/sec)", t1_total_ms, single_dist.throughput_ops_sec);
    println!("  - 双代理协同总耗时 (T_2)     : {:.3} ms (Throughput: {:.1} ops/sec)", t2_total_ms, (TOTAL_EDITS as f64) / (t2_total_ms / 1000.0));
    println!("  - 实测加速比 S = T_1 / T_2   : {:.2}x", speedup);
    println!("  - 并行效率 E = S / 2         : {:.2}%", parallel_efficiency);
    println!("  - 阿姆达尔可并行化比例 (f)    : {:.2}%", amdahl_f * 100.0);
    println!("--------------------------------------------------------------------------------");
    println!(" [延迟分布分位数对比 (ms)]");
    println!("  指标         | 单代理 (T_1)   | 双代理 (T_2)   | 变化");
    println!("  ------------ | -------------- | -------------- | ------------");
    println!("  Mean (均值)  | {:>10.4} ms | {:>10.4} ms | {:>+8.2}%", single_dist.mean_ms, dual_dist.mean_ms, (dual_dist.mean_ms - single_dist.mean_ms)/single_dist.mean_ms * 100.0);
    println!("  P50 (中位数) | {:>10.4} ms | {:>10.4} ms | {:>+8.2}%", single_dist.p50_ms, dual_dist.p50_ms, (dual_dist.p50_ms - single_dist.p50_ms)/single_dist.p50_ms * 100.0);
    println!("  P95 (长尾)   | {:>10.4} ms | {:>10.4} ms | {:>+8.2}%", single_dist.p95_ms, dual_dist.p95_ms, (dual_dist.p95_ms - single_dist.p95_ms)/single_dist.p95_ms * 100.0);
    println!("  StdDev (σ)   | {:>10.4} ms | {:>10.4} ms |", single_dist.stddev_ms, dual_dist.stddev_ms);
    println!("--------------------------------------------------------------------------------");

    // ------------------------------------------------------------------------
    // 4. 严格数学不变式验证 (Losslessness & Monotonicity)
    // ------------------------------------------------------------------------
    let ws = ws_dual.lock().unwrap();
    let final_dual_doc = ws.docs.get(file_id).unwrap();

    assert_eq!(
        final_dual_doc.current_revision,
        1 + TOTAL_EDITS as u64,
        "[数学不变式 1] 双代理版本单调递增性: 最终修订号必须等于初始版本 + 总事务数"
    );

    // 内容无损性检验：所有 Agent A 和 Agent B 的函数必须均存在于最终文档中
    for i in 1..=edits_per_agent {
        let fn_a = format!("mod_a_{i}");
        assert!(
            final_dual_doc.content.contains(&fn_a),
            "[数学不变式 2] 无损性检验失败: Agent A 的修改 {} 丢失!",
            fn_a
        );
        let fn_b = format!("mod_b_{i}");
        assert!(
            final_dual_doc.content.contains(&fn_b),
            "[数学不变式 2] 无损性检验失败: Agent B 的修改 {} 丢失!",
            fn_b
        );
    }
    println!("  [PASS] 数学不变式检验全部通过: 无数据丢失，版本严格单调递增收敛!\n");
}

// ============================================================================
// 测试 2: NEMA v1.5 模式下单代理 vs 双代理语义合并速度压测
// ============================================================================
#[test]
fn test_math_bench_nema_mode_single_vs_dual_speed() {
    println!("\n================================================================================");
    println!(" [Math Benchmark 2] NEMA-mode Semantic Co-Editing Speed & Closure");
    println!("================================================================================");

    let base_nema = " \\1 1\n\\%2\n\\# Component Status\nCore Online\n\n\n";
    const NEMA_EDITS: usize = 60;
    let file_id = "knowledge/system.nema";

    // 1. 单代理顺序写入 NEMA 记录
    let mut ws_single = CoEditWorkspace::default();
    ws_single.open_doc(file_id, base_nema, "nema", "system", "init");

    let t0 = Instant::now();
    for i in 1..=NEMA_EDITS {
        let cur_rev = ws_single.docs.get(file_id).unwrap().current_revision;
        let cur_content = ws_single.docs.get(file_id).unwrap().content.clone();

        // 规范去除末尾 3 LFs 追加记录后补齐
        let trimmed = cur_content.trim_end_matches('\n');
        let new_text = format!("{trimmed}\nRecord_{i} Active\n\n\n");

        let res = ws_single
            .coedit_full(FullCoEditRequest {
                file_id: file_id.into(),
                author: "agent-a".into(),
                new_text,
                base_revision: Some(cur_rev),
                op_id: Some(format!("single-nema-{i}")),
                mode: Some(CollabMode::Nema),
            })
            .expect("Single NEMA coedit must succeed");

        assert_eq!(res["ok"], true);
    }
    let t1_ms = t0.elapsed().as_secs_f64() * 1000.0;

    // 2. 双代理并发语义追加 NEMA 记录
    let ws_dual = Arc::new(Mutex::new(CoEditWorkspace::default()));
    {
        let mut ws = ws_dual.lock().unwrap();
        ws.open_doc(file_id, base_nema, "nema", "system", "init");
    }

    let edits_half = NEMA_EDITS / 2;
    let t0_dual = Instant::now();

    let ws_a = Arc::clone(&ws_dual);
    let ws_b = Arc::clone(&ws_dual);

    let handle_a = std::thread::spawn(move || {
        for i in 1..=edits_half {
            let (cur_rev, cur_content) = {
                let ws = ws_a.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };
            let trimmed = cur_content.trim_end_matches('\n');
            let new_text = format!("{trimmed}\nAgentA_Item_{i} Ready\n\n\n");

            let res = {
                let mut ws = ws_a.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-a".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("dual-nema-a-{i}")),
                    mode: Some(CollabMode::Nema),
                })
                .unwrap()
            };
            assert_eq!(res["ok"], true);
        }
    });

    let handle_b = std::thread::spawn(move || {
        for j in 1..=edits_half {
            let (cur_rev, cur_content) = {
                let ws = ws_b.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };
            let trimmed = cur_content.trim_end_matches('\n');
            let new_text = format!("{trimmed}\nAgentB_Item_{j} Verified\n\n\n");

            let res = {
                let mut ws = ws_b.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-b".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("dual-nema-b-{j}")),
                    mode: Some(CollabMode::Nema),
                })
                .unwrap()
            };
            assert_eq!(res["ok"], true);
        }
    });

    handle_a.join().unwrap();
    handle_b.join().unwrap();
    let t2_ms = t0_dual.elapsed().as_secs_f64() * 1000.0;

    let speedup = t1_ms / t2_ms;
    let efficiency = (speedup / 2.0) * 100.0;

    println!("--------------------------------------------------------------------------------");
    println!(" [实测结果统计汇总 (NEMA 语义合并模式)]");
    println!("  - 总编辑事务数量 (N)         : {} 次", NEMA_EDITS);
    println!("  - 单代理基准总耗时 (T_1)     : {:.3} ms (Throughput: {:.1} ops/sec)", t1_ms, (NEMA_EDITS as f64) / (t1_ms / 1000.0));
    println!("  - 双代理协同总耗时 (T_2)     : {:.3} ms (Throughput: {:.1} ops/sec)", t2_ms, (NEMA_EDITS as f64) / (t2_ms / 1000.0));
    println!("  - 实测加速比 S = T_1 / T_2   : {:.2}x", speedup);
    println!("  - 并行效率 E = S / 2         : {:.2}%", efficiency);
    println!("--------------------------------------------------------------------------------");

    // 语法闭包与无损检验
    let ws = ws_dual.lock().unwrap();
    let final_nema_doc = ws.docs.get(file_id).unwrap();

    let parse_check = nema_codec::parse_source(&final_nema_doc.content);
    assert!(
        parse_check.is_ok(),
        "[数学闭包] NEMA 协同合并结果必须保证形式化语法闭包有效"
    );

    for i in 1..=edits_half {
        assert!(final_nema_doc.content.contains(&format!("AgentA_Item_{i} Ready")));
        assert!(final_nema_doc.content.contains(&format!("AgentB_Item_{i} Verified")));
    }
    println!("  [PASS] NEMA 语义合并语法封闭性与无损集合恒等式全部成立!\n");
}
