//! editcore — 超大文件 (GB 级) 内存映射与惰性分段加载引擎.
//!
//! ## 设计与架构
//!
//! 在编辑或检索上百兆乃至数 GB 级的超大日志/数据文件时，直接将全文或全量行索引读入内存
//! 会导致严重的内存峰值（甚至 OOM）。
//!
//! 本模块通过以下两级惰性机制解决超大文件处理问题：
//! 1. **虚拟内存分块按需缓存 (Chunk Cache)**:
//!    - 将底层物理文件切分为 64KB 大小的虚拟块 (Chunk)。
//!    - 维护容量上限为 64 块 (约 4MB) 的 LRU 块级缓存。
//!    - 任意字节切片 (`read_slice`) 与跨块读取均自动映射到对应块，只加载活跃页面。
//! 2. **稀疏行索引 (Sparse Line Index)**:
//!    - 仅对每 256 行记录一次绝对字节偏移检查点 (Checkpoint)。
//!    - 针对 1000 万行的超大文件，仅需维护约 39,000 个 `u64` 整数（占用内存仅 ~312KB）。
//!    - 行定位时，通过 O(1) 检查点定位基准行，随后借助 SIMD (`memchr`) 在 64KB 块内扫描
//!      剩余至多 255 个换行符，耗时通常在微秒级。
//! 3. **高性能关键字搜索 (`find_next`)**:
//!    - 基于 `memchr::memmem::Finder` SIMD 加速的流式滑窗匹配。
//!    - 滑窗自动保留 `query.len() - 1` 的边界重叠 (Overlap)，保证跨 64KB 块边界的关键字无遗漏。
//! 4. **100% 纯 Safe Rust**:
//!    - 严格遵循 `#![forbid(unsafe_code)]`，跨平台且无系统 mmap 的虚假映射崩溃隐患。

use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// 虚拟块大小: 64 KB
const CHUNK_SIZE: u64 = 64 * 1024;

/// 块缓存上限: 64 个 64KB 块 (共计 4 MB 常驻内存)
const MAX_CACHE_CHUNKS: usize = 64;

/// 稀疏行索引步长: 每 256 行记录一个字节偏移检查点
const SPARSE_INTERVAL: usize = 256;

/// 块级 LRU 缓存容器
struct ChunkCache {
    /// (chunk_idx, chunk_bytes), 越靠后表示最近越常访问 (MRU)
    entries: Vec<(u64, Vec<u8>)>,
}

impl ChunkCache {
    fn new() -> Self {
        Self {
            entries: Vec::with_capacity(MAX_CACHE_CHUNKS),
        }
    }
}

/// 内部可变状态 (受 Mutex 保护，对外暴露纯只读并发接口 &self)
struct InnerState {
    file: File,
    cache: ChunkCache,
}

impl InnerState {
    /// 获取或加载指定的 64KB 虚拟块
    fn ensure_chunk(&mut self, chunk_idx: u64, file_size: u64) -> io::Result<&[u8]> {
        if let Some(pos) = self.cache.entries.iter().position(|(idx, _)| *idx == chunk_idx) {
            let entry = self.cache.entries.remove(pos);
            self.cache.entries.push(entry);
            return Ok(&self.cache.entries.last().unwrap().1);
        }

        let chunk_start = chunk_idx * CHUNK_SIZE;
        let chunk_len = ((file_size.saturating_sub(chunk_start)) as usize).min(CHUNK_SIZE as usize);
        let mut buf = vec![0u8; chunk_len];

        self.file.seek(SeekFrom::Start(chunk_start))?;
        self.file.read_exact(&mut buf)?;

        if self.cache.entries.len() >= MAX_CACHE_CHUNKS {
            self.cache.entries.remove(0); // 逐出最久未使用的块
        }
        self.cache.entries.push((chunk_idx, buf));
        Ok(&self.cache.entries.last().unwrap().1)
    }

