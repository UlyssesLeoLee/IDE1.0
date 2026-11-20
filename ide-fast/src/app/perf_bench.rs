//! 数学化性能基准与渐近复杂度验证体系:
//!
//! 1. **完整数理统计分布**: 热身 (Warmup) + 多次采样 ($N$), 输出中位数 ($P_{50}$)、均值 ($\mu$)、
//!    无偏样本标准差 ($\sigma$)、变异系数 ($\text{CV} = \sigma / \mu$)、Student's $t$ 95% 置信区间
//!    ($\text{CI}_{95} = \pm t_{0.975, N-1}\sigma/\sqrt{N}$, 下界截断至 $\ge 0$)、四分位距 ($\text{IQR} = P_{75} - P_{25}$)、
//!    Tukey 1.5×IQR 离群值计数与剔除离群值均值、$P_{90} / P_{95} / P_{99}$ 分位点 ($N \ge 5$ 时报告)、
//!    吞吐量 ($\Theta$) 与 SLA 硬性门限校验。
//! 2. **渐近复杂度数学验算**: 通过对多尺度工作负载 $N \in \{5\,000,\, 20\,000,\, 80\,000,\, 200\,000\}$ 的实测耗时做
//!    对数-对数最小二乘回归 ($\ln T(N) = \alpha \ln N + \beta$), 计算经验复杂度指数 $\alpha$ 与拟合优度 $R^2$,
//!    数学证明稳态渲染 / 单次编辑保持 $O(1)$ ($\alpha \le 0.25$) 而非退化为 $O(N)$, 全文打开 / 搜索保持线性 $O(N)$ ($\alpha \le 1.25$)。
//!
//! 运行 (由 `tests/perf-math-bench.ps1` 或 `ide-fast/tests/perf.ps1` 调用):
//!   cargo test -p ide-fast --release perf_bench -- --ignored --nocapture --test-threads=1

use super::*;
use crate::doc::Doc;
use std::time::Instant;

/// 双侧 95% Student's t 临界值 ($t_{0.975, \nu}$)
pub fn t_crit_95(df: usize) -> f64 {
    const TABLE: [f64; 30] = [
        12.706204736432095,
        4.302652729911275,
        3.182446305284263,
        2.7764451051977987,
        2.5705818366147395,
        2.4469118487916806,
        2.3646242510102993,
        2.3060041350333704,
        2.2621571627409915,
        2.2281388519649385,
        2.200985160082949,
        2.1788128296634177,
        2.1603686564610127,
        2.1447866879169273,
        2.131449545559323,
        2.1199052992210112,
        2.1098155778331806,
        2.10092204024096,
        2.093024054408263,
        2.0859634472658364,
        2.079613844727662,
        2.0738730679040147,
        2.0686576104190406,
        2.0638985616280205,
        2.059538552753294,
        2.055529438642871,
        2.0518305164802833,
        2.048407141795244,
        2.045229642132703,
        2.0422724563012373,
    ];
    if df == 0 {
        0.0
    } else if df <= TABLE.len() {
        TABLE[df - 1]
    } else {
        // Cornish-Fisher 大自由度渐近展开
        let z = 1.959963984540054_f64;
        let d = df as f64;
        z + (z.powi(3) + z) / (4.0 * d)
            + (5.0 * z.powi(5) + 16.0 * z.powi(3) + 3.0 * z) / (96.0 * d * d)
    }
}

/// 单个基准测试的完整数理统计摘要
#[derive(Debug, Clone)]
pub struct BenchStats {
    pub name: &'static str,
    pub n: usize,
    pub warmup: usize,
    pub median_ms: f64,
    pub mean_ms: f64,
    pub std_dev_ms: f64,
    pub cv: f64,
    pub ci95_ms: f64,
    pub ci95_low_ms: f64,
    pub ci95_high_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub p90_ms: Option<f64>,
    pub p95_ms: Option<f64>,
    pub p99_ms: Option<f64>,
    pub iqr_ms: f64,
    pub outliers: usize,
    pub trimmed_mean_ms: f64,
    pub throughput: f64,
    pub unit: &'static str,
    pub sla_limit_ms: Option<f64>,
    pub sla_pass: bool,
}

