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
