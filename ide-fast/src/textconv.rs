//! 选区 / 文档文字的纯函数: 编码内容转换 (codecnv-*)、Base64 / uuencode 解码、彩色 HTML、控制字符表、日期时间戳。
//! 没有 UI、没有全局状态, 全部可单测; 语义对照 Tauri 版 (`editor.html` 的 CODECNV / decodeBase64Text / decodeUu / colorHtmlOf / stampNow)。

use editcore::document_io::{reinterpret_text, Charset};
use editcore::syntax_styling::Theme;
use editcore::type_config::TypeConfig;

// ---------------------------------------------------------------- codecnv-*

/// (命令后缀, 菜单文字): 和 Tauri 版 / Sakura 的「コード変換」同名同序
#[allow(dead_code)]
pub const CODECNV: &[(&str, &str)] = &[
    ("auto2sjis", "自动判别 → SJIS"),
    ("email", "邮件 (JIS) → SJIS"),
    ("euc2sjis", "EUC → SJIS"),
    ("unicode2sjis", "Unicode → SJIS"),
    ("unicodebe2sjis", "UnicodeBE → SJIS"),
    ("utf82sjis", "UTF-8 → SJIS"),
    ("utf72sjis", "UTF-7 → SJIS"),
    ("sjis2jis", "SJIS → JIS"),
    ("sjis2euc", "SJIS → EUC"),
    ("sjis2utf8", "SJIS → UTF-8"),
    ("sjis2utf7", "SJIS → UTF-7"),
];

/// 命令后缀 -> (按什么编码编回字节, 再按什么编码解读); 解读编码为 None = 自动判别
fn codecnv_pair(id: &str) -> Option<(Charset, Option<Charset>)> {
    use Charset::*;
    Some(match id {
        "auto2sjis" => (ShiftJis, None),
        "email" => (ShiftJis, Some(Iso2022Jp)),
        "euc2sjis" => (ShiftJis, Some(EucJp)),
        "unicode2sjis" => (ShiftJis, Some(Utf16Le)),
        "unicodebe2sjis" => (ShiftJis, Some(Utf16Be)),
        "utf82sjis" => (ShiftJis, Some(Utf8)),
        "utf72sjis" => (ShiftJis, Some(Utf7)),
        "sjis2jis" => (Iso2022Jp, Some(ShiftJis)),
        "sjis2euc" => (EucJp, Some(ShiftJis)),
        "sjis2utf8" => (Utf8, Some(ShiftJis)),
        "sjis2utf7" => (Utf7, Some(ShiftJis)),
        _ => return None,
    })
}

/// 对一段文字做编码内容转换 (Sakura「コード変換」: 编回字节再换一种编码解读)。
/// 自动判别依次试 EUC-JP / UTF-8 / JIS, 取第一个没有替换符且确实变了的结果。
pub fn codecnv(id: &str, text: &str) -> Result<String, String> {
    let (enc, dec) = codecnv_pair(id).ok_or_else(|| format!("未知的编码转换: {id}"))?;
    let clean = |e: String| e.trim_start_matches("UNMAPPABLE:").to_string();
    match dec {
        Some(dec) => reinterpret_text(text, enc, dec).map_err(clean),
        None => {
            for cand in [Charset::EucJp, Charset::Utf8, Charset::Iso2022Jp] {
                if let Ok(o) = reinterpret_text(text, enc, cand) {
                    if !o.contains('\u{FFFD}') && o != text {
                        return Ok(o);
                    }
                }
            }
            Err("自动判别: 没有找到合适的编码".into())
        }
    }
}

// ---------------------------------------------------------------- Base64 / uuencode

/// 标准 / URL 安全 base64 解码: 忽略空白, `=` 之后的都不要; 其他字符 = 错
pub fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits, mut n) = (0u32, 0u32, 0usize);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return Err("不是合法的 Base64 (含有字母数字 + / = 以外的字符)".into()),
        };
        n += 1;
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if n == 0 {
        return Err("没有可解码的 Base64 内容".into());
    }
    if n % 4 == 1 {
        return Err("Base64 长度不对 (被截断了?)".into());
    }
    Ok(out)
}

