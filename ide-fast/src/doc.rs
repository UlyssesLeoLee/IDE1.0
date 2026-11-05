//! 文档模型: 行向量 + 光标 / 选区 + 撤销栈, 编辑和移动全部交给 `editcore` (和 Tauri 版前端走同一套 Rust 语义)。
//!
//! editcore 的编辑 / 移动 / 查找是**无状态**的: 调用方交上光标周围需要的几行, 它返回一个行补丁 + 新光标 / 选区。
//! 这里就是那个"调用方": 组请求 -> 调 `editcore` -> 套补丁 -> 记撤销。所以 Sakura 的键位语义 (词移动、行首智能 Home、
//! 自动缩进、整行剪切……) 不用在 GUI 里重写第二份。

use crate::find::FindOpts;
use editcore::bookmark::{self, Bm, BmReq};
use editcore::document_io::{self, AtomicWriteOpts, Bom, Charset, Eol, SaveSpec};
use editcore::edit_cmd::{self, EditReq};
use editcore::nav::{self, NavReq};
use editcore::search_ops::{self, Hit, SearchReq};
use std::path::{Path, PathBuf};

/// 一次可撤销的行补丁 (撤销 = 把插入的行换回删掉的行)
#[derive(Clone)]
struct Undo {
    start: usize,
    removed: Vec<String>,
    inserted: Vec<String>,
    before: ([usize; 2], Option<[usize; 4]>),
    after: [usize; 2],
}

pub struct Doc {
    /// 手动指定的文件类型名 (None = 按扩展名判定)
    pub syntax: Option<String>,
    #[allow(dead_code)]
    pub id: u64,
    pub path: Option<PathBuf>,
    pub name: String,
    pub lines: Vec<String>,
    pub cursor: [usize; 2],
    /// 选区 [r0, c0, r1, c1] (已排好序) 与锚点
    pub sel: Option<[usize; 4]>,
    pub anchor: Option<[usize; 2]>,
    /// 上下移动时记住的目标列
    pub goal_col: Option<usize>,
    pub dirty: bool,
    pub charset: Charset,
    pub bom: Bom,
    pub eol: Eol,
    undo: Vec<Undo>,
    redo: Vec<Undo>,
    pub tip: String,
    /// 编辑区该滚到哪 (行); 光标移动后置位, 绘制时消费
    pub reveal: bool,
    /// 文本版本号: 每次改行内容 +1 (大纲等缓存据此判断要不要重算)
    pub rev: u64,
    /// 书签 (按行; 行增删时跟着移动)
    pub bookmarks: Vec<Bm>,
    /// 矩形选区 [r0, c0, r1, c1] (未归一化: 锚点在前, 光标在后)
    pub box_sel: Option<[usize; 4]>,
    /// 额外的光标 (多光标; 主光标是 `cursor`)
    pub extra: Vec<[usize; 2]>,
    /// 键盘宏录制中 / 上一次录下的宏
    pub rec: Option<Vec<RecEv>>,
    pub last_macro: Vec<RecEv>,
    /// 已折叠的范围 (起始行, 结束行含): 起始行还显示, 后面的行隐藏
    pub folds: Vec<(usize, usize)>,
    /// 标签分组 (0 = 不分组, 1..=5 = 红 橙 绿 蓝 紫)
    pub group: u8,
    /// 改写模式 (Ins) / 只读
    pub overwrite: bool,
    pub readonly: bool,
    /// 是 Grep 结果文档 (只读; 在命中行上按 Enter / 双击跳到那个文件)
    pub grep_result: bool,
    /// 读 / 写文件时的 (修改时间, 大小): 窗口重新获得焦点时拿它判断文件有没有被外部改过
    pub stamp: Option<(std::time::SystemTime, u64)>,
    /// 读进来时换行符混用 (状态栏提示)
    pub mixed_eol: bool,
    /// 编辑偏好 (首选项里的自动缩进 / 智能缩进 / Tab 插入空格); App 在新建 / 打开 / 改首选项时统一写入
    pub auto_indent: bool,
    pub smart_indent: bool,
    pub insert_spaces: bool,
    /// 基线: 打开 / 上次保存时的全文 (修改行标记据此比较)
    pub base: Vec<String>,
    /// 修改行标记缓存: (rev, 结果)
    pub changed_cache: Option<(u64, Vec<bool>)>,
    /// F6 范围选择模式: 开着时方向键 / 翻页键都在扩大选区 (不用按 Shift)
    pub sel_mode: bool,
    /// 大文件自动拆页状态 (None = 普通文档整篇载入; Some = MmapBuffer 分页文档)
    pub pagination: Option<PaginationState>,
    /// 协同编辑远程协作者/AI光标与选区状态
    pub remote_cursors: Vec<ide_agent_router::RemoteCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaginationState {
    pub page_size: usize,
    pub current_page: usize,
    pub total_pages: usize,
    pub total_lines: usize,
    pub file_size: u64,
}

/// 插件补丁一次最多插入的行数 (和 Tauri 版 applyPluginEdit 相同)
pub const PATCH_MAX_LINES: usize = 200_000;

/// 超过 HARD 直接拒绝 (整篇载入内存, 太大会拖垮编辑器); 「打开前要确认」的阈值在首选项里 (max_file_mb)
pub const HARD_LIMIT: u64 = 32 * 1024 * 1024;

/// 前 8 KB 里有 NUL 且不是带 BOM 的 UTF-16 = 二进制
fn looks_binary(p: &Path) -> bool {
    use std::io::Read;
    let mut buf = [0u8; 8192];
    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    let n = f.read(&mut buf).unwrap_or(0);
    let b = &buf[..n];
    if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        return false;
    }
    b.contains(&0)
}

pub fn stamp_of(p: &Path) -> Option<(std::time::SystemTime, u64)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

/// 录下来的一步 (回放时原样再走一遍 editcore)
#[derive(Clone, Debug, PartialEq)]
pub enum RecEv {
    Edit(String, Option<String>),
    Nav(String, bool, bool),
}

impl Doc {
    pub fn untitled(id: u64, n: usize) -> Self {
        Doc {
            id,
            syntax: None,
            path: None,
            name: if n <= 1 {
                "无标题".into()
            } else {
                format!("无标题 {n}")
            },
            lines: vec![String::new()],
            cursor: [0, 0],
            sel: None,
            anchor: None,
            goal_col: None,
            dirty: false,
            charset: Charset::Utf8,
            bom: Bom::None,
            eol: Eol::Crlf,
            undo: vec![],
            redo: vec![],
            tip: String::new(),
            reveal: false,
            rev: 0,
            bookmarks: vec![],
            box_sel: None,
            extra: vec![],
            rec: None,
            last_macro: vec![],
            folds: vec![],
            group: 0,
            overwrite: false,
            readonly: false,
            grep_result: false,
            stamp: None,
            mixed_eol: false,
            auto_indent: true,
            smart_indent: false,
            insert_spaces: false,
            base: vec![String::new()],
            changed_cache: None,
            sel_mode: false,
            pagination: None,
            remote_cursors: vec![],
        }
    }

    pub fn open(id: u64, path: &Path) -> Result<Self, String> {
        Self::open_with(id, path, None)
    }