impl BenchStats {
    /// Hyndman-Fan Type 7 连续线性插值分位点 (与 R / NumPy / SciPy 一致)
    pub fn percentile(sorted: &[f64], p: f64) -> f64 {
        let n = sorted.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return sorted[0];
        }
        let pos = p.clamp(0.0, 1.0) * (n - 1) as f64;
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        let frac = pos - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }

    pub fn from_samples(
        name: &'static str,
        warmup: usize,
        raw: &[f64],
        work_units: f64,
        unit: &'static str,
        sla_limit_ms: Option<f64>,
    ) -> Self {
        let n = raw.len();
        assert!(n > 0, "BenchStats requires at least 1 sample");
        let mut sorted = raw.to_vec();
        sorted.sort_by(|a, b| a.total_cmp(b));

        let sum: f64 = sorted.iter().sum();
        let mean_ms = sum / n as f64;
        let std_dev_ms = if n > 1 {
            let var = sorted
                .iter()
                .map(|x| {
                    let d = x - mean_ms;
                    d * d
                })
                .sum::<f64>()
                / (n - 1) as f64;
            var.sqrt()
        } else {
            0.0
        };
        let cv = if mean_ms > 1e-12 {
            std_dev_ms / mean_ms
        } else {
            0.0
        };
        let ci95_ms = t_crit_95(n.saturating_sub(1)) * std_dev_ms / (n as f64).sqrt();
        let ci95_low_ms = (mean_ms - ci95_ms).max(0.0);
        let ci95_high_ms = mean_ms + ci95_ms;
        let min_ms = sorted[0];
        let max_ms = sorted[n - 1];
        let p25 = Self::percentile(&sorted, 0.25);
        let median_ms = Self::percentile(&sorted, 0.50);
        let p75 = Self::percentile(&sorted, 0.75);
        let (p90_ms, p95_ms, p99_ms) = if n >= 5 {
            (
                Some(Self::percentile(&sorted, 0.90)),
                Some(Self::percentile(&sorted, 0.95)),
                Some(Self::percentile(&sorted, 0.99)),
            )
        } else {
            (None, None, None)
        };
        let iqr_ms = p75 - p25;
        let (outliers, trimmed_mean_ms) = if n >= 4 {
            let lo_fence = p25 - 1.5 * iqr_ms;
            let hi_fence = p75 + 1.5 * iqr_ms;
            let mut inlier_sum = 0.0;
            let mut inlier_cnt = 0usize;
            let mut out_cnt = 0usize;
            for &x in &sorted {
                if x < lo_fence || x > hi_fence {
                    out_cnt += 1;
                } else {
                    inlier_sum += x;
                    inlier_cnt += 1;
                }
            }
            let tmean = if inlier_cnt > 0 {
                inlier_sum / inlier_cnt as f64
            } else {
                mean_ms
            };
            (out_cnt, tmean)
        } else {
            (0, mean_ms)
        };
        let throughput = if work_units > 0.0 && mean_ms > 1e-12 {
            work_units / (mean_ms / 1000.0)
        } else {
            0.0
        };
        let sla_pass = sla_limit_ms.map_or(true, |lim| median_ms <= lim);

        Self {
            name,
            n,
            warmup,
            median_ms,
            mean_ms,
            std_dev_ms,
            cv,
            ci95_ms,
            ci95_low_ms,
            ci95_high_ms,
            min_ms,
            max_ms,
            p90_ms,
            p95_ms,
            p99_ms,
            iqr_ms,
            outliers,
            trimmed_mean_ms,
            throughput,
            unit,
            sla_limit_ms,
            sla_pass,
        }
    }

    pub fn emit(&self) {
        let round3 = |v: f64| (v * 1000.0).round() / 1000.0;
        // 兼容旧版 perf.ps1 行协议
        println!("PERF {} {:.3}", self.name, self.median_ms);
        // 结构化数学分布行
        let obj = serde_json::json!({
            "name": self.name,
            "n": self.n,
            "warmup": self.warmup,
            "median_ms": round3(self.median_ms),
            "mean_ms": round3(self.mean_ms),
            "std_dev_ms": round3(self.std_dev_ms),
            "cv": (self.cv * 10000.0).round() / 10000.0,
            "ci95_ms": round3(self.ci95_ms),
            "ci95_low_ms": round3(self.ci95_low_ms),
            "ci95_high_ms": round3(self.ci95_high_ms),
            "min_ms": round3(self.min_ms),
            "max_ms": round3(self.max_ms),
            "p90_ms": self.p90_ms.map(round3),
            "p95_ms": self.p95_ms.map(round3),
            "p99_ms": self.p99_ms.map(round3),
            "iqr_ms": round3(self.iqr_ms),
            "outliers": self.outliers,
            "trimmed_mean_ms": round3(self.trimmed_mean_ms),
            "throughput": (self.throughput * 100.0).round() / 100.0,
            "unit": self.unit,
            "sla_limit_ms": self.sla_limit_ms,
            "sla_pass": self.sla_pass,
        });
        println!("PERF_MATH {}", obj);
    }
}

