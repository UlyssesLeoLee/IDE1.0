//! 终端面板。两种用法: (1) 常驻 shell 会话 (默认): 一个 shell 进程一直活着, 输入行直接写进它的标准输入, `cd` / 环境变量会保留;
//! 输出经 `LineBuffer` 处理 ANSI 转义 / SGR 颜色 / `\r` 回车 / `\b` 退格及光标移动后再显示。(2) 一次性执行 (`editcore::exec_cmd::run`, 有超时和输出上限):
//! 选区作标准输入或输出替换选区时用。全面支持 16 色、256 色与 24 位 TrueColor，兼容 PowerShell 交互、Git 与 Hermes 等现代 CLI 工具。

use editcore::exec_cmd::{self, ExecOut};
use eframe::egui;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

/// 会话输出最多留这么多字节, 超出从最前面丢 (按行)
const MAX_KEEP: usize = 1 << 20;
/// 「保留行数」设置的范围 (默认 5000); 字节上限 `MAX_KEEP` 仍然同时生效
pub const MIN_SCROLLBACK: usize = 100;
pub const MAX_SCROLLBACK: usize = 100_000;

/// 标准 16 色 ANSI 调色板
pub const ANSI_PALETTE: [[u8; 3]; 16] = [
    // Standard 8 (0-7)
    [0, 0, 0],       // 0: Black
    [205, 49, 49],   // 1: Red
    [13, 188, 121],  // 2: Green
    [229, 229, 16],  // 3: Yellow
    [36, 114, 200],  // 4: Blue
    [188, 63, 188],  // 5: Magenta
    [17, 168, 205],  // 6: Cyan
    [229, 229, 229], // 7: White
    // Bright 8 (8-15)
    [102, 102, 102], // 8: Bright Black (Gray)
    [241, 76, 76],   // 9: Bright Red
    [35, 209, 139],  // 10: Bright Green
    [245, 245, 67],  // 11: Bright Yellow
    [59, 142, 234],  // 12: Bright Blue
    [214, 112, 214], // 13: Bright Magenta
    [41, 184, 219],  // 14: Bright Cyan
    [255, 255, 255], // 15: Bright White
];

/// 256 色查找转换
pub fn ansi_256_to_rgb(idx: u8) -> [u8; 3] {
    if idx < 16 {
        return ANSI_PALETTE[idx as usize];
    }
    if idx >= 232 {
        let val = 8 + (idx - 232) * 10;
        return [val, val, val];
    }
    let n = idx - 16;
    let b = (n % 6) * 51;
    let g = ((n / 6) % 6) * 51;
    let r = (n / 36) * 51;
    [r, g, b]
}

/// 字符样式属性
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TermStyle {
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dim: bool,
}

/// 终端单元格 (字符 + 样式)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TermCell {
    pub c: char,
    pub style: TermStyle,
}

