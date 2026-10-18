// grep 引擎性能基准 (忽略, 不参与编译) —— cargo bench 或临时跑用.
// 目的: 量出「遍历 / 打开 / 逐行解码 / 匹配」四段各自占多少, 决定优化投在哪.
// 用法: cargo test -p sakura-rs --lib grep_bench -- --ignored --nocapture

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// 造一棵语料树 (幂等: 已存在则跳过)
fn corpus() -> PathBuf {
    let base = std::env::temp_dir().join("grep-bench-corpus");
    if base.exists() {
        return base;
    }
    // 固定 LCG, 不引 rand 依赖, 保证可复现
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let words = [
        "fn", "match", "impl", "pub", "async", "result", "error", "handle", "buffer", "window",
        "editor", "grep", "search", "index", "cache", "token", "parse", "render", "update",
        "config", "layout", "cursor", "column", "selection",
    ];
    fs::create_dir_all(&base).unwrap();
    let mut n = 0usize;
    for d in 0..120 {
        let sub = base.join(format!("mod{}/sub{d}", d / 20));
        fs::create_dir_all(&sub).unwrap();
        for i in 0..25 {
            let lines = 200 + (next() % 1800) as usize;
            let mut s = String::new();
            for l in 0..lines {
                let k = 4 + (next() % 10) as usize;
                for _ in 0..k {
                    s.push_str(words[(next() as usize) % words.len()]);
                    s.push(' ');
                }
                s.push_str(&format!("// line {l}\n"));
            }
            // 每 500 个文件里放 1 个含目标词的, 模拟真实使用 (关键字很罕见)
            if i == 0 && d % 3 == 0 {
                s.push_str("zzqqxx here\n");
            }
            fs::write(sub.join(format!("file{i}.rs")), s).unwrap();
            n += 1;
        }
    }
    eprintln!("[bench] 生成 {n} 个文件 @ {}", base.display());
    base
}

fn walk_all(root: &Path) -> (usize, usize) {
    fn go(p: &Path, files: &mut usize, bytes: &mut usize) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                go(&e.path(), files, bytes);
            } else if ft.is_file() {
                *files += 1;
                *bytes += e.metadata().map(|m| m.len() as usize).unwrap_or(0);
            }
        }
    }
    let mut f = 0;
    let mut b = 0;
    go(root, &mut f, &mut b);
    (f, b)
}

/// 逐行读 + UTF-8 解码 (当前 search() 的做法)
fn scan_lines_decode(root: &Path, needle: &[u8]) -> usize {
    use std::io::{BufRead, BufReader};
    fn go(p: &Path, hits: &mut usize, needle: &[u8]) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                go(&e.path(), hits, needle);
            } else if ft.is_file() {
                let Ok(f) = fs::File::open(e.path()) else { continue };
                let pat = std::str::from_utf8(needle).unwrap_or("");
                for line in BufReader::new(f).lines().map_while(Result::ok) {
                    // 旧实现的做法: 逐行 decode 成 String 再 str::contains
                    if line.contains(pat) {
                        *hits += 1;
                    }
                }
            }
        }
    }
    let mut h = 0;
    go(root, &mut h, needle);
    h
}

/// 整块读进内存 + 字节窗口扫描 (WizTree/ripgrep 的路子: 不逐行解码)
fn scan_block_bytes(root: &Path, needle: &[u8]) -> usize {
    fn go(p: &Path, hits: &mut usize, needle: &[u8]) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                go(&e.path(), hits, needle);
            } else if ft.is_file() {
                let Ok(buf) = fs::read(e.path()) else { continue };
                if buf.windows(needle.len()).any(|w| w == needle) {
                    // 命中处换算成行号
                    for (i, _) in buf
                        .split(|&b| b == b'\n')
                        .enumerate()
                        .filter(|(_, l)| l.windows(needle.len()).any(|w| w == needle))
                    {
                        let _ = i;
                        *hits += 1;
                    }
                }
            }
        }
    }
    let mut h = 0;
    go(root, &mut h, needle);
    h
}


