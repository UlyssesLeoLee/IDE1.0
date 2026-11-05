use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use editcore::MmapBuffer;

/// 辅助生成临时测试文件路径并在测试结束时自动清理
struct TempFileGuard {
    path: PathBuf,
}

impl TempFileGuard {
    fn new(name: &str) -> Self {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "mmap_test_{}_{}_{}.tmp",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(unique);
        Self { path }
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn test_100k_lines_large_file_and_random_access() {
    let guard = TempFileGuard::new("100k_lines");
    const TOTAL_LINES: usize = 100_000;

    // 1. 生成 100,000 行的大临时文件
    {
        let file = File::create(&guard.path).expect("failed to create temp file");
        let mut writer = BufWriter::with_capacity(128 * 1024, file);
        for i in 0..TOTAL_LINES {
            writeln!(
                writer,
                "Line {:06}: Payload data for large file mmap verification with token #{}",
                i,
                i * 7
            )
            .expect("write line failed");
        }
        writer.flush().expect("flush failed");
    }

    // 2. 打开 MmapBuffer 并验证基础元数据
    let buffer = MmapBuffer::open(&guard.path).expect("MmapBuffer::open failed");
    assert!(!buffer.is_empty());
    assert_eq!(buffer.line_count(), TOTAL_LINES, "line_count mismatch");
    assert!(buffer.file_size() > 5 * 1024 * 1024, "file size should be > 5MB");

    // 3. 测试首尾与边界行
    let line_0 = buffer.get_line(0).expect("line 0 should exist");
    assert_eq!(
        line_0,
        "Line 000000: Payload data for large file mmap verification with token #0"
    );

    let line_last = buffer.get_line(TOTAL_LINES - 1).expect("last line should exist");
    assert_eq!(
        line_last,
        format!(
            "Line {:06}: Payload data for large file mmap verification with token #{}",
            TOTAL_LINES - 1,
            (TOTAL_LINES - 1) * 7
        )
    );

    assert!(buffer.get_line(TOTAL_LINES).is_none(), "out of bound line must be None");
    assert!(buffer.get_line(TOTAL_LINES + 100).is_none());

    // 4. 测试检查点邻近行 (SPARSE_INTERVAL = 256)
    for target in [255, 256, 257, 511, 512, 513, 1023, 1024, 1025] {
        let line = buffer.get_line(target).expect("checkpoint neighbor line should exist");
        assert_eq!(
            line,
            format!(
                "Line {:06}: Payload data for large file mmap verification with token #{}",
                target,
                target * 7
            )
        );
    }

    // 5. 测试随机跳行读取
    let random_indices = [42, 1337, 9999, 45678, 77777, 88888, 99998];
    for &idx in &random_indices {
        let line = buffer.get_line(idx).unwrap_or_else(|| panic!("line {} should exist", idx));
        assert_eq!(
            line,
            format!(
                "Line {:06}: Payload data for large file mmap verification with token #{}",
                idx,
                idx * 7
            )
        );
    }

    // 6. 测试批量窗口切片行读取 (例如 UI 视口加载 50 行)
    let slice_50 = buffer.get_lines_range(12345, 50);
    assert_eq!(slice_50.len(), 50);
    for (offset, line) in slice_50.iter().enumerate() {
        let actual_idx = 12345 + offset;
        assert_eq!(
            line,
            &format!(
                "Line {:06}: Payload data for large file mmap verification with token #{}",
                actual_idx,
                actual_idx * 7
            )
        );
    }

    // 测试窗口超出文件末尾时截断返回
    let slice_tail = buffer.get_lines_range(TOTAL_LINES - 20, 50);
    assert_eq!(slice_tail.len(), 20);

    // 测试越界起始行返回空
    let slice_empty = buffer.get_lines_range(TOTAL_LINES + 10, 20);
    assert!(slice_empty.is_empty());

    // 7. 测试基于 SIMD 的快速关键字搜索
    let target_needle = b"Line 077777";
    let found_offset = buffer.find_next(target_needle, 0).expect("keyword should be found");

    let read_back = buffer
        .read_slice(found_offset, target_needle.len())
        .expect("read_slice at found offset failed");
    assert_eq!(read_back, target_needle);

    // 从找到位置后一位继续搜索，应该搜不到相同的特定编号
    assert!(buffer.find_next(target_needle, found_offset + 1).is_none());

    // 搜索通用前缀并推进
    let mut next_offset = 0;
    let mut match_count = 0;
    while let Some(pos) = buffer.find_next(b"Line 0000", next_offset) {
        match_count += 1;
        next_offset = pos + 1;
        if match_count >= 10 {
            break;
        }
    }
    assert_eq!(match_count, 10);
}

#[test]
fn test_boundary_empty_file() {
    let guard = TempFileGuard::new("empty");
    File::create(&guard.path).expect("create empty failed");

    let buffer = MmapBuffer::open(&guard.path).expect("open empty failed");
    assert!(buffer.is_empty());
    assert_eq!(buffer.line_count(), 0);
    assert_eq!(buffer.file_size(), 0);
    assert!(buffer.get_line(0).is_none());
    assert!(buffer.get_lines_range(0, 10).is_empty());
    assert_eq!(buffer.read_slice(0, 0).unwrap(), Vec::<u8>::new());
    assert!(buffer.read_slice(0, 1).is_err());
    assert_eq!(buffer.find_next(b"", 0), Some(0));
    assert_eq!(buffer.find_next(b"any", 0), None);
}

#[test]
fn test_boundary_single_line_no_newline() {
    let guard = TempFileGuard::new("single_no_nl");
    {
        let mut f = File::create(&guard.path).unwrap();
        f.write_all(b"Hello world without trailing newline").unwrap();
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    assert!(!buffer.is_empty());
    assert_eq!(buffer.line_count(), 1);
    assert_eq!(
        buffer.get_line(0),
        Some("Hello world without trailing newline".to_string())
    );
    assert_eq!(buffer.get_line(1), None);
    assert_eq!(
        buffer.get_lines_range(0, 10),
        vec!["Hello world without trailing newline".to_string()]
    );
}

#[test]
fn test_boundary_single_line_with_newline() {
    let guard = TempFileGuard::new("single_with_nl");
    {
        let mut f = File::create(&guard.path).unwrap();
        f.write_all(b"Hello world with trailing newline\n").unwrap();
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    assert_eq!(buffer.line_count(), 1);
    assert_eq!(
        buffer.get_line(0),
        Some("Hello world with trailing newline".to_string())
    );
    assert_eq!(buffer.get_line(1), None);
}

#[test]
fn test_boundary_crlf_format() {
    let guard = TempFileGuard::new("crlf");
    {
        let mut f = File::create(&guard.path).unwrap();
        f.write_all(b"First line\r\nSecond line\r\nThird line").unwrap();
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.get_line(0), Some("First line".to_string()));
    assert_eq!(buffer.get_line(1), Some("Second line".to_string()));
    assert_eq!(buffer.get_line(2), Some("Third line".to_string()));
    assert_eq!(buffer.get_line(3), None);

    let range = buffer.get_lines_range(0, 3);
    assert_eq!(range, vec!["First line", "Second line", "Third line"]);
}

#[test]
fn test_boundary_multiple_empty_lines() {
    let guard = TempFileGuard::new("empty_lines");
    {
        let mut f = File::create(&guard.path).unwrap();
        f.write_all(b"\n\n\n").unwrap();
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.get_line(0), Some("".to_string()));
    assert_eq!(buffer.get_line(1), Some("".to_string()));
    assert_eq!(buffer.get_line(2), Some("".to_string()));
    assert_eq!(buffer.get_line(3), None);
}

#[test]
fn test_ultra_long_line_and_cross_chunk() {
    let guard = TempFileGuard::new("long_line");
    // 生成大于 64KB (例如 200KB) 的单行，随后跟普通短行
    const LONG_LEN: usize = 200 * 1024;
    {
        let mut f = File::create(&guard.path).unwrap();
        let long_bytes = vec![b'X'; LONG_LEN];
        f.write_all(&long_bytes).unwrap();
        f.write_all(b"\nSecond line\n").unwrap();
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    assert_eq!(buffer.line_count(), 2);

    let l0 = buffer.get_line(0).expect("line 0 should exist");
    assert_eq!(l0.len(), LONG_LEN);
    assert!(l0.chars().all(|c| c == 'X'));

    let l1 = buffer.get_line(1).expect("line 1 should exist");
    assert_eq!(l1, "Second line");

    // 测试跨 64KB 边界的切片读取
    let slice = buffer.read_slice(65530, 20).unwrap();
    assert_eq!(slice.len(), 20);
    assert_eq!(slice, vec![b'X'; 20]);

    // 检索第二行
    let pos = buffer.find_next(b"Second line", 0).expect("keyword must be found");
    assert_eq!(pos, LONG_LEN as u64 + 1);
}

#[test]
fn test_cross_chunk_boundary_search() {
    let guard = TempFileGuard::new("cross_boundary_search");
    // 构造一个文件，让关键字恰好跨越第 64KB 边界 (65536)
    // 65530 .. 65545 写入 "SPLIT_ACROSS_CHUNK"
    let target_needle = b"SPLIT_ACROSS_CHUNK";
    let start_pos = 65536 - 6; // 6 字节在第一块，其余在第二块
    let total_len = 128 * 1024;

    {
        let mut data = vec![b'.'; total_len];
        data[start_pos..start_pos + target_needle.len()].copy_from_slice(target_needle);
        let mut f = File::create(&guard.path).unwrap();
        f.write_all(&data).unwrap();
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    let found = buffer
        .find_next(target_needle, 0)
        .expect("must find keyword across 64KB chunk boundary");
    assert_eq!(found, start_pos as u64);

    let read = buffer.read_slice(found, target_needle.len()).unwrap();
    assert_eq!(read, target_needle);
}


#[test]
fn test_auto_pagination_and_page_switching() {
    let guard = TempFileGuard::new("auto_pagination");
    let total_lines = 25_000;
    {
        let mut f = File::create(&guard.path).unwrap();
        for i in 0..total_lines {
            writeln!(f, "Record line #{}", i).unwrap();
        }
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    assert_eq!(buffer.line_count(), total_lines);

    let page_size = 10_000;
    assert_eq!(buffer.page_count(page_size), 3);

    // 第 0 页
    let (p0_lines, info0) = buffer.get_page(0, page_size);
    assert_eq!(p0_lines.len(), 10_000);
    assert_eq!(info0.page_index, 0);
    assert_eq!(info0.total_pages, 3);
    assert_eq!(info0.start_line, 0);
    assert_eq!(info0.end_line, 10_000);
    assert_eq!(p0_lines[0], "Record line #0");
    assert_eq!(p0_lines[9999], "Record line #9999");

    // 第 1 页
    let (p1_lines, info1) = buffer.get_page(1, page_size);
    assert_eq!(p1_lines.len(), 10_000);
    assert_eq!(info1.page_index, 1);
    assert_eq!(info1.start_line, 10_000);
    assert_eq!(info1.end_line, 20_000);
    assert_eq!(p1_lines[0], "Record line #10000");

    // 第 2 页 (尾页，仅 5000 行)
    let (p2_lines, info2) = buffer.get_page(2, page_size);
    assert_eq!(p2_lines.len(), 5_000);
    assert_eq!(info2.page_index, 2);
    assert_eq!(info2.start_line, 20_000);
    assert_eq!(info2.end_line, 25_000);
    assert_eq!(p2_lines[4999], "Record line #24999");

    // 分页索引换算
    assert_eq!(buffer.page_for_line(0, page_size), 0);
    assert_eq!(buffer.page_for_line(9999, page_size), 0);
    assert_eq!(buffer.page_for_line(10000, page_size), 1);
    assert_eq!(buffer.page_for_line(24999, page_size), 2);
}

#[test]
fn test_offset_to_line_and_line_to_offset() {
    let guard = TempFileGuard::new("offset_line_map");
    {
        let mut f = File::create(&guard.path).unwrap();
        for i in 0..1000 {
            writeln!(f, "Line number {:05}", i).unwrap();
        }
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    for target_line in [0, 1, 10, 255, 256, 500, 789, 999] {
        let offset = buffer.line_to_offset(target_line).expect("offset must exist");
        let mapped_line = buffer.offset_to_line(offset);
        assert_eq!(mapped_line, target_line, "line {} mapped back to {}", target_line, mapped_line);
        let line_text = buffer.get_line(target_line).unwrap();
        assert_eq!(line_text, format!("Line number {:05}", target_line));
    }
}

#[test]
fn test_cross_page_search_matches() {
    let guard = TempFileGuard::new("cross_page_search");
    let total_lines = 30_000;
    {
        let mut f = File::create(&guard.path).unwrap();
        for i in 0..total_lines {
            if i == 500 {
                writeln!(f, "prefix text TARGET_ALERT on page 0").unwrap();
            } else if i == 15_230 {
                writeln!(f, "middle TARGET_ALERT on page 1").unwrap();
            } else if i == 28_999 {
                writeln!(f, "tail TARGET_ALERT on page 2").unwrap();
            } else {
                writeln!(f, "ordinary background log data {:06}", i).unwrap();
            }
        }
    }

    let buffer = MmapBuffer::open(&guard.path).unwrap();
    let page_size = 10_000;
    let matches = buffer.search_matches("TARGET_ALERT", page_size, true, 10);
    assert_eq!(matches.len(), 3, "must find all 3 matches across 3 different pages");

    // 命中 1: 第 0 页, 行 500
    assert_eq!(matches[0].page_index, 0);
    assert_eq!(matches[0].global_line, 500);
    assert_eq!(matches[0].line_in_page, 500);
    assert_eq!(matches[0].col, 12);
    assert!(matches[0].text.contains("TARGET_ALERT"));

    // 命中 2: 第 1 页, 行 15230 (页内行 5230)
    assert_eq!(matches[1].page_index, 1);
    assert_eq!(matches[1].global_line, 15_230);
    assert_eq!(matches[1].line_in_page, 5_230);
    assert_eq!(matches[1].col, 7);
    assert!(matches[1].text.contains("TARGET_ALERT"));

    // 命中 3: 第 2 页, 行 28999 (页内行 8999)
    assert_eq!(matches[2].page_index, 2);
    assert_eq!(matches[2].global_line, 28_999);
    assert_eq!(matches[2].line_in_page, 8_999);
    assert_eq!(matches[2].col, 5);
    assert!(matches[2].text.contains("TARGET_ALERT"));
}