/// 渐近复杂度最小二乘幂律拟合模型: $\ln T(N) = \alpha \ln N + \beta$
#[derive(Debug, Clone)]
pub struct ComplexityModel {
    pub name: &'static str,
    pub expected: &'static str,
    pub max_alpha: f64,
    pub scales: Vec<usize>,
    pub medians_ms: Vec<f64>,
    pub alpha: f64,
    pub beta: f64,
    pub r_squared: f64,
    pub pass: bool,
}

impl ComplexityModel {
    pub fn fit_ols(
        name: &'static str,
        expected: &'static str,
        max_alpha: f64,
        scales: &[usize],
        medians_ms: &[f64],
    ) -> Self {
        assert_eq!(scales.len(), medians_ms.len());
        assert!(scales.len() >= 2, "OLS fit needs >= 2 scale points");
        let k = scales.len() as f64;
        let xs: Vec<f64> = scales.iter().map(|&n| (n.max(1) as f64).ln()).collect();
        let ys: Vec<f64> = medians_ms.iter().map(|&t| t.max(1e-6).ln()).collect();
        let x_bar = xs.iter().sum::<f64>() / k;
        let y_bar = ys.iter().sum::<f64>() / k;

        let mut sxx = 0.0;
        let mut sxy = 0.0;
        let mut syy = 0.0;
        for i in 0..scales.len() {
            let dx = xs[i] - x_bar;
            let dy = ys[i] - y_bar;
            sxx += dx * dx;
            sxy += dx * dy;
            syy += dy * dy;
        }

        let alpha = if sxx > 1e-12 { sxy / sxx } else { 0.0 };
        let beta = y_bar - alpha * x_bar;
        let r_squared = if sxx <= 1e-12 {
            0.0
        } else if syy <= 1e-12 {
            // 恒定响应时间 (零残差 O(1) 常数函数): 完全拟合 R^2 = 1.0
            1.0
        } else {
            ((sxy * sxy) / (sxx * syy)).clamp(0.0, 1.0)
        };
        let pass = alpha <= max_alpha;

        Self {
            name,
            expected,
            max_alpha,
            scales: scales.to_vec(),
            medians_ms: medians_ms.to_vec(),
            alpha,
            beta,
            r_squared,
            pass,
        }
    }

    pub fn emit(&self) {
        let obj = serde_json::json!({
            "name": self.name,
            "expected": self.expected,
            "max_alpha": self.max_alpha,
            "scales": self.scales,
            "medians_ms": self.medians_ms.iter().map(|x| (x * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            "alpha": (self.alpha * 10000.0).round() / 10000.0,
            "beta": (self.beta * 10000.0).round() / 10000.0,
            "r_squared": (self.r_squared * 10000.0).round() / 10000.0,
            "pass": self.pass,
        });
        println!("PERF_COMPLEXITY {}", obj);
    }
}

fn measure_stats(
    name: &'static str,
    warmup: usize,
    samples: usize,
    work_units: f64,
    unit: &'static str,
    sla_limit_ms: Option<f64>,
    mut f: impl FnMut(),
) -> BenchStats {
    for _ in 0..warmup {
        f();
    }
    let mut raw = Vec::with_capacity(samples);
    for _ in 0..samples {
        let t = Instant::now();
        f();
        raw.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let stats = BenchStats::from_samples(name, warmup, &raw, work_units, unit, sla_limit_ms);
    stats.emit();
    stats
}

fn code_line(i: usize) -> String {
    format!("fn item_{i}(x: u32) -> u32 {{ let y = x + {i}; /* comment */ y * 2 }}")
}

/// 约 `bytes` 字节的多行代码文本
fn code_text(bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 100);
    let mut i = 0;
    while s.len() < bytes {
        s.push_str(&code_line(i));
        s.push('\n');
        i += 1;
    }
    s
}

/// 生成 `n_lines` 行的基准源码文本
fn lines_text(n_lines: usize) -> String {
    let mut s = String::with_capacity(n_lines * 76);
    for i in 0..n_lines {
        s.push_str(&code_line(i));
        s.push('\n');
    }
    s
}

/// RAII 临时基准文件目录 (测试结束或 panic 展开时自动清理)
struct TempBenchDir(PathBuf);
impl TempBenchDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("IDE1.0-perf-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, text).unwrap();
        p
    }
}
impl Drop for TempBenchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 无头跑一帧整个界面
fn frame(a: &mut App, ctx: &egui::Context) {
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0))),
        ..Default::default()
    };
    let mut out = ctx.run_ui(raw, |ui| a.ui_main(ui));
    out.textures_delta.clear();
    // 铺成三角网格也是每帧真实开销 (GPU 上传不算)
    let _ = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
}