/// uuencode 解码 (忽略 `begin` / `end` / 单独的反引号行); 每行首字符是该行字节数
pub fn uudecode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut any = false;
    for raw in s.lines() {
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with("begin ") || line == "end" || line == "`" {
            continue;
        }
        let b = line.as_bytes();
        let n = ((b[0].wrapping_sub(32)) & 63) as usize;
        if n == 0 {
            continue;
        }
        if b.iter().any(|&c| !(32..=96).contains(&c)) {
            return Err("不是合法的 uuencode (含有范围外的字符)".into());
        }
        any = true;
        let mut bytes = Vec::with_capacity(n + 3);
        let mut i = 1;
        while bytes.len() < n {
            let g = |k: usize| (b.get(i + k).copied().unwrap_or(32).wrapping_sub(32)) & 63;
            let (c0, c1, c2, c3) = (g(0), g(1), g(2), g(3));
            bytes.extend([(c0 << 2) | (c1 >> 4), (c1 << 4) | (c2 >> 2), (c2 << 6) | c3]);
            i += 4;
            if i > b.len() + 3 {
                break;
            }
        }
        bytes.truncate(n);
        out.extend(bytes);
    }
    if !any {
        return Err("没有可解码的 uuencode 内容".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------- 彩色 HTML (copy-html)

fn esc(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("    "),
            c => out.push(c),
        }
    }
}

/// 选中的这些行变成带颜色的 `<pre>`; `first_no` 是第一行的行号 (从 0 起), `numbers` = 每行前面带行号
pub fn color_html(lines: &[String], first_no: usize, numbers: bool, cfg: Option<&TypeConfig>, theme: &Theme, default: [u8; 3]) -> String {
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let width = (first_no + lines.len()).to_string().len();
    let mut out = String::from("<pre style=\"font-family:Consolas,monospace\">");
    for (k, line) in lines.iter().enumerate() {
        if k > 0 {
            out.push('\n');
        }
        if numbers {
            out.push_str(&format!("<span style=\"color:#888888\">{:>width$} </span>", first_no + k + 1));
        }
        let chars: Vec<char> = line.chars().collect();
        let mut at = 0usize;
        let span = |from: usize, to: usize, color: [u8; 3], out: &mut String| {
            if to > from {
                let seg: String = chars[from..to].iter().collect();
                out.push_str(&format!("<span style=\"color:{}\">", hex(color)));
                esc(&seg, out);
                out.push_str("</span>");
            }
        };
        if let Some(cfg) = cfg {
            for (s, e, kind) in cfg.highlight_line(line) {
                let (s, e) = ((s.max(0) as usize).min(chars.len()), (e.max(0) as usize).min(chars.len()));
                if s < at || e <= s {
                    continue;
                }
                span(at, s, default, &mut out);
                let c = theme.get(kind).fg;
                span(s, e, [c.0, c.1, c.2], &mut out);
                at = e;
            }
        }
        span(at, chars.len(), default, &mut out);
    }
    out.push_str("</pre>");
    out
}

// ---------------------------------------------------------------- 控制字符表

/// 控制字符名 (0..=31 之后是 DEL)
pub const CTRL_NAMES: [&str; 33] = [
    "NUL", "SOH", "STX", "ETX", "EOT", "ENQ", "ACK", "BEL", "BS", "TAB", "LF", "VT", "FF", "CR", "SO", "SI", "DLE", "DC1", "DC2", "DC3", "DC4", "NAK", "SYN", "ETB", "CAN", "EM",
    "SUB", "ESC", "FS", "GS", "RS", "US", "DEL",
];

/// 控制字符表第 i 项 (0..=32) 对应的字符: 0..=31 原码, 32 = DEL (0x7F)
pub fn ctrl_char(i: usize) -> Option<char> {
    match i {
        0..=31 => char::from_u32(i as u32),
        32 => Some('\u{7f}'),
        _ => None,
    }
}

// ---------------------------------------------------------------- 日期 / 时间 / 戳记