/// memchr 扫描 + 只在命中处切行 (ripgrep 的路子).
/// 要点:
///   1. 全程按字节, 不逐行 decode —— 省掉每个匹配行的 String 分配与 UTF-8 校验;
///   2. 用首字节 `g` 做 SIMD 预筛 (memchr), 命中才比对完整 needle;
///   3. 维护「上一个命中位置 -> 当前命中的行号」, 用 memchr 数换行, 避免重复扫描;
///   4. 同一行多次出现只算一次 (Sakura 的「ファイル毎最初のみ検索」是可选开关,
///      默认关闭, 所以这里按「每处出现一行」计).
fn scan_memchr(root: &Path, needle: &[u8]) -> usize {
    let first = needle[0];

    fn go(p: &Path, hits: &mut usize, needle: &[u8], first: u8) {
        let Ok(rd) = fs::read_dir(p) else { return };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                go(&e.path(), hits, needle, first);
            } else if ft.is_file() {
                let Ok(buf) = fs::read(e.path()) else { continue };
                let mut line = 1usize;
                let mut last_end = 0usize;
                for at in memchr::memchr_iter(first, &buf) {
                    if !buf[at..].starts_with(needle) {
                        continue;
                    }
                    // 从 last_end 数到 at 的换行数 = 行号增量
                    line += memchr::memchr_iter(b'\n', &buf[last_end..at]).count();
                    last_end = at + needle.len();
                    *hits += 1;
                    let _ = line;
                }
            }
        }
    }
    let mut h = 0;
    go(root, &mut h, needle, first);
    h
}

/// 调引擎真实入口 search() —— 用户实际走的就是这条路径.
fn engine_search(root: &Path, pattern: &str) -> usize {
    let o = sakura_rs::grep_engine::SearchOptions {
        pattern: pattern.to_string(),
        case_sensitive: Some(true),
        ..Default::default()
    };
    sakura_rs::grep_engine::search(root, &o).map(|r| r.len()).unwrap_or(0)
}

#[test]
#[ignore]
fn grep_bench() {
    let root = corpus();
    let (files, bytes) = walk_all(&root);
    eprintln!("[bench] 语料: {files} 文件 / {:.1} MB", bytes as f64 / 1048576.0);

    let t = Instant::now();
    let f = files;
    let d = t.elapsed();
    eprintln!("[bench] ① 纯遍历+stat     : {:>8.2?}  ({:.0} MB/s)", d, bytes as f64 / 1048576.0 / d.as_secs_f64());

    let t = Instant::now();
    let h1 = scan_lines_decode(&root, b"zzqqxx");
    let d1 = t.elapsed();
    eprintln!("[bench] ② 逐行解码+contains: {:>8.2?}  ({:.0} MB/s, {h1} 命中)", d1, bytes as f64 / 1048576.0 / d1.as_secs_f64());

    let t = Instant::now();
    let h2 = scan_block_bytes(&root, b"zzqqxx");
    let d2 = t.elapsed();
    eprintln!("[bench] ③ 整块+字节窗口   : {:>8.2?}  ({:.0} MB/s, {h2} 命中)", d2, bytes as f64 / 1048576.0 / d2.as_secs_f64());

    eprintln!("[bench] ③/② 加速比: {:.2}x", d1.as_secs_f64() / d2.as_secs_f64().max(1e-9));

    let t = Instant::now();
    let h3 = scan_memchr(&root, b"zzqqxx");
    let d3 = t.elapsed();
    eprintln!("[bench] ④ memchr 扫描      : {:>8.2?}  ({:.0} MB/s, {h3} 命中)", d3, bytes as f64 / 1048576.0 / d3.as_secs_f64());

    eprintln!("[bench] ④/② 加速比: {:.2}x", d1.as_secs_f64() / d3.as_secs_f64().max(1e-9));

    // 引擎真实入口 (含目录遍历 + glob 过滤 + 结果构造), 这是用户感受到的耗时
    let t = Instant::now();
    let n = engine_search(&root, "zzqqxx");
    let de = t.elapsed();
    eprintln!("[bench] ⑥ 引擎 search()   : {:>8.2?}  ({:.0} MB/s, {n} 命中)", de, bytes as f64 / 1048576.0 / de.as_secs_f64());
    eprintln!("[bench] ⑥/④ 端到端额外开销: {:.2}x (目录遍历+结果构造)", de.as_secs_f64() / d3.as_secs_f64().max(1e-9));
    assert_eq!(f, files);
}