/// 数学公式与回归模型常驻单元测试 (每次 cargo test 都自动校验)
#[test]
fn math_stats_and_ols_complexity_fitting_are_exact() {
    let st = BenchStats::from_samples(
        "synthetic",
        1,
        &[50.0, 10.0, 40.0, 20.0, 30.0],
        100.0,
        "ops/s",
        Some(35.0),
    );
    assert_eq!(st.n, 5);
    assert!((st.median_ms - 30.0).abs() < 1e-9);
    assert!((st.mean_ms - 30.0).abs() < 1e-9);
    assert!((st.std_dev_ms - 250.0f64.sqrt()).abs() < 1e-9);
    let expected_ci95 = t_crit_95(4) * 250.0f64.sqrt() / 5.0f64.sqrt();
    assert!((st.ci95_ms - expected_ci95).abs() < 1e-9);
    assert!((st.ci95_low_ms - (30.0 - expected_ci95)).abs() < 1e-9);
    assert!((st.ci95_high_ms - (30.0 + expected_ci95)).abs() < 1e-9);
    assert!((st.iqr_ms - 20.0).abs() < 1e-9);
    assert_eq!(st.outliers, 0);
    assert!((st.trimmed_mean_ms - 30.0).abs() < 1e-9);
    assert!((st.min_ms - 10.0).abs() < 1e-9);
    assert!((st.max_ms - 50.0).abs() < 1e-9);
    assert!(st.p90_ms.is_some());
    assert!(st.p95_ms.is_some());
    assert!(st.p99_ms.is_some());
    assert!(st.sla_pass);

    // 小样本 + 高方差: n < 5 时高分位点应返回 None, 且 CI 下界不得为负
    let small_hv =
        BenchStats::from_samples("small_high_var", 0, &[0.1, 0.1, 100.0], 1.0, "ops/s", None);
    assert_eq!(small_hv.n, 3);
    assert_eq!(small_hv.ci95_low_ms, 0.0);
    assert!(small_hv.ci95_high_ms > small_hv.mean_ms);
    assert!(small_hv.p90_ms.is_none());
    assert!(small_hv.p95_ms.is_none());
    assert!(small_hv.p99_ms.is_none());

    // Tukey 1.5*IQR 离群值识别与去极值均值
    let with_outlier = BenchStats::from_samples(
        "tukey_outlier",
        0,
        &[10.0, 10.2, 10.1, 9.9, 10.0, 100.0],
        1.0,
        "ops/s",
        None,
    );
    assert_eq!(with_outlier.outliers, 1);
    assert!((with_outlier.trimmed_mean_ms - 10.04).abs() < 1e-9);

    let o1 = ComplexityModel::fit_ols(
        "const",
        "O(1)",
        0.25,
        &[5_000, 20_000, 80_000, 200_000],
        &[2.5, 2.5, 2.5, 2.5],
    );
    assert!(o1.alpha.abs() < 1e-9, "O(1) alpha={}", o1.alpha);
    assert!((o1.r_squared - 1.0).abs() < 1e-9);
    assert!(o1.pass);

    let on = ComplexityModel::fit_ols(
        "linear",
        "O(N)",
        1.20,
        &[5_000, 20_000, 80_000, 200_000],
        &[5.0, 20.0, 80.0, 200.0],
    );
    assert!((on.alpha - 1.0).abs() < 1e-9, "O(N) alpha={}", on.alpha);
    assert!((on.r_squared - 1.0).abs() < 1e-9);
    assert!(on.pass);

    let on2 = ComplexityModel::fit_ols(
        "quad",
        "O(N)",
        1.20,
        &[100, 1_000, 10_000],
        &[0.01, 1.0, 100.0],
    );
    assert!((on2.alpha - 2.0).abs() < 1e-9, "O(N^2) alpha={}", on2.alpha);
    assert!(!on2.pass, "O(N^2) must fail O(N) gate");
}

