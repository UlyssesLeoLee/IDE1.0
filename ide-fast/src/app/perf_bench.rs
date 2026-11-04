//! 性能回归基准 (只在测试构建里, 默认 `#[ignore]`): 用无头的 egui 上下文量文件打开 / 滚动一帧 / 全部查找 / 撤销。
//!
//! 运行 (由 `tests/perf.ps1` 调用, 也可手动):
//!   cargo test -p ide-fast --release perf_bench -- --ignored --nocapture --test-threads=1
//! 每项输出一行 `PERF <名字> <毫秒>` (多次取中位数); 数字随机器而变, 只用来比较「改前 / 改后」。

use super::*;
use crate::doc::Doc;
use std::time::Instant;

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

fn median_ms(n: usize, mut f: impl FnMut()) -> f64 {
    let mut v: Vec<f64> = (0..n)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn report(name: &str, ms: f64) {
    println!("PERF {name} {ms:.3}");
}

fn tmp(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ide-fast-perf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
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

#[test]
#[ignore]
fn perf_bench_open() {
    for (label, mb) in [("open_5mb_ms", 5usize), ("open_30mb_ms", 30)] {
        let p = tmp(&format!("{label}.txt"), &code_text(mb * 1024 * 1024));
        let ms = median_ms(3, || {
            let d = Doc::open(1, &p).expect("打开");
            assert!(d.lines.len() > 1000);
        });
        report(label, ms);
    }
    let p = tmp("l200k.txt", &(0..200_000).map(|i| code_line(i) + "\n").collect::<String>());
    report("open_200k_lines_ms", median_ms(3, || assert_eq!(Doc::open(1, &p).unwrap().lines.len(), 200_001)));
    let one = "word ".repeat(1024 * 1024 / 5);
    let p = tmp("oneline.txt", &one);
    report("open_1mb_single_line_ms", median_ms(3, || assert_eq!(Doc::open(1, &p).unwrap().lines.len(), 1)));
}

#[test]
#[ignore]
fn perf_bench_find_undo() {
    let p = tmp("find200k.rs", &(0..200_000).map(|i| code_line(i) + "\n").collect::<String>());
    let d = Doc::open(1, &p).unwrap();
    let o = crate::find::FindOpts::default();
    report("find_all_many_hits_200k_ms", median_ms(5, || assert!(d.find_hits("let y", o).unwrap().len() >= 5000)));
    report("find_all_no_hit_200k_ms", median_ms(5, || assert!(d.find_hits("zzzq", o).unwrap().is_empty())));
    let re = crate::find::FindOpts { regex: true, ..o };
    report("find_all_regex_200k_ms", median_ms(5, || assert!(!d.find_hits(r"item_\d+7\(", re).unwrap().is_empty())));

    // 撤销: 在大文档中间做 10k 次独立编辑 (撤销栈上限 5000, 实际可撤销步数见 PERF_undo_steps)
    let mut d = Doc::open(2, &p).unwrap();
    d.goto(100_000, 0);
    let t = Instant::now();
    for i in 0..10_000 {
        d.goto(100_000 + i % 50, 3);
        d.type_text("a");
    }
    report("type_10k_edits_200k_doc_ms", t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let mut steps = 0;
    while !d.undo_is_empty() {
        d.undo();
        steps += 1;
    }
    report("undo_all_200k_doc_ms", t.elapsed().as_secs_f64() * 1000.0);
    report("undo_steps", steps as f64);
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
    report("type_undo_10k_small_doc_ms", t.elapsed().as_secs_f64() * 1000.0);
}

#[test]
#[ignore]
fn perf_bench_frames() {
    let g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut a = App::build(vec![], None, Trace::new(), Settings::default(), false);
    let ctx = egui::Context::default();
    // 空文档的一帧 (界面本身的开销)
    for _ in 0..3 {
        frame(&mut a, &ctx);
    }
    report("frame_empty_ms", median_ms(20, || frame(&mut a, &ctx)));

    let p = tmp("scroll200k.rs", &(0..200_000).map(|i| code_line(i) + "\n").collect::<String>());
    a.docs[0] = Doc::open(1, &p).unwrap();
    for _ in 0..3 {
        frame(&mut a, &ctx);
    }
    report("frame_200k_steady_ms", median_ms(20, || frame(&mut a, &ctx)));
    // 滚动: 光标每帧跳到新位置 (reveal 把视口带过去), 量带排版的帧
    let mut row = 1000;
    report("frame_200k_scroll_ms", median_ms(30, || {
        row += 97;
        a.docs[0].goto(row, 0);
        frame(&mut a, &ctx);
    }));

    // 1 MB 单行: 一帧要排的是一行 100 万字符
    let p = tmp("one.txt", &"word ".repeat(1024 * 1024 / 5));
    a.docs[0] = Doc::open(2, &p).unwrap();
    report("frame_1mb_single_line_first_ms", median_ms(1, || frame(&mut a, &ctx)));
    report("frame_1mb_single_line_steady_ms", median_ms(5, || frame(&mut a, &ctx)));
    drop(g);
}