/// 墙上时间的各个分量 (本地时区)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Clock {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// 现在的本地时间 (chrono 的 `clock` 特性在 Windows 上走 GetLocalTime, 不需要 unsafe)
pub fn local_now() -> Clock {
    use chrono::{Datelike, Timelike};
    let n = chrono::Local::now();
    Clock { year: n.year(), month: n.month(), day: n.day(), hour: n.hour(), minute: n.minute(), second: n.second() }
}

impl Clock {
    /// `YYYY/MM/DD`
    pub fn date(&self) -> String {
        format!("{:04}/{:02}/{:02}", self.year, self.month, self.day)
    }

    /// `HH:MM:SS`
    pub fn time(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

/// 戳记内容
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StampKind {
    Date,
    Time,
    #[default]
    DateTime,
}

/// 戳记放在哪: 行首 (缩进之后, 带方括号) / 光标处
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StampPos {
    #[default]
    LineStart,
    AtCursor,
}

impl StampKind {
    pub fn from_u8(n: u8) -> Self {
        match n { 0 => Self::Date, 1 => Self::Time, _ => Self::DateTime }
    }

    pub fn to_u8(self) -> u8 {
        match self { Self::Date => 0, Self::Time => 1, Self::DateTime => 2 }
    }

    pub fn text(self, c: &Clock) -> String {
        match self {
            Self::Date => c.date(),
            Self::Time => c.time(),
            Self::DateTime => format!("{} {}", c.date(), c.time()),
        }
    }
}

impl StampPos {
    pub fn from_u8(n: u8) -> Self {
        if n == 1 { Self::AtCursor } else { Self::LineStart }
    }

    pub fn to_u8(self) -> u8 {
        match self { Self::LineStart => 0, Self::AtCursor => 1 }
    }
}

/// 行首戳记: 在缩进之后插入 `[戳记] `; 返回新行和插入的字符数 (光标在缩进之后要往后挪这么多)
pub fn stamp_line_start(line: &str, stamp: &str) -> (String, usize) {
    let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let ins = format!("[{stamp}] ");
    let n = ins.chars().count();
    (format!("{indent}{ins}{}", &line[indent.len()..]), n)
}

/// 「复制路径与光标位置」的文字: `路径(行,列)`, 行列从 1 数 (同 Tauri `copy-tag`; 没存盘的文档用文档名)
pub fn path_pos_tag(path_or_name: &str, cursor: [usize; 2]) -> String {
    format!("{}({},{})", path_or_name, cursor[0] + 1, cursor[1] + 1)
}

/// 「在折行处加换行复制」: 每一行按字符数 `cols` 切开, 切口和原有的换行都写成 CRLF; 空行保持空行 (同 Tauri `copy-addcrlf`)
pub fn wrap_with_crlf(text: &str, cols: usize) -> String {
    let cols = cols.max(1);
    text.split('\n')
        .map(|line| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let cs: Vec<char> = line.chars().collect();
            cs.chunks(cols).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("\r\n")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// 「打开同名 C/C++ 头 / 源文件」的候选路径, 按尝试顺序: 源文件 (c cpp cc cxx) 找头文件 (h hpp hh hxx), 头文件找源文件, 其它扩展名当作源文件找头文件
/// (同 Tauri `open-hc`); 没有扩展名 = 没有候选
pub fn hc_counterparts(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    const SRC: [&str; 4] = ["c", "cpp", "cc", "cxx"];
    const HDR: [&str; 4] = ["h", "hpp", "hh", "hxx"];
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else { return vec![] };
    let want = if HDR.contains(&ext.to_ascii_lowercase().as_str()) { SRC } else { HDR };
    want.iter().map(|e| path.with_extension(e)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sjis_garble(s: &str, as_enc: Charset) -> String {
        // 把 s 当成 as_enc 的字节, 再按 SJIS 解读 (模拟被误读成 SJIS 的文本)
        reinterpret_text(s, as_enc, Charset::ShiftJis).unwrap()
    }

    #[test]
    fn codecnv_table_covers_every_pair_and_rejects_unknown() {
        for (id, _) in CODECNV {
            assert!(codecnv_pair(id).is_some(), "{id}");
        }
        assert!(codecnv("nope", "x").is_err());
    }

    #[test]
    fn mojibake_is_repaired_and_round_trips() {
        // 注意: EUC 的假名字节都在 0xA1-0xDF 之内才能被 SJIS 无损读成半角假名 (如「ん」0xA4F3 不行)
        let text = "あいうえお";
        // EUC 的字节被当 SJIS 读成乱码 -> euc2sjis 还原
        let garbled = reinterpret_text(text, Charset::EucJp, Charset::ShiftJis).unwrap();
        assert_ne!(garbled, text);
        assert_eq!(codecnv("euc2sjis", &garbled).unwrap(), text);
        // UTF-8 同理
        let g8 = sjis_garble("こんにちは", Charset::Utf8);
        assert_eq!(codecnv("utf82sjis", &g8).unwrap(), "こんにちは");
        // 反向: SJIS -> EUC 之后再 euc2sjis 回到原文
        let to_euc = codecnv("sjis2euc", text).unwrap();
        assert_eq!(codecnv("euc2sjis", &to_euc).unwrap(), text);
        // 邮件 (JIS 转义序列)
        let to_jis = codecnv("sjis2jis", text).unwrap();
        assert!(to_jis.contains('\u{1b}'), "JIS 带转义序列");
        assert_eq!(codecnv("email", &to_jis).unwrap(), text);
    }

    #[test]
    fn auto_detect_picks_the_first_clean_candidate_or_fails() {
        let garbled = reinterpret_text("あいうえお", Charset::EucJp, Charset::ShiftJis).unwrap();
        assert_eq!(codecnv("auto2sjis", &garbled).unwrap(), "あいうえお");
        assert!(codecnv("auto2sjis", "plain ascii").is_err(), "没变化 = 找不到合适的编码");
    }

    #[test]
    fn unmappable_chars_report_without_internal_prefix() {
        let e = codecnv("sjis2euc", "😀").unwrap_err();
        assert!(!e.starts_with("UNMAPPABLE"), "{e}");
    }

    #[test]
    fn base64_decodes_variants_and_rejects_garbage() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGVs\r\nbG8").unwrap(), b"hello", "空白 / 缺填充都行");
        assert_eq!(base64_decode("-_8=").unwrap(), vec![0xfb, 0xff], "URL 安全字母表");
        assert!(base64_decode("a$b=").is_err());
        assert!(base64_decode("   ").is_err());
        assert!(base64_decode("A").is_err(), "单个字符凑不出一个字节");
    }

    #[test]
    fn uudecode_handles_headers_and_partial_lines() {
        let enc = "begin 644 a.txt\n#86)C\n`\nend\n";
        assert_eq!(uudecode(enc).unwrap(), b"abc");
        assert_eq!(uudecode("#86)C").unwrap(), b"abc", "没有 begin / end 也行");
        // 61 字节一行 + 余下: 用 "Cat" 经典例子
        assert_eq!(uudecode("#0V%T\n").unwrap(), b"Cat");
        assert!(uudecode("begin 644 x\nend\n").is_err());
        assert!(uudecode("#\u{e9}\u{e9}\u{e9}\u{e9}").is_err());
    }

    #[test]
    fn color_html_escapes_numbers_and_colors() {
        let lines = vec!["a<b & \"c\"".to_string(), "\tx".to_string()];
        let html = color_html(&lines, 8, true, None, &Theme::dark(), [1, 2, 3]);
        assert!(html.starts_with("<pre") && html.ends_with("</pre>"));
        assert!(html.contains("a&lt;b &amp; &quot;c&quot;"), "{html}");
        assert!(html.contains("color:#010203"));
        assert!(html.contains(" 9 </span>") && html.contains("10 </span>"), "行号右对齐: {html}");
        assert!(html.contains("    x"), "Tab 展开");
        assert_eq!(html.matches('\n').count(), 1);
        let rust = editcore::type_config::TypeRegistry::default_registry();
        let cfg = rust.detect("a.rs").cloned();
        let h2 = color_html(&["fn main() {}".to_string()], 0, false, cfg.as_ref(), &Theme::dark(), [200, 200, 200]);
        assert!(h2.matches("<span").count() >= 2, "关键字单独上色: {h2}");
    }

    #[test]
    fn control_char_table_is_complete() {
        assert_eq!(CTRL_NAMES.len(), 33);
        assert_eq!(ctrl_char(0), Some('\0'));
        assert_eq!(ctrl_char(27), Some('\u{1b}'));
        assert_eq!(CTRL_NAMES[27], "ESC");
        assert_eq!(ctrl_char(32), Some('\u{7f}'));
        assert_eq!(CTRL_NAMES[32], "DEL");
        assert_eq!(ctrl_char(33), None);
    }

    #[test]
    fn clock_formats_and_stamp_kinds() {
        let c = Clock { year: 2026, month: 3, day: 7, hour: 4, minute: 5, second: 9 };
        assert_eq!(c.date(), "2026/03/07");
        assert_eq!(c.time(), "04:05:09");
        assert_eq!(StampKind::DateTime.text(&c), "2026/03/07 04:05:09");
        assert_eq!(StampKind::Date.text(&c), "2026/03/07");
        for k in [StampKind::Date, StampKind::Time, StampKind::DateTime] {
            assert_eq!(StampKind::from_u8(k.to_u8()), k);
        }
        assert_eq!(StampPos::from_u8(StampPos::AtCursor.to_u8()), StampPos::AtCursor);
        assert_eq!(StampKind::from_u8(200), StampKind::DateTime, "settings.json 里的坏值回到默认");
    }

    #[test]
    fn local_clock_is_plausible() {
        let c = local_now();
        assert!(c.year >= 2024 && (1..=12).contains(&c.month) && (1..=31).contains(&c.day) && c.hour < 24 && c.minute < 60 && c.second < 62);
    }

    #[test]
    fn line_start_stamp_goes_after_the_indent() {
        assert_eq!(stamp_line_start("  \tfoo", "T"), ("  \t[T] foo".to_string(), 4));
        assert_eq!(stamp_line_start("", "T"), ("[T] ".to_string(), 4));
        assert_eq!(stamp_line_start("日本", "T").0, "[T] 日本");
    }

    #[test]
    fn path_and_position_tag_is_one_based() {
        assert_eq!(path_pos_tag("C:\\src\\a.rs", [0, 0]), "C:\\src\\a.rs(1,1)");
        assert_eq!(path_pos_tag("无标题", [11, 4]), "无标题(12,5)");
    }

    #[test]
    fn wrapping_copy_cuts_by_characters_and_uses_crlf() {
        assert_eq!(wrap_with_crlf("abcdefg", 3), "abc\r\ndef\r\ng");
        assert_eq!(wrap_with_crlf("ab\n\ncd", 3), "ab\r\n\r\ncd", "原有的换行和空行保持");
        assert_eq!(wrap_with_crlf("日本語テキスト", 3), "日本語\r\nテキス\r\nト", "按字符数切, 不按字节");
        assert_eq!(wrap_with_crlf("abcd\r\nef", 2), "ab\r\ncd\r\nef", "已有的 CRLF 不重复");
        assert_eq!(wrap_with_crlf("abc", 0), "a\r\nb\r\nc", "列数为 0 当作 1, 不死循环");
    }

    #[test]
    fn header_and_source_counterparts() {
        use std::path::{Path, PathBuf};
        let p = |v: &[&str]| v.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(hc_counterparts(Path::new("d/foo.cpp")), p(&["d/foo.h", "d/foo.hpp", "d/foo.hh", "d/foo.hxx"]));
        assert_eq!(hc_counterparts(Path::new("d/foo.H")), p(&["d/foo.c", "d/foo.cpp", "d/foo.cc", "d/foo.cxx"]), "扩展名不分大小写");
        assert_eq!(hc_counterparts(Path::new("foo.hpp"))[0], PathBuf::from("foo.c"));
        assert_eq!(hc_counterparts(Path::new("a.b/foo.txt"))[0], PathBuf::from("a.b/foo.h"), "其它扩展名当作源文件");
        assert!(hc_counterparts(Path::new("Makefile")).is_empty());
    }
}