    /// 读取连续字节切片，自动处理跨 64KB 块拼接
    fn read_bytes(&mut self, offset: u64, len: usize, file_size: u64) -> io::Result<Vec<u8>> {
        if len == 0 {
            if offset <= file_size {
                return Ok(Vec::new());
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("read_bytes: offset {} past file_size {}", offset, file_size),
                ));
            }
        }

        if offset.checked_add(len as u64).map_or(true, |end| end > file_size) {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "read_slice past EOF: offset={}, len={}, file_size={}",
                    offset, len, file_size
                ),
            ));
        }

        let mut result = Vec::with_capacity(len);
        let mut cur_offset = offset;
        let end_offset = offset + len as u64;

        while cur_offset < end_offset {
            let chunk_idx = cur_offset / CHUNK_SIZE;
            let in_chunk_offset = (cur_offset % CHUNK_SIZE) as usize;
            let chunk_data = self.ensure_chunk(chunk_idx, file_size)?;

            let available = chunk_data.len().saturating_sub(in_chunk_offset);
            let to_read = (end_offset - cur_offset).min(available as u64) as usize;
            if to_read == 0 {
                break;
            }

            result.extend_from_slice(&chunk_data[in_chunk_offset..in_chunk_offset + to_read]);
            cur_offset += to_read as u64;
        }

        Ok(result)
    }

    /// 在缓存与磁盘中快速定位指定偏移量后的第一个 `\n` 换行符
    fn find_next_newline(&mut self, from_offset: u64, file_size: u64) -> io::Result<Option<u64>> {
        let mut offset = from_offset;
        while offset < file_size {
            let chunk_idx = offset / CHUNK_SIZE;
            let in_chunk = (offset % CHUNK_SIZE) as usize;
            let chunk_data = self.ensure_chunk(chunk_idx, file_size)?;

            if in_chunk < chunk_data.len() {
                let slice = &chunk_data[in_chunk..];
                if let Some(pos) = memchr::memchr(b'\n', slice) {
                    return Ok(Some(offset + pos as u64));
                }
            }

            offset = (chunk_idx + 1) * CHUNK_SIZE;
        }
        Ok(None)
    }
}


/// 自动拆页分页信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageInfo {
    /// 当前页索引 (0-based)
    pub page_index: usize,
    /// 总页数 (>= 1)
    pub total_pages: usize,
    /// 每页行数 (例如 10,000 行)
    pub page_size: usize,
    /// 当前页起始全局逻辑行 (0-based)
    pub start_line: usize,
    /// 当前页结束全局逻辑行 (exclusive, 0-based)
    pub end_line: usize,
    /// 文件总逻辑行数
    pub total_lines: usize,
}

/// 跨页查询命中项
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MmapMatch {
    /// 全局逻辑行号 (0-based)
    pub global_line: usize,
    /// 所在分页索引 (0-based)
    pub page_index: usize,
    /// 在当前分页内的行号 (0-based)
    pub line_in_page: usize,
    /// 匹配处列号 (0-based 字符偏移)
    pub col: usize,
    /// 所在行的完整文本 (已去 EOL)
    pub text: String,
}

/// 超大文件虚拟内存分块与稀疏行索引缓冲区
pub struct MmapBuffer {
    path: PathBuf,
    file_size: u64,
    total_lines: usize,
    /// 检查点: 存储 line 0, 256, 512, ... 各逻辑行的起始绝对字节偏移
    checkpoints: Vec<u64>,
    inner: Mutex<InnerState>,
}

impl MmapBuffer {
    /// 打开指定路径的超大文件并建立稀疏行索引
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let mut file = File::open(&path_buf)?;
        let file_size = file.metadata()?.len();

        let mut checkpoints = Vec::new();
        let mut total_lines = 0;

