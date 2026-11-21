//! 严格数学测试：实测 2 个代理共同编辑与单个代理编辑的速度差与并行效率
//!
//! 对比两种架构:
//!   Old: Arc<Mutex<CoEditWorkspace>>  — 全局互斥，同文件编辑完全串行
//!   New: ConcurrentCoEditWorkspace    — OCC 3-Phase (snapshot→lock-free merge→CAS commit)
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

use ide_mcp::merge::{CoEditWorkspace, CollabMode, ConcurrentCoEditWorkspace, FullCoEditRequest};
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

        let variance: f64 = latencies
            .iter()
            .map(|&x| (x - mean_ms).powi(2))
            .sum::<f64>()
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
// 测试 1: Git 模式 — OCC 并发 vs 旧 Mutex 串行 vs 单代理基线
// ============================================================================
#[test]
fn test_math_bench_git_mode_single_vs_dual_speed() {
    println!("\n================================================================================");
    println!(" [Math Benchmark 1] Git-mode Co-Editing Speed: Single vs OCC-Dual vs Mutex-Dual");
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
    // 1. 单代理顺序编辑基线 (Single Agent Baseline) — 使用 OCC workspace
    // ------------------------------------------------------------------------
    let ws_single = ConcurrentCoEditWorkspace::default();
    let file_id = "project/src/math_service.rs";
    ws_single.open_doc(file_id, base_code, "rust", "system", "init");

    let mut single_latencies = Vec::with_capacity(TOTAL_EDITS);
    let t0 = Instant::now();

    for i in 1..=TOTAL_EDITS {
        let op_start = Instant::now();
        let (cur_rev, cur_content) = ws_single.snapshot_doc(file_id).unwrap();

        let new_text = format!(
            "{cur_content}\n// Agent-A function #{i}\npub fn op_a_{i}(x: i32) -> i32 {{ x + {i} }}\n"
        );

        let res = ws_single
            .coedit_concurrent(FullCoEditRequest {
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

    // 单代理不变量验证
    let (single_final_rev, _) = ws_single.snapshot_doc(file_id).unwrap();
    assert_eq!(
        single_final_rev,
        1 + TOTAL_EDITS as u64,
        "单代理版本号必须单调递增恰好 N"
    );

    // ------------------------------------------------------------------------
    // 2. OCC 双代理并发协同 (New Architecture — ConcurrentCoEditWorkspace)
    // ------------------------------------------------------------------------
    let ws_occ = Arc::new(ConcurrentCoEditWorkspace::default());
    ws_occ.open_doc(file_id, base_code, "rust", "system", "init");

    let edits_per_agent = TOTAL_EDITS / 2;
    let dual_occ_t0 = Instant::now();

    let ws_a = Arc::clone(&ws_occ);
    let ws_b = Arc::clone(&ws_occ);

    let handle_a = std::thread::spawn(move || {
        let mut lats = Vec::with_capacity(edits_per_agent);
        for i in 1..=edits_per_agent {
            let start = Instant::now();
            let (cur_rev, cur_content) = ws_a.snapshot_doc(file_id).unwrap();

            let new_text = format!(
                "// [Module A Head]\npub fn mod_a_{i}() -> &'static str {{ \"mod_a_{i}\" }}\n\n{cur_content}"
            );

            let res = ws_a
                .coedit_concurrent(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-a".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("occ-agent-a-{i}")),
                    mode: Some(CollabMode::Git),
                })
                .expect("OCC Agent A coedit must succeed");

            assert_eq!(res["ok"], true);
            lats.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        lats
    });

    let handle_b = std::thread::spawn(move || {
        let mut lats = Vec::with_capacity(edits_per_agent);
        for j in 1..=edits_per_agent {
            let start = Instant::now();
            let (cur_rev, cur_content) = ws_b.snapshot_doc(file_id).unwrap();

            let new_text = format!(
                "{cur_content}\n// [Module B Tail]\npub fn mod_b_{j}() -> &'static str {{ \"mod_b_{j}\" }}\n"
            );

            let res = ws_b
                .coedit_concurrent(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-b".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("occ-agent-b-{j}")),
                    mode: Some(CollabMode::Git),
                })
                .expect("OCC Agent B coedit must succeed");

            assert_eq!(res["ok"], true);
            lats.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        lats
    });

    let lats_a = handle_a.join().unwrap();
    let lats_b = handle_b.join().unwrap();
    let t2_occ_ms = dual_occ_t0.elapsed().as_secs_f64() * 1000.0;

    let mut all_occ_lats = lats_a;
    all_occ_lats.extend(lats_b);
    let occ_dist = MathDistribution::compute(all_occ_lats);

    // ------------------------------------------------------------------------
    // 3. 旧 Mutex 双代理并发 (Old Architecture — Arc<Mutex<CoEditWorkspace>>)
    // ------------------------------------------------------------------------
    let ws_mutex = Arc::new(Mutex::new(CoEditWorkspace::default()));
    {
        let mut ws = ws_mutex.lock().unwrap();
        ws.open_doc(file_id, base_code, "rust", "system", "init");
    }

    let dual_mutex_t0 = Instant::now();

    let ws_ma = Arc::clone(&ws_mutex);
    let ws_mb = Arc::clone(&ws_mutex);

    let handle_ma = std::thread::spawn(move || {
        let mut lats = Vec::with_capacity(edits_per_agent);
        for i in 1..=edits_per_agent {
            let start = Instant::now();
            let (cur_rev, cur_content) = {
                let ws = ws_ma.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };

            let new_text = format!(
                "// [Module A Head]\npub fn mod_a_{i}() -> &'static str {{ \"mod_a_{i}\" }}\n\n{cur_content}"
            );

            let res = {
                let mut ws = ws_ma.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-a".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("mutex-agent-a-{i}")),
                    mode: Some(CollabMode::Git),
                })
                .expect("Mutex Agent A coedit must succeed")
            };

            assert_eq!(res["ok"], true);
            lats.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        lats
    });

    let handle_mb = std::thread::spawn(move || {
        let mut lats = Vec::with_capacity(edits_per_agent);
        for j in 1..=edits_per_agent {
            let start = Instant::now();
            let (cur_rev, cur_content) = {
                let ws = ws_mb.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };

            let new_text = format!(
                "{cur_content}\n// [Module B Tail]\npub fn mod_b_{j}() -> &'static str {{ \"mod_b_{j}\" }}\n"
            );

            let res = {
                let mut ws = ws_mb.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-b".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("mutex-agent-b-{j}")),
                    mode: Some(CollabMode::Git),
                })
                .expect("Mutex Agent B coedit must succeed")
            };

            assert_eq!(res["ok"], true);
            lats.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        lats
    });

    let lats_ma = handle_ma.join().unwrap();
    let lats_mb = handle_mb.join().unwrap();
    let t2_mutex_ms = dual_mutex_t0.elapsed().as_secs_f64() * 1000.0;

    let mut all_mutex_lats = lats_ma;
    all_mutex_lats.extend(lats_mb);
    let mutex_dist = MathDistribution::compute(all_mutex_lats);

    // ------------------------------------------------------------------------
    // 4. 数学模型计算
    // ------------------------------------------------------------------------
    let speedup_occ = t1_total_ms / t2_occ_ms;
    let speedup_mutex = t1_total_ms / t2_mutex_ms;
    let efficiency_occ = (speedup_occ / 2.0) * 100.0;
    let efficiency_mutex = (speedup_mutex / 2.0) * 100.0;
    let improvement = t2_mutex_ms / t2_occ_ms;

    println!("--------------------------------------------------------------------------------");
    println!(" [实测结果统计汇总 (Git 模式)]");
    println!("  - 总编辑事务数量 (N)             : {} 次", TOTAL_EDITS);
    println!("  - 单代理基准总耗时 (T_1)         : {:.3} ms", t1_total_ms);
    println!("  ─────────────────────────────────────────────────────────");
    println!("  - [OCC]   双代理总耗时 (T_2)     : {:.3} ms", t2_occ_ms);
    println!("  - [OCC]   加速比 S = T_1/T_2     : {:.2}x", speedup_occ);
    println!(
        "  - [OCC]   并行效率 E = S/2       : {:.2}%",
        efficiency_occ
    );
    println!("  ─────────────────────────────────────────────────────────");
    println!("  - [Mutex] 双代理总耗时 (T_2)     : {:.3} ms", t2_mutex_ms);
    println!("  - [Mutex] 加速比 S = T_1/T_2     : {:.2}x", speedup_mutex);
    println!(
        "  - [Mutex] 并行效率 E = S/2       : {:.2}%",
        efficiency_mutex
    );
    println!("  ─────────────────────────────────────────────────────────");
    println!(
        "  >>> OCC vs Mutex 提速比           : {:.2}x <<<",
        improvement
    );
    println!("--------------------------------------------------------------------------------");
    println!(" [延迟分布分位数对比 (ms)]");
    println!("  指标         | 单代理         | OCC 双代理     | Mutex 双代理");
    println!("  ------------ | -------------- | -------------- | --------------");
    println!(
        "  Mean (均值)  | {:>10.4} ms | {:>10.4} ms | {:>10.4} ms",
        single_dist.mean_ms, occ_dist.mean_ms, mutex_dist.mean_ms
    );
    println!(
        "  P50 (中位数) | {:>10.4} ms | {:>10.4} ms | {:>10.4} ms",
        single_dist.p50_ms, occ_dist.p50_ms, mutex_dist.p50_ms
    );
    println!(
        "  P95 (长尾)   | {:>10.4} ms | {:>10.4} ms | {:>10.4} ms",
        single_dist.p95_ms, occ_dist.p95_ms, mutex_dist.p95_ms
    );
    println!(
        "  StdDev       | {:>10.4} ms | {:>10.4} ms | {:>10.4} ms",
        single_dist.stddev_ms, occ_dist.stddev_ms, mutex_dist.stddev_ms
    );
    println!("--------------------------------------------------------------------------------");

    // ------------------------------------------------------------------------
    // 5. 数学不变式验证 (OCC workspace)
    // ------------------------------------------------------------------------
    let (occ_final_rev, occ_final_content) = ws_occ.snapshot_doc(file_id).unwrap();

    assert_eq!(
        occ_final_rev,
        1 + TOTAL_EDITS as u64,
        "[数学不变式 1] OCC 双代理版本单调递增性: 最终修订号必须等于 1 + N"
    );

    for i in 1..=edits_per_agent {
        let fn_a = format!("mod_a_{i}");
        assert!(
            occ_final_content.contains(&fn_a),
            "[数学不变式 2] 无损性检验失败: OCC Agent A 的修改 {} 丢失!",
            fn_a
        );
        let fn_b = format!("mod_b_{i}");
        assert!(
            occ_final_content.contains(&fn_b),
            "[数学不变式 2] 无损性检验失败: OCC Agent B 的修改 {} 丢失!",
            fn_b
        );
    }

    // Mutex workspace 同样验证
    let ws = ws_mutex.lock().unwrap();
    let final_mutex_doc = ws.docs.get(file_id).unwrap();
    assert_eq!(
        final_mutex_doc.current_revision,
        1 + TOTAL_EDITS as u64,
        "[数学不变式 1] Mutex 双代理版本单调递增性"
    );
    for i in 1..=edits_per_agent {
        assert!(final_mutex_doc.content.contains(&format!("mod_a_{i}")));
        assert!(final_mutex_doc.content.contains(&format!("mod_b_{i}")));
    }

    println!("  [PASS] 数学不变式检验全部通过: 无数据丢失，版本严格单调递增收敛!");
    println!(
        "  [PASS] OCC 加速比: {:.2}x, Mutex 加速比: {:.2}x, OCC 相对提速: {:.2}x\n",
        speedup_occ, speedup_mutex, improvement
    );
}

// ============================================================================
// 测试 2: NEMA v1.5 模式 — OCC 并发 vs 旧 Mutex 串行 vs 单代理基线
// ============================================================================
#[test]
fn test_math_bench_nema_mode_single_vs_dual_speed() {
    println!("\n================================================================================");
    println!(" [Math Benchmark 2] NEMA-mode Semantic Co-Editing Speed & Closure");
    println!("================================================================================");

    let base_nema = " \\1 1\n\\%2\n\\# Component Status\nCore Online\n\n\n";
    const NEMA_EDITS: usize = 60;
    let file_id = "knowledge/system.nema";

    // 1. 单代理顺序写入 NEMA 记录 (OCC workspace)
    let ws_single = ConcurrentCoEditWorkspace::default();
    ws_single.open_doc(file_id, base_nema, "nema", "system", "init");

    let t0 = Instant::now();
    for i in 1..=NEMA_EDITS {
        let (cur_rev, cur_content) = ws_single.snapshot_doc(file_id).unwrap();

        let trimmed = cur_content.trim_end_matches('\n');
        let new_text = format!("{trimmed}\nRecord_{i} Active\n\n\n");

        let res = ws_single
            .coedit_concurrent(FullCoEditRequest {
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

    // 2. OCC 双代理并发 NEMA
    let ws_occ = Arc::new(ConcurrentCoEditWorkspace::default());
    ws_occ.open_doc(file_id, base_nema, "nema", "system", "init");

    let edits_half = NEMA_EDITS / 2;
    let t0_occ = Instant::now();

    let ws_a = Arc::clone(&ws_occ);
    let ws_b = Arc::clone(&ws_occ);

    let handle_a = std::thread::spawn(move || {
        for i in 1..=edits_half {
            let (cur_rev, cur_content) = ws_a.snapshot_doc(file_id).unwrap();
            let trimmed = cur_content.trim_end_matches('\n');
            let new_text = format!("{trimmed}\nAgentA_Item_{i} Ready\n\n\n");

            let res = ws_a
                .coedit_concurrent(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-a".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("occ-nema-a-{i}")),
                    mode: Some(CollabMode::Nema),
                })
                .unwrap();
            assert_eq!(res["ok"], true);
        }
    });

    let handle_b = std::thread::spawn(move || {
        for j in 1..=edits_half {
            let (cur_rev, cur_content) = ws_b.snapshot_doc(file_id).unwrap();
            let trimmed = cur_content.trim_end_matches('\n');
            let new_text = format!("{trimmed}\nAgentB_Item_{j} Verified\n\n\n");

            let res = ws_b
                .coedit_concurrent(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-b".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("occ-nema-b-{j}")),
                    mode: Some(CollabMode::Nema),
                })
                .unwrap();
            assert_eq!(res["ok"], true);
        }
    });

    handle_a.join().unwrap();
    handle_b.join().unwrap();
    let t2_occ_ms = t0_occ.elapsed().as_secs_f64() * 1000.0;

    // 3. 旧 Mutex 双代理并发 NEMA
    let ws_mutex = Arc::new(Mutex::new(CoEditWorkspace::default()));
    {
        let mut ws = ws_mutex.lock().unwrap();
        ws.open_doc(file_id, base_nema, "nema", "system", "init");
    }

    let t0_mutex = Instant::now();

    let ws_ma = Arc::clone(&ws_mutex);
    let ws_mb = Arc::clone(&ws_mutex);

    let handle_ma = std::thread::spawn(move || {
        for i in 1..=edits_half {
            let (cur_rev, cur_content) = {
                let ws = ws_ma.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };
            let trimmed = cur_content.trim_end_matches('\n');
            let new_text = format!("{trimmed}\nAgentA_Item_{i} Ready\n\n\n");

            let res = {
                let mut ws = ws_ma.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-a".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("mutex-nema-a-{i}")),
                    mode: Some(CollabMode::Nema),
                })
                .unwrap()
            };
            assert_eq!(res["ok"], true);
        }
    });

    let handle_mb = std::thread::spawn(move || {
        for j in 1..=edits_half {
            let (cur_rev, cur_content) = {
                let ws = ws_mb.lock().unwrap();
                let doc = ws.docs.get(file_id).unwrap();
                (doc.current_revision, doc.content.clone())
            };
            let trimmed = cur_content.trim_end_matches('\n');
            let new_text = format!("{trimmed}\nAgentB_Item_{j} Verified\n\n\n");

            let res = {
                let mut ws = ws_mb.lock().unwrap();
                ws.coedit_full(FullCoEditRequest {
                    file_id: file_id.into(),
                    author: "agent-b".into(),
                    new_text,
                    base_revision: Some(cur_rev),
                    op_id: Some(format!("mutex-nema-b-{j}")),
                    mode: Some(CollabMode::Nema),
                })
                .unwrap()
            };
            assert_eq!(res["ok"], true);
        }
    });

    handle_ma.join().unwrap();
    handle_mb.join().unwrap();
    let t2_mutex_ms = t0_mutex.elapsed().as_secs_f64() * 1000.0;

    let speedup_occ = t1_ms / t2_occ_ms;
    let speedup_mutex = t1_ms / t2_mutex_ms;
    let improvement = t2_mutex_ms / t2_occ_ms;

    println!("--------------------------------------------------------------------------------");
    println!(" [实测结果统计汇总 (NEMA 语义合并模式)]");
    println!("  - 总编辑事务数量 (N)             : {} 次", NEMA_EDITS);
    println!("  - 单代理基准总耗时 (T_1)         : {:.3} ms", t1_ms);
    println!("  ─────────────────────────────────────────────────────────");
    println!("  - [OCC]   双代理总耗时 (T_2)     : {:.3} ms", t2_occ_ms);
    println!("  - [OCC]   加速比 S = T_1/T_2     : {:.2}x", speedup_occ);
    println!("  ─────────────────────────────────────────────────────────");
    println!("  - [Mutex] 双代理总耗时 (T_2)     : {:.3} ms", t2_mutex_ms);
    println!("  - [Mutex] 加速比 S = T_1/T_2     : {:.2}x", speedup_mutex);
    println!("  ─────────────────────────────────────────────────────────");
    println!(
        "  >>> OCC vs Mutex 提速比           : {:.2}x <<<",
        improvement
    );
    println!("--------------------------------------------------------------------------------");

    // 语法闭包与无损检验 (OCC)
    let (_, occ_final_content) = ws_occ.snapshot_doc(file_id).unwrap();
    let parse_check = nema_codec::parse_source(&occ_final_content);
    assert!(
        parse_check.is_ok(),
        "[数学闭包] OCC NEMA 协同合并结果必须保证形式化语法闭包有效"
    );

    for i in 1..=edits_half {
        assert!(occ_final_content.contains(&format!("AgentA_Item_{i} Ready")));
        assert!(occ_final_content.contains(&format!("AgentB_Item_{i} Verified")));
    }

    // Mutex 同样验证
    let ws = ws_mutex.lock().unwrap();
    let mutex_final = ws.docs.get(file_id).unwrap();
    assert!(nema_codec::parse_source(&mutex_final.content).is_ok());
    for i in 1..=edits_half {
        assert!(mutex_final
            .content
            .contains(&format!("AgentA_Item_{i} Ready")));
        assert!(mutex_final
            .content
            .contains(&format!("AgentB_Item_{i} Verified")));
    }

    println!("  [PASS] NEMA 语义合并语法封闭性与无损集合恒等式全部成立!");
    println!(
        "  [PASS] OCC 加速比: {:.2}x, Mutex 加速比: {:.2}x, OCC 相对提速: {:.2}x\n",
        speedup_occ, speedup_mutex, improvement
    );
}