#[test]
#[ignore]
fn perf_bench_open() {
    let tmp = TempBenchDir::new("open");
    for (label, mb, sla) in [("open_5mb_ms", 5usize, 150.0), ("open_30mb_ms", 30, 800.0)] {
        let p = tmp.write(&format!("{label}.txt"), &code_text(mb * 1024 * 1024));
        let st = measure_stats(label, 1, 5, mb as f64, "MB/s", Some(sla), || {
            let d = Doc::open(1, &p).expect("打开");
            assert!(d.lines.len() > 1000);
        });
        assert!(
            st.sla_pass,
            "{} exceeded SLA {:?}",
            st.name, st.sla_limit_ms
        );
    }
    let p = tmp.write("l200k.txt", &lines_text(200_000));
    let st_200k = measure_stats(
        "open_200k_lines_ms",
        1,
        5,
        200_000.0,
        "lines/s",
        Some(250.0),
        || assert_eq!(Doc::open(1, &p).unwrap().lines.len(), 200_001),
    );
    assert!(st_200k.sla_pass);

    let one = "word ".repeat(1024 * 1024 / 5);
    let p = tmp.write("oneline.txt", &one);
    let st_one = measure_stats(
        "open_1mb_single_line_ms",
        1,
        5,
        1.0,
        "MB/s",
        Some(50.0),
        || assert_eq!(Doc::open(1, &p).unwrap().lines.len(), 1),
    );
    assert!(st_one.sla_pass);
}