/// 连续具有相同样式的文本片段
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TermSpan {
    pub text: String,
    pub style: TermStyle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Esc {
    None,
    /// 刚收到 ESC
    Got,
    /// CSI: ESC [ ... 终止字节 0x40-0x7E
    Csi,
    /// OSC: ESC ] ... BEL 或 ESC \
    Osc,
    /// OSC 里又收到 ESC
    OscEsc,
}

/// 终端文本缓冲: 已完成的行 (带样式) + 正在写的当前行 (带光标列与样式)。完整解析 ANSI 转义与 SGR。
pub struct LineBuffer {
    done_spans: std::collections::VecDeque<Vec<TermSpan>>,
    done_text: String,
    line: Vec<TermCell>,
    col: usize,
    esc: Esc,
    csi: String,
    osc: String,
    osc_cmd_done: bool,
    osc_cwd: Option<String>,
    curr_style: TermStyle,
    ver: u64,
    done_lines: usize,
    max_lines: usize,
}

impl LineBuffer {
    pub fn new() -> Self {
        LineBuffer {
            done_spans: std::collections::VecDeque::new(),
            done_text: String::new(),
            line: Vec::new(),
            col: 0,
            esc: Esc::None,
            csi: String::new(),
            osc: String::new(),
            osc_cmd_done: false,
            osc_cwd: None,
            curr_style: TermStyle::default(),
            ver: 0,
            done_lines: 0,
            max_lines: MAX_SCROLLBACK,
        }
    }

    pub fn clear(&mut self) {
        let (v, m) = (self.ver + 1, self.max_lines);
        *self = LineBuffer::new();
        self.ver = v;
        self.max_lines = m;
        self.osc.clear();
        self.osc_cmd_done = false;
        self.osc_cwd = None;
    }

    pub fn take_osc_completion(&mut self) -> (bool, Option<String>) {
        (std::mem::take(&mut self.osc_cmd_done), self.osc_cwd.take())
    }

    pub fn at_line_start(&self) -> bool {
        self.col == 0
    }

    fn finish_osc(&mut self) {
        if let Some(payload) = self.osc.strip_prefix("133;IDE10;") {
            let p = payload.trim();
            if !p.is_empty() {
                self.osc_cwd = Some(p.to_string());
            }
            self.osc_cmd_done = true;
        }
        self.osc.clear();
    }

    /// 最多保留的行数 (夹到合法范围); 变小了立刻裁掉多的
    pub fn set_scrollback(&mut self, lines: usize) {
        let n = lines.clamp(MIN_SCROLLBACK, MAX_SCROLLBACK);
        if n != self.max_lines {
            self.max_lines = n;
            self.trim_lines();
            self.ver += 1;
        }
    }

    /// 超过行数上限就从最前面丢整行
    fn trim_lines(&mut self) {
        if self.done_lines <= self.max_lines {
            return;
        }
        let drop = self.done_lines - self.max_lines;
        if drop > 0 {
            if drop >= self.done_spans.len() {
                self.done_spans.clear();
                self.done_text.clear();
                self.done_lines = 0;
            } else {
                self.done_spans.drain(..drop);
                let at = self
                    .done_text
                    .match_indices('\n')
                    .nth(drop - 1)
                    .map(|(i, _)| i + 1)
                    .unwrap_or(self.done_text.len());
                self.done_text.drain(..at);
                self.done_lines = self.max_lines;
            }
        }
    }

    fn trim_bytes(&mut self) {
        if self.done_text.len() <= MAX_KEEP {
            return;
        }
        let cut = self.done_text.len() - MAX_KEEP / 2;
        let cut = (cut..self.done_text.len())
            .find(|&i| self.done_text.is_char_boundary(i))
            .unwrap_or(self.done_text.len());
        let at = self.done_text[cut..]
            .find('\n')
            .map(|i| cut + i + 1)
            .unwrap_or(cut);

        let dropped_lines = self.done_text[..at].matches('\n').count();
        self.done_text.drain(..at);
        if dropped_lines >= self.done_spans.len() {
            self.done_spans.clear();
            self.done_lines = 0;
        } else {
            self.done_spans.drain(..dropped_lines);
            self.done_lines = self.done_spans.len();
        }
    }

    /// 喂入一段输出文本 (转义序列可以被切在两次调用之间)
    pub fn version(&self) -> u64 {
        self.ver
    }

    pub fn feed(&mut self, text: &str) {
        self.ver += 1;
        for c in text.chars() {
            match self.esc {
                Esc::Got => {
                    self.esc = match c {
                        '[' => {
                            self.csi.clear();
                            Esc::Csi
                        }
                        ']' => {
                            self.osc.clear();
                            Esc::Osc
                        }
                        _ => Esc::None, // 其它两字节序列直接丢
                    };
                    continue;
                }
                Esc::Csi => {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        self.csi_done(c);
                        self.esc = Esc::None;
                    } else {
                        self.csi.push(c);
                    }
                    continue;
                }
                Esc::Osc => {
                    self.esc = match c {
                        '\u{7}' => {
                            self.finish_osc();
                            Esc::None
                        }
                        '\u{1b}' => Esc::OscEsc,
                        ch => {
                            if self.osc.len() < 4096 {
                                self.osc.push(ch);
                            }
                            Esc::Osc
                        }
                    };
                    continue;
                }
                Esc::OscEsc => {
                    self.esc = if c == '\\' {
                        self.finish_osc();
                        Esc::None
                    } else {
                        Esc::Osc
                    };
                    continue;
                }
                Esc::None => {}
            }
            match c {
                '\u{1b}' => self.esc = Esc::Got,
                '\n' => self.newline(),
                '\r' => self.col = 0,
                '\u{8}' => self.col = self.col.saturating_sub(1),
                '\t' => {
                    let to = (self.col / 8 + 1) * 8;
                    while self.col < to {
                        self.put(' ');
                    }
                }
                c if c.is_control() => {}
                c => self.put(c),
            }
        }
    }

    fn put(&mut self, c: char) {
        let cell = TermCell {
            c,
            style: self.curr_style,
        };
        while self.line.len() < self.col {
            self.line.push(TermCell {
                c: ' ',
                style: self.curr_style,
            });
        }
        if self.col < self.line.len() {
            self.line[self.col] = cell;
        } else {
            self.line.push(cell);
        }
        self.col += 1;
    }

    fn newline(&mut self) {
        let mut spans: Vec<TermSpan> = Vec::new();
        let mut cur_span: Option<TermSpan> = None;
        let mut line_text = String::with_capacity(self.line.len());

        for cell in self.line.drain(..) {
            line_text.push(cell.c);
            match cur_span.as_mut() {
                Some(s) if s.style == cell.style => {
                    s.text.push(cell.c);
                }
                _ => {
                    if let Some(s) = cur_span.take() {
                        spans.push(s);
                    }
                    cur_span = Some(TermSpan {
                        text: cell.c.to_string(),
                        style: cell.style,
                    });
                }
            }
        }
        if let Some(s) = cur_span {
            spans.push(s);
        }

        let trimmed_text = line_text.trim_end();
        self.done_text.push_str(trimmed_text);
        self.done_text.push('\n');
        self.done_spans.push_back(spans);
        self.done_lines += 1;
        self.col = 0;

        if self.done_lines > self.max_lines + self.max_lines / 4 {
            self.trim_lines();
        }
        if self.done_text.len() > MAX_KEEP {
            self.trim_bytes();
        }
    }

    fn parse_single_arg(&self, default: usize) -> usize {
        self.csi
            .trim_start_matches('?')
            .split([';', ':'])
            .next()
            .and_then(|x| x.parse().ok())
            .unwrap_or(default)
    }

    /// 处理光标向上移动 (如 Hermes CLI 进度条 / 多行状态行重绘)
    fn cursor_up(&mut self, n: usize) {
        let count = n.max(1);
        for _ in 0..count {
            if let Some(last_spans) = self.done_spans.pop_back() {
                if self.line.is_empty() {
                    for span in last_spans {
                        for c in span.text.chars() {
                            self.line.push(TermCell {
                                c,
                                style: span.style,
                            });
                        }
                    }
                    self.col = self.line.len();
                }
                self.done_lines = self.done_lines.saturating_sub(1);
                if let Some(idx) = self.done_text.trim_end_matches('\n').rfind('\n') {
                    self.done_text.truncate(idx + 1);
                } else {
                    self.done_text.clear();
                }
            }
        }
    }

    /// 处理 SGR (Select Graphic Rendition) 文本格式与 16/256/TrueColor 颜色序列
    fn handle_sgr(&mut self) {
        if self.csi.is_empty() {
            self.curr_style = TermStyle::default();
            return;
        }
        let parts: Vec<&str> = self.csi.split([';', ':']).collect();
        let mut idx = 0;
        while idx < parts.len() {
            let code: u32 = parts[idx].parse().unwrap_or(0);
            match code {
                0 => self.curr_style = TermStyle::default(),
                1 => self.curr_style.bold = true,
                2 => self.curr_style.dim = true,
                3 => self.curr_style.italic = true,
                4 => self.curr_style.underline = true,
                7 => {
                    std::mem::swap(&mut self.curr_style.fg, &mut self.curr_style.bg);
                }
                22 => {
                    self.curr_style.bold = false;
                    self.curr_style.dim = false;
                }
                23 => self.curr_style.italic = false,
                24 => self.curr_style.underline = false,
                30..=37 => {
                    self.curr_style.fg = Some(ANSI_PALETTE[(code - 30) as usize]);
                }
                38 => {
                    if idx + 1 < parts.len() {
                        let mode = parts[idx + 1];
                        if mode == "5" && idx + 2 < parts.len() {
                            if let Ok(color_idx) = parts[idx + 2].parse::<u8>() {
                                self.curr_style.fg = Some(ansi_256_to_rgb(color_idx));
                            }
                            idx += 2;
                        } else if mode == "2" && idx + 4 < parts.len() {
                            let r: u8 = parts[idx + 2].parse().unwrap_or(0);
                            let g: u8 = parts[idx + 3].parse().unwrap_or(0);
                            let b: u8 = parts[idx + 4].parse().unwrap_or(0);
                            self.curr_style.fg = Some([r, g, b]);
                            idx += 4;
                        }
                    }
                }
                39 => self.curr_style.fg = None,
                40..=47 => {
                    self.curr_style.bg = Some(ANSI_PALETTE[(code - 40) as usize]);
                }
                48 => {
                    if idx + 1 < parts.len() {
                        let mode = parts[idx + 1];
                        if mode == "5" && idx + 2 < parts.len() {
                            if let Ok(color_idx) = parts[idx + 2].parse::<u8>() {
                                self.curr_style.bg = Some(ansi_256_to_rgb(color_idx));
                            }
                            idx += 2;
                        } else if mode == "2" && idx + 4 < parts.len() {
                            let r: u8 = parts[idx + 2].parse().unwrap_or(0);
                            let g: u8 = parts[idx + 3].parse().unwrap_or(0);
                            let b: u8 = parts[idx + 4].parse().unwrap_or(0);
                            self.curr_style.bg = Some([r, g, b]);
                            idx += 4;
                        }
                    }
                }
                49 => self.curr_style.bg = None,
                90..=97 => {
                    self.curr_style.fg = Some(ANSI_PALETTE[(code - 90 + 8) as usize]);
                }
                100..=107 => {
                    self.curr_style.bg = Some(ANSI_PALETTE[(code - 100 + 8) as usize]);
                }
                _ => {}
            }
            idx += 1;
        }
    }

    /// 处理 CSI 序列
    fn csi_done(&mut self, fin: char) {
        match fin {
            'm' => self.handle_sgr(),
            'A' => {
                let n = self.parse_single_arg(1);
                self.cursor_up(n);
            }
            'B' => {
                let n = self.parse_single_arg(1);
                for _ in 0..n {
                    self.newline();
                }
            }
            'C' => {
                let n = self.parse_single_arg(1);
                self.col = (self.col + n).min(4096);
            }
            'D' => {
                let n = self.parse_single_arg(1);
                self.col = self.col.saturating_sub(n);
            }
            'G' => {
                let n = self.parse_single_arg(1);
                self.col = (n.saturating_sub(1)).min(4096);
            }
            'H' | 'f' => {
                let parts: Vec<usize> =
                    self.csi.split(';').filter_map(|s| s.parse().ok()).collect();
                let col = parts.get(1).copied().unwrap_or(1);
                self.col = col.saturating_sub(1).min(4096);
            }
            'K' => {
                let n = self.parse_single_arg(0);
                match n {
                    0 => self.line.truncate(self.col),
                    1 => {
                        for cell in self.line.iter_mut().take(self.col) {
                            cell.c = ' ';
                        }
                    }
                    2 => {
                        self.line.clear();
                        self.col = 0;
                    }
                    _ => {}
                }
            }
            'J' => {
                let n = self.parse_single_arg(0);
                if n == 2 || n == 3 {
                    self.clear();
                }
            }
            _ => {}
        }
    }

    /// 生成富文本排版作业 (支持 ANSI 16色 / 256色 / TrueColor 以及下划线与粗体)
    pub fn build_layout_job(
        &self,
        tpx: f32,
        default_fg: egui::Color32,
        prefix_text: &str,
    ) -> egui::text::LayoutJob {
        use egui::text::{LayoutJob, TextFormat};
        let mut job = LayoutJob::default();

        if !prefix_text.is_empty() {
            job.append(
                prefix_text,
                0.0,
                TextFormat {
                    font_id: egui::FontId::monospace(tpx),
                    color: default_fg,
                    ..Default::default()
                },
            );
        }

        // 渲染已完成的行 (带样式)
        for line in &self.done_spans {
            for span in line {
                let fg_color = if let Some([r, g, b]) = span.style.fg {
                    egui::Color32::from_rgb(r, g, b)
                } else if span.style.dim {
                    default_fg.linear_multiply(0.6)
                } else {
                    default_fg
                };
                let bg_color = if let Some([r, g, b]) = span.style.bg {
                    egui::Color32::from_rgb(r, g, b)
                } else {
                    egui::Color32::TRANSPARENT
                };

                let format = TextFormat {
                    font_id: egui::FontId::monospace(tpx),
                    color: fg_color,
                    background: bg_color,
                    italics: span.style.italic,
                    underline: if span.style.underline {
                        egui::Stroke::new(1.0, fg_color)
                    } else {
                        egui::Stroke::NONE
                    },
                    ..Default::default()
                };
                job.append(&span.text, 0.0, format);
            }
            job.append(
                "\n",
                0.0,
                TextFormat {
                    font_id: egui::FontId::monospace(tpx),
                    color: default_fg,
                    ..Default::default()
                },
            );
        }

        // 渲染当前正在写入的行 (合并相同样式的连续单元)
        if !self.line.is_empty() {
            let mut cur_style: Option<TermStyle> = None;
            let mut cur_text = String::new();

            for cell in &self.line {
                match cur_style {
                    Some(st) if st == cell.style => {
                        cur_text.push(cell.c);
                    }
                    _ => {
                        if let Some(st) = cur_style {
                            let fg_color = if let Some([r, g, b]) = st.fg {
                                egui::Color32::from_rgb(r, g, b)
                            } else if st.dim {
                                default_fg.linear_multiply(0.6)
                            } else {
                                default_fg
                            };
                            let bg_color = if let Some([r, g, b]) = st.bg {
                                egui::Color32::from_rgb(r, g, b)
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            job.append(
                                &cur_text,
                                0.0,
                                TextFormat {
                                    font_id: egui::FontId::monospace(tpx),
                                    color: fg_color,
                                    background: bg_color,
                                    italics: st.italic,
                                    underline: if st.underline {
                                        egui::Stroke::new(1.0, fg_color)
                                    } else {
                                        egui::Stroke::NONE
                                    },
                                    ..Default::default()
                                },
                            );
                            cur_text.clear();
                        }
                        cur_style = Some(cell.style);
                        cur_text.push(cell.c);
                    }
                }
            }
            if let Some(st) = cur_style {
                let fg_color = if let Some([r, g, b]) = st.fg {
                    egui::Color32::from_rgb(r, g, b)
                } else if st.dim {
                    default_fg.linear_multiply(0.6)
                } else {
                    default_fg
                };
                let bg_color = if let Some([r, g, b]) = st.bg {
                    egui::Color32::from_rgb(r, g, b)
                } else {
                    egui::Color32::TRANSPARENT
                };
                job.append(
                    &cur_text,
                    0.0,
                    TextFormat {
                        font_id: egui::FontId::monospace(tpx),
                        color: fg_color,
                        background: bg_color,
                        italics: st.italic,
                        underline: if st.underline {
                            egui::Stroke::new(1.0, fg_color)
                        } else {
                            egui::Stroke::NONE
                        },
                        ..Default::default()
                    },
                );
            }
        }

        job
    }

    /// 全部可见文本 (已完成的行 + 当前行)
    pub fn text(&self) -> String {
        let mut s = self.done_text.clone();
        for cell in &self.line {
            s.push(cell.c);
        }
        s
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ShellKind {
    PowerShell,
    Cmd,
    PosixSh,
}

/// 常驻 shell 进程: 两个读线程把 stdout / stderr 的字节送回来
struct Session {
    kind: ShellKind,
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Vec<u8>>,
    /// 上次没解码完的尾巴 (多字节字符被切在两块之间)
    tail: Vec<u8>,
}

fn pump(mut r: impl Read + Send + 'static, tx: std::sync::mpsc::Sender<Vec<u8>>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
}

/// 解码 PowerShell 管道重定向输出中的 CLIXML 错误流，转为高亮 ANSI Red 文本
pub fn clean_powershell_clixml(raw: &str) -> String {
    if !raw.contains("#< CLIXML") {
        return raw.to_string();
    }
    let mut out = String::new();
    let mut rest = raw;

    while let Some(start) = rest.find("#< CLIXML") {
        out.push_str(&rest[..start]);
        let clixml_part = &rest[start..];
        let mut err_text = String::new();
        let end_idx = if let Some(close) = clixml_part.find("</Objs>") {
            close + 7
        } else {
            clixml_part.len()
        };

        let slice = &clixml_part[..end_idx];
        let tag = "<S S=\"Error\">";
        let mut search_from = 0;
        while let Some(tag_pos) = slice[search_from..].find(tag) {
            let val_start = search_from + tag_pos + tag.len();
            if let Some(tag_end) = slice[val_start..].find("</S>") {
                let encoded = &slice[val_start..val_start + tag_end];
                let decoded = encoded
                    .replace("_x000D__x000A_", "\n")
                    .replace("_x000A_", "\n")
                    .replace("_x000D_", "\r")
                    .replace("_x001B_", "\x1b")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&")
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'");
                err_text.push_str(&decoded);
                search_from = val_start + tag_end + 4;
            } else {
                break;
            }
        }

        if !err_text.is_empty() {
            out.push_str("\x1b[91m");
            out.push_str(err_text.trim_end());
            out.push_str("\x1b[0m\n");
        }

        rest = &clixml_part[end_idx..];
    }
    out.push_str(rest);
    out
}

impl Session {
    fn start(cwd: &std::path::Path) -> Result<Session, String> {
        #[cfg(windows)]
        let (mut child, kind) = {
            use std::os::windows::process::CommandExt;
            let mut c = Command::new("powershell.exe");
            // 使用 `-Command -` 非回显流式模式: 不打印重复提示符或原始输入回显, 且保留跨指令变量/$PWD/别名
            c.args([
                "-NoLogo",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "-",
            ]);
            c.envs([
                ("TERM", "xterm-256color"),
                ("COLORTERM", "truecolor"),
                ("CLICOLOR", "1"),
                ("CLICOLOR_FORCE", "1"),
                ("FORCE_COLOR", "3"),
                ("ANSICON", "1"),
                ("PYTHONUNBUFFERED", "1"),
                ("PYTHONUTF8", "1"),
                ("PYTHONIOENCODING", "utf-8"),
            ]);
            c.current_dir(cwd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(0x0800_0000);
            match c.spawn() {
                Ok(child) => (child, ShellKind::PowerShell),
                Err(_) => {
                    let mut c = Command::new("cmd.exe");
                    c.current_dir(cwd)
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .creation_flags(0x0800_0000);
                    let ch = c.spawn().map_err(|e| format!("无法启动 shell: {e}"))?;
                    (ch, ShellKind::Cmd)
                }
            }
        };
        #[cfg(not(windows))]
        let (mut child, kind) = {
            let sh = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
            let mut c = Command::new(sh);
            c.envs([
                ("TERM", "xterm-256color"),
                ("COLORTERM", "truecolor"),
                ("CLICOLOR", "1"),
                ("CLICOLOR_FORCE", "1"),
                ("FORCE_COLOR", "3"),
                ("PYTHONUNBUFFERED", "1"),
                ("PYTHONUTF8", "1"),
                ("PYTHONIOENCODING", "utf-8"),
            ]);
            c.current_dir(cwd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let ch = c.spawn().map_err(|e| format!("无法启动 shell: {e}"))?;
            (ch, ShellKind::PosixSh)
        };

        let (tx, rx) = channel();
        pump(child.stdout.take().ok_or("没有标准输出")?, tx.clone());
        pump(child.stderr.take().ok_or("没有标准错误")?, tx);
        let stdin = child.stdin.take().ok_or("没有标准输入")?;
        let mut session = Session {
            kind,
            child,
            stdin,
            rx,
            tail: vec![],
        };
        #[cfg(windows)]
        if session.kind == ShellKind::PowerShell {
            let _ = session.send_line(
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::InputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; $ProgressPreference = 'SilentlyContinue'; if (Get-Variable PSStyle -ErrorAction SilentlyContinue) { $PSStyle.OutputRendering = 'Ansi' }",
            );
        }
        Ok(session)
    }

    fn send_line(&mut self, line: &str) -> bool {
        let nl: &[u8] = if cfg!(windows) { b"\r\n" } else { b"\n" };
        let clean = line.replace("\r\n", "\n").replace('\r', "\n");
        for sub in clean.split('\n') {
            if self.stdin.write_all(sub.as_bytes()).is_err() || self.stdin.write_all(nl).is_err() {
                return false;
            }
        }
        self.stdin.flush().is_ok()
    }

    fn send_ctrl_c(&mut self) -> bool {
        self.stdin.write_all(&[0x03, b'\r', b'\n']).is_ok() && self.stdin.flush().is_ok()
    }

    /// 取已到的输出并解码 (UTF-8 被切断的尾巴留到下一块; 其余走 editcore 的自动判别与 CLIXML 清洗)
    fn drain(&mut self) -> String {
        let mut bytes = std::mem::take(&mut self.tail);
        while let Ok(b) = self.rx.try_recv() {
            bytes.extend(b);
        }
        if bytes.is_empty() {
            return String::new();
        }
        if let Err(e) = std::str::from_utf8(&bytes) {
            if e.error_len().is_none() {
                self.tail = bytes.split_off(e.valid_up_to());
            }
        }
        let decoded = editcore::document_io::decode_document(&bytes, None).content;
        clean_powershell_clixml(&decoded)
    }

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[inline]
fn str_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// 检测当前进程是否以系统管理员特权运行 (纯 Safe Rust + OnceLock 进程级缓存, 无裸盘访问告警与零子进程开销)
pub fn detect_is_admin() -> bool {
    static IS_ADMIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IS_ADMIN.get_or_init(|| {
        #[cfg(windows)]
        {
            let sys_root = std::env::var_os("SystemRoot")
                .or_else(|| std::env::var_os("WINDIR"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
            std::fs::read_dir(
                sys_root
                    .join("System32")
                    .join("config")
                    .join("systemprofile"),
            )
            .is_ok()
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("id")
                .arg("-u")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
                .unwrap_or(false)
        }
    })
}

pub struct Terminal {
    pub open: bool,
    pub input: String,
    pub cursor_pos: usize,
    pub is_admin: bool,
    /// 无论是一次性命令还是常驻 Shell，当前是否有命令正在执行中
    pub is_executing: bool,
    /// 请求下一帧把键盘焦点放进终端输入框 (点「+ 新建终端」等触发)
    pub focus_input: bool,
    /// 一次性命令的输出 (会话输出在 `buf` 里, 显示用 `display()`)
    pub output: String,
    output_rev: u64,
    pub history: Vec<String>,
    hist_pos: Option<usize>,
    replace_doc: Option<u64>,
    pub replace_out: Option<(u64, String)>,
    rx: Option<Receiver<(String, Result<ExecOut, String>)>>,
    /// 用常驻 shell 会话 (false = 每条命令一次性执行)
    pub persistent: bool,
    session: Option<Session>,
    buf: LineBuffer,
    /// 终端实际所在的工作目录 (跟踪 cd 指令)
    pub session_cwd: Option<PathBuf>,
    /// 最近有输入 / 输出到这个时刻之前, 界面用快节奏刷新; 之后空闲慢刷新
    hot_until: Option<std::time::Instant>,
    shown: std::cell::RefCell<Option<((u64, usize, u64, u64), String)>>,
    job_cache:
        std::cell::RefCell<Option<((u64, usize, u64, u64, u32, [u8; 4]), egui::text::LayoutJob)>>,
}

impl Terminal {
    pub fn new() -> Self {
        Terminal {
            open: false,
            input: String::new(),
            cursor_pos: 0,
            is_admin: detect_is_admin(),
            is_executing: false,
            focus_input: false,
            output: String::new(),
            output_rev: 0,
            history: vec![],
            hist_pos: None,
            replace_doc: None,
            replace_out: None,
            rx: None,
            persistent: true,
            session: None,
            buf: LineBuffer::new(),
            session_cwd: None,
            hot_until: None,
            shown: Default::default(),
            job_cache: Default::default(),
        }
    }

    /// 无论一次性还是常驻会话，当前是否有命令正在执行中 (与 PowerShell 动作状态完全对齐)
    pub fn running(&self) -> bool {
        self.rx.is_some() || self.is_executing
    }

    /// 常驻 shell 会话是否已建立
    pub fn has_session(&self) -> bool {
        self.session.is_some()
    }

    /// 下一次界面刷新间隔 (毫秒); None = 不用刷新
    pub fn repaint_ms(&self) -> Option<u64> {
        if self.rx.is_some()
            || self
                .hot_until
                .is_some_and(|t| t > std::time::Instant::now())
        {
            Some(80)
        } else if self.session.is_some() {
            Some(500)
        } else {
            None
        }
    }

    fn heat(&mut self) {
        self.hot_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    }

    /// 需要持续刷新界面 (有会话或有命令在跑)
    #[allow(dead_code)]
    pub fn busy(&self) -> bool {
        self.rx.is_some() || self.session.is_some()
    }

    /// 执行输入框里的命令 (`cls` / `clear` 清屏)。没有选区管道时进常驻会话, 否则一次性执行
    pub fn submit(&mut self, cwd: PathBuf, stdin: Option<String>, replace_doc: Option<u64>) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() || self.running() {
            return;
        }
        self.input.clear();
        self.cursor_pos = 0;
        self.hist_pos = None;
        if matches!(cmd.as_str(), "cls" | "clear") {
            self.clear();
            return;
        }
        self.history.push(cmd.clone());
        if self.persistent && stdin.is_none() && replace_doc.is_none() {
            self.session_send(&cwd, &cmd);
            return;
        }
        let prompt_prefix = if self.is_admin { "PS [ADMIN]" } else { "PS" };
        let p_color = if self.is_admin {
            "\x1b[93m"
        } else {
            "\x1b[96m"
        };
        self.output.push_str(&format!(
            "{prompt_prefix} {}> {cmd}{}\n",
            cwd.display(),
            if stdin.is_some() {
                "   (选区作标准输入)"
            } else {
                ""
            }
        ));
        self.output_rev = self.output_rev.wrapping_add(1);
        self.buf.feed(&format!(
            "{p_color}{prompt_prefix} {}> \x1b[0m\x1b[1;37m{cmd}\x1b[0m{}\n",
            cwd.display(),
            if stdin.is_some() {
                "   \x1b[90m(选区作标准输入)\x1b[0m"
            } else {
                ""
            }
        ));
        self.replace_doc = replace_doc;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = exec_cmd::run(&cwd, &cmd, stdin.as_deref(), Some(60_000));
            let _ = tx.send((cmd, r));
        });
        self.rx = Some(rx);
    }

    /// 外观设置里的保留行数
    pub fn set_scrollback(&mut self, lines: usize) {
        self.buf.set_scrollback(lines);
    }

    /// 清屏 (一次性输出和会话输出都清)
    pub fn clear(&mut self) {
        self.output.clear();
        self.output_rev = self.output_rev.wrapping_add(1);
        self.buf.clear();
    }

    /// 要显示的富文本排版作业 (带版本号 + 内容哈希缓存, 同长度替换不会碰撞)
    pub fn layout_job(&self, tpx: f32, default_fg: egui::Color32) -> egui::text::LayoutJob {
        let key = (
            self.output_rev,
            self.output.len(),
            str_hash(&self.output),
            self.buf.version(),
            tpx.to_bits(),
            default_fg.to_array(),
        );
        let mut cache = self.job_cache.borrow_mut();
        if cache.as_ref().map(|c| c.0) != Some(key) {
            let job = self.buf.build_layout_job(tpx, default_fg, &self.output);
            *cache = Some((key, job));
        }
        cache.as_ref().expect("刚算过").1.clone()
    }

    /// 要显示的全部文本 (纯文本模式，供搜索与剪贴板用): 一次性命令的输出在前, 会话输出在后
    pub fn display(&self) -> String {
        let key = (
            self.output_rev,
            self.output.len(),
            str_hash(&self.output),
            self.buf.version(),
        );
        let mut c = self.shown.borrow_mut();
        if c.as_ref().map(|c| c.0) != Some(key) {
            let mut s = self.output.clone();
            s.push_str(&self.buf.text());
            *c = Some((key, s));
        }
        c.as_ref().map(|c| c.1.clone()).unwrap_or_default()
    }

    /// 结束会话 (下一条命令会重新开一个)
    pub fn restart(&mut self) {
        self.is_executing = false;
        if self.session.take().is_some() {
            self.buf.feed("\n\x1b[90m[会话已重启]\x1b[0m\n");
        }
    }

    /// 发送 Ctrl+C 中断信号给正在运行的子程序
    pub fn interrupt(&mut self, _cwd: &std::path::Path) {
        self.is_executing = false;
        if let Some(s) = self.session.as_mut() {
            if s.send_ctrl_c() {
                self.buf.feed("^C\n");
                self.heat();
                return;
            }
        }
        self.restart();
    }

    /// 新建或重启常驻 shell 会话并聚焦输入框 (点「+ 新建终端」触发)
    pub fn start_session(&mut self, cwd: &std::path::Path) {
        let had = self.session.take().is_some();
        self.persistent = true;
        self.focus_input = true;
        self.is_admin = detect_is_admin();
        self.session_cwd = Some(cwd.to_path_buf());
        match Session::start(cwd) {
            Ok(s) => {
                self.session = Some(s);
                let role = if self.is_admin {
                    "管理员特权 (Administrator)"
                } else {
                    "标准用户权限 (Standard User)"
                };
                if had {
                    self.buf.feed(&format!(
                        "\x1b[96m\n[已新建终端会话 · {role}] 工作目录: {}\x1b[0m\n",
                        cwd.display()
                    ));
                } else {
                    self.buf.feed(&format!(
                        "\x1b[96m[终端会话已就绪 · {role}] 工作目录: {}\x1b[0m\n",
                        cwd.display()
                    ));
                }
                self.heat();
            }
            Err(e) => {
                self.buf.feed(&format!("\x1b[91m[{e}]\x1b[0m\n"));
            }
        }
    }

    /// 向活动终端会话发送脚本代码并立即执行 (如 PowerShell 选区一键执行)
    pub fn execute_script(&mut self, cwd: &std::path::Path, code: &str) {
        let trimmed = code.trim();
        if trimmed.is_empty() {
            return;
        }
        self.session_send(cwd, trimmed);
    }

    fn session_send(&mut self, cwd: &std::path::Path, cmd: &str) {
        let trimmed = cmd.trim();
        let effective_cwd = self
            .session_cwd
            .clone()
            .unwrap_or_else(|| cwd.to_path_buf());
        let prompt_prefix = if self.is_admin { "PS [ADMIN]" } else { "PS" };
        let p_color = if self.is_admin {
            "\x1b[93m"
        } else {
            "\x1b[96m"
        };
        if matches!(trimmed, "exit" | "quit") {
            if !self.buf.at_line_start() {
                self.buf.feed("\n");
            }
            self.buf.feed(&format!(
                "{p_color}{prompt_prefix} {}> \x1b[0m\x1b[1;37m{cmd}\x1b[0m\n",
                effective_cwd.display()
            ));
            self.buf.feed("\x1b[90m[终端会话已退出]\x1b[0m\n");
            self.session = None;
            self.is_executing = false;
            return;
        }
        if !self.session.as_mut().map(|s| s.alive()).unwrap_or(false) {
            match Session::start(&effective_cwd) {
                Ok(s) => self.session = Some(s),
                Err(e) => {
                    self.buf.feed(&format!("\x1b[91m[{e}]\x1b[0m\n"));
                    self.is_executing = false;
                    return;
                }
            }
        }
        if !self.buf.at_line_start() {
            self.buf.feed("\n");
        }
        self.buf.feed(&format!(
            "{p_color}{prompt_prefix} {}> \x1b[0m\x1b[1;37m{cmd}\x1b[0m\n",
            effective_cwd.display()
        ));
        self.heat();
        let shell_kind = self.session.as_ref().map(|s| s.kind);
        let (wrapper, track_done) = match shell_kind {
            Some(ShellKind::PowerShell) => (
                format!(
                    "{cmd}\r\n[Console]::Out.Write(\"$([char]27)]133;IDE10;$($PWD.Path)$([char]7)\"); [Console]::Out.Flush()"
                ),
                true,
            ),
            Some(ShellKind::PosixSh) => (
                format!("{cmd}\nprintf '\\033]133;IDE10;%s\\007' \"$PWD\""),
                true,
            ),
            Some(ShellKind::Cmd) | None => (cmd.to_string(), false),
        };
        self.is_executing = track_done;
        if let Some(s) = self.session.as_mut() {
            if !s.send_line(&wrapper) {
                self.buf.feed("\x1b[90m[shell 已退出]\x1b[0m\n");
                self.session = None;
                self.is_executing = false;
            }
        }
        self.track_cd(cmd, cwd);
    }

    fn track_cd(&mut self, cmd: &str, default_cwd: &std::path::Path) {
        let trimmed = cmd.trim();
        let target = trimmed
            .strip_prefix("cd ")
            .or_else(|| trimmed.strip_prefix("Set-Location "))
            .or_else(|| trimmed.strip_prefix("chdir "))
            .map(str::trim);

        if let Some(path_str) = target {
            let clean = path_str.trim_matches(['\'', '"']);
            let base = self
                .session_cwd
                .clone()
                .unwrap_or_else(|| default_cwd.to_path_buf());
            let candidate = if clean == "~" {
                std::env::var_os("USERPROFILE")
                    .or_else(|| std::env::var_os("HOME"))
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
            } else if std::path::Path::new(clean).is_absolute() {
                PathBuf::from(clean)
            } else {
                base.join(clean)
            };
            let mut resolved = if let Ok(canon) = candidate.canonicalize() {
                canon
            } else {
                candidate
            };
            let s = resolved.to_string_lossy().to_string();
            if let Some(stripped) = s.strip_prefix(r"\\?\") {
                resolved = PathBuf::from(stripped);
            }
            if resolved.exists() {
                self.session_cwd = Some(resolved);
            }
        }
    }

    /// 每帧调用: 取会话输出和后台命令输出; 有新内容返回 true
    pub fn poll(&mut self) -> bool {
        let mut fresh = false;
        if let Some(s) = self.session.as_mut() {
            let t = s.drain();
            if !t.is_empty() {
                // 先喂入状态机，天然支持任意 chunk 边界拆分并自动剔除 OSC 序列
                self.buf.feed(&t);
                let (done, osc_cwd) = self.buf.take_osc_completion();
                if done {
                    if self.is_executing {
                        self.focus_input = true;
                    }
                    self.is_executing = false;
                }
                if let Some(raw_path) = osc_cwd {
                    let trimmed_path = raw_path.strip_prefix(r"\\?\").unwrap_or(&raw_path);
                    let pbuf = PathBuf::from(trimmed_path);
                    if pbuf.is_dir() {
                        self.session_cwd = Some(pbuf);
                    }
                }
                fresh = true;
                self.hot_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            }
            if !s.alive() {
                let rest = s.drain();
                self.buf.feed(&rest);
                let _ = self.buf.take_osc_completion();
                self.buf.feed("\n[shell 已退出]\n");
                self.session = None;
                self.is_executing = false;
                fresh = true;
            }
        }
        self.poll_exec() || fresh
    }

    /// 取一次性命令的输出
    fn poll_exec(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok((_, Ok(o))) => {
                if let Some(id) = self.replace_doc.take() {
                    if o.code.unwrap_or(0) == 0 && !o.timed_out {
                        self.replace_out = Some((id, o.stdout.clone()));
                    }
                }
                self.output.push_str(&o.stdout);
                if !o.stderr.is_empty() {
                    self.output.push_str(&clean_powershell_clixml(&o.stderr));
                }
                if o.timed_out {
                    self.output.push_str("[超时, 已结束进程]\n");
                } else if o.code.unwrap_or(0) != 0 {
                    self.output
                        .push_str(&format!("[退出码 {}]\n", o.code.unwrap_or(-1)));
                }
                if !self.output.ends_with('\n') {
                    self.output.push('\n');
                }
                self.output_rev = self.output_rev.wrapping_add(1);
                self.rx = None;
                true
            }
            Ok((_, Err(e))) => {
                self.output.push_str(&format!("[执行失败: {e}]\n"));
                self.output_rev = self.output_rev.wrapping_add(1);
                self.rx = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(_) => {
                self.rx = None;
                false
            }
        }
    }

    /// ↑ / ↓ 翻历史
    pub fn history_step(&mut self, up: bool) {
        if self.history.is_empty() {
            return;
        }
        let n = self.history.len();
        let pos = match (self.hist_pos, up) {
            (None, true) => n - 1,
            (Some(p), true) => p.saturating_sub(1),
            (Some(p), false) if p + 1 < n => p + 1,
            _ => {
                self.hist_pos = None;
                self.input.clear();
                self.cursor_pos = 0;
                return;
            }
        };
        self.hist_pos = Some(pos);
        self.input = self.history[pos].clone();
        self.cursor_pos = self.input.len();
    }

    /// 获取当前终端环境的动态命令行提示符 (实时反映跟踪的 cd 目录)
    pub fn prompt_str(&self, cwd: &std::path::Path) -> String {
        let pfx = if self.is_admin { "PS [ADMIN]" } else { "PS" };
        let eff = self.session_cwd.as_deref().unwrap_or(cwd);
        format!("{pfx} {}> ", eff.display())
    }

    /// 取消当前命令行输入 (Ctrl+C: 终止输入并输出 ^C 换行)
    pub fn cancel_input(&mut self, cwd: &std::path::Path) {
        let p = self.prompt_str(cwd);
        self.buf.feed(&format!("{p}{}^C\n", self.input));
        self.input.clear();
        self.cursor_pos = 0;
        self.hist_pos = None;
        self.heat();
    }

    /// Tab 键自动补全当前工作目录下的文件或子路径
    pub fn tab_complete(&mut self, cwd: &std::path::Path) {
        let trimmed = self.input.trim_end();
        let last_word = trimmed.split_whitespace().last().unwrap_or("");
        if last_word.is_empty() {
            return;
        }
        let (dir_to_scan, prefix) = match last_word.rfind(['/', '\\']) {
            Some(idx) => (cwd.join(&last_word[..idx]), &last_word[idx + 1..]),
            None => (cwd.to_path_buf(), last_word),
        };
        if let Ok(entries) = std::fs::read_dir(&dir_to_scan) {
            let mut matches: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        Some(if is_dir { format!("{name}/") } else { name })
                    } else {
                        None
                    }
                })
                .collect();
            matches.sort();
            if let Some(m) = matches.first() {
                let base_len = self.input.len() - prefix.len();
                self.input.truncate(base_len);
                self.input.push_str(m);
                self.cursor_pos = self.input.len();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(t: &mut Terminal, until: impl Fn(&Terminal) -> bool) {
        for _ in 0..1500 {
            t.poll();
            if until(t) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[test]
    fn runs_a_command_in_the_background_and_collects_output() {
        let mut t = Terminal::new();
        t.persistent = false;
        t.input = "echo hello-fast".into();
        t.submit(std::env::temp_dir(), None, None);
        assert!(t.running());
        wait(&mut t, |t| !t.running());
        assert!(t.output.contains("hello-fast"), "{}", t.output);
        assert!(!t.running());
        t.input = "cls".into();
        t.submit(std::env::temp_dir(), None, None);
        assert!(t.output.is_empty());
        assert_eq!(t.history, vec!["echo hello-fast"]);
    }

    #[test]
    fn selection_goes_to_stdin_and_output_comes_back_for_replacement() {
        let mut t = Terminal::new();
        t.input = if cfg!(windows) {
            "findstr \"^\"".into()
        } else {
            "cat".into()
        };
        t.submit(std::env::temp_dir(), Some("piped-text".into()), Some(7));
        wait(&mut t, |t| !t.running());
        let (id, out) = t.replace_out.take().expect("成功的命令把输出交回来");
        assert_eq!(id, 7);
        assert!(out.contains("piped-text"), "{out}");
    }

    #[test]
    fn persistent_shell_keeps_state_between_commands() {
        let mut t = Terminal::new();
        let d = std::env::temp_dir();
        t.input = if cfg!(windows) {
            "$IDEFAST_X = 'kept-value'".into()
        } else {
            "IDEFAST_X=kept-value".into()
        };
        t.submit(d.clone(), None, None);
        assert!(t.busy(), "会话在跑");
        assert!(t.running(), "常驻会话命令执行期间 running() 必须为 true");
        wait(&mut t, |t| !t.running());
        t.input = if cfg!(windows) {
            "Write-Output $IDEFAST_X".into()
        } else {
            "echo $IDEFAST_X".into()
        };
        t.submit(d, None, None);
        wait(&mut t, |t| {
            !t.running() && t.display().matches("kept-value").count() >= 2
        });
        // 第一次出现在本地回显的赋值命令里, 第二次才是 Write-Output 打出来的值: 至少 2 次, 说明变量跨命令保留了
        assert!(
            t.display().matches("kept-value").count() >= 2,
            "{}",
            t.display()
        );
        assert!(
            !t.display().contains(".Path)") && !t.display().contains("]133;IDE10;"),
            "OSC 133 状态序列绝不可泄漏或误转义至终端显示: {}",
            t.display()
        );
        t.restart();
        assert!(!t.busy());
        assert!(!t.running());
        assert!(t.display().contains("会话已重启"));
    }

    #[test]
    fn line_buffer_handles_cr_backspace_and_ansi() {
        let mut b = LineBuffer::new();
        b.feed("abc\rXY\n");
        assert_eq!(b.text(), "XYc\n", "回车后覆盖写");
        b.feed("ab\u{8}Z\n");
        assert_eq!(b.text(), "XYc\naZ\n", "退格后覆盖");
        b.feed("\u{1b}[31mred\u{1b}[0m \u{1b}]0;title\u{7}ok\n");
        assert_eq!(b.text(), "XYc\naZ\nred ok\n", "颜色和标题序列被吃掉");
        b.feed("progress 10%\r\u{1b}[Kdone");
        assert!(b.text().ends_with("done"), "{}", b.text());
        assert!(!b.text().contains("progress"), "擦行后旧内容消失");
    }

    #[test]
    fn escape_sequences_may_be_split_across_chunks() {
        let mut b = LineBuffer::new();
        b.feed("a\u{1b}");
        b.feed("[3");
        b.feed("1mb\n");
        assert_eq!(b.text(), "ab\n");
        b.feed("x\u{1b}[2Jy");
        assert_eq!(b.text(), "y", "2J 清屏");
    }

    #[test]
    fn scrollback_setting_keeps_only_the_newest_lines() {
        let mut b = LineBuffer::new();
        b.set_scrollback(MIN_SCROLLBACK);
        for i in 0..1000 {
            b.feed(&format!("line {i}\n"));
        }
        let t = b.text();
        let n = t.lines().count();
        assert!(
            (MIN_SCROLLBACK..=MIN_SCROLLBACK + MIN_SCROLLBACK / 4 + 1).contains(&n),
            "{n}"
        );
        assert!(t.ends_with("line 999\n") && !t.contains("line 0\n"));
        b.set_scrollback(1000);
        b.set_scrollback(1); // 夹到下限 (不会变成 0 行),并且立刻裁到这个行数
        assert_eq!(b.text().lines().count(), MIN_SCROLLBACK);
        b.clear();
        for _ in 0..500 {
            b.feed("y\n");
        }
        assert!(
            b.text().lines().count() <= MIN_SCROLLBACK + MIN_SCROLLBACK / 4 + 1,
            "clear 之后设置还在"
        );
    }

    #[test]
    fn line_buffer_trims_old_output_when_huge() {
        let mut b = LineBuffer::new();
        let line = "x".repeat(999) + "\n";
        for _ in 0..1200 {
            b.feed(&line);
        }
        assert!(b.text().len() <= MAX_KEEP, "{}", b.text().len());
        assert!(b.text().ends_with(&line));
    }

    #[test]
    fn test_execute_script_sends_to_terminal() {
        let mut t = Terminal::new();
        t.execute_script(&PathBuf::from("."), "Write-Host 'hello from powershell'");
        assert!(t.busy());
        assert!(t.display().contains("Write-Host 'hello from powershell'"));
    }

    #[test]
    fn test_admin_detection_and_prompt_formatting() {
        let is_admin = detect_is_admin();
        let mut t = Terminal::new();
        assert_eq!(t.is_admin, is_admin);

        let cwd = PathBuf::from("E:\\test_dir");
        t.is_admin = true;
        assert_eq!(t.prompt_str(&cwd), "PS [ADMIN] E:\\test_dir> ");

        t.is_admin = false;
        assert_eq!(t.prompt_str(&cwd), "PS E:\\test_dir> ");
    }

    #[test]
    fn test_cancel_input_and_history_navigation() {
        let mut t = Terminal::new();
        let cwd = PathBuf::from("E:\\work");
        t.input = "git commit -m 'test'".into();
        t.cursor_pos = t.input.len();

        // 模拟按下 Ctrl+C 取消输入
        t.cancel_input(&cwd);
        assert!(t.input.is_empty());
        assert_eq!(t.cursor_pos, 0);
        assert!(t.buf.text().contains("git commit -m 'test'^C\n"));

        // 历史记录测试
        t.history = vec!["cargo check".into(), "cargo test".into()];
        t.history_step(true);
        assert_eq!(t.input, "cargo test");
        assert_eq!(t.cursor_pos, "cargo test".len());

        t.history_step(true);
        assert_eq!(t.input, "cargo check");

        t.history_step(false);
        assert_eq!(t.input, "cargo test");
    }

    #[test]
    fn test_tab_complete_matching_files() {
        let mut t = Terminal::new();
        let temp = std::env::temp_dir().join("ide_tab_test");
        let _ = std::fs::create_dir_all(&temp);
        let _ = std::fs::write(temp.join("alpha_one.txt"), "1");
        let _ = std::fs::write(temp.join("alpha_two.txt"), "2");

        t.input = "cat alp".into();
        t.cursor_pos = t.input.len();
        t.tab_complete(&temp);

        // 应当补全为匹配的文件名
        assert!(t.input.starts_with("cat alpha_"));
        assert!(t.input.ends_with(".txt"));

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_ansi_color_parsing_and_layout_job() {
        let mut b = LineBuffer::new();
        // 16色 (红色前景 31, 绿色背景 42), 粗体 (1), 下划线 (4)
        b.feed("\u{1b}[31;42;1;4mError Alert\u{1b}[0m normal\n");
        assert_eq!(b.text(), "Error Alert normal\n");

        let job = b.build_layout_job(12.0, egui::Color32::WHITE, "");
        assert!(job.text.contains("Error Alert normal"));
        // 验证有多种样式分段
        assert!(job.sections.len() >= 2);

        // 256 色测试: 38;5;196 (亮红)
        let mut b2 = LineBuffer::new();
        b2.feed("\u{1b}[38;5;196m256color\u{1b}[0m\n");
        assert_eq!(b2.text(), "256color\n");
        let job2 = b2.build_layout_job(12.0, egui::Color32::WHITE, "");
        assert_eq!(
            job2.sections[0].format.color,
            egui::Color32::from_rgb(255, 0, 0)
        );

        // 24 位 TrueColor 测试: 38;2;123;45;67
        let mut b3 = LineBuffer::new();
        b3.feed("\u{1b}[38;2;123;45;67mTrueColor\u{1b}[0m\n");
        assert_eq!(b3.text(), "TrueColor\n");
        let job3 = b3.build_layout_job(12.0, egui::Color32::WHITE, "");
        assert_eq!(
            job3.sections[0].format.color,
            egui::Color32::from_rgb(123, 45, 67)
        );
    }

    #[test]
    fn test_clean_powershell_clixml() {
        let clixml = "#< CLIXML\r\n<Objs Version=\"1.1.0.1\" xmlns=\"http://schemas.microsoft.com/powershell/2004/04\"><S S=\"Error\">Get-Content : 找不到路径_x000D__x000A_</S><S S=\"Error\">所在的行:1 字符: 1_x000D__x000A_</S></Objs>";
        let cleaned = clean_powershell_clixml(clixml);
        assert!(cleaned.starts_with("\x1b[91m"));
        assert!(cleaned.contains("Get-Content : 找不到路径"));
        assert!(cleaned.contains("所在的行:1 字符: 1"));
        assert!(!cleaned.contains("<Objs"));
        assert!(!cleaned.contains("_x000D__x000A_"));
        assert!(cleaned.contains("\x1b[0m"));
    }

    #[test]
    fn test_cursor_up_reedit_for_spinners_and_hermes() {
        let mut b = LineBuffer::new();
        b.feed("Thinking...\n");
        assert_eq!(b.text(), "Thinking...\n");
        // CLI 发送光标上移 1 行 + 清行 + 覆盖为完成状态
        b.feed("\u{1b}[1A\u{1b}[2KCompleted Response\n");
        assert_eq!(b.text(), "Completed Response\n");
        assert!(!b.text().contains("Thinking"));
    }

    #[test]
    fn test_track_cd_and_session_directory() {
        let mut t = Terminal::new();
        let base = std::env::temp_dir().join("ide_term_cd_test");
        let _ = std::fs::create_dir_all(base.join("sub_dir"));
        t.start_session(&base);
        assert_eq!(t.session_cwd.as_ref(), Some(&base));

        // 模拟执行 cd sub_dir
        t.track_cd("cd sub_dir", &base);
        assert!(t.session_cwd.as_ref().unwrap().ends_with("sub_dir"));
        assert!(t.prompt_str(&base).contains("sub_dir"));

        // 模拟执行 cd ..
        let cur = t.session_cwd.clone().unwrap_or_else(|| base.clone());
        t.track_cd("cd ..", &cur);
        assert_eq!(t.session_cwd.as_ref(), Some(&base));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_layout_job_invalidates_on_same_length_output_replacement() {
        let mut t = Terminal::new();
        t.output = "ok\n".into();
        let fg = egui::Color32::WHITE;
        assert_eq!(t.display(), "ok\n");
        assert_eq!(t.layout_job(13.0, fg).text, "ok\n");

        // 同长度内容替换: 缓存必须立即失效并反映新文本
        t.output = "no\n".into();
        t.output_rev = t.output_rev.wrapping_add(1);
        assert_eq!(t.display(), "no\n");
        assert_eq!(t.layout_job(13.0, fg).text, "no\n");
    }

    #[test]
    fn osc_133_split_across_chunks_completes_and_updates_cwd() {
        let mut b = LineBuffer::new();
        b.feed("\u{1b}]133;ID");
        assert_eq!(b.take_osc_completion(), (false, None));
        b.feed("E10;C:\\Win");
        assert_eq!(b.take_osc_completion(), (false, None));
        b.feed("dows\u{7}");
        assert_eq!(
            b.take_osc_completion(),
            (true, Some("C:\\Windows".to_string()))
        );
        assert!(b.text().is_empty(), "OSC 133 不得残留字符: {:?}", b.text());
        assert_eq!(b.take_osc_completion(), (false, None));
    }

    #[cfg(windows)]
    #[test]
    fn powershell_cd_updates_real_cwd_via_osc_133() {
        let mut t = Terminal::new();
        let base = std::env::temp_dir().join("ide_ps_osc_cd_test");
        let sub = base.join("nested_leaf");
        let _ = std::fs::create_dir_all(&sub);
        t.start_session(&sub);

        t.input = "cd ..".into();
        t.submit(sub.clone(), None, None);
        assert!(t.running());
        wait(&mut t, |t| !t.running());

        let cur = t
            .session_cwd
            .as_ref()
            .expect("OSC 133 应回传更新后的工作目录")
            .to_string_lossy()
            .to_lowercase();
        let expected_name = "ide_ps_osc_cd_test";
        assert!(cur.ends_with(expected_name), "got cwd: {cur}");
        assert!(
            !t.display().contains("133;IDE10") && !t.display().contains(".Path)"),
            "终端输出不得包含控制标记泄漏: {}",
            t.display()
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
