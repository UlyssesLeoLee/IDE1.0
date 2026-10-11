//! sakura-rs bench — buffer insert throughput
//!
//! 运行: cargo run -p sakura-rs --example bench

use sakura_rs::buffer::{DocLineMgr, DocLine};
use std::time::Instant;

fn main() {
    println!("=== sakura-rs core ops bench ===\n");

    let mut buf = DocLineMgr::new();
    let mut line = DocLine::new("warmup line", 1);
    line.insert_str(11, " tail");
    println!("warmup: line text = {:?}, char_len = {}", line.text, line.char_len());

    let n = 10_000;
    let s = "the quick brown fox jumps over the lazy dog";

    println!("\ninsert_str {n} ops on same line");
    let t = Instant::now();
    for i in 0..n {
        line.insert_str(i % (line.char_len() as usize + 1) as i32, s);
        if line.char_len() > 5000 {
            line.remove_range_chars(0, 4000);
        }
    }
    let d = t.elapsed();
    println!(
        "  total = {:?}, avg = {:?}/op, throughput = {:.0} ops/s, line_char_len = {}",
        d, d / n as u32, n as f64 / d.as_secs_f64(), line.char_len()
    );

    println!("\n=== done ===");
}