        if file_size > 0 {
            checkpoints.push(0u64); // 第 0 行从字节偏移 0 开始

            let mut line_count = 0usize;
            let mut byte_pos = 0u64;
            let mut last_byte_was_newline = false;
            let mut buf = [0u8; 64 * 1024];

            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                let mut chunk = &buf[..n];
                let mut chunk_offset = 0;

                while let Some(rel) = memchr::memchr(b'\n', chunk) {
                    line_count += 1;
                    let newline_global_pos = byte_pos + chunk_offset as u64 + rel as u64;
                    let next_line_start = newline_global_pos + 1;

                    if next_line_start < file_size && line_count % SPARSE_INTERVAL == 0 {
                        checkpoints.push(next_line_start);
                    }

                    chunk_offset += rel + 1;
                    chunk = &chunk[rel + 1..];
                }

                last_byte_was_newline = buf[n - 1] == b'\n';
                byte_pos += n as u64;
            }

            total_lines = if last_byte_was_newline {
                line_count
            } else {
                line_count + 1
            };

            file.seek(SeekFrom::Start(0))?;
        }

        Ok(Self {
            path: path_buf,
            file_size,
            total_lines,
            checkpoints,
            inner: Mutex::new(InnerState {
                file,
                cache: ChunkCache::new(),
            }),
        })
    }

    /// 获取文件的逻辑行数
    #[inline]
    pub fn line_count(&self) -> usize {
        self.total_lines
    }

    /// 获取文件总字节大小
    #[inline]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// 判断文件是否为空文件
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.file_size == 0
    }

    /// 获取文件路径引用
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取单行文本内容 (去除末尾换行符 \r\n 或 \n)
    pub fn get_line(&self, line_idx: usize) -> Option<String> {
        self.get_lines_range(line_idx, 1).into_iter().next()
    }

    /// 批量窗口切片读取指定范围的行 (例如视口渲染 50~100 行)
    pub fn get_lines_range(&self, start_line: usize, count: usize) -> Vec<String> {
        if count == 0 || start_line >= self.total_lines {
            return Vec::new();
        }

        let end_line = (start_line + count).min(self.total_lines);
        let actual_count = end_line - start_line;
        let mut result = Vec::with_capacity(actual_count);

        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        // 1. 利用稀疏索引快速跳转到最近的检查点
        let cp_idx = start_line / SPARSE_INTERVAL;
        if cp_idx >= self.checkpoints.len() {
            return Vec::new();
        }
        let mut cur_offset = self.checkpoints[cp_idx];
        let step_needed = start_line % SPARSE_INTERVAL;

        // 2. 扫描跳过剩余步长内的换行符
        for _ in 0..step_needed {
            match inner.find_next_newline(cur_offset, self.file_size) {
                Ok(Some(nl_pos)) => {
                    cur_offset = nl_pos + 1;
                }
                _ => return result,
            }
        }

        // 3. 顺序提取 actual_count 行
        for _ in 0..actual_count {
            if cur_offset > self.file_size {
                break;
            }

            let line_end = match inner.find_next_newline(cur_offset, self.file_size) {
                Ok(Some(nl_pos)) => nl_pos,
                Ok(None) => self.file_size,
                Err(_) => break,
            };

            let len = (line_end.saturating_sub(cur_offset)) as usize;
            let raw = match inner.read_bytes(cur_offset, len, self.file_size) {
                Ok(bytes) => bytes,
                Err(_) => break,
            };

            // 去除末尾可能存在的 \r (CRLF 格式)
            let slice = if raw.last() == Some(&b'\r') {
                &raw[..raw.len() - 1]
            } else {
                &raw[..]
            };

            result.push(String::from_utf8_lossy(slice).into_owned());
            cur_offset = line_end + 1;
        }

        result
    }

    /// 读取任意起始字节偏移与长度的二进制切片
    pub fn read_slice(&self, byte_offset: u64, len: usize) -> io::Result<Vec<u8>> {
        let mut inner = self.inner.lock().map_err(|e| {
            io::Error::other(format!("Mutex poison error: {}", e))
        })?;
        inner.read_bytes(byte_offset, len, self.file_size)
    }

    /// 基于快速字节扫描的高性能关键字搜索
    ///
    /// 借助 `memchr::memmem::Finder` 与 64KB 块滑动窗口，
    /// 并在窗口边界维持 `query.len() - 1` 重叠，确保跨块匹配准确且高速。
    pub fn find_next(&self, query: &[u8], from_offset: u64) -> Option<u64> {
        if query.is_empty() {
            return if from_offset <= self.file_size {
                Some(from_offset)
            } else {
                None
            };
        }

        if from_offset >= self.file_size {
            return None;
        }

        if (self.file_size - from_offset) < query.len() as u64 {
            return None;
        }

        let finder = memchr::memmem::Finder::new(query);
        let overlap = query.len().saturating_sub(1);
        let buffer_size = (CHUNK_SIZE as usize).max(query.len() * 2);
        let mut cur_offset = from_offset;

        let mut inner = self.inner.lock().ok()?;

        while cur_offset < self.file_size {
            let remaining = (self.file_size - cur_offset) as usize;
            let read_len = remaining.min(buffer_size);
            if read_len < query.len() {
                break;
            }

            let data = inner.read_bytes(cur_offset, read_len, self.file_size).ok()?;
            if let Some(pos) = finder.find(&data) {
                return Some(cur_offset + pos as u64);
            }

            let advance = read_len.saturating_sub(overlap);
            if advance == 0 {
                break;
            }
            cur_offset += advance as u64;
        }

        None
    }

    /// 默认分页大小: 10,000 行
    pub const DEFAULT_PAGE_SIZE: usize = 10_000;

    /// 计算指定分页大小下的总页数 (>= 1)
    #[inline]
    pub fn page_count(&self, page_size: usize) -> usize {
        let size = page_size.max(1);
        if self.total_lines == 0 {
            1
        } else {
            self.total_lines.div_ceil(size)
        }
    }

    /// 获取指定分页的元信息
    pub fn page_info(&self, page_index: usize, page_size: usize) -> PageInfo {
        let total_pages = self.page_count(page_size);
        let p_idx = page_index.min(total_pages.saturating_sub(1));
        let size = page_size.max(1);
        let start_line = p_idx * size;
        let end_line = (start_line + size).min(self.total_lines);
        PageInfo {
            page_index: p_idx,
            total_pages,
            page_size: size,
            start_line,
            end_line,
            total_lines: self.total_lines,
        }
    }

    /// 读取指定分页的所有行与分页元信息 (开箱即用)
    pub fn get_page(&self, page_index: usize, page_size: usize) -> (Vec<String>, PageInfo) {
        let info = self.page_info(page_index, page_size);
        let count = info.end_line.saturating_sub(info.start_line);
        let lines = self.get_lines_range(info.start_line, count);
        (lines, info)
    }

    /// 根据全局逻辑行号计算所在分页索引 (0-based)
    #[inline]
    pub fn page_for_line(&self, line: usize, page_size: usize) -> usize {
        let size = page_size.max(1);
        line / size
    }

    /// 将绝对字节偏移映射到全局逻辑行号 (0-based)
    ///
    /// 算法: 基于 checkpoints 二分查找定位最近检查点 (O(log N)),
    /// 随后在 64KB 块内借助 SIMD 快速扫描跳过至多 255 个换行符。微秒级完成。
    pub fn offset_to_line(&self, byte_offset: u64) -> usize {
        if self.checkpoints.is_empty() || byte_offset == 0 {
            return 0;
        }
        let target = byte_offset.min(self.file_size);
        let cp_idx = match self.checkpoints.binary_search(&target) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let mut line = cp_idx * SPARSE_INTERVAL;
        let mut cur_offset = self.checkpoints[cp_idx];

        let mut inner = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return line,
        };

        while cur_offset < target {
            match inner.find_next_newline(cur_offset, self.file_size) {
                Ok(Some(nl_pos)) if nl_pos < target => {
                    line += 1;
                    cur_offset = nl_pos + 1;
                }
                _ => break,
            }
        }
        line.min(self.total_lines.saturating_sub(1))
    }

    /// 将全局逻辑行号映射到绝对起始字节偏移
    pub fn line_to_offset(&self, line: usize) -> Option<u64> {
        if line >= self.total_lines {
            return None;
        }
        let cp_idx = line / SPARSE_INTERVAL;
        if cp_idx >= self.checkpoints.len() {
            return None;
        }
        let mut cur_offset = self.checkpoints[cp_idx];
        let step = line % SPARSE_INTERVAL;
        let mut inner = self.inner.lock().ok()?;

        for _ in 0..step {
            match inner.find_next_newline(cur_offset, self.file_size) {
                Ok(Some(nl)) => cur_offset = nl + 1,
                _ => return None,
            }
        }
        Some(cur_offset)
    }

    /// 大小写敏感 / 不敏感关键字搜索
    pub fn find_next_case(&self, query: &str, from_offset: u64, case_sensitive: bool) -> Option<u64> {
        if query.is_empty() {
            return if from_offset <= self.file_size {
                Some(from_offset)
            } else {
                None
            };
        }
        if case_sensitive {
            return self.find_next(query.as_bytes(), from_offset);
        }

        if from_offset >= self.file_size {
            return None;
        }

        let q_lower = query.to_lowercase();
        let q_bytes = q_lower.as_bytes();
        if (self.file_size - from_offset) < q_bytes.len() as u64 {
            return None;
        }

        let overlap = q_bytes.len().saturating_sub(1);
        let buffer_size = (CHUNK_SIZE as usize).max(q_bytes.len() * 2);
        let mut cur_offset = from_offset;
        let mut inner = self.inner.lock().ok()?;

        while cur_offset < self.file_size {
            let remaining = (self.file_size - cur_offset) as usize;
            let read_len = remaining.min(buffer_size);
            if read_len < q_bytes.len() {
                break;
            }

            let data = inner.read_bytes(cur_offset, read_len, self.file_size).ok()?;
            let s = String::from_utf8_lossy(&data).to_lowercase();
            if let Some(byte_pos) = s.find(&q_lower) {
                return Some(cur_offset + byte_pos as u64);
            }

            let advance = read_len.saturating_sub(overlap);
            if advance == 0 {
                break;
            }
            cur_offset += advance as u64;
        }

        None
    }

    /// 超大文件跨页流式高速查询
    ///
    /// 无需将全部行或文本装入内存，通过 SIMD 滑窗匹配和稀疏索引反查，
    /// 精确定位每一个匹配项的全局行号、分页编号与分页内行号。
    pub fn search_matches(
        &self,
        query: &str,
        page_size: usize,
        case_sensitive: bool,
        max_matches: usize,
    ) -> Vec<MmapMatch> {
        if query.is_empty() || max_matches == 0 {
            return Vec::new();
        }
        let size = page_size.max(1);
        let mut matches = Vec::new();
        let mut cur_offset = 0u64;

        while let Some(hit_offset) = self.find_next_case(query, cur_offset, case_sensitive) {
            let global_line = self.offset_to_line(hit_offset);
            let line_text = self.get_line(global_line).unwrap_or_default();
            let line_start = self.line_to_offset(global_line).unwrap_or(hit_offset);
            let byte_in_line = (hit_offset.saturating_sub(line_start)) as usize;
            let col = line_text
                .char_indices()
                .take_while(|(b, _)| *b < byte_in_line)
                .count();

            let page_index = global_line / size;
            let line_in_page = global_line % size;

            matches.push(MmapMatch {
                global_line,
                page_index,
                line_in_page,
                col,
                text: line_text,
            });

            if matches.len() >= max_matches {
                break;
            }

            cur_offset = hit_offset.saturating_add(query.len().max(1) as u64);
            if cur_offset >= self.file_size {
                break;
            }
        }

        matches
    }

}
