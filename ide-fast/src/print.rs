//! 打印 / 页面设置 / 打印预览: 文档按页面设置分页, 排成带页眉页脚的 HTML 交给系统默认浏览器 (`window.print()` 弹出系统打印对话框)。
//! 应用内的预览窗口用同一套分页, 不自带打印排版引擎, 也不拉任何依赖。

use std::path::PathBuf;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// 纸张
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Paper {
    #[default]
    A4,
    Letter,
    A3,
}

impl Paper {
    /// (宽, 高) 毫米
    pub fn mm(self) -> (f32, f32) {
        match self {
            Paper::A4 => (210.0, 297.0),
            Paper::Letter => (215.9, 279.4),
            Paper::A3 => (297.0, 420.0),
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Paper::A4 => "A4",
            Paper::Letter => "Letter",
            Paper::A3 => "A3",
        }
    }
    pub fn from_code(c: &str) -> Paper {
        match c {
            "Letter" => Paper::Letter,
            "A3" => Paper::A3,
            _ => Paper::A4,
        }
    }
}

/// 页面设置 (打印 / 预览共用, 存进设置文件)
#[derive(Clone, PartialEq, Debug)]
pub struct PageSetup {
    pub paper: Paper,
    pub landscape: bool,
    /// 字号 (pt) 与四周页边距 (mm)
    pub font_pt: f32,
    pub margin_mm: f32,
    pub line_numbers: bool,
    pub header: bool,
    pub footer: bool,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            paper: Paper::A4,
            landscape: false,
            font_pt: 10.0,
            margin_mm: 14.0,
            line_numbers: true,
            header: true,
            footer: true,
        }
    }
}

const LINE_H: f32 = 1.35; // 行高 / 字号
const CHAR_W: f32 = 0.6; // 等宽字宽 / 字号
const MM_PT: f32 = 72.0 / 25.4;

impl PageSetup {
    /// 把从文件里读到的不可信数值夹回合理范围
    pub fn clamped(mut self) -> Self {
        self.font_pt = if self.font_pt.is_finite() {
            self.font_pt.clamp(6.0, 24.0)
        } else {
            10.0
        };
        self.margin_mm = if self.margin_mm.is_finite() {
            self.margin_mm.clamp(5.0, 40.0)
        } else {
            14.0
        };
        self
    }

