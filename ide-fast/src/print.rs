//! 打印 / 打印预览: 把文档排成一页带打印样式的 HTML, 交给系统默认浏览器 (`window.print()` 弹出系统打印对话框, 里面就是预览)。
//! 不自带打印排版引擎, 也不拉任何依赖; 浏览器负责分页、纸张、打印机选择。

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

/// 打印页: 页眉文件名, 每行带行号 (可关), 长行折行; `auto_print` = 载入后自动弹出打印对话框
pub fn page_html(name: &str, lines: &[String], line_numbers: bool, font_pt: f32, auto_print: bool) -> String {
    let mut body = String::new();
    let width = lines.len().to_string().len();
    for (i, l) in lines.iter().enumerate() {
        if line_numbers {
            body.push_str(&format!("<span class=n>{:>w$}</span>", i + 1, w = width));
        }
        body.push_str(&esc(l));
        body.push('\n');
    }
    format!(
        "<!doctype html><meta charset=utf-8><title>{t}</title><style>\
@page{{margin:14mm}}body{{margin:0}}h1{{font:600 11pt sans-serif;margin:0 0 6pt;border-bottom:1px solid #888}}\
pre{{font:{f}pt/1.35 Consolas,'Cascadia Mono',monospace;white-space:pre-wrap;word-break:break-all;tab-size:4;margin:0}}\
.n{{color:#888;margin-right:1em;user-select:none}}</style><h1>{t}</h1><pre>{b}</pre>{s}",
        t = esc(name),
        f = font_pt,
        b = body,
        s = if auto_print { "<script>addEventListener('load',()=>setTimeout(()=>print(),200))</script>" } else { "" }
    )
}

/// 写临时文件并用系统默认程序打开; 返回文件路径
pub fn open_for_print(name: &str, lines: &[String], line_numbers: bool, font_pt: f32) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join("ide-fast-print");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect();
    let path = dir.join(format!("{safe}.html"));
    std::fs::write(&path, page_html(name, lines, line_numbers, font_pt, true)).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let r = std::process::Command::new("cmd").args(["/C", "start", "", &path.display().to_string()]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&path).spawn();
    r.map_err(|e| format!("无法打开浏览器: {e}"))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_escapes_html_numbers_lines_and_can_auto_print() {
        let lines = vec!["<b>&\"x\"</b>".to_string(), "第二行".to_string()];
        let h = page_html("a<b>.txt", &lines, true, 10.0, true);
        assert!(h.contains("&lt;b&gt;&amp;&quot;x&quot;&lt;/b&gt;"), "{h}");
        assert!(h.contains("<title>a&lt;b&gt;.txt</title>"));
        assert!(h.contains("<span class=n>1</span>") && h.contains("<span class=n>2</span>"));
        assert!(h.contains("print()"));
        assert!(!page_html("a", &lines, false, 10.0, false).contains("class=n>1"));
    }
}