    /// `forced` = 指定编码标签 (以指定编码重新打开); None = 自动判别
    pub fn open_with(id: u64, path: &Path, forced: Option<&str>) -> Result<Self, String> {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if size > HARD_LIMIT {
            if let Ok(mmap) = editcore::MmapBuffer::open(path) {
                let page_size = editcore::MmapBuffer::DEFAULT_PAGE_SIZE;
                let (lines, pinfo) = mmap.get_page(0, page_size);
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "?".into());
                let page_state = PaginationState {
                    page_size: pinfo.page_size,
                    current_page: pinfo.page_index,
                    total_pages: pinfo.total_pages,
                    total_lines: pinfo.total_lines,
                    file_size: size,
                };
                return Ok(Doc {
                    id,
                    syntax: None,
                    path: Some(path.to_path_buf()),
                    name,
                    base: lines.clone(),
                    changed_cache: None,
                    sel_mode: false,
                    lines,
                    cursor: [0, 0],
                    sel: None,
                    anchor: None,
                    goal_col: None,
                    dirty: false,
                    charset: Charset::Utf8,
                    bom: Bom::None,
                    eol: Eol::Lf,
                    undo: vec![],
                    redo: vec![],
                    tip: format!(
                        "超大文件 ({} MB) 自动拆页: 第 1/{} 页 (每页 {} 行, 全文 {} 行) [Alt+PageDown 翻页]",
                        size / 1024 / 1024,
                        page_state.total_pages,
                        page_state.page_size,
                        page_state.total_lines
                    ),
                    reveal: true,
                    rev: 0,
                    bookmarks: vec![],
                    box_sel: None,
                    extra: vec![],
                    rec: None,
                    last_macro: vec![],
                    folds: vec![],
                    group: 0,
                    overwrite: false,
                    readonly: true,
                    grep_result: false,
                    stamp: None,
                    mixed_eol: false,
                    auto_indent: true,
                    smart_indent: false,
                    insert_spaces: false,
                    pagination: Some(page_state),
                    remote_cursors: vec![],
                });
            }
            return Err(format!(
                "文件太大 ({} MB, 上限 {} MB): 高速版把整篇载入内存, 请用专门的大文件查看器",
                size / 1024 / 1024,
                HARD_LIMIT / 1024 / 1024
            ));
        }
        if looks_binary(path) {
            return Err("看起来是二进制文件 (含 NUL 字节), 不按文本打开".into());
        }
        let d = document_io::read_doc(path, forced)?;
        let mut lines: Vec<String> = d.content.split('\n').map(str::to_string).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "?".into());
        Ok(Doc {
            id,
            syntax: None,
            path: Some(path.to_path_buf()),
            name,
            base: lines.clone(),
            changed_cache: None,
            sel_mode: false,
            lines,
            cursor: [0, 0],
            sel: None,
            anchor: None,
            goal_col: None,
            dirty: false,
            charset: d.charset,
            bom: d.bom,
            eol: d.eol,
            undo: vec![],
            redo: vec![],
            tip: format!(
                "{} · {} · {}",
                d.charset.label(),
                d.eol.label(),
                if d.lossy {
                    "有无法解码的字节"
                } else {
                    "已读取"
                }
            ),
            reveal: true,
            rev: 0,
            bookmarks: vec![],
            box_sel: None,
            extra: vec![],
            rec: None,
            last_macro: vec![],
            folds: vec![],
            group: 0,
            overwrite: false,
            readonly: false,
            grep_result: false,
            stamp: stamp_of(path),
            mixed_eol: d.mixed_eol,
            auto_indent: true,
            smart_indent: false,
            insert_spaces: false,
            pagination: None,
            remote_cursors: vec![],
        })
    }

    /// 磁盘上的文件和读进来时不一样了 (被别的程序改过 / 删了)
    pub fn changed_on_disk(&self) -> bool {
        match (&self.path, self.stamp) {
            (Some(p), Some(st)) => stamp_of(p) != Some(st),
            _ => false,
        }
    }

    /// 从磁盘重新载入 (丢弃内存里的修改); 保留标签分组 / 只读
    pub fn reload(&mut self) -> Result<(), String> {
        let Some(p) = self.path.clone() else {
            return Err("还没有保存过, 没有可重新载入的文件".into());
        };
        let mut n = Doc::open(self.id, &p)?;
        n.group = self.group;
        n.readonly = self.readonly;
        n.rev = self.rev + 1;
        n.tip = "已从磁盘重新载入".into();
        *self = n;
        Ok(())
    }

    /// 以指定编码重新打开 (丢弃内存里的修改); 保留分组 / 只读
    pub fn reopen_as(&mut self, charset: Charset) -> Result<(), String> {
        let Some(p) = self.path.clone() else {
            return Err("还没有保存过, 没有可重新打开的文件".into());
        };
        let mut n = Doc::open_with(self.id, &p, Some(charset.label()))?;
        n.group = self.group;
        n.readonly = self.readonly;
        n.rev = self.rev + 1;
        n.tip = format!("已按 {} 重新打开", charset.label());
        *self = n;
        Ok(())
    }

    /// 改保存时的换行符 (内存里始终是 LF, 只是元数据; 改了算「已修改」好提醒用户保存)
    pub fn set_eol(&mut self, eol: Eol) {
        if self.eol != eol {
            self.eol = eol;
            self.dirty = true;
        }
        self.mixed_eol = false;
        self.tip = format!("保存时的换行符: {}", eol.label());
    }

    /// 改保存时的字符编码; BOM 跟着编码走 (UTF-16 必带, 其余编码不带; UTF-8 保留原来有没有)
    pub fn set_charset(&mut self, cs: Charset) {
        if self.charset != cs {
            self.charset = cs;
            self.bom = match cs {
                Charset::Utf16Le => Bom::Utf16Le,
                Charset::Utf16Be => Bom::Utf16Be,
                Charset::Utf8 if self.bom == Bom::Utf8 => Bom::Utf8,
                _ => Bom::None,
            };
            self.dirty = true;
        }
        self.tip = format!("保存时的编码: {}", cs.label());
    }

    /// 切换 UTF-8 的 BOM (别的编码没有可选的 BOM)
    pub fn toggle_bom(&mut self) {
        if self.charset != Charset::Utf8 {
            self.tip = "只有 UTF-8 可以选择带不带 BOM".into();
            return;
        }
        self.bom = if self.bom == Bom::Utf8 {
            Bom::None
        } else {
            Bom::Utf8
        };
        self.dirty = true;
        self.tip = if self.bom == Bom::Utf8 {
            "保存时带 BOM".into()
        } else {
            "保存时不带 BOM".into()
        };
    }

    /// 当前文档是否为 PowerShell 脚本 (.ps1, .psm1, .psd1 或语法标记)
    pub fn is_powershell(&self) -> bool {
        let name = self.name.to_ascii_lowercase();
        name.ends_with(".ps1")
            || name.ends_with(".psm1")
            || name.ends_with(".psd1")
            || self
                .syntax
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case("powershell"))
                .unwrap_or(false)
    }

    /// 获取当前选中的纯文本 (若无选区则返回空字符串)
    pub fn selected_text(&self) -> String {
        let Some(s) = self.sel else {
            return String::new();
        };
        let (r0, c0, r1, c1) = (s[0], s[1], s[2], s[3]);
        if r0 >= self.lines.len() {
            return String::new();
        }
        let mut out = Vec::new();
        for r in r0..=r1.min(self.lines.len() - 1) {
            let line = &self.lines[r];
            let chars: Vec<char> = line.chars().collect();
            let start = if r == r0 { c0.min(chars.len()) } else { 0 };
            let end = if r == r1 { c1.min(chars.len()) } else { chars.len() };
            if start <= end {
                out.push(chars[start..end].iter().collect::<String>());
            }
        }
        out.join("\n")
    }

    /// 文件属性 (给「文件属性」对话框)
    pub fn properties(&self) -> String {
        let chars: usize = self.lines.iter().map(|l| l.chars().count()).sum::<usize>()
            + self.lines.len().saturating_sub(1);
        let (size, mtime) = match self.path.as_ref().and_then(|p| std::fs::metadata(p).ok()) {
            Some(m) => (
                format!("{} 字节", m.len()),
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| format!("{} (Unix 秒)", d.as_secs()))
                    .unwrap_or_else(|| "?".into()),
            ),
            None => ("(尚未保存)".into(), "-".into()),
        };
        let bom = match self.bom {
            Bom::None => "无",
            Bom::Utf8 => "UTF-8",
            Bom::Utf16Le => "UTF-16LE",
            Bom::Utf16Be => "UTF-16BE",
        };
        format!("名称: {}\n路径: {}\n大小: {}\n修改时间: {}\n行数: {}\n字符数: {}\n编码: {}\nBOM: {}\n换行符: {}{}\n只读: {}",
            self.name, self.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "(无)".into()), size, mtime, self.lines.len(), chars,
            self.charset.label(), bom, self.eol.label(), if self.mixed_eol { " (原文件混用)" } else { "" }, if self.readonly { "是" } else { "否" })
    }

    /// 光标处 (或光标前一个字符) 的括弧: (本身的位置, 另一半的位置); 复用 editcore::bracket (与 Tauri 版同一套)
    pub fn bracket_pair(&self) -> Option<([usize; 2], [usize; 2])> {
        let [r, c] = self.cursor;
        // 光标处和前一个字符都不是括弧就不用交行 (括弧强调每次光标一动都会问, 要便宜)
        const BR: &str = "()[]{}<>（）「」『』【】［］｛｝〈〉《》〔〕";
        let line = self.lines.get(r)?;
        let here = line.chars().nth(c).is_some_and(|ch| BR.contains(ch))
            || c.checked_sub(1)
                .and_then(|p| line.chars().nth(p))
                .is_some_and(|ch| BR.contains(ch));
        if !here {
            return None;
        }
        // 只交上光标上下一段行 (editcore 本身最多扫 30000 个字符)
        let (a, b) = (r.saturating_sub(1500), (r + 1500).min(self.lines.len()));
        let req = editcore::bracket::BracketReq {
            row: r,
            col: c,
            total_rows: self.lines.len(),
            rows: (a..b).map(|i| (i, self.lines[i].clone())).collect(),
        };
        editcore::bracket::find_pair(&req).map(|res| ([r, res.at], res.pair))
    }

    /// 对括弧跳转 (Ctrl+[ / Ctrl+]); 返回是否跳了
    pub fn jump_to_pair(&mut self) -> bool {
        match self.bracket_pair() {
            Some((_, p)) => {
                self.goto(p[0], p[1]);
                true
            }
            None => {
                self.tip = "没有对应的括弧".into();
                false
            }
        }
    }

    pub fn text_lf(&self) -> String {
        self.lines.join("\n")
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        let spec = SaveSpec {
            charset: self.charset,
            bom: self.bom,
            eol: self.eol,
            allow_lossy: false,
            write: AtomicWriteOpts {
                clear_readonly: false,
                sync: true,
            },
        };
        document_io::save_document(path, &self.text_lf(), &spec).map_err(|e| e.to_string())?;
        self.path = Some(path.to_path_buf());
        self.name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.name.clone());
        self.dirty = false;
        self.stamp = stamp_of(path);
        self.base = self.lines.clone();
        self.changed_cache = None;
        Ok(())
    }

    /// 每行是否相对基线改过 (按 rev 缓存)
    pub fn changed(&mut self) -> &[bool] {
        if self.changed_cache.as_ref().map(|c| c.0) != Some(self.rev) {
            self.changed_cache = Some((
                self.rev,
                crate::view::changed_lines(&self.base, &self.lines),
            ));
        }
        &self.changed_cache.as_ref().expect("刚算过").1
    }

    /// 双缓冲区域级回滚 (Staging -> Certified Base)
    /// 将指定行范围 [start_line, end_line] 恢复为基线版本 (base) 中的内容，
    /// 若超出基线版本行数，则用基线版本对应范围替换。
    pub fn rollback_zone(&mut self, start_line: usize, end_line: usize) -> bool {
        if start_line > end_line {
            return false;
        }
        let base_len = self.base.len();
        let curr_len = self.lines.len();
        if start_line >= base_len && start_line >= curr_len {
            return false;
        }

        let actual_start = start_line.min(curr_len);
        let actual_end = end_line.min(curr_len.saturating_sub(1));

        let base_slice: Vec<String> = if start_line < base_len {
            let base_end = end_line.min(base_len.saturating_sub(1));
            self.base[start_line..=base_end].to_vec()
        } else {
            vec![]
        };

        if actual_start <= actual_end && actual_end < curr_len {
            self.lines.splice(actual_start..=actual_end, base_slice);
        } else if actual_start == curr_len {
            self.lines.extend(base_slice);
        }

        if self.lines.is_empty() {
            self.lines.push(String::new());
        }

        self.dirty = self.lines != self.base;
        self.rev += 1;
        self.changed_cache = None;
        true
    }

    pub fn row_len(&self, r: usize) -> usize {
        self.lines.get(r).map(|l| l.chars().count()).unwrap_or(0)
    }

    /// 光标周围 + 选区涉及的行 (editcore 只要这些; 大文件里每次按键不用把整篇交出去)
    fn rows_for(&self, extra: &[usize]) -> Vec<(usize, String)> {
        let mut set: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        let (r, _) = (self.cursor[0], self.cursor[1]);
        for d in [r.saturating_sub(1), r, r + 1] {
            if d < self.lines.len() {
                set.insert(d);
            }
        }
        if let Some(s) = self.sel {
            for x in s[0]..=s[2].min(self.lines.len().saturating_sub(1)) {
                set.insert(x);
            }
        }
        for &e in extra {
            if e < self.lines.len() {
                set.insert(e);
            }
        }
        set.into_iter()
            .map(|i| (i, self.lines[i].clone()))
            .collect()
    }

    /// 是否为 MmapBuffer 自动拆页的超大文件
    pub fn is_paginated(&self) -> bool {
        self.pagination.is_some()
    }

    /// 切换到指定页 (0-based)
    pub fn goto_page(&mut self, target_page: usize) -> bool {
        let Some(pinfo) = &self.pagination else {
            return false;
        };
        let page_size = pinfo.page_size;
        let total_pages = pinfo.total_pages;
        if target_page >= total_pages {
            return false;
        }
        let Some(path) = &self.path else {
            return false;
        };
        let Ok(mmap) = editcore::MmapBuffer::open(path) else {
            return false;
        };
        let (new_lines, new_pinfo) = mmap.get_page(target_page, page_size);
        self.lines = new_lines;
        self.base = self.lines.clone();
        self.changed_cache = None;
        self.cursor = [0, 0];
        self.sel = None;
        self.anchor = None;
        self.goal_col = None;
        self.reveal = true;
        self.rev += 1;
        self.pagination = Some(PaginationState {
            page_size: new_pinfo.page_size,
            current_page: new_pinfo.page_index,
            total_pages: new_pinfo.total_pages,
            total_lines: new_pinfo.total_lines,
            file_size: mmap.file_size(),
        });
        self.tip = format!(
            "已切换到第 {}/{} 页 (行 {}-{}, 全文 {} 行)",
            new_pinfo.page_index + 1,
            new_pinfo.total_pages,
            new_pinfo.start_line + 1,
            new_pinfo.end_line,
            new_pinfo.total_lines
        );
        true
    }

    /// 下一页 (Alt+PageDown)
    pub fn next_page(&mut self) -> bool {
        if let Some(p) = &self.pagination {
            let next = p.current_page + 1;
            if next < p.total_pages {
                return self.goto_page(next);
            }
        }
        false
    }

    /// 上一页 (Alt+PageUp)
    pub fn prev_page(&mut self) -> bool {
        if let Some(p) = &self.pagination {
            if p.current_page > 0 {
                return self.goto_page(p.current_page - 1);
            }
        }
        false
    }

    /// 跨页定位全局行 (0-based global line number)
    pub fn goto_global_line(&mut self, global_line: usize, col: usize) -> bool {
        let Some(p) = self.pagination.clone() else {
            self.goto(global_line, col);
            return true;
        };
        let page_size = p.page_size;
        let target_page = global_line / page_size;
        let line_in_page = global_line % page_size;
        if target_page != p.current_page {
            if !self.goto_page(target_page) {
                return false;
            }
        }
        self.sel_mode = false;
        let max_r = self.lines.len().saturating_sub(1);
        self.cursor = [line_in_page.min(max_r), col];
        self.clamp();
        self.sel = None;
        self.anchor = None;
        self.goal_col = None;
        self.reveal = true;
        if let Some(p_now) = &self.pagination {
            self.tip = format!(
                "第 {}/{} 页 (全局行 {}, 全文 {} 行)",
                p_now.current_page + 1,
                p_now.total_pages,
                global_line + 1,
                p_now.total_lines
            );
        }
        true
    }

    /// 光标跳到 (行, 列) 并滚到可见处 (大纲 / Grep 结果 / 书签用)
    pub fn goto(&mut self, row: usize, col: usize) {
        if let Some(p) = &self.pagination {
            if row >= self.lines.len() || row >= p.page_size {
                self.goto_global_line(row, col);
                return;
            }
        }
        self.sel_mode = false;
        self.cursor = [row.min(self.lines.len().saturating_sub(1)), col];
        self.clamp();
        self.sel = None;
        self.anchor = None;
        self.goal_col = None;
        self.reveal = true;
    }

    fn shift_bookmarks(&mut self, start: usize, remove: usize, insert: usize) {
        self.bookmarks.retain_mut(|b| {
            if b.row >= start + remove {
                b.row = b.row + insert - remove;
                true
            } else {
                b.row < start + insert || b.row < start // 被替换范围里的: 还在新范围内就保留
            }
        });
    }

    /// 缩进折叠: 从 `row` 往下, 缩进比它深的连续行 (中间空行算在内, 结尾空行不算) 是它的折叠范围
    pub fn fold_range_at(&self, row: usize) -> Option<(usize, usize)> {
        let indent = |l: &str| -> Option<usize> {
            if l.trim().is_empty() {
                return None;
            }
            Some(
                l.chars()
                    .take_while(|c| c.is_whitespace())
                    .map(|c| if c == '\t' { 4 } else { 1 })
                    .sum(),
            )
        };
        let base = indent(self.lines.get(row)?)?;
        let mut end = row;
        for r in row + 1..self.lines.len() {
            match indent(&self.lines[r]) {
                None => continue,
                Some(i) if i > base => end = r,
                Some(_) => break,
            }
        }
        (end > row).then_some((row, end))
    }

    pub fn toggle_fold(&mut self, row: usize) {
        if let Some(i) = self.folds.iter().position(|f| f.0 == row) {
            self.folds.remove(i);
        } else if let Some(r) = self.fold_range_at(row) {
            self.folds.push(r);
            // 光标落在被折起来的行里: 移回折叠起始行
            if self.cursor[0] > r.0 && self.cursor[0] <= r.1 {
                self.goto(r.0, 0);
            }
        }
    }

    pub fn is_hidden(&self, row: usize) -> bool {
        self.folds.iter().any(|&(s, e)| row > s && row <= e)
    }

    fn splice(&mut self, start: usize, remove: usize, insert: Vec<String>) {
        if remove != insert.len() {
            self.folds.clear(); // 行数变了: 折叠范围作废 (改行内容不影响)
        }
        self.rev += 1;
        self.shift_bookmarks(start, remove, insert.len());
        let end = (start + remove).min(self.lines.len());
        self.lines.splice(start.min(self.lines.len())..end, insert);
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
    }

    fn clamp(&mut self) {
        let last = self.lines.len().saturating_sub(1);
        self.cursor[0] = self.cursor[0].min(last);
        self.cursor[1] = self.cursor[1].min(self.row_len(self.cursor[0]));
        let r = self.cursor[0];
        self.folds.retain(|&(s, e)| !(r > s && r <= e)); // 光标移进折叠范围就展开它
    }

    /// 执行一条 editcore 编辑命令; 返回要写进剪贴板的文字 (复制 / 剪切类)
    pub fn edit(&mut self, cmd: &str, text: Option<String>) -> Option<String> {
        if self.readonly
            && !matches!(
                cmd,
                "copy"
                    | "copy-lines"
                    | "copy-crlf"
                    | "copy-linenum"
                    | "copy-passage"
                    | "select-word"
            )
        {
            self.tip = "只读: 不能修改 (编辑 → 只读 可以关掉)".into();
            return None;
        }
        self.sel_mode = false;
        if let Some(r) = self.rec.as_mut() {
            r.push(RecEv::Edit(cmd.to_string(), text.clone()));
        }
        // 矩形选区 / 多光标: 键入、退格、删除、回车、复制、剪切走各自的命令; 别的命令先退出这两种模式
        let mut cmd = cmd.to_string();
        let mut text = text;
        if self.box_sel.is_some() {
            cmd = match cmd.as_str() {
                "insert-text" => "box-insert".into(),
                "paste" => {
                    text = text.map(|t| t.lines().next().unwrap_or("").to_string());
                    "box-insert".into()
                }
                "backspace" => "box-backspace".into(),
                "delete" => "box-delete".into(),
                "new-line" => "box-enter".into(),
                "copy" => "box-copy".into(),
                "cut" => "box-cut".into(),
                c if c.starts_with("box-") => c.to_string(),
                other => {
                    self.box_sel = None;
                    other.to_string()
                }
            };
        } else if !self.extra.is_empty() {
            cmd = match cmd.as_str() {
                "insert-text" => "multi-insert".into(),
                "paste" => "multi-insert".into(),
                "backspace" => "multi-backspace".into(),
                c if c.starts_with("multi-") => c.to_string(),
                other => {
                    self.extra.clear();
                    other.to_string()
                }
            };
        }
        let multi = cmd.starts_with("multi-");
        let mut all_cursors: Vec<[usize; 2]> = vec![];
        let mut extra_rows: Vec<usize> = vec![];
        if multi {
            all_cursors.push(self.cursor);
            all_cursors.extend(self.extra.iter().copied());
            all_cursors.sort();
            extra_rows = all_cursors
                .iter()
                .flat_map(|c| [c[0].saturating_sub(1), c[0], c[0] + 1])
                .collect();
        }
        if let Some(b) = self.box_sel {
            extra_rows.extend(b[0].min(b[2])..=b[0].max(b[2]));
        }
        let req = EditReq {
            cmd: cmd.clone(),
            row: self.cursor[0],
            col: self.cursor[1],
            sel: self.sel,
            total_rows: self.lines.len(),
            auto_indent: self.auto_indent,
            smart_indent: self.smart_indent,
            overwrite: self.overwrite,
            text,
            box_sel: self.box_sel,
            cursors: all_cursors,
            rows: self.rows_for(&extra_rows),
            ..Default::default()
        };
        let Some(res) = edit_cmd::run(&req) else {
            self.tip = format!("未知命令: {cmd}");
            return None;
        };
        if let Some(t) = &res.tip {
            self.tip = t.clone();
        }
        if let Some(patches) = res.patches.clone() {
            // 多光标: 几段补丁 (起始行是改动前的行号), 从下往上套; 整体记成一步撤销
            let old = self.lines.clone();
            let before = (self.cursor, self.sel);
            let mut ps = patches;
            ps.sort_by_key(|a| std::cmp::Reverse(a.0));
            for (s, n, ins) in ps {
                self.splice(s, n, ins);
            }
            self.undo.push(Undo {
                start: 0,
                removed: old,
                inserted: self.lines.clone(),
                before,
                after: res.cursor,
            });
            self.redo.clear();
            self.dirty = true;
        }
        if let Some(cs) = &res.cursors {
            self.extra = cs.iter().skip(1).copied().collect();
        }
        if cmd.starts_with("box-") {
            self.box_sel = res.box_sel;
        }
        if res.changed && res.patches.is_none() {
            let (start, remove) = (res.start, res.remove);
            let removed: Vec<String> = self
                .lines
                .iter()
                .skip(start)
                .take(remove)
                .cloned()
                .collect();
            let before = (self.cursor, self.sel);
            self.splice(start, remove, res.insert.clone());
            self.push_undo(Undo {
                start,
                removed,
                inserted: res.insert.clone(),
                before,
                after: res.cursor,
            });
            self.dirty = true;
            self.redo.clear();
        }
        self.cursor = res.cursor;
        self.sel = res.sel;
        self.anchor = res.anchor;
        if res.changed || res.sel.is_some() {
            self.goal_col = None;
        }
        self.clamp();
        self.reveal = true;
        res.clip
    }

    /// 连续单字符键入合并成一步撤销 (同一行、紧挨着)
    fn push_undo(&mut self, u: Undo) {
        if let Some(p) = self.undo.last_mut() {
            if p.start == u.start
                && p.inserted.len() == 1
                && u.removed.len() == 1
                && u.inserted.len() == 1
                && p.after == u.before.0
                && u.before.1.is_none()
                && u.inserted[0].chars().count() == p.inserted[0].chars().count() + 1
            {
                p.inserted = u.inserted;
                p.after = u.after;
                return;
            }
        }
        self.undo.push(u);
        if self.undo.len() > 5000 {
            self.undo.remove(0);
        }
    }

    /// 把 [start, start+remove) 行换成 insert, 记一步撤销, 光标放到 cursor
    pub fn replace_rows(
        &mut self,
        start: usize,
        remove: usize,
        insert: Vec<String>,
        cursor: [usize; 2],
    ) {
        let removed: Vec<String> = self
            .lines
            .iter()
            .skip(start)
            .take(remove)
            .cloned()
            .collect();
        if removed == insert {
            return;
        }
        let before = (self.cursor, self.sel);
        self.splice(start, remove, insert.clone());
        self.undo.push(Undo {
            start,
            removed,
            inserted: insert,
            before,
            after: cursor,
        });
        self.redo.clear();
        self.dirty = true;
        self.cursor = cursor;
        self.sel = None;
        self.anchor = None;
        self.clamp();
        self.reveal = true;
    }

    /// 插件 (editor.write) 交来的行补丁: 范围 / 行数 / 类型校验通过才套用, 一步撤销。越界一律拒绝, 文档不动
    pub fn apply_line_patch(
        &mut self,
        start: usize,
        remove: usize,
        insert: Vec<String>,
        cursor: Option<[usize; 2]>,
    ) -> Result<(), String> {
        if self.readonly {
            return Err("只读文档, 插件的修改已忽略".into());
        }
        if start
            .checked_add(remove)
            .map_or(true, |e| e > self.lines.len())
        {
            return Err("插件返回的修改越界, 已拒绝".into());
        }
        if insert.len() > PATCH_MAX_LINES {
            return Err("插件返回的修改行数过多, 已拒绝".into());
        }
        // 光标默认落在补丁起点; 越界的光标夹回文档内
        let c = cursor.unwrap_or([start, 0]);
        let new_len = self.lines.len() - remove + insert.len();
        let insert = if new_len == 0 {
            vec![String::new()]
        } else {
            insert
        };
        let total = self.lines.len() - remove + insert.len();
        let row = c[0].min(total.saturating_sub(1));
        self.replace_rows(start, remove, insert, [row, c[1]]);
        Ok(())
    }

    /// 选区覆盖的整行 (含首尾); 选区终点在行首且跨行时不算那一行。没有选区 = None
    pub fn selected_rows(&self) -> Option<(usize, usize)> {
        let s = self.sel?;
        Some((
            s[0],
            if s[3] == 0 && s[2] > s[0] {
                s[2] - 1
            } else {
                s[2]
            },
        ))
    }

    /// 整行变换 (排序 / 去重 / 去空白…): 作用在选中的行, 没有选区 = 全文
    pub fn lines_op(&mut self, kind: &str) {
        let (r0, r1) = self
            .selected_rows()
            .unwrap_or((0, self.lines.len().saturating_sub(1)));
        let r1 = r1.min(self.lines.len().saturating_sub(1));
        let new = crate::cmds::lines_op(kind, self.lines[r0..=r1].to_vec());
        let n = new.len();
        self.replace_rows(
            r0,
            r1 - r0 + 1,
            if n == 0 { vec![String::new()] } else { new },
            [r0, 0],
        );
        self.tip = format!("已处理 {} 行", r1 - r0 + 1);
    }

    /// SPACE 缩进 / 反缩进 (同 Tauri `indent-space` / `unindent-space`, 语义是 editcore::edit_ops 里 Sakura 实测过的): 选中的行
    /// (没有选区 = 光标所在行) 行首加 1 个空格, 空行不动; 反向则去掉行首的 1 个空格 (没有就不动, 不碰 Tab)。没有选区时光标留在原来那一行, 列随缩进量移动; 有选区时缩进后仍选着这几行
    pub fn space_indent(&mut self, reverse: bool) {
        if self.readonly {
            self.tip = "只读: 不能修改 (编辑 → 只读 可以关掉)".into();
            return;
        }
        let (r0, r1) = self
            .selected_rows()
            .unwrap_or((self.cursor[0], self.cursor[0]));
        let r1 = r1.min(self.lines.len() - 1);
        let op = if reverse {
            "unindent-space"
        } else {
            "indent-space"
        };
        let Some(new) = editcore::edit_ops::transform_lines(op, &self.lines[r0..=r1]) else {
            return;
        };
        let had_sel = self.sel.is_some();
        let [row, col] = self.cursor;
        let i = row.clamp(r0, r1) - r0;
        let (old_len, new_len) = (self.lines[r0 + i].chars().count(), new[i].chars().count());
        let col = if new_len >= old_len {
            col + (new_len - old_len)
        } else {
            col.saturating_sub(old_len - new_len)
        };
        self.replace_rows(r0, r1 - r0 + 1, new, [row, col]);
        if had_sel {
            // 有选区时缩进后仍选着这几行 (Sakura 同样), 可以连续按
            let end = self.row_len(r1);
            self.sel = Some([r0, 0, r1, end]);
            self.anchor = Some([r0, 0]);
            self.cursor = [r1, end];
        }
        self.tip = format!(
            "SPACE {} {} 行",
            if reverse { "反缩进" } else { "缩进" },
            r1 - r0 + 1
        );
    }

    /// 下一个 / 上一个函数 (按大纲条目所在行, 到头绕回): 宏里的 `S_FuncListNext` 用; 没有函数标记返回 false
    pub fn step_function(&mut self, forward: bool) -> bool {
        let rows = crate::sidebar::outline_rows(&self.name, &self.lines);
        match crate::sidebar::step_row(&rows, self.cursor[0], forward) {
            Some(r) => {
                self.goto(r, 0);
                true
            }
            None => false,
        }
    }

    /// 选区文字变换 (大小写 / 全半角 / Tab 与空格); 没有选区什么也不做
    pub fn transform_sel(&mut self, kind: &str, tab: usize) {
        if self.sel.is_none() {
            self.tip = "先选中要转换的文字".into();
            return;
        }
        let (cursor, sel, anchor) = (self.cursor, self.sel, self.anchor);
        let Some(text) = self.edit("copy", None) else {
            return;
        };
        // copy 会把选区清掉; 放回去, paste 才是「替换选区」
        self.cursor = cursor;
        self.sel = sel;
        self.anchor = anchor;
        self.edit("paste", Some(crate::cmds::transform(kind, &text, tab)));
    }

    /// 一次缩进插入的文字: 制表符; 开了「Tab 插入空格」就补空格到下一个 Tab 位 (col = 当前列, 整行缩进传 0)
    pub fn indent_unit(&self, tab: usize, col: usize) -> String {
        if !self.insert_spaces {
            return "\t".into();
        }
        let t = tab.max(1);
        " ".repeat(t - col % t)
    }

    /// 缩进 / 反缩进: 多行选区时对每个非空行加 / 减一级; 单行且不是选区时 Tab 照常插入制表符
    pub fn indent(&mut self, outdent: bool, tab: usize) {
        let multi = self.selected_rows().map(|(a, b)| b > a).unwrap_or(false);
        if !outdent && !multi {
            let u = self.indent_unit(tab, self.cursor[1]);
            self.type_text(&u);
            return;
        }
        let (r0, r1) = self
            .selected_rows()
            .unwrap_or((self.cursor[0], self.cursor[0]));
        let r1 = r1.min(self.lines.len() - 1);
        let unit = self.indent_unit(tab, 0);
        let new: Vec<String> = self.lines[r0..=r1]
            .iter()
            .map(|l| {
                if !outdent {
                    if l.is_empty() {
                        String::new()
                    } else {
                        format!("{unit}{l}")
                    }
                } else if let Some(rest) = l.strip_prefix('\t') {
                    rest.to_string()
                } else {
                    let n = l.chars().take(tab.max(1)).take_while(|c| *c == ' ').count();
                    l.chars().skip(n).collect()
                }
            })
            .collect();
        let last_len = new.last().map(|l| l.chars().count()).unwrap_or(0);
        let keep_sel = multi;
        self.replace_rows(r0, r1 - r0 + 1, new, [r1, last_len]);
        if keep_sel {
            self.sel = Some([r0, 0, r1, last_len]);
            self.anchor = Some([r0, 0]);
        }
    }

    /// 三击: 选中整行 (含换行; 最后一行到行尾)
    pub fn select_line(&mut self, row: usize) {
        let row = row.min(self.lines.len() - 1);
        let end = if row + 1 < self.lines.len() {
            [row + 1, 0]
        } else {
            [row, self.lines[row].chars().count()]
        };
        self.box_sel = None;
        self.extra.clear();
        self.anchor = Some([row, 0]);
        self.sel = Some([row, 0, end[0], end[1]]);
        self.cursor = end;
        self.reveal = true;
    }

    pub fn type_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.contains('\n') || text.contains('\r') {
            let t = text.replace("\r\n", "\n").replace('\r', "\n");
            self.edit("paste", Some(t));
        } else {
            self.edit("insert-text", Some(text.to_string()));
        }
    }

    /// 移动键 (← → ↑ ↓ Home End PageUp PageDown, 含 Ctrl / Shift)
    pub fn nav(&mut self, key: &str, ctrl: bool, shift: bool, page_rows: usize) {
        let shift = shift || self.sel_mode; // F6 范围选择模式: 不按 Shift 也在扩选 (宏里记成带 Shift 的移动)
        if let Some(r) = self.rec.as_mut() {
            r.push(RecEv::Nav(key.to_string(), ctrl, shift));
        }
        self.box_sel = None;
        self.extra.clear();
        let anchor = if shift {
            self.anchor.or(Some(self.cursor)).filter(|_| true)
        } else {
            None
        };
        let anchor = match (shift, self.sel, self.anchor) {
            (true, Some(_), Some(a)) => Some(a),
            (true, _, _) => anchor,
            _ => None,
        };
        let req = NavReq {
            key: key.into(),
            ctrl,
            shift,
            row: self.cursor[0],
            col: self.cursor[1],
            goal_col: self.goal_col,
            sel: self.sel,
            anchor,
            total_rows: self.lines.len(),
            rows: self.rows_for(&[]),
            page_rows,
        };
        if let Some(r) = nav::step(&req) {
            self.cursor = [r.row, r.col];
            self.goal_col = r.goal_col;
            self.sel = r.sel;
            self.anchor = r.anchor;
            self.clamp();
            self.reveal = true;
        }
    }

    /// 命令存在才执行 (宏里用: 不认识的命令要报错而不是悄悄跳过)
    pub fn try_edit(&mut self, cmd: &str) -> bool {
        let known = edit_cmd::run(&EditReq {
            cmd: cmd.into(),
            row: 0,
            col: 0,
            total_rows: 1,
            rows: vec![(0, String::new())],
            ..Default::default()
        })
        .is_some();
        if known {
            self.edit(cmd, None);
        }
        known
    }

    /// 宏的「移动光标」: select 为真时从当前位置 (或锚点) 选到那里
    pub fn move_to(&mut self, row: usize, col: usize, select: bool) {
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        if select {
            let a = self.anchor.unwrap_or(self.cursor);
            self.anchor = Some(a);
            self.set_sel(a, [row, col]);
        } else {
            self.sel = None;
            self.anchor = None;
        }
        self.cursor = [row, col];
        self.reveal = true;
    }

    /// 选到上 / 下一个段落边界 (空行): 先跳过光标所在的空行, 再走到下一个空行的行首; 到头就选到文首 / 文末。连续按继续扩大
    pub fn select_para(&mut self, forward: bool) {
        let n = self.lines.len();
        let blank = |l: &String| l.trim().is_empty();
        let mut r = self.cursor[0].min(n - 1);
        let target = if forward {
            while r + 1 < n && blank(&self.lines[r]) {
                r += 1;
            }
            while r + 1 < n && !blank(&self.lines[r]) {
                r += 1;
            }
            if blank(&self.lines[r]) {
                [r, 0]
            } else {
                [n - 1, self.row_len(n - 1)]
            }
        } else {
            while r > 0 && blank(&self.lines[r]) {
                r -= 1;
            }
            while r > 0 && !blank(&self.lines[r]) {
                r -= 1;
            }
            if blank(&self.lines[r]) {
                [r, 0]
            } else {
                [0, 0]
            }
        };
        let a = self.anchor.unwrap_or(self.cursor);
        self.anchor = Some(a);
        self.set_sel(a, target);
        self.cursor = target;
        self.goal_col = None;
        self.reveal = true;
    }

    /// 跳到下 / 上一处改动 (相对打开 / 上次保存) 的起始行; 没有了返回 false
    pub fn step_changed(&mut self, forward: bool) -> bool {
        let rows = self.changed().to_vec();
        let start = |r: usize| rows[r] && (r == 0 || !rows[r - 1]);
        let cur = self.cursor[0];
        let hit = if forward {
            (cur + 1..rows.len()).find(|&r| start(r))
        } else {
            (0..cur).rev().find(|&r| start(r))
        };
        match hit {
            Some(r) => {
                self.goto(r, 0);
                true
            }
            None => false,
        }
    }

    /// 选到下一个 / 上一个改动块 (Sakura 的 (选择) 下一个 / 上一个变更行): 光标走到改动块起始行, 选区从原来的锚点 (没有选区就是原光标) 拉到那里
    pub fn step_changed_sel(&mut self, forward: bool) -> bool {
        let anchor = self
            .anchor
            .filter(|_| self.sel.is_some())
            .unwrap_or(self.cursor);
        if !self.step_changed(forward) {
            return false;
        }
        let to = self.cursor;
        self.cursor = anchor;
        self.anchor = Some(anchor);
        self.move_to(to[0], to[1], true);
        true
    }

    /// 选区文字, 没有选区 = 当前行 (复制类命令用)
    pub fn selection_or_line(&self) -> String {
        match self.sel {
            Some(r) => range_text(&self.lines, r),
            None => self.lines[self.cursor[0].min(self.lines.len() - 1)].clone(),
        }
    }

    pub fn select_all(&mut self) {
        self.sel_mode = false;
        let last = self.lines.len() - 1;
        self.sel = Some([0, 0, last, self.row_len(last)]);
        self.anchor = Some([0, 0]);
        self.cursor = [last, self.row_len(last)];
    }

    pub fn click(&mut self, row: usize, col: usize, extend: bool) {
        self.sel_mode = false;
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        if extend {
            let a = self.anchor.unwrap_or(self.cursor);
            self.anchor = Some(a);
            self.set_sel(a, [row, col]);
        } else {
            self.anchor = Some([row, col]);
            self.sel = None;
        }
        self.cursor = [row, col];
        self.goal_col = None;
    }

    pub fn box_start(&mut self, row: usize, col: usize) {
        self.sel_mode = false;
        let row = row.min(self.lines.len() - 1);
        self.sel = None;
        self.extra.clear();
        self.box_sel = Some([row, col, row, col]);
        self.cursor = [row, col];
    }

    /// 键盘扩展矩形选区 (Sakura 的 `*_BoxSel` 系列): 锚点是现有矩形的起点 (没有矩形就是光标), 光标按 `key` 走 `times` 步 (`ctrl` = 按词 / 到文首文末 / 半页),
    /// 矩形从锚点拉到新光标处。列按字符数算
    pub fn box_nav(&mut self, key: &str, ctrl: bool, times: usize) {
        let anchor = self.box_sel.map(|b| [b[0], b[1]]).unwrap_or(self.cursor);
        for _ in 0..times.max(1) {
            self.nav(key, ctrl, false, 20);
        }
        let [r, c] = self.cursor;
        self.box_start(anchor[0], anchor[1]);
        self.box_drag(r, c);
        self.cursor = [r, c];
    }

    pub fn box_drag(&mut self, row: usize, col: usize) {
        if let Some(b) = self.box_sel.as_mut() {
            b[2] = row.min(self.lines.len() - 1);
            b[3] = col;
            self.cursor = [
                b[2],
                col.min(self.lines.get(b[2]).map(|l| l.chars().count()).unwrap_or(0)),
            ];
        }
    }

    /// Ctrl+点击: 加 / 去一个光标
    pub fn toggle_cursor(&mut self, row: usize, col: usize) {
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        self.box_sel = None;
        self.sel = None;
        if let Some(i) = self.extra.iter().position(|c| *c == [row, col]) {
            self.extra.remove(i);
        } else if [row, col] != self.cursor {
            self.extra.push([row, col]);
        }
    }

    pub fn drag_to(&mut self, row: usize, col: usize) {
        let row = row.min(self.lines.len() - 1);
        let col = col.min(self.row_len(row));
        let a = self.anchor.unwrap_or(self.cursor);
        self.set_sel(a, [row, col]);
        self.cursor = [row, col];
    }

    fn set_sel(&mut self, a: [usize; 2], b: [usize; 2]) {
        self.sel = if a == b {
            None
        } else if (a[0], a[1]) <= (b[0], b[1]) {
            Some([a[0], a[1], b[0], b[1]])
        } else {
            Some([b[0], b[1], a[0], a[1]])
        };
    }

    /// F6: 范围选择模式开 / 关。开时以当前光标为锚点 (已有选区就接着它扩), 之后方向键扩选; 关时选区保留
    pub fn toggle_sel_mode(&mut self) {
        if self.sel_mode {
            self.sel_mode = false;
            self.tip = "范围选择模式: 关".into();
            return;
        }
        self.box_sel = None;
        self.extra.clear();
        if self.sel.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.sel_mode = true;
        self.tip = "范围选择模式: 用方向键扩选 (F6 / Esc 结束)".into();
    }

    /// 选区文字, 没有选区 = 整篇; 同时返回那个范围
    pub fn selection_or_all(&self) -> ([usize; 4], String) {
        let range = self.sel.unwrap_or_else(|| {
            let last = self.lines.len() - 1;
            [0, 0, last, self.row_len(last)]
        });
        (range, range_text(&self.lines, range))
    }

    /// 把选区 (没有选区 = 整篇) 的文字交给 `f` 换成新文字, 一步撤销, 结果保持选中 (Sakura 的 コード変換 / 变换类命令)
    pub fn convert_selection(&mut self, f: impl FnOnce(&str) -> Result<String, String>) -> bool {
        if self.readonly {
            self.tip = "只读: 不能修改 (编辑 → 只读 可以关掉)".into();
            return false;
        }
        let (range, text) = self.selection_or_all();
        let out = match f(&text) {
            Ok(o) => o,
            Err(e) => {
                self.tip = e;
                return false;
            }
        };
        self.box_sel = None;
        self.extra.clear();
        self.anchor = Some([range[0], range[1]]);
        self.sel = Some(range);
        self.cursor = [range[2], range[3]];
        self.edit("paste", Some(out));
        let end = self.cursor;
        self.set_sel([range[0], range[1]], end);
        self.anchor = Some([range[0], range[1]]);
        true
    }

    /// 在光标处插入一个控制字符 (LF / CR 当成换行; 其他原样插入)
    pub fn insert_control(&mut self, c: char) {
        match c {
            '\n' | '\r' => {
                self.edit("new-line", None);
            }
            c => {
                self.edit("insert-text", Some(c.to_string()));
            }
        }
    }

    /// 在光标处插入戳记: 行首 = 缩进之后 `[戳记] ` (整行一步撤销); 光标处 = 直接插入
    pub fn insert_stamp(&mut self, stamp: &str, pos: crate::textconv::StampPos) {
        if self.readonly {
            self.tip = "只读: 不能修改 (编辑 → 只读 可以关掉)".into();
            return;
        }
        match pos {
            crate::textconv::StampPos::AtCursor => {
                self.edit("paste", Some(stamp.to_string()));
            }
            crate::textconv::StampPos::LineStart => {
                let r = self.cursor[0];
                let (line, n) = crate::textconv::stamp_line_start(&self.lines[r], stamp);
                let indent = self.lines[r]
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .count();
                let col = if self.cursor[1] >= indent {
                    self.cursor[1] + n
                } else {
                    self.cursor[1]
                };
                self.replace_rows(r, 1, vec![line], [r, col]);
            }
        }
        self.tip = format!("已插入戳记: {stamp}");
    }

    /// 增量查找 (带方向): 从 `start` 起向前 / 向后找第一处, 命中本身在 start 上也算
    pub fn find_from_dir(
        &mut self,
        needle: &str,
        o: FindOpts,
        forward: bool,
        start: [usize; 2],
    ) -> Result<bool, String> {
        self.find_at(needle, o, forward, start, false)
    }

    #[cfg(test)]
    pub fn undo_is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    pub fn undo(&mut self) {
        let Some(u) = self.undo.pop() else {
            self.tip = "没有可撤销的操作".into();
            return;
        };
        self.box_sel = None;
        self.extra.clear();
        self.splice(u.start, u.inserted.len(), u.removed.clone());
        self.cursor = u.before.0;
        self.sel = u.before.1;
        self.anchor = None;
        self.redo.push(u);
        self.dirty = true;
        self.clamp();
        self.reveal = true;
    }

    pub fn redo(&mut self) {
        let Some(u) = self.redo.pop() else {
            self.tip = "没有可重做的操作".into();
            return;
        };
        self.box_sel = None;
        self.extra.clear();
        self.splice(u.start, u.removed.len(), u.inserted.clone());
        self.cursor = u.after;
        self.sel = None;
        self.anchor = None;
        self.undo.push(u);
        self.dirty = true;
        self.clamp();
        self.reveal = true;
    }

    // ---------------- 书签 (editcore::bookmark) ----------------
    pub fn bookmark(&mut self, op: &str) {
        let req = BmReq {
            op: op.into(),
            list: self.bookmarks.clone(),
            row: self.cursor[0],
            col: 0,
        };
        let Some(res) = bookmark::run(&req) else {
            return;
        };
        if let Some(l) = res.list {
            self.bookmarks = l;
            self.tip = if res.added {
                "已设置书签".into()
            } else {
                "已取消书签".into()
            };
        }
        if let Some(t) = res.target {
            self.goto(t.row, 0);
            self.tip = format!("书签 {}/{}", res.index + 1, res.count);
        } else if op != "toggle" {
            self.tip = "没有书签".into();
        }
    }

    // ---------------- 键盘宏 (录制 / 回放: 把录下的编辑和移动原样再走一遍 editcore) ----------------
    pub fn macro_toggle(&mut self) {
        match self.rec.take() {
            Some(r) => {
                self.tip = format!("宏录制结束 ({} 步) — Ctrl+Shift+L 回放", r.len());
                self.last_macro = r;
            }
            None => {
                self.rec = Some(vec![]);
                self.tip = "● 录制宏… 再按 Ctrl+Shift+M 结束".into();
            }
        }
    }

    pub fn macro_play(&mut self) {
        let m = self.last_macro.clone();
        if m.is_empty() {
            self.tip = "还没有录下的宏".into();
            return;
        }
        for ev in m {
            match ev {
                RecEv::Edit(c, t) => {
                    self.edit(&c, t);
                }
                RecEv::Nav(k, c, s) => self.nav(&k, c, s, 20),
            }
        }
        self.tip = "宏已回放".into();
    }

    // ---------------- 查找 ----------------
    fn search_req(&self, op: &str, needle: &str, o: FindOpts) -> SearchReq {
        // Migemo: 查找串换成转义过的候选正则 (不能和用户自己的正则 / 单词单位混用)
        let (needle, regex, word) = match o
            .migemo
            .then(|| crate::find::migemo_regex(needle))
            .flatten()
        {
            Some(p) => (p, true, false),
            None => (needle.to_string(), o.regex, o.word),
        };
        SearchReq {
            op: op.into(),
            needle,
            match_case: o.case,
            use_regex: regex,
            whole_word: word,
            wrap: o.wrap,
            lines: self.lines.clone(),
            ..Default::default()
        }
    }

    /// 正则写错了的说明 (没写错 / 没开正则 / 查找串空 = None); 查找栏边输入边提示
    pub fn find_error(&self, needle: &str, o: FindOpts) -> Option<String> {
        if needle.is_empty() || !o.regex {
            return None;
        }
        let mut req = self.search_req("validate", needle, o);
        req.lines = vec![]; // 只编译, 不需要正文
        search_ops::run(&req).and_then(|r| r.error)
    }

    /// 全部命中 (按位置排序, 最多 5001 处); 查找串空 = 没有
    pub fn find_hits(&self, needle: &str, o: FindOpts) -> Result<Vec<Hit>, String> {
        if needle.is_empty() {
            return Ok(vec![]);
        }
        let res = search_ops::run(&self.search_req("hits", needle, o)).unwrap_or_default();
        match res.error {
            Some(e) => Err(e),
            None => Ok(res.hits),
        }
    }

    fn select_hit(&mut self, h: &Hit) {
        self.anchor = Some([h.row, h.col]);
        self.sel = Some([h.row, h.col, h.row, h.col + h.len]);
        self.cursor = [h.row, h.col + h.len];
        self.reveal = true;
    }

    /// 从 `pos` 起找下一处 / 上一处; `on_sel` = pos 正是当前选中的命中的起点 (从它的"下一处"找)。找到就选中它
    fn find_at(
        &mut self,
        needle: &str,
        o: FindOpts,
        forward: bool,
        pos: [usize; 2],
        on_sel: bool,
    ) -> Result<bool, String> {
        if needle.is_empty() {
            return Ok(false);
        }
        let mut req = self.search_req("step", needle, o);
        req.row = pos[0];
        req.col = pos[1];
        req.forward = forward;
        req.sel_start = if on_sel { Some(pos) } else { None };
        let Some(res) = search_ops::run(&req) else {
            return Ok(false);
        };
        if let Some(e) = res.error {
            return Err(e);
        }
        match res.hit {
            Some(h) => {
                self.select_hit(&h);
                self.tip = format!(
                    "{}/{}{}",
                    res.index + 1,
                    if res.total > 5000 {
                        "5000+".to_string()
                    } else {
                        res.total.to_string()
                    },
                    if res.wrapped { " (已折返)" } else { "" }
                );
                Ok(true)
            }
            None => {
                // 一处也没有 / 有, 但按"不折返"已经到头了: 提示不一样
                self.tip = if res.total == 0 {
                    format!("没有找到: {needle}")
                } else if forward {
                    "已到文件末尾 (未折返)".into()
                } else {
                    "已到文件开头 (未折返)".into()
                };
                Ok(false)
            }
        }
    }

    /// 从光标 (有选区 = 选区起点) 找下一处 / 上一处; 找到就选中它
    pub fn find_step(&mut self, needle: &str, o: FindOpts, forward: bool) -> Result<bool, String> {
        let (pos, on_sel) = match self.sel {
            Some(s) => ([s[0], s[1]], true),
            None => (self.cursor, false),
        };
        self.find_at(needle, o, forward, pos, on_sel)
    }

    /// 增量查找: 从 `start` 起向后找第一处 (命中本身在 start 上也算); 不把 start 当成"当前命中"
    pub fn find_from(
        &mut self,
        needle: &str,
        o: FindOpts,
        start: [usize; 2],
    ) -> Result<bool, String> {
        self.find_at(needle, o, true, start, false)
    }

    /// 单行选区的文字 (查找框预填用); 没有选区 / 跨行 / 空 = None
    pub fn selected_text_single(&self) -> Option<String> {
        let s = self.sel?;
        if s[0] != s[2] || s[3] <= s[1] {
            return None;
        }
        Some(
            self.lines
                .get(s[0])?
                .chars()
                .skip(s[1])
                .take(s[3] - s[1])
                .collect(),
        )
    }

    /// 光标所在的单词 (Ctrl+F3 用); 光标不在词上 = None
    pub fn word_at_cursor(&self) -> Option<String> {
        use editcore::word::is_word_char;
        let cs: Vec<char> = self.lines.get(self.cursor[0])?.chars().collect();
        let c = self.cursor[1].min(cs.len());
        let (mut a, mut b) = (c, c);
        while a > 0 && is_word_char(cs[a - 1]) {
            a -= 1;
        }
        while b < cs.len() && is_word_char(cs[b]) {
            b += 1;
        }
        if a == b {
            None
        } else {
            Some(cs[a..b].iter().collect())
        }
    }

    /// 置换: 选中的刚好是一处命中就换掉它 ($1 \1 $& 在正则下展开), 然后跳到下一处; 选中的不是命中就只是找下一处。返回是否真换了
    pub fn replace_one(&mut self, needle: &str, repl: &str, o: FindOpts) -> Result<bool, String> {
        if self.readonly {
            self.tip = "只读文档不能置换".into();
            return Ok(false);
        }
        if needle.is_empty() {
            return Ok(false);
        }
        let mut replaced = false;
        if let Some(seg) = self.selected_text_single() {
            let s = self.sel.unwrap_or_default();
            let mut req = self.search_req("match_expand", needle, o);
            req.lines = vec![];
            req.seg = seg;
            req.repl = repl.into();
            // 没匹配上 = None; 只匹配了一部分 = text 为 None: 都当作"选中的不是一处命中"
            if let Some(res) = search_ops::run(&req) {
                if let Some(e) = res.error {
                    return Err(e);
                }
                if let Some(text) = res.text {
                    let line = &self.lines[s[0]];
                    let new: String = line
                        .chars()
                        .take(s[1])
                        .chain(text.chars())
                        .chain(line.chars().skip(s[3]))
                        .collect();
                    let cur = [s[0], s[1] + text.chars().count()];
                    self.replace_rows(s[0], 1, vec![new], cur);
                    replaced = true;
                }
            }
        }
        let found = self.find_step(needle, o, true)?;
        if replaced {
            self.tip = if found {
                format!("置换了 1 处; {}", self.tip)
            } else {
                "置换了 1 处; 后面没有了".into()
            };
        }
        Ok(replaced)
    }

    /// 全部置换 (`in_sel` 且有选区 = 只换选区里的); 一步撤销, 光标放到最后一处置换之后。返回置换数
    pub fn replace_all(
        &mut self,
        needle: &str,
        repl: &str,
        o: FindOpts,
        in_sel: bool,
    ) -> Result<usize, String> {
        if self.readonly {
            self.tip = "只读文档不能置换".into();
            return Ok(0);
        }
        if needle.is_empty() {
            return Ok(0);
        }
        let mut req = self.search_req("replace_all", needle, o);
        req.repl = repl.into();
        if in_sel {
            req.sel = self.sel;
        }
        let Some(res) = search_ops::run(&req) else {
            return Ok(0);
        };
        if let Some(e) = res.error {
            return Err(e);
        }
        if res.count == 0 {
            self.tip = format!("没有找到: {needle}");
            return Ok(0);
        }
        // 撤销记录只覆盖改动过的那段行 (不是整篇)
        let first = res.rows.iter().map(|r| r.0).min().unwrap_or(0);
        let last = res.rows.iter().map(|r| r.0).max().unwrap_or(0);
        let removed: Vec<String> = self.lines[first..=last].to_vec();
        for (r, t) in res.rows {
            if r < self.lines.len() {
                self.lines[r] = t;
            }
        }
        let inserted: Vec<String> = self.lines[first..=last].to_vec();
        let before = (self.cursor, self.sel);
        let cur = [res.last_row.unwrap_or(first), res.last_end];
        self.cursor = cur;
        self.sel = None;
        self.anchor = None;
        self.rev += 1;
        self.undo.push(Undo {
            start: first,
            removed,
            inserted,
            before,
            after: cur,
        });
        self.redo.clear();
        self.dirty = true;
        self.reveal = true;
        self.tip = format!("置换了 {} 处", res.count);
        Ok(res.count)
    }
}