#[test]
#[ignore]
fn perf_bench_find_undo() {
    let tmp = TempBenchDir::new("find_undo");
    let p = tmp.write("find200k.rs", &lines_text(200_000));
    let d = Doc::open(1, &p).unwrap();
    let o = crate::find::FindOpts::default();
    let s1 = measure_stats(
        "find_all_many_hits_200k_ms",
        1,
        7,
        5001.0,
        "hits/s",
        Some(50.0),
        || assert!(d.find_hits("let y", o).unwrap().len() >= 5000),
    );
    assert!(s1.sla_pass);

    let s2 = measure_stats(
        "find_all_no_hit_200k_ms",
        1,
        7,
        200_000.0,
        "lines/s",
        Some(50.0),
        || assert!(d.find_hits("zzzq", o).unwrap().is_empty()),
    );
    assert!(s2.sla_pass);

    let re = crate::find::FindOpts { regex: true, ..o };
    let s3 = measure_stats(
        "find_all_regex_200k_ms",
        1,
        7,
        200_000.0,
        "lines/s",
        Some(80.0),
        || assert!(!d.find_hits(r"item_\d+7\(", re).unwrap().is_empty()),
    );
    assert!(s3.sla_pass);

    let one_mb = "word ".repeat(1024 * 1024 / 5);
    let p_1mb = tmp.write("find_1mb_single_line.txt", &one_mb);
    let d_1mb = Doc::open(11, &p_1mb).unwrap();
    let s_1mb = measure_stats(
        "find_all_1mb_single_line_ms",
        1,
        7,
        5001.0,
        "hits/s",
        Some(15.0),
        || assert!(d_1mb.find_hits("word", o).unwrap().len() >= 5000),
    );
    assert!(
        s_1mb.sla_pass,
        "find_all_1mb_single_line_ms exceeded 15ms SLA: {:.3}ms",
        s_1mb.median_ms
    );

    // 撤销: 在大文档中间做 10k 次独立编辑 (VecDeque 环形队列 O(1) 淘汰上限 5000)
    let mut edits_samples = Vec::with_capacity(5);
    let mut undo_samples = Vec::with_capacity(5);
    let mut undo_steps = 0usize;
    for iter in 0..6 {
        let mut d = Doc::open(2, &p).unwrap();
        d.goto(100_000, 0);
        let t = Instant::now();
        for i in 0..10_000 {
            d.goto(100_000 + i % 50, 3);
            d.type_text("a");
        }
        let edit_ms = t.elapsed().as_secs_f64() * 1000.0;

        let t = Instant::now();
        let mut steps = 0;
        while !d.undo_is_empty() {
            d.undo();
            steps += 1;
        }
        let undo_ms = t.elapsed().as_secs_f64() * 1000.0;
        if iter > 0 {
            edits_samples.push(edit_ms);
            undo_samples.push(undo_ms);
        }
        undo_steps = steps;
    }
    let s_edits = BenchStats::from_samples(
        "type_10k_edits_200k_doc_ms",
        1,
        &edits_samples,
        10_000.0,
        "edits/s",
        Some(200.0),
    );
    s_edits.emit();
    assert!(s_edits.sla_pass);

    let s_undo = BenchStats::from_samples(
        "undo_all_200k_doc_ms",
        1,
        &undo_samples,
        undo_steps as f64,
        "undos/s",
        Some(50.0),
    );
    s_undo.emit();
    println!("PERF undo_steps {:.3}", undo_steps as f64);

    let mut small_samples = Vec::with_capacity(5);
    for iter in 0..6 {
        let mut s = Doc::untitled(3, 1);
        s.lines = (0..1000).map(code_line).collect();
        let t = Instant::now();
        for i in 0..10_000 {
            s.goto(i % 900, 3);
            s.type_text("a");
        }
        while !s.undo_is_empty() {
            s.undo();
        }
        if iter > 0 {
            small_samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let st_small = BenchStats::from_samples(
        "type_undo_10k_small_doc_ms",
        1,
        &small_samples,
        10_000.0,
        "ops/s",
        Some(200.0),
    );
    st_small.emit();
    assert!(st_small.sla_pass);
}

#[test]
#[ignore]
fn perf_bench_frames() {
    let g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = TempBenchDir::new("frames");
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    let ctx = egui::Context::default();
    let s_empty = measure_stats("frame_empty_ms", 3, 25, 1.0, "frames/s", Some(4.0), || {
        frame(&mut a, &ctx)
    });
    assert!(s_empty.sla_pass);

    let p = tmp.write("scroll200k.rs", &lines_text(200_000));
    a.docs[0] = Doc::open(1, &p).unwrap();
    let s_steady = measure_stats(
        "frame_200k_steady_ms",
        3,
        25,
        1.0,
        "frames/s",
        Some(5.0),
        || frame(&mut a, &ctx),
    );
    assert!(s_steady.sla_pass);

    let mut row = 1000;
    let s_scroll = measure_stats(
        "frame_200k_scroll_ms",
        3,
        30,
        1.0,
        "frames/s",
        Some(8.0),
        || {
            row += 97;
            a.docs[0].goto(row, 0);
            frame(&mut a, &ctx);
        },
    );
    assert!(s_scroll.sla_pass);

    let p = tmp.write("one.txt", &"word ".repeat(1024 * 1024 / 5));
    a.docs[0] = Doc::open(2, &p).unwrap();
    let s_1mb_first = measure_stats(
        "frame_1mb_single_line_first_ms",
        0,
        1,
        1.0,
        "frames/s",
        Some(30.0),
        || frame(&mut a, &ctx),
    );
    assert!(s_1mb_first.sla_pass);

    let s_1mb_steady = measure_stats(
        "frame_1mb_single_line_steady_ms",
        1,
        10,
        1.0,
        "frames/s",
        Some(5.0),
        || frame(&mut a, &ctx),
    );
    assert!(s_1mb_steady.sla_pass);
    drop(g);
}

#[test]
#[ignore]
fn perf_bench_startup_and_ops() {
    let g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = TempBenchDir::new("startup_ops");

    // 1. 启动 App::build (0 磁盘写盘 + 快速主题/字体配置)
    let st_build = measure_stats(
        "startup_app_build_ms",
        1,
        5,
        1.0,
        "boots/s",
        Some(120.0),
        || {
            let app = App::build(vec![], None, Trace::new(), Settings::default(), false);
            std::hint::black_box(app.docs.len());
        },
    );
    assert!(st_build.sla_pass);

    // 2. 冷实例 App::build + 首帧完整渲染 + 三角化网格生成
    let st_first = measure_stats(
        "startup_first_frame_ms",
        0,
        5,
        1.0,
        "boots/s",
        Some(150.0),
        || {
            let mut app = App::build(vec![], None, Trace::new(), Settings::default(), false);
            let ctx = egui::Context::default();
            frame(&mut app, &ctx);
        },
    );
    assert!(st_first.sla_pass);

    // 3. 200k 行文档内 1,000 次连续单字符编辑 + 增量 changed() 缓存刷新
    let p = tmp.write("ops200k.rs", &lines_text(200_000));
    let mut doc = Doc::open(1, &p).unwrap();
    let _ = doc.changed();
    let st_edit = measure_stats(
        "op_single_char_edit_1k_200k_ms",
        2,
        15,
        1000.0,
        "edits/s",
        Some(25.0),
        || {
            for i in 0..1000usize {
                doc.goto(100_000 + (i % 64), 4);
                doc.type_text("x");
                std::hint::black_box(doc.changed().len());
            }
        },
    );
    assert!(st_edit.sla_pass);

    // 4. 200k 行文档内 1,000 次光标与选区导航
    let st_nav = measure_stats(
        "op_cursor_nav_1k_200k_ms",
        2,
        15,
        1000.0,
        "navs/s",
        Some(15.0),
        || {
            for i in 0..1000usize {
                let key = match i & 3 {
                    0 => "ArrowDown",
                    1 => "ArrowRight",
                    2 => "End",
                    _ => "Home",
                };
                doc.nav(key, false, false, 40);
            }
        },
    );
    assert!(st_nav.sla_pass);

    // 5. 1,000 行 Rust 语法词法高亮切片
    let reg = TypeRegistry::default_registry();
    let rust_cfg = reg.detect("main.rs").unwrap();
    let sample_lines: Vec<String> = (0..1000).map(code_line).collect();
    let st_hl = measure_stats(
        "op_highlight_1k_lines_ms",
        2,
        15,
        1000.0,
        "lines/s",
        Some(15.0),
        || {
            let mut toks = 0usize;
            for line in &sample_lines {
                toks += rust_cfg.highlight_line(line).len();
            }
            std::hint::black_box(toks);
        },
    );
    assert!(st_hl.sla_pass);

    // 6. 100,000 次 i18n trz O(1) 查询
    crate::i18n::set(crate::i18n::Lang::Zh);
    let st_i18n = measure_stats(
        "op_i18n_trz_100k_ms",
        2,
        15,
        100_000.0,
        "lookups/s",
        Some(10.0),
        || {
            let mut acc = 0usize;
            for _ in 0..100_000 {
                acc += std::hint::black_box(crate::i18n::trz("文件")).len();
            }
            std::hint::black_box(acc);
        },
    );
    assert!(st_i18n.sla_pass);
    drop(g);
}

#[test]
#[ignore]
fn perf_bench_mathematical_complexity() {
    let g = crate::i18n::TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let tmp = TempBenchDir::new("complexity");
    let scales = [5_000usize, 20_000, 80_000, 200_000];
    let files: Vec<(usize, PathBuf)> = scales
        .iter()
        .map(|&n| (n, tmp.write(&format!("scale_{n}.rs"), &lines_text(n))))
        .collect();

    // 1. 单行编辑 + 增量 changed() 复杂度应为 O(1) (与总行数 N 无关, alpha <= 0.25)
    let edit_names = [
        "scale_edit_500_changed_5k",
        "scale_edit_500_changed_20k",
        "scale_edit_500_changed_80k",
        "scale_edit_500_changed_200k",
    ];
    let mut edit_medians = Vec::with_capacity(scales.len());
    for (idx, (n, p)) in files.iter().enumerate() {
        let mut doc = Doc::open(1, p).unwrap();
        let _ = doc.changed();
        let st = measure_stats(edit_names[idx], 2, 12, 500.0, "edits/s", None, || {
            for k in 0..500usize {
                doc.goto((n / 2) + (k % 32), 4);
                doc.type_text("z");
                std::hint::black_box(doc.changed().len());
            }
        });
        edit_medians.push(st.median_ms);
    }
    let m_edit = ComplexityModel::fit_ols(
        "complexity_single_edit_and_changed_o1",
        "O(1)",
        0.25,
        &scales,
        &edit_medians,
    );
    m_edit.emit();
    assert!(
        m_edit.pass,
        "Edit + changed() scaling violated O(1): alpha={:.4} (limit {:.2})",
        m_edit.alpha, m_edit.max_alpha
    );

    // 2. 稳态单帧渲染复杂度应为 O(1) (视口虚拟化, alpha <= 0.25)
    // 先预热字体字形图集与 egui 纹理, 防止首尺度承担一次性初始化耗时
    let mut app = App::build(vec![], None, Trace::new(), Settings::default(), false);
    let ctx = egui::Context::default();
    app.docs[0] = Doc::open(1, &files[0].1).unwrap();
    let _ = app.docs[0].changed();
    for _ in 0..5 {
        frame(&mut app, &ctx);
    }
    let frame_names = [
        "scale_steady_10_frames_5k",
        "scale_steady_10_frames_20k",
        "scale_steady_10_frames_80k",
        "scale_steady_10_frames_200k",
    ];
    let mut frame_medians = Vec::with_capacity(scales.len());
    for (idx, (_, p)) in files.iter().enumerate() {
        app.docs[0] = Doc::open(1, p).unwrap();
        let _ = app.docs[0].changed();
        let st = measure_stats(frame_names[idx], 3, 12, 10.0, "frames/s", None, || {
            for _ in 0..10 {
                frame(&mut app, &ctx);
            }
        });
        frame_medians.push(st.median_ms);
    }
    let m_frame = ComplexityModel::fit_ols(
        "complexity_steady_frame_render_o1",
        "O(1)",
        0.25,
        &scales,
        &frame_medians,
    );
    m_frame.emit();
    assert!(
        m_frame.pass,
        "Steady frame rendering violated O(1): alpha={:.4} (limit {:.2})",
        m_frame.alpha, m_frame.max_alpha
    );

    // 3. 全文无命中扫描查找复杂度应 <= O(N) (alpha <= 1.25)
    let o = crate::find::FindOpts::default();
    let find_names = [
        "scale_find_no_hit_5k",
        "scale_find_no_hit_20k",
        "scale_find_no_hit_80k",
        "scale_find_no_hit_200k",
    ];
    let mut find_medians = Vec::with_capacity(scales.len());
    for (idx, (_, p)) in files.iter().enumerate() {
        let doc = Doc::open(1, p).unwrap();
        let st = measure_stats(find_names[idx], 2, 10, 1.0, "scans/s", None, || {
            assert!(doc.find_hits("zzzq_nomatch", o).unwrap().is_empty());
        });
        find_medians.push(st.median_ms);
    }
    let m_find = ComplexityModel::fit_ols(
        "complexity_find_all_on",
        "O(N)",
        1.25,
        &scales,
        &find_medians,
    );
    m_find.emit();
    assert!(
        m_find.pass,
        "Find scan violated O(N): alpha={:.4} (limit {:.2})",
        m_find.alpha, m_find.max_alpha
    );

    // 4. 文件打开解析复杂度应 <= O(N) (alpha <= 1.25)
    let open_names = [
        "scale_doc_open_5k",
        "scale_doc_open_20k",
        "scale_doc_open_80k",
        "scale_doc_open_200k",
    ];
    let mut open_medians = Vec::with_capacity(scales.len());
    for (idx, (&n, p)) in scales.iter().zip(files.iter().map(|(_, p)| p)).enumerate() {
        let st = measure_stats(open_names[idx], 1, 7, n as f64, "lines/s", None, || {
            let d = Doc::open(1, p).unwrap();
            assert_eq!(d.lines.len(), n + 1);
        });
        open_medians.push(st.median_ms);
    }
    let m_open = ComplexityModel::fit_ols(
        "complexity_doc_open_on",
        "O(N)",
        1.25,
        &scales,
        &open_medians,
    );
    m_open.emit();
    assert!(
        m_open.pass,
        "Doc::open violated O(N): alpha={:.4} (limit {:.2})",
        m_open.alpha, m_open.max_alpha
    );

    drop(g);
}