    fn page_mm(&self) -> (f32, f32) {
        let (w, h) = self.paper.mm();
        if self.landscape {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// 一页能放的 (行数, 每行列数); 页眉 / 页脚各占两行
    pub fn capacity(&self) -> (usize, usize) {
        let (w, h) = self.page_mm();
        let (cw, ch) = (
            (w - 2.0 * self.margin_mm).max(10.0) * MM_PT,
            (h - 2.0 * self.margin_mm).max(10.0) * MM_PT,
        );
        let rows = ((ch / (self.font_pt * LINE_H)) as usize)
            .saturating_sub(2 * (self.header as usize + self.footer as usize));
        let cols = (cw / (self.font_pt * CHAR_W)) as usize;
        (rows.max(1), cols.max(8))
    }
}

/// 显示宽度 (CJK 记 2 列), 用来估计折行
fn width_of(c: char) -> usize {
    if c as u32 >= 0x1100 && !c.is_control() {
        2
    } else {
        1
    }
}

/// 一个逻辑行折成几个显示行 (每行 `cols` 列; Tab 按 4 列)
pub fn wrapped_rows(line: &str, cols: usize) -> usize {
    let (mut rows, mut used) = (1, 0);
    for c in line.chars() {
        let w = if c == '\t' { 4 } else { width_of(c) };
        if used + w > cols {
            rows += 1;
            used = 0;
        }
        used += w;
    }
    rows
}

/// 分页: 每页是 [起始行, 结束行) 的逻辑行区间; 不拆开一个逻辑行
pub fn paginate(
    lines: &[String],
    rows_per_page: usize,
    cols: usize,
) -> Vec<std::ops::Range<usize>> {
    let (mut pages, mut start, mut used) = (vec![], 0, 0);
    for (i, l) in lines.iter().enumerate() {
        let r = wrapped_rows(l, cols);
        if used > 0 && used + r > rows_per_page {
            pages.push(start..i);
            start = i;
            used = 0;
        }
        used += r;
    }
    pages.push(start..lines.len());
    pages
}

/// 打印页: 按页面设置分页, 每页带页眉 (文件名) / 页脚 (页码); `auto_print` = 载入后自动弹出打印对话框
pub fn page_html(name: &str, lines: &[String], ps: &PageSetup, auto_print: bool) -> String {
    let (rows, cols) = ps.capacity();
    let pages = paginate(lines, rows, cols);
    let width = lines.len().to_string().len();
    let mut body = String::new();
    for (pi, r) in pages.iter().enumerate() {
        body.push_str("<section>");
        if ps.header {
            body.push_str(&format!("<h1>{}</h1>", esc(name)));
        }
        body.push_str("<pre>");
        for i in r.clone() {
            if ps.line_numbers {
                body.push_str(&format!("<span class=n>{:>w$}</span>", i + 1, w = width));
            }
            body.push_str(&esc(&lines[i]));
            body.push('\n');
        }
        body.push_str("</pre>");
        if ps.footer {
            body.push_str(&format!("<footer>{} / {}</footer>", pi + 1, pages.len()));
        }
        body.push_str("</section>");
    }
    format!(
        "<!doctype html><meta charset=utf-8><title>{t}</title><style>\
@page{{size:{sz}{ori};margin:{m}mm}}body{{margin:0}}section{{break-after:page}}section:last-child{{break-after:auto}}\
h1{{font:600 {hf}pt sans-serif;margin:0 0 6pt;border-bottom:1px solid #888}}footer{{font:{hf}pt sans-serif;text-align:center;margin-top:6pt;color:#555}}\
pre{{font:{f}pt/{lh} Consolas,'Cascadia Mono',monospace;white-space:pre-wrap;word-break:break-all;tab-size:4;margin:0}}\
.n{{color:#888;margin-right:1em;user-select:none}}</style>{b}{s}",
        t = esc(name),
        sz = ps.paper.code(),
        ori = if ps.landscape { " landscape" } else { "" },
        m = ps.margin_mm,
        hf = (ps.font_pt * 1.1).max(7.0),
        f = ps.font_pt,
        lh = LINE_H,
        b = body,
        s = if auto_print { "<script>addEventListener('load',()=>setTimeout(()=>print(),200))</script>" } else { "" }
    )
}

/// 写临时文件并用系统默认程序打开; 返回文件路径
pub fn open_for_print(name: &str, lines: &[String], ps: &PageSetup) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("ide-fast-print");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dir.join(format!("{safe}.html"));
    std::fs::write(&path, page_html(name, lines, ps, true)).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let r = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.display().to_string()])
            .creation_flags(0x0800_0000)
            .spawn() // CREATE_NO_WINDOW
    };
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&path).spawn();
    r.map_err(|e| format!("无法打开浏览器: {e}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("line {i}")).collect()
    }

    #[test]
    fn page_escapes_html_numbers_lines_and_can_auto_print() {
        let lines = vec!["<b>&\"x\"</b>".to_string(), "第二行".to_string()];
        let h = page_html("a<b>.txt", &lines, &PageSetup::default(), true);
        assert!(h.contains("&lt;b&gt;&amp;&quot;x&quot;&lt;/b&gt;"), "{h}");
        assert!(h.contains("<title>a&lt;b&gt;.txt</title>"));
        assert!(h.contains("<span class=n>1</span>") && h.contains("<span class=n>2</span>"));
        assert!(h.contains("print()") && h.contains("1 / 1"));
        let off = PageSetup {
            line_numbers: false,
            header: false,
            footer: false,
            ..PageSetup::default()
        };
        let h2 = page_html("a", &lines, &off, false);
        assert!(!h2.contains("class=n>1") && !h2.contains("<footer>") && !h2.contains("<h1>"));
    }

    #[test]
    fn pagination_splits_by_rows_and_counts_wrapped_rows() {
        assert_eq!(paginate(&s(25), 10, 80), vec![0..10, 10..20, 20..25]);
        let long = vec!["x".repeat(200), "a".into(), "b".into()];
        assert_eq!(wrapped_rows(&long[0], 80), 3);
        assert_eq!(
            paginate(&long, 3, 80),
            vec![0..1, 1..3],
            "折行后占满一页的长行自成一页"
        );
        assert_eq!(paginate(&[], 10, 80), vec![0..0], "空文档也有一页");
        assert_eq!(wrapped_rows("中中中", 4), 2, "CJK 记两列");
    }

    #[test]
    fn capacity_follows_paper_margin_font_and_orientation() {
        let a4 = PageSetup::default();
        let (r, c) = a4.capacity();
        assert!(r > 40 && r < 80 && c > 70 && c < 120, "{r} {c}");
        let land = PageSetup {
            landscape: true,
            ..a4.clone()
        };
        assert!(land.capacity().1 > c && land.capacity().0 < r);
        assert!(
            PageSetup {
                font_pt: 20.0,
                ..a4.clone()
            }
            .capacity()
            .0 < r
        );
        assert_eq!(
            PageSetup {
                header: false,
                footer: false,
                ..a4
            }
            .capacity()
            .0,
            r + 4
        );
    }

    #[test]
    fn untrusted_setup_numbers_are_clamped() {
        let p = PageSetup {
            font_pt: f32::NAN,
            margin_mm: 1e9,
            ..PageSetup::default()
        }
        .clamped();
        assert_eq!((p.font_pt, p.margin_mm), (10.0, 40.0));
        assert_eq!(Paper::from_code("???"), Paper::A4);
    }
}