/// [r0,c0)-[r1,c1) 范围内的文字 (行间用 LF; 列是字符下标, 越界夹回行尾)
pub fn range_text(lines: &[String], s: [usize; 4]) -> String {
    let mut out = String::new();
    for r in s[0]..=s[2].min(lines.len().saturating_sub(1)) {
        let chars: Vec<char> = lines[r].chars().collect();
        let a = if r == s[0] { s[1].min(chars.len()) } else { 0 };
        let b = if r == s[2] {
            s[3].min(chars.len())
        } else {
            chars.len()
        };
        if r > s[0] {
            out.push('\n');
        }
        out.extend(&chars[a..b.max(a)]);
    }
    out
}

/// 把一行里的 Tab 展开成空格 (到下一个 Tab 位), 并给出 "字符下标 -> 显示列" 的映射 (长度 = 字符数 + 1)
pub fn expand_tabs(line: &str, tab: usize) -> (String, Vec<usize>) {
    let tab = tab.max(1);
    let mut out = String::with_capacity(line.len());
    let mut map = Vec::with_capacity(line.len() + 1);
    let mut col = 0usize;
    for ch in line.chars() {
        map.push(col);
        if ch == '\t' {
            let n = tab - (col % tab);
            out.extend(std::iter::repeat(' ').take(n));
            col += n;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    map.push(col);
    (out, map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(lines: &[&str]) -> Doc {
        let mut d = Doc::untitled(1, 1);
        d.lines = lines.iter().map(|s| s.to_string()).collect();
        d
    }

    #[test]
    fn plugin_line_patch_is_validated_and_one_undo_step() {
        let mut d = doc(&["a", "b", "c"]);
        assert!(d.apply_line_patch(2, 2, vec![], None).is_err(), "越界拒绝");
        assert!(
            d.apply_line_patch(usize::MAX, 2, vec![], None).is_err(),
            "溢出拒绝"
        );
        assert!(
            d.apply_line_patch(0, 0, vec!["x".to_string(); PATCH_MAX_LINES + 1], None)
                .is_err(),
            "行数过多拒绝"
        );
        assert_eq!(d.lines, vec!["a", "b", "c"], "被拒绝的补丁不改文档");
        d.apply_line_patch(1, 1, vec!["B1".into(), "B2".into()], Some([3, 1]))
            .unwrap();
        assert_eq!(d.lines, vec!["a", "B1", "B2", "c"]);
        assert_eq!(d.cursor, [3, 1]);
        d.undo();
        assert_eq!(d.lines, vec!["a", "b", "c"], "一步撤销");
        d.readonly = true;
        assert!(d.apply_line_patch(0, 1, vec![], None).is_err(), "只读拒绝");
        d.readonly = false;
        d.apply_line_patch(0, 3, vec![], Some([9, 0])).unwrap();
        assert_eq!(d.lines, vec![""], "删光后至少留一个空行");
    }

    #[test]
    fn typing_enter_backspace_go_through_editcore() {
        let mut d = doc(&[""]);
        d.type_text("a");
        d.type_text("b");
        assert_eq!(d.lines, vec!["ab"]);
        d.edit("new-line", None);
        assert_eq!(
            (d.lines.clone(), d.cursor),
            (vec!["ab".to_string(), "".to_string()], [1, 0])
        );
        d.edit("backspace", None);
        assert_eq!(
            (d.lines.clone(), d.cursor),
            (vec!["ab".to_string()], [0, 2])
        );
    }

    #[test]
    fn consecutive_typing_is_one_undo_step() {
        let mut d = doc(&[""]);
        for c in ["h", "e", "l", "l", "o"] {
            d.type_text(c);
        }
        assert_eq!(d.lines[0], "hello");
        d.undo();
        assert_eq!(d.lines[0], "", "整段键入一步撤销");
        d.redo();
        assert_eq!(d.lines[0], "hello");
    }

    #[test]
    fn nav_keys_use_sakura_semantics_and_selection() {
        let mut d = doc(&["foo bar", "x"]);
        d.nav("ArrowRight", true, false, 10); // Ctrl+→ = 下一个词
        assert_eq!(d.cursor, [0, 4]);
        d.nav("End", false, true, 10); // Shift+End 选到行尾
        assert_eq!(d.sel, Some([0, 4, 0, 7]));
        d.nav("ArrowDown", false, false, 10);
        assert_eq!(d.sel, None);
    }

    #[test]
    fn find_and_replace_all() {
        let o = FindOpts::default();
        let mut d = doc(&["foo Foo", "foo"]);
        assert!(d.find_step("foo", o, true).unwrap());
        assert_eq!(d.sel, Some([0, 0, 0, 3]));
        assert!(d.find_step("foo", o, true).unwrap());
        assert_eq!(d.sel, Some([0, 4, 0, 7]));
        assert_eq!(d.replace_all("foo", "X", o, false).unwrap(), 3);
        assert_eq!(d.lines, vec!["X X", "X"]);
        assert_eq!(d.cursor, [1, 1], "光标在最后一处置换之后");
        d.undo();
        assert_eq!(d.lines, vec!["foo Foo", "foo"]);
    }

    #[test]
    fn find_step_backward_whole_word_and_no_wrap() {
        let mut d = doc(&["foo foobar foo", "x"]);
        let w = FindOpts {
            word: true,
            ..FindOpts::default()
        };
        assert!(d.find_step("foo", w, true).unwrap());
        assert!(d.find_step("foo", w, true).unwrap());
        assert_eq!(d.sel, Some([0, 11, 0, 14]), "foobar 里的 foo 不算整词");
        assert!(
            d.find_step("foo", w, false).unwrap(),
            "往回找 = 第一处, 不会又停在当前这一处"
        );
        assert_eq!(d.sel, Some([0, 0, 0, 3]));
        // 不折返: 到头说"到头", 没有这个词说"没有找到"
        let n = FindOpts { wrap: false, ..w };
        assert!(!d.find_step("foo", n, false).unwrap());
        assert!(d.tip.contains("文件开头"), "{}", d.tip);
        assert!(!d.find_step("zzz", n, true).unwrap());
        assert!(d.tip.contains("没有找到"), "{}", d.tip);
        d.sel = Some([0, 11, 0, 14]);
        assert!(!d.find_step("foo", n, true).unwrap());
        assert!(d.tip.contains("文件末尾"), "{}", d.tip);
        assert!(d
            .find_step("foo", FindOpts { wrap: true, ..n }, true)
            .unwrap());
        assert!(d.tip.contains("已折返"), "{}", d.tip);
    }

    #[test]
    fn replace_all_in_selection_only_and_places_cursor() {
        let o = FindOpts::default();
        let mut d = doc(&["a a a", "a a a", "a a a"]);
        d.sel = Some([0, 2, 1, 3]); // 第 1 行从第 3 列起, 到第 2 行第 4 列
        assert_eq!(
            d.replace_all("a", "b", o, true).unwrap(),
            4,
            "选区里只有这 4 个"
        );
        assert_eq!(d.lines, vec!["a b b", "b b a", "a a a"]);
        assert_eq!(d.cursor, [1, 3]);
        assert_eq!(d.sel, None);
        d.undo();
        assert_eq!(d.lines, vec!["a a a", "a a a", "a a a"]);
        assert_eq!(d.replace_all("a", "z", o, false).unwrap(), 9);
        assert_eq!(d.cursor, [2, 5]);
        assert_eq!(d.replace_all("nope", "z", o, false).unwrap(), 0);
        d.readonly = true;
        assert_eq!(
            d.replace_all("z", "y", o, false).unwrap(),
            0,
            "只读文档不置换"
        );
        assert_eq!(d.lines[0], "z z z");
    }

    #[test]
    fn replace_one_expands_captures_and_ignores_non_hit_selections() {
        let rx = FindOpts {
            regex: true,
            ..FindOpts::default()
        };
        let mut d = doc(&["key=value", "other"]);
        d.sel = Some([0, 0, 0, 9]);
        assert!(d.replace_one(r"(\w+)=(\w+)", "$2:$1", rx).unwrap());
        assert_eq!(d.lines[0], "value:key");
        // 选中的只是命中的一部分 = 不是一处命中: 不置换, 只找下一个
        let mut d2 = doc(&["foobar foo"]);
        d2.sel = Some([0, 0, 0, 3]);
        let w = FindOpts {
            word: true,
            ..FindOpts::default()
        };
        assert!(
            !d2.replace_one("foobar", "X", w).unwrap(),
            "foo 不是 foobar 的命中"
        );
        assert_eq!(d2.lines[0], "foobar foo");
        let mut d3 = doc(&["abc abc"]);
        d3.sel = Some([0, 0, 0, 2]); // "ab" 不是 "abc" 的完整命中
        assert!(!d3.replace_one("abc", "X", FindOpts::default()).unwrap());
        assert_eq!(d3.lines[0], "abc abc");
        assert_eq!(d3.sel, Some([0, 4, 0, 7]), "只是跳到下一处");
        // 普通模式: $1 原样当文字
        let mut d4 = doc(&["a"]);
        d4.sel = Some([0, 0, 0, 1]);
        d4.replace_one("a", "$1", FindOpts::default()).unwrap();
        assert_eq!(d4.lines[0], "$1");
    }

    #[test]
    fn find_helpers_selection_text_word_and_validation() {
        let mut d = doc(&["let alpha_1 = 2;"]);
        d.cursor = [0, 6];
        assert_eq!(d.word_at_cursor().as_deref(), Some("alpha_1"));
        d.cursor = [0, 3]; // 空格后一个字符: 光标紧挨着 let 的末尾
        assert_eq!(d.word_at_cursor().as_deref(), Some("let"));
        d.cursor = [0, 12];
        assert_eq!(d.word_at_cursor(), None);
        d.sel = Some([0, 4, 0, 11]);
        assert_eq!(d.selected_text_single().as_deref(), Some("alpha_1"));
        let rx = FindOpts {
            regex: true,
            ..FindOpts::default()
        };
        assert!(d.find_error("(", rx).is_some());
        assert!(
            d.find_error("(", FindOpts::default()).is_none(),
            "没开正则不检查"
        );
        assert!(d.find_error("a+", rx).is_none());
        assert_eq!(d.find_hits("a", FindOpts::default()).unwrap().len(), 2);
        assert!(d.find_hits("(", rx).is_err());
    }

    #[test]
    fn tabs_expand_to_tab_stops() {
        let (s, m) = expand_tabs("a\tb", 4);
        assert_eq!(s, "a   b");
        assert_eq!(m, vec![0, 1, 4, 5]);
    }

    #[test]
    fn bookmarks_toggle_navigate_and_follow_line_edits() {
        let mut d = doc(&["a", "b", "c", "d"]);
        d.goto(1, 0);
        d.bookmark("toggle");
        d.goto(3, 0);
        d.bookmark("toggle");
        d.goto(0, 0);
        d.bookmark("next");
        assert_eq!(d.cursor[0], 1);
        d.bookmark("next");
        assert_eq!(d.cursor[0], 3);
        d.goto(0, 0);
        d.edit("add-line-above", None); // 上面多一行: 书签整体下移
        assert_eq!(
            d.bookmarks.iter().map(|b| b.row).collect::<Vec<_>>(),
            vec![2, 4]
        );
    }

    #[test]
    fn box_selection_types_into_every_row() {
        let mut d = doc(&["abcd", "efgh", "ijkl"]);
        d.box_start(0, 1);
        d.box_drag(2, 1);
        d.type_text("X");
        assert_eq!(d.lines, vec!["aXbcd", "eXfgh", "iXjkl"]);
        d.undo();
        assert_eq!(d.lines, vec!["abcd", "efgh", "ijkl"]);
    }

    #[test]
    fn multi_cursor_inserts_everywhere_in_one_undo_step() {
        let mut d = doc(&["ab", "cd"]);
        d.goto(0, 0);
        d.toggle_cursor(1, 0);
        d.type_text("-");
        assert_eq!(d.lines, vec!["-ab", "-cd"]);
        assert_eq!(d.extra.len(), 1);
        d.edit("backspace", None);
        assert_eq!(d.lines, vec!["ab", "cd"]);
        d.undo();
        d.undo();
        assert_eq!(d.lines, vec!["ab", "cd"]);
    }

    #[test]
    fn keyboard_macro_records_and_replays() {
        let mut d = doc(&["x", "y"]);
        d.macro_toggle();
        d.nav("End", false, false, 10);
        d.type_text("!");
        d.nav("ArrowDown", false, false, 10);
        d.macro_toggle();
        assert_eq!(d.lines, vec!["x!", "y"]);
        d.goto(0, 0);
        d.macro_play();
        assert_eq!(d.lines[0], "x!!");
        d.macro_play();
        assert_eq!(d.lines[1], "y!", "第二次回放是在第 2 行接着做");
    }

    #[test]
    fn indentation_folds_hide_rows_and_unfold_when_the_cursor_enters() {
        let mut d = doc(&[
            "fn a() {", "    x", "", "    y", "}", "fn b() {", "    z", "}",
        ]);
        assert_eq!(d.fold_range_at(0), Some((0, 3)));
        assert_eq!(d.fold_range_at(1), None);
        d.toggle_fold(0);
        assert!(d.is_hidden(1) && d.is_hidden(3) && !d.is_hidden(4) && !d.is_hidden(0));
        d.goto(2, 0); // 光标进折叠范围 -> 展开
        assert!(d.folds.is_empty());
        d.toggle_fold(5);
        d.edit("add-line-below", None); // 行数变了: 折叠清掉
        assert!(d.folds.is_empty());
    }

    #[test]
    fn line_ops_indent_readonly_and_overwrite() {
        let mut d = Doc::untitled(1, 1);
        d.type_text(
            "b
a
b
",
        );
        d.select_all();
        d.lines_op("sort-asc");
        assert_eq!(d.lines, vec!["a", "b", "b", ""], "选区止于行首的那一行不算");
        d.select_all();
        d.lines_op("unique");
        assert_eq!(d.lines, vec!["a", "b", ""]);
        d.undo();
        assert_eq!(d.lines, vec!["a", "b", "b", ""], "整行变换是一步撤销");
        // 多行选区 Tab 缩进 / Shift+Tab 反缩进
        d.select_all();
        d.indent(false, 4);
        assert_eq!(d.lines, vec!["	a", "	b", "	b", ""]);
        d.indent(true, 4);
        assert_eq!(d.lines, vec!["a", "b", "b", ""]);
        // 选区大小写
        d.goto(0, 0);
        d.nav("ArrowRight", false, true, 10);
        d.transform_sel("upper", 4);
        assert_eq!(d.lines[0], "A");
        // 只读: 改不了
        d.readonly = true;
        let before = d.lines.clone();
        d.type_text("zzz");
        assert_eq!(d.lines, before);
        d.readonly = false;
        // 改写: 吃掉光标后的字符
        d.goto(1, 0);
        d.overwrite = true;
        d.type_text("X");
        assert_eq!(d.lines[1], "X");
    }

    #[test]
    fn binary_files_are_refused_and_external_changes_are_detected() {
        let d = std::env::temp_dir().join(format!("ide-fast-ext-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("bin.dat"), [1u8, 0, 2, 0, 3]).unwrap();
        assert!(Doc::open(1, &d.join("bin.dat"))
            .err()
            .unwrap()
            .contains("二进制"));
        let f = d.join("a.txt");
        std::fs::write(
            &f, "one
",
        )
        .unwrap();
        let mut doc = Doc::open(1, &f).unwrap();
        assert!(!doc.changed_on_disk());
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(
            &f, "two two
",
        )
        .unwrap();
        assert!(doc.changed_on_disk(), "大小变了就算被外部改过");
        doc.group = 3;
        doc.reload().unwrap();
        assert_eq!(doc.lines[0], "two two");
        assert_eq!(doc.group, 3, "重载保留标签分组");
        assert!(!doc.changed_on_disk());
        let _ = std::fs::remove_dir_all(&d);
    }
    #[test]
    fn changed_lines_follow_edits_and_reset_on_save() {
        let mut d = doc(&["a", "b", "c"]);
        d.base = d.lines.clone();
        assert_eq!(d.changed(), &[false, false, false]);
        d.lines[1] = "B".into();
        d.rev += 1;
        assert_eq!(d.changed(), &[false, true, false]);
        let p = std::env::temp_dir().join(format!("ide-fast-chg-{}.txt", std::process::id()));
        d.save_to(&p).unwrap();
        assert_eq!(d.changed(), &[false, false, false], "保存后基线更新");
        let _ = std::fs::remove_file(&p);
    }
    #[test]
    fn eol_charset_bom_changes_mark_dirty_and_follow_rules() {
        let mut d = doc(&["a"]);
        d.eol = Eol::Crlf;
        d.set_eol(Eol::Lf);
        assert!(d.dirty && d.eol == Eol::Lf);
        d.dirty = false;
        d.set_eol(Eol::Lf);
        assert!(!d.dirty, "没变就不算修改");
        d.set_charset(Charset::Utf16Le);
        assert_eq!(d.bom, Bom::Utf16Le);
        d.set_charset(Charset::Utf8);
        assert_eq!(d.bom, Bom::None);
        d.toggle_bom();
        assert_eq!(d.bom, Bom::Utf8);
        d.set_charset(Charset::Gbk);
        d.toggle_bom();
        assert_eq!(d.bom, Bom::None, "GBK 没有可选 BOM");
    }

    #[test]
    fn reopen_with_another_encoding_rereads_the_file() {
        let dir = std::env::temp_dir().join(format!("ide-fast-reopen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("g.txt");
        std::fs::write(&f, [0xD6u8, 0xD0, 0xCE, 0xC4, b'a', b'b', b'c']).unwrap(); // "中文abc" 的 GBK 字节
        let mut d = Doc::open(1, &f).unwrap();
        d.group = 2;
        d.reopen_as(Charset::Gbk).unwrap();
        assert_eq!(d.lines, vec!["中文abc"]);
        assert_eq!((d.charset, d.group), (Charset::Gbk, 2));
        let mut u = Doc::untitled(2, 1);
        assert!(u.reopen_as(Charset::Utf8).is_err());
        assert!(d.properties().contains("GBK"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn indent_preferences_reach_the_editing_commands() {
        // Tab 插入空格: 补到下一个 Tab 位; 整行缩进用满一个 Tab 宽度
        let mut d = doc(&["ab"]);
        d.insert_spaces = true;
        d.goto(0, 2);
        d.indent(false, 4);
        assert_eq!(d.lines[0], "ab  ");
        let mut m = doc(&["x", "", "y"]);
        m.insert_spaces = true;
        m.sel = Some([0, 0, 2, 1]);
        m.indent(false, 4);
        assert_eq!(m.lines, vec!["    x", "", "    y"], "空行不缩进");
        let mut t = doc(&["ab"]);
        t.goto(0, 2);
        t.indent(false, 4);
        assert_eq!(t.lines[0], "ab\t", "默认仍是制表符");
        // 自动缩进开 / 关: 回车后新行是否带上一行的缩进
        let mut on = doc(&["    foo"]);
        on.goto(0, 7);
        on.edit("new-line", None);
        assert_eq!(on.lines[1], "    ", "自动缩进开 = 沿用上一行缩进");
        let mut off = doc(&["    foo"]);
        off.auto_indent = false;
        off.goto(0, 7);
        off.edit("new-line", None);
        assert_eq!(off.lines[1], "", "自动缩进关 = 新行顶格");
    }

    #[test]
    fn bracket_pair_and_jump() {
        let mut d = doc(&["fn a(x) {", "  y[0]", "}"]);
        d.goto(0, 8);
        assert_eq!(d.bracket_pair(), Some(([0, 8], [2, 0])));
        assert!(d.jump_to_pair());
        assert_eq!(d.cursor, [2, 0]);
        d.goto(1, 0);
        assert!(!d.jump_to_pair());
        assert_eq!(d.cursor, [1, 0]);
    }

    #[test]
    fn paragraph_selection_grows_to_blank_lines_and_ends() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec![
            "a".into(),
            "b".into(),
            "".into(),
            "c".into(),
            "d".into(),
            "".into(),
            "e".into(),
        ];
        d.goto(0, 1);
        d.select_para(true);
        assert_eq!(d.sel, Some([0, 1, 2, 0]), "到第一个空行的行首");
        d.select_para(true);
        assert_eq!(
            d.sel,
            Some([0, 1, 5, 0]),
            "连续按: 跳过空行, 扩到下一个空行"
        );
        d.select_para(true);
        assert_eq!(d.sel, Some([0, 1, 6, 1]), "最后一段没有空行 = 选到文末");
        d.goto(6, 0);
        d.select_para(false);
        assert_eq!(d.sel, Some([5, 0, 6, 0]));
        d.goto(1, 0);
        d.select_para(false);
        assert_eq!(d.sel, Some([0, 0, 1, 0]), "向上到文首");
    }

    #[test]
    fn space_indent_follows_the_sakura_semantics() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["a".into(), "  b".into(), "\tc".into(), "".into()];
        d.goto(1, 3);
        d.space_indent(false);
        assert_eq!(d.lines[1], "   b", "行首加 1 个空格 (不是 Tab 宽度个)");
        assert_eq!(d.cursor, [1, 4], "光标列跟着缩进量走");
        d.space_indent(true);
        assert_eq!(d.lines[1], "  b");
        assert_eq!(d.cursor, [1, 3]);
        d.goto(0, 0);
        d.space_indent(true);
        assert_eq!(d.lines[0], "a", "行首没有空格: 不动");
        d.sel = Some([0, 0, 3, 0]);
        d.anchor = Some([0, 0]);
        d.space_indent(false);
        assert_eq!(
            &d.lines[..],
            [" a", "   b", " \tc", ""],
            "选中的每一行加 1 个空格, 空行保持空行"
        );
        assert!(d.sel.is_some(), "缩进后仍选着这几行, 可以连续按");
        d.space_indent(true);
        assert_eq!(
            &d.lines[..],
            ["a", "  b", "\tc", ""],
            "反缩进只去 1 个空格, 不碰 Tab"
        );
        d.readonly = true;
        d.space_indent(false);
        assert_eq!(d.lines[0], "a", "只读: 不改");
    }

    #[test]
    fn modified_line_selecting_extends_from_the_anchor() {
        let mut d = Doc::untitled(1, 1);
        d.lines = (0..10).map(|i| format!("l{i}")).collect();
        d.base = d.lines.clone();
        d.lines[2] = "x".into();
        d.lines[7] = "z".into();
        d.rev += 1;
        d.goto(0, 0);
        assert!(d.step_changed_sel(true));
        assert_eq!(
            (d.cursor, d.sel),
            ([2, 0], Some([0, 0, 2, 0])),
            "从原光标选到第一个改动块首"
        );
        assert!(d.step_changed_sel(true));
        assert_eq!(
            (d.cursor, d.sel),
            ([7, 0], Some([0, 0, 7, 0])),
            "继续按: 锚点不动, 选区接着拉长"
        );
        assert!(!d.step_changed_sel(true), "后面没有了: 选区保持");
        assert_eq!(d.sel, Some([0, 0, 7, 0]));
        assert!(d.step_changed_sel(false));
        assert_eq!(
            (d.cursor, d.sel),
            ([2, 0], Some([0, 0, 2, 0])),
            "往回按: 选区收缩"
        );
        d.goto(9, 0);
        assert!(d.step_changed_sel(false));
        assert_eq!(d.sel, Some([7, 0, 9, 0]), "从下往上选: 选区按先后排好");
    }

    #[test]
    fn selection_or_line_falls_back_to_the_current_line() {
        let mut d = Doc::untitled(1, 1);
        d.lines = vec!["abc".into(), "def".into(), "ghi".into()];
        d.goto(1, 1);
        assert_eq!(d.selection_or_line(), "def");
        d.sel = Some([0, 1, 1, 2]);
        assert_eq!(d.selection_or_line(), "bc\nde");
    }

    #[test]
    fn modified_line_stepping_visits_each_block_start() {
        let mut d = Doc::untitled(1, 1);
        d.lines = (0..10).map(|i| format!("l{i}")).collect();
        d.base = d.lines.clone();
        d.lines[2] = "x".into();
        d.lines[3] = "y".into();
        d.lines[7] = "z".into();
        d.rev += 1;
        d.goto(0, 0);
        assert!(d.step_changed(true));
        assert_eq!(d.cursor[0], 2, "连续改动的一块只停在块首");
        assert!(d.step_changed(true));
        assert_eq!(d.cursor[0], 7);
        assert!(!d.step_changed(true), "后面没有了");
        assert!(d.step_changed(false));
        assert_eq!(d.cursor[0], 2);
        assert!(!d.step_changed(false));
    }

    #[test]
    fn powershell_file_detection_and_selection_text() {
        let mut d = Doc::untitled(1, 1);
        d.name = "deploy.ps1".into();
        assert!(d.is_powershell());

        d.name = "module.psm1".into();
        assert!(d.is_powershell());

        d.name = "test.txt".into();
        assert!(!d.is_powershell());
        d.syntax = Some("PowerShell".into());
        assert!(d.is_powershell());

        d.lines = vec![
            "Write-Host 'Line 1'".into(),
            "Get-Process | Where-Object CPU -gt 10".into(),
            "Stop-Service foo".into(),
        ];
        assert_eq!(d.selected_text(), "");

        // 选取第 0 行后半部分至第 1 行前半部分
        d.sel = Some([0, 6, 1, 11]);
        assert_eq!(d.selected_text(), "Host 'Line 1'\nGet-Process");
    }

    #[test]
    fn test_doc_pagination_and_cross_page_navigation() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let path = dir.join(format!("test_doc_paginated_{}.txt", std::process::id()));
        {
            let mut file = std::fs::File::create(&path).expect("create file");
            for i in 0..25_000 {
                writeln!(file, "Line {:06}: Data row for testing pagination", i).unwrap();
            }
            file.flush().unwrap();
        }

        let mmap = editcore::MmapBuffer::open(&path).expect("open mmap");
        let (lines, pinfo) = mmap.get_page(0, 10_000);
        let mut d = Doc::untitled(100, 1);
        d.path = Some(path.clone());
        d.lines = lines;
        d.base = d.lines.clone();
        d.pagination = Some(PaginationState {
            page_size: pinfo.page_size,
            current_page: pinfo.page_index,
            total_pages: pinfo.total_pages,
            total_lines: pinfo.total_lines,
            file_size: mmap.file_size(),
        });

        assert!(d.is_paginated());
        assert_eq!(d.pagination.as_ref().unwrap().current_page, 0);
        assert_eq!(d.pagination.as_ref().unwrap().total_pages, 3);
        assert_eq!(d.lines.len(), 10_000);
        assert!(d.lines[0].starts_with("Line 000000:"));

        // 翻到下一页 (Page 1: 10,000..19,999)
        assert!(d.next_page());
        assert_eq!(d.pagination.as_ref().unwrap().current_page, 1);
        assert_eq!(d.lines.len(), 10_000);
        assert!(d.lines[0].starts_with("Line 010000:"));

        // 翻到下一页 (Page 2: 20,000..24,999)
        assert!(d.next_page());
        assert_eq!(d.pagination.as_ref().unwrap().current_page, 2);
        assert_eq!(d.lines.len(), 5_000);
        assert!(d.lines[0].starts_with("Line 020000:"));

        // 最后一页再 next 应该返回 false
        assert!(!d.next_page());

        // 上一页 (Page 1)
        assert!(d.prev_page());
        assert_eq!(d.pagination.as_ref().unwrap().current_page, 1);

        // 跨页定位全局行: 跳到全局第 23,450 行 (在第 2 页，第 3,450 行)
        assert!(d.goto_global_line(23_450, 4));
        assert_eq!(d.pagination.as_ref().unwrap().current_page, 2);
        assert_eq!(d.cursor, [3450, 4]);
        assert!(d.lines[3450].starts_with("Line 023450:"));

        // 通过 Doc::goto 进行跨页跳转 (行号 500: 在第 0 页)
        d.goto(500, 2);
        assert_eq!(d.cursor, [500, 2]);

        // 通过 Doc::goto 传入全局大行号 (15,230: 在第 1 页，行 5,230)
        d.goto(15_230, 3);
        assert_eq!(d.pagination.as_ref().unwrap().current_page, 1);
        assert_eq!(d.cursor, [5230, 3]);
        assert!(d.lines[5230].starts_with("Line 015230:"));

        let _ = std::fs::remove_file(path);
    }
}
