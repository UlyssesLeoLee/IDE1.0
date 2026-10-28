//! 文档级读写: 自动判别编码 (Sakura 的 SJIS/JIS/EUC/UTF-8/UTF-16/CESU-8/UTF-7/Latin1/代码页),
//! 记住 BOM 与换行 (CRLF/LF/CR), 保存时原样写回。
//!
//! 内部统一用 LF 的 `String` 表示文档文本; 磁盘上的编码/BOM/换行作为元数据随文档走,
//! 这样「按 X 编码重新打开」「改保存编码」「改换行」都只是改元数据。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Charset {
    Utf8,
    ShiftJis,
    EucJp,
    Iso2022Jp,
    Utf16Le,
    Utf16Be,
    Latin1,
    Cesu8,
    Utf7,
    Gbk,
    Big5,
    EucKr,
}

impl Charset {
    pub const ALL: [Charset; 12] = [
        Charset::Utf8, Charset::ShiftJis, Charset::EucJp, Charset::Iso2022Jp, Charset::Utf16Le, Charset::Utf16Be,
        Charset::Latin1, Charset::Cesu8, Charset::Utf7, Charset::Gbk, Charset::Big5, Charset::EucKr,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::ShiftJis => "SJIS",
            Self::EucJp => "EUC-JP",
            Self::Iso2022Jp => "JIS",
            Self::Utf16Le => "Unicode",
            Self::Utf16Be => "UnicodeBE",
            Self::Latin1 => "Latin1",
            Self::Cesu8 => "CESU-8",
            Self::Utf7 => "UTF-7",
            Self::Gbk => "GBK",
            Self::Big5 => "Big5",
            Self::EucKr => "EUC-KR",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        let u = s.trim().to_ascii_uppercase().replace('_', "-");
        Some(match u.as_str() {
            "UTF-8" | "UTF8" => Self::Utf8,
            "SJIS" | "SHIFT-JIS" | "SHIFTJIS" | "CP932" | "MS932" | "WINDOWS-31J" => Self::ShiftJis,
            "EUC-JP" | "EUCJP" | "EUC" => Self::EucJp,
            "JIS" | "ISO-2022-JP" | "ISO2022JP" => Self::Iso2022Jp,
            "UNICODE" | "UTF-16LE" | "UTF16LE" | "UTF-16" => Self::Utf16Le,
            "UNICODEBE" | "UTF-16BE" | "UTF16BE" => Self::Utf16Be,
            "LATIN1" | "ISO-8859-1" | "WINDOWS-1252" | "CP1252" => Self::Latin1,
            "CESU-8" | "CESU8" => Self::Cesu8,
            "UTF-7" | "UTF7" => Self::Utf7,
            "GBK" | "GB2312" | "CP936" => Self::Gbk,
            "BIG5" | "CP950" => Self::Big5,
            "EUC-KR" | "EUCKR" | "CP949" => Self::EucKr,
            _ => return None,
        })
    }

    fn rs(self) -> Option<&'static encoding_rs::Encoding> {
        use encoding_rs as e;
        Some(match self {
            Self::Utf8 => e::UTF_8,
            Self::ShiftJis => e::SHIFT_JIS,
            Self::EucJp => e::EUC_JP,
            Self::Iso2022Jp => e::ISO_2022_JP,
            Self::Utf16Le => e::UTF_16LE,
            Self::Utf16Be => e::UTF_16BE,
            Self::Latin1 => e::WINDOWS_1252,
            Self::Gbk => e::GBK,
            Self::Big5 => e::BIG5,
            Self::EucKr => e::EUC_KR,
            Self::Cesu8 | Self::Utf7 => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Bom {
    None,
    Utf8,
    Utf16Le,
    Utf16Be,
}

impl Bom {
    fn bytes(self) -> &'static [u8] {
        match self {
            Bom::None => &[],
            Bom::Utf8 => &[0xEF, 0xBB, 0xBF],
            Bom::Utf16Le => &[0xFF, 0xFE],
            Bom::Utf16Be => &[0xFE, 0xFF],
        }
    }
    pub fn from_label(s: &str) -> Option<Self> {
        Some(match s {
            "none" | "" => Bom::None,
            "utf8" | "utf-8" => Bom::Utf8,
            "utf16le" | "utf-16le" => Bom::Utf16Le,
            "utf16be" | "utf-16be" => Bom::Utf16Be,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Eol {
    Crlf,
    Lf,
    Cr,
}

impl Eol {
    pub fn label(self) -> &'static str {
        match self {
            Eol::Crlf => "CRLF",
            Eol::Lf => "LF",
            Eol::Cr => "CR",
        }
    }
    pub fn from_label(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "crlf" => Eol::Crlf,
            "lf" => Eol::Lf,
            "cr" => Eol::Cr,
            _ => return None,
        })
    }
    pub fn seq(self) -> &'static str {
        match self {
            Eol::Crlf => "\r\n",
            Eol::Lf => "\n",
            Eol::Cr => "\r",
        }
    }
}

/// 出现最多的换行种类 (没有换行 → LF); 第二个值 = 是否混用。
pub fn detect_eol(s: &str) -> (Eol, bool) {
    let b = s.as_bytes();
    let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\r' if i + 1 < b.len() && b[i + 1] == b'\n' => { crlf += 1; i += 1; }
            b'\r' => cr += 1,
            b'\n' => lf += 1,
            _ => {}
        }
        i += 1;
    }
    let kinds = (crlf > 0) as u8 + (lf > 0) as u8 + (cr > 0) as u8;
    let eol = if crlf >= lf && crlf >= cr && crlf > 0 { Eol::Crlf } else if cr > lf { Eol::Cr } else { Eol::Lf };
    (eol, kinds > 1)
}

pub fn normalize_to_lf(s: &str) -> String {
    if !s.contains('\r') { return s.to_string(); }
    s.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn apply_eol(lf: &str, eol: Eol) -> String {
    match eol {
        Eol::Lf => lf.to_string(),
        _ => lf.replace('\n', eol.seq()),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DocFile {
    /// LF 换行的文本
    pub content: String,
    pub charset: Charset,
    pub bom: Bom,
    pub eol: Eol,
    pub mixed_eol: bool,
    /// 解码时遇到了非法字节 (已用 U+FFFD 替代)
    pub lossy: bool,
}

// ------------------------------------------------------------------ 判别 ---
fn sniff_bom(b: &[u8]) -> (Option<Charset>, Bom) {
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) { (Some(Charset::Utf8), Bom::Utf8) }
    else if b.starts_with(&[0xFF, 0xFE]) { (Some(Charset::Utf16Le), Bom::Utf16Le) }
    else if b.starts_with(&[0xFE, 0xFF]) { (Some(Charset::Utf16Be), Bom::Utf16Be) }
    else { (None, Bom::None) }
}

fn halfwidth_kana_ratio(s: &str) -> f64 {
    let total = s.chars().count().max(1) as f64;
    s.chars().filter(|c| ('\u{FF61}'..='\u{FF9F}').contains(c)).count() as f64 / total
}

pub fn detect_charset(b: &[u8]) -> Charset {
    if let (Some(c), _) = sniff_bom(b) { return c; }
    // ISO-2022-JP 是 7 位编码, 本身就是合法 UTF-8, 所以必须先于 UTF-8 判断
    if b.windows(3).any(|w| w == b"\x1b$B" || w == b"\x1b$@" || w == b"\x1b(J") { return Charset::Iso2022Jp; }
    if b.is_empty() || std::str::from_utf8(b).is_ok() { return Charset::Utf8; }
    let (sj, sj_err) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(b);
    let (eu, eu_err) = encoding_rs::EUC_JP.decode_without_bom_handling(b);
    match (sj_err, eu_err) {
        (false, true) => Charset::ShiftJis,
        (true, false) => Charset::EucJp,
        (false, false) => if halfwidth_kana_ratio(&eu) < halfwidth_kana_ratio(&sj) { Charset::EucJp } else { Charset::ShiftJis },
        (true, true) => {
            let (_, gbk_err) = encoding_rs::GBK.decode_without_bom_handling(b);
            if !gbk_err { Charset::Gbk } else { Charset::Latin1 }
        }
    }
}

// ------------------------------------------------------------- 手写编解码 ---
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn utf7_decode(b: &[u8]) -> (String, bool) {
    let mut out = String::new();
    let mut lossy = false;
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'+' { out.push(b[i] as char); i += 1; continue; }
        i += 1;
        if i < b.len() && b[i] == b'-' { out.push('+'); i += 1; continue; }
        let (mut acc, mut bits) = (0u32, 0u32);
        let mut units: Vec<u16> = Vec::new();
        while i < b.len() {
            let c = b[i];
            let v = match B64.iter().position(|&x| x == c) { Some(v) => v as u32, None => break };
            acc = (acc << 6) | v; bits += 6; i += 1;
            if bits >= 16 { bits -= 16; units.push(((acc >> bits) & 0xFFFF) as u16); acc &= (1 << bits) - 1; }
        }
        if i < b.len() && b[i] == b'-' { i += 1; }
        let s = String::from_utf16_lossy(&units);
        if s.contains('\u{FFFD}') { lossy = true; }
        out.push_str(&s);
    }
    (out, lossy)
}

fn utf7_encode(s: &str) -> Vec<u8> {
    fn direct(c: char) -> bool { c.is_ascii_alphanumeric() || "'(),-./:? \t\r\n".contains(c) }
    let mut out = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '+' { out.extend_from_slice(b"+-"); i += 1; continue; }
        if direct(c) { out.push(c as u8); i += 1; continue; }
        let mut units: Vec<u16> = Vec::new();
        while i < chars.len() && !direct(chars[i]) && chars[i] != '+' {
            let mut buf = [0u16; 2];
            units.extend_from_slice(chars[i].encode_utf16(&mut buf));
            i += 1;
        }
        let (mut acc, mut bits) = (0u32, 0u32);
        out.push(b'+');
        for u in units {
            acc = (acc << 16) | u as u32; bits += 16;
            while bits >= 6 { bits -= 6; out.push(B64[((acc >> bits) & 0x3F) as usize]); acc &= (1 << bits) - 1; }
        }
        if bits > 0 { out.push(B64[((acc << (6 - bits)) & 0x3F) as usize]); }
        if i < chars.len() && (B64.contains(&(chars[i] as u8)) && chars[i].is_ascii()) || chars.get(i) == Some(&'-') { out.push(b'-'); }
    }
    out
}

fn cesu8_decode(b: &[u8]) -> (String, bool) {
    let mut units: Vec<u16> = Vec::new();
    let mut lossy = false;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let (cp, n) = if c < 0x80 { (c as u32, 1) }
        else if c & 0xE0 == 0xC0 && i + 1 < b.len() { (((c as u32 & 0x1F) << 6) | (b[i + 1] as u32 & 0x3F), 2) }
        else if c & 0xF0 == 0xE0 && i + 2 < b.len() { (((c as u32 & 0x0F) << 12) | ((b[i + 1] as u32 & 0x3F) << 6) | (b[i + 2] as u32 & 0x3F), 3) }
        else if c & 0xF8 == 0xF0 && i + 3 < b.len() {          // 真正的 4 字节 UTF-8 也容忍
            let cp = ((c as u32 & 0x07) << 18) | ((b[i + 1] as u32 & 0x3F) << 12) | ((b[i + 2] as u32 & 0x3F) << 6) | (b[i + 3] as u32 & 0x3F);
            (cp, 4)
        } else { lossy = true; (0xFFFD, 1) };
        i += n;
        if cp >= 0x10000 { let v = cp - 0x10000; units.push(0xD800 + (v >> 10) as u16); units.push(0xDC00 + (v & 0x3FF) as u16); }
        else { units.push(cp as u16); }
    }
    (String::from_utf16_lossy(&units), lossy)
}

fn cesu8_encode(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for u in s.encode_utf16() {
        let u = u as u32;
        if u < 0x80 { out.push(u as u8); }
        else if u < 0x800 { out.push(0xC0 | (u >> 6) as u8); out.push(0x80 | (u & 0x3F) as u8); }
        else { out.push(0xE0 | (u >> 12) as u8); out.push(0x80 | ((u >> 6) & 0x3F) as u8); out.push(0x80 | (u & 0x3F) as u8); }
    }
    out
}

// ------------------------------------------------------------------ 公开 ---
/// 解码磁盘字节. `forced` = 用户指定编码 (按 X 编码重开); None = 自动判别。
pub fn decode_document(bytes: &[u8], forced: Option<Charset>) -> DocFile {
    let (bom_cs, bom) = sniff_bom(bytes);
    let charset = forced.unwrap_or_else(|| detect_charset(bytes));
    // BOM 只在与编码相容时才剥掉 (强制 SJIS 打开一个带 UTF-8 BOM 的文件时, BOM 当普通字节解码)
    let strip = match (bom_cs, charset) {
        (Some(a), b) if a == b => bom.bytes().len(),
        _ => 0,
    };
    let body = &bytes[strip..];
    let bom = if strip > 0 { bom } else { Bom::None };
    let (text, lossy) = match charset {
        Charset::Utf7 => utf7_decode(body),
        Charset::Cesu8 => cesu8_decode(body),
        cs => {
            let (s, err) = cs.rs().expect("rs encoding").decode_without_bom_handling(body);
            (s.into_owned(), err)
        }
    };
    let (eol, mixed) = detect_eol(&text);
    DocFile { content: normalize_to_lf(&text), charset, bom, eol, mixed_eol: mixed, lossy }
}

/// 把 LF 文本编码成磁盘字节. 有字符在目标编码里无法表示时返回这些字符 (调用方决定是否仍要写)。
pub fn encode_document(lf_text: &str, charset: Charset, bom: Bom, eol: Eol) -> Result<Vec<u8>, Vec<char>> {
    let text = apply_eol(lf_text, eol);
    let mut out: Vec<u8> = bom.bytes().to_vec();
    match charset {
        Charset::Utf8 => out.extend_from_slice(text.as_bytes()),
        Charset::Cesu8 => out.extend(cesu8_encode(&text)),
        Charset::Utf7 => out.extend(utf7_encode(&text)),
        Charset::Utf16Le => for u in text.encode_utf16() { out.extend_from_slice(&u.to_le_bytes()); },
        Charset::Utf16Be => for u in text.encode_utf16() { out.extend_from_slice(&u.to_be_bytes()); },
        cs => {
            let enc = cs.rs().expect("rs encoding");
            let (bytes, _, had) = enc.encode(&text);
            if had {
                let mut bad: Vec<char> = Vec::new();
                for ch in text.chars() {
                    let mut b = [0u8; 4];
                    let (_, _, e) = enc.encode(ch.encode_utf8(&mut b));
                    if e && !bad.contains(&ch) { bad.push(ch); if bad.len() >= 20 { break; } }
                }
                return Err(bad);
            }
            out.extend_from_slice(&bytes);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JP: &str = "日本語のテキスト\nこんにちは世界ｱｲｳ\n";

    #[test]
    fn roundtrip_every_charset() {
        for cs in [Charset::Utf8, Charset::ShiftJis, Charset::EucJp, Charset::Iso2022Jp, Charset::Utf16Le, Charset::Utf16Be, Charset::Cesu8, Charset::Utf7] {
            let text = if cs == Charset::Iso2022Jp { "日本語のテキスト\nこんにちは世界\n" } else { JP };
            let bom = match cs { Charset::Utf16Le => Bom::Utf16Le, Charset::Utf16Be => Bom::Utf16Be, _ => Bom::None };
            let bytes = encode_document(text, cs, bom, Eol::Crlf).unwrap_or_else(|e| panic!("{cs:?} encode: {e:?}"));
            let d = decode_document(&bytes, Some(cs));
            assert_eq!(d.content, text, "{cs:?}");
            assert_eq!(d.eol, Eol::Crlf, "{cs:?}");
            assert!(!d.lossy, "{cs:?}");
        }
    }

    #[test]
    fn auto_detects_japanese_encodings() {
        for (cs, text) in [(Charset::ShiftJis, "日本語のテキスト\n二行目\n"), (Charset::EucJp, "日本語のテキスト\n二行目\n"), (Charset::Utf8, "日本語\n"), (Charset::Iso2022Jp, "日本語のテキスト\n")] {
            let bytes = encode_document(text, cs, Bom::None, Eol::Lf).unwrap();
            assert_eq!(detect_charset(&bytes), cs, "{cs:?}");
            assert_eq!(decode_document(&bytes, None).content, text);
        }
    }

    #[test]
    fn bom_and_eol_are_remembered() {
        let mut raw = vec![0xEF, 0xBB, 0xBF];
        raw.extend_from_slice("a\r\nb\r\nc\n".as_bytes());
        let d = decode_document(&raw, None);
        assert_eq!((d.charset, d.bom, d.eol, d.mixed_eol), (Charset::Utf8, Bom::Utf8, Eol::Crlf, true));
        assert_eq!(d.content, "a\nb\nc\n");
        assert_eq!(detect_eol("a\rb\rc").0, Eol::Cr);
        assert_eq!(detect_eol("abc").0, Eol::Lf);
        let back = encode_document(&d.content, d.charset, d.bom, Eol::Crlf).unwrap();
        assert_eq!(&back[..3], &[0xEF, 0xBB, 0xBF]);
        assert_eq!(&back[3..], "a\r\nb\r\nc\r\n".as_bytes());
    }

    #[test]
    fn unmappable_characters_are_reported_not_replaced() {
        let r = encode_document("hi 😀 ok", Charset::ShiftJis, Bom::None, Eol::Lf);
        assert_eq!(r, Err(vec!['😀']));
        assert!(encode_document("hi 😀", Charset::Utf16Le, Bom::Utf16Le, Eol::Lf).is_ok());
    }

    #[test]
    fn utf16_surrogate_pairs_and_cesu8() {
        let s = "a😀b";
        let d = decode_document(&encode_document(s, Charset::Utf16Le, Bom::Utf16Le, Eol::Lf).unwrap(), None);
        assert_eq!((d.content.as_str(), d.charset), (s, Charset::Utf16Le));
        let c = encode_document(s, Charset::Cesu8, Bom::None, Eol::Lf).unwrap();
        assert_eq!(c.len(), 1 + 6 + 1, "补充平面字符在 CESU-8 里是 6 字节 (两个代理各 3 字节)");
        assert_eq!(decode_document(&c, Some(Charset::Cesu8)).content, s);
    }

    #[test]
    fn utf7_plus_sign_and_forced_charset_keeps_foreign_bom() {
        assert_eq!(decode_document(b"1+-1 = 2", Some(Charset::Utf7)).content, "1+1 = 2");
        let mut raw = vec![0xEF, 0xBB, 0xBF];
        raw.extend_from_slice(b"abc");
        let d = decode_document(&raw, Some(Charset::ShiftJis));
        assert_eq!(d.bom, Bom::None, "强制 SJIS 时 UTF-8 BOM 不剥");
    }
}

// ---------------------------------------------------------- 文件级 helper ---
impl Bom {
    pub fn name(self) -> &'static str {
        match self { Bom::None => "none", Bom::Utf8 => "utf8", Bom::Utf16Le => "utf16le", Bom::Utf16Be => "utf16be" }
    }
}

/// 读文件并解码. `forced` = 编码名 (按该编码重新打开), None = 自动判别。
pub fn read_doc(path: &std::path::Path, forced: Option<&str>) -> Result<DocFile, String> {
    let cs = match forced {
        Some(l) if !l.is_empty() && !l.eq_ignore_ascii_case("auto") => {
            Some(Charset::from_label(l).ok_or_else(|| format!("不支持的编码: {l}"))?)
        }
        _ => None,
    };
    let bytes = std::fs::read(path).map_err(|e| format!("读取失败: {e}"))?;
    Ok(decode_document(&bytes, cs))
}

/// 保存参数 (全部可省: 省略 = 沿用旧行为, 原样写 UTF-8)。
#[derive(Debug, Clone, Default)]
pub struct WriteSpec {
    pub encoding: Option<String>,
    pub bom: Option<String>,
    pub eol: Option<String>,
    /// 目标编码里无法表示的字符: false = 报错 (不静默替换), true = 仍写 (无法表示的字符变成 ?)
    pub force: bool,
    /// 保存前把旧文件备份到 `.sakura-backup/` (保留最近几代; 备份失败不挡保存, 只是不备份)
    pub backup: bool,
}

/// 写文件. `content` 是 LF 文本 (指定了 encoding 时); 没指定 encoding 就按旧行为原样写 UTF-8。
pub fn write_doc(path: &std::path::Path, content: &str, spec: &WriteSpec) -> Result<usize, String> {
    if spec.backup {
        let _ = crate::auto_backup::backup_before_save(path, &crate::auto_backup::SaveBackupConfig::default());
    }
    // 原子写: 先写同目录临时文件再换名, 中途失败 / 断电原文件完好; 只读文件明确拒绝而不是半写
    let write = |bytes: &[u8]| atomic_write(path, bytes, AtomicWriteOpts { clear_readonly: false, sync: true }).map_err(|e| format!("写入失败: {e}"));
    let Some(enc_label) = spec.encoding.as_deref().filter(|s| !s.is_empty()) else {
        write(content.as_bytes())?;
        return Ok(content.len());
    };
    let cs = Charset::from_label(enc_label).ok_or_else(|| format!("不支持的编码: {enc_label}"))?;
    let bom = match spec.bom.as_deref() {
        Some(l) => Bom::from_label(l).ok_or_else(|| format!("不支持的 BOM: {l}"))?,
        None => match cs { Charset::Utf16Le => Bom::Utf16Le, Charset::Utf16Be => Bom::Utf16Be, _ => Bom::None },
    };
    let eol = match spec.eol.as_deref() {
        Some(l) => Eol::from_label(l).ok_or_else(|| format!("不支持的换行: {l}"))?,
        None => Eol::Lf,
    };
    let bytes = match encode_document(content, cs, bom, eol) {
        Ok(b) => b,
        Err(bad) if spec.force => {
            let fixed: String = content.chars().map(|c| if bad.contains(&c) { '?' } else { c }).collect();
            encode_document(&fixed, cs, bom, eol).map_err(|e| format!("编码失败: {e:?}"))?
        }
        Err(bad) => {
            let list: String = bad.iter().collect();
            return Err(format!("UNMAPPABLE:{}:编码 {} 无法表示这些字符: {}", cs.label(), cs.label(), list));
        }
    };
    write(&bytes)?;
    Ok(bytes.len())
}

/// Sakura「コード変換」: 把文本按 `encode_as` 编成字节, 再按 `decode_as` 解读。
/// - `EUC→SJIS`: 文档里是被当成 SJIS 误读的 EUC 文字 → `reinterpret(text, ShiftJis, EucJp)` 还原
/// - `SJIS→JIS`: 转成 JIS 转义序列字节再按 SJIS 显示 → `reinterpret(text, Iso2022Jp, ShiftJis)`
pub fn reinterpret_text(text: &str, encode_as: Charset, decode_as: Charset) -> Result<String, String> {
    let bytes = encode_document(text, encode_as, Bom::None, Eol::Lf).map_err(|bad| {
        let list: String = bad.iter().take(8).collect();
        format!("UNMAPPABLE:编码 {} 无法表示这些字符: {}", encode_as.label(), list)
    })?;
    // UTF-16 解读时字节数是奇数: 最后一个字节凑不成一个码元, Sakura 原样保留成字符 (不变成替换符)
    if matches!(decode_as, Charset::Utf16Le | Charset::Utf16Be) && bytes.len() % 2 == 1 {
        let (head, last) = bytes.split_at(bytes.len() - 1);
        let mut out = decode_document(head, Some(decode_as)).content;
        out.push(last[0] as char);
        return Ok(out);
    }
    Ok(decode_document(&bytes, Some(decode_as)).content)
}

#[cfg(test)]
mod reinterpret_tests {
    use super::*;

    #[test]
    fn odd_utf16_tail_byte_is_kept() {
        // Sakura: "abc" を UNICODE として解釈 → "扡c"
        assert_eq!(reinterpret_text("abc", Charset::ShiftJis, Charset::Utf16Le).unwrap(), "扡c");
    }
}

#[cfg(test)]
mod reinterpret_tests2 {
    use super::*;

    #[test]
    fn euc_mojibake_is_repaired_by_reinterpreting() {
        // "あいうえお" を EUC-JP で保存したバイト列 (全部 0xA1-0xDF) を SJIS として読むと半角カナの列になる (損失なしで往復できる)
        let euc = encode_document("あいうえお", Charset::EucJp, Bom::None, Eol::Lf).unwrap();
        let as_sjis = decode_document(&euc, Some(Charset::ShiftJis)).content;
        assert_ne!(as_sjis, "あいうえお");
        assert_eq!(reinterpret_text(&as_sjis, Charset::ShiftJis, Charset::EucJp).unwrap(), "あいうえお");
    }

    #[test]
    fn utf8_text_shown_as_sjis_roundtrips() {
        let utf8 = encode_document("こんにちは", Charset::Utf8, Bom::None, Eol::Lf).unwrap();
        let as_sjis = decode_document(&utf8, Some(Charset::ShiftJis)).content;
        assert_eq!(reinterpret_text(&as_sjis, Charset::ShiftJis, Charset::Utf8).unwrap(), "こんにちは");
    }

    #[test]
    fn unmappable_is_reported() {
        let e = reinterpret_text("😀", Charset::ShiftJis, Charset::Utf8).unwrap_err();
        assert!(e.starts_with("UNMAPPABLE:"));
    }
}

// ================================================================ 安全保存 ===
// WBS-02 1.5 / 1.6 后端: 同目录临时文件 + 重命名; 编码有损时返回标志; 外部改动检测。

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// 原子写失败的类型化错误。出错时原文件保持不变, 临时文件已清理。
#[derive(Debug)]
pub enum AtomicWriteError {
    /// 目标是只读的且调用方没有允许清除只读属性
    ReadOnly(PathBuf),
    /// 目标没有父目录 / 没有文件名
    InvalidPath(PathBuf),
    /// 目标是个目录
    IsDirectory(PathBuf),
    /// 某个阶段的 IO 错误 (stage: "create-temp" / "write" / "sync" / "clear-readonly" / "rename")
    Io { stage: &'static str, source: std::io::Error },
}

impl std::fmt::Display for AtomicWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly(p) => write!(f, "READONLY:文件是只读的: {}", p.display()),
            Self::InvalidPath(p) => write!(f, "路径无效: {}", p.display()),
            Self::IsDirectory(p) => write!(f, "目标是目录: {}", p.display()),
            Self::Io { stage, source } => write!(f, "写入失败 ({stage}): {source}"),
        }
    }
}

impl std::error::Error for AtomicWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self { Self::Io { source, .. } => Some(source), _ => None }
    }
}

/// 原子写选项。
#[derive(Debug, Clone, Copy, Default)]
pub struct AtomicWriteOpts {
    /// 目标是只读时: false = 拒绝 (ReadOnly 错误, 原文件不动); true = 清除只读属性后覆盖 (结果文件可写)
    pub clear_readonly: bool,
    /// 写完后 fsync (默认 false 以保持速度; 保存用户文档时建议 true)
    pub sync: bool,
}

static TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn tmp_path_for(path: &Path) -> Option<PathBuf> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let name = path.file_name()?.to_string_lossy().into_owned();
    let n = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Some(dir.join(format!(".{name}.{}.{n}.tmp", std::process::id())))
}

fn atomic_write_inner(path: &Path, bytes: &[u8], opts: AtomicWriteOpts, fail_after_write: bool) -> Result<(), AtomicWriteError> {
    // 目标是符号链接时写到链接指向的真实文件, 不替换链接本身
    let real: PathBuf = match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    };
    let path = real.as_path();
    let existing = std::fs::metadata(path).ok();
    if let Some(m) = &existing {
        if m.is_dir() { return Err(AtomicWriteError::IsDirectory(path.to_path_buf())); }
        if m.permissions().readonly() && !opts.clear_readonly {
            return Err(AtomicWriteError::ReadOnly(path.to_path_buf()));
        }
    }
    let tmp = tmp_path_for(path).ok_or_else(|| AtomicWriteError::InvalidPath(path.to_path_buf()))?;
    let cleanup = |e: AtomicWriteError| { let _ = std::fs::remove_file(&tmp); e };
    let io = |stage: &'static str| move |source: std::io::Error| AtomicWriteError::Io { stage, source };

    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(io("create-temp"))?;
    if let Err(e) = f.write_all(bytes) { drop(f); return Err(cleanup(io("write")(e))); }
    if fail_after_write { drop(f); return Err(cleanup(io("write")(std::io::Error::other("injected")))); }
    if opts.sync { if let Err(e) = f.sync_all() { drop(f); return Err(cleanup(io("sync")(e))); } }
    drop(f);
    // 沿用原文件的权限 (非只读时); 只读目标被允许清除时新文件保持可写
    if let Some(m) = &existing {
        if !m.permissions().readonly() {
            let _ = std::fs::set_permissions(&tmp, m.permissions());
        } else {
            let mut p = m.permissions();
            p.set_readonly(false);
            if let Err(e) = std::fs::set_permissions(path, p) {
                return Err(cleanup(io("clear-readonly")(e)));
            }
        }
    }
    // std::fs::rename 在 Windows 上用 MoveFileExW(REPLACE_EXISTING), 在 Unix 上是 rename(2): 都会原子替换已有目标
    std::fs::rename(&tmp, path).map_err(|e| cleanup(io("rename")(e)))
}

/// 原子写: 先写同目录临时文件, 再重命名覆盖目标。中途失败时原文件完好。
pub fn atomic_write(path: &Path, bytes: &[u8], opts: AtomicWriteOpts) -> Result<(), AtomicWriteError> {
    atomic_write_inner(path, bytes, opts, false)
}

/// 保存一份文档所需的元数据。
#[derive(Debug, Clone, Copy)]
pub struct SaveSpec {
    pub charset: Charset,
    pub bom: Bom,
    pub eol: Eol,
    /// 目标编码无法表示的字符: false = 不写, 返回 Unmappable; true = 仍写 (无法表示的字符变成 ?) 并置 lossy 标志
    pub allow_lossy: bool,
    pub write: AtomicWriteOpts,
}

#[derive(Debug)]
pub enum SaveError {
    /// 编码无法表示这些字符, 且未允许有损 (文件没有被改动)
    Unmappable { charset: Charset, chars: Vec<char> },
    Write(AtomicWriteError),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unmappable { charset, chars } => {
                let list: String = chars.iter().collect();
                write!(f, "UNMAPPABLE:{}:编码 {} 无法表示这些字符: {}", charset.label(), charset.label(), list)
            }
            Self::Write(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// 保存结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOutcome {
    pub bytes_written: usize,
    /// true = 有字符在目标编码里无法表示, 已被替换成 '?' (仅 allow_lossy 时出现)
    pub lossy: bool,
    /// 被替换的字符 (去重, 最多 20 个)
    pub replaced: Vec<char>,
}

/// 把 LF 文本按文档的 编码/BOM/换行 编码, 并原子写入。无法表示的字符不会被静默替换。
pub fn save_document(path: &Path, lf_text: &str, spec: &SaveSpec) -> Result<SaveOutcome, SaveError> {
    let (bytes, replaced) = match encode_document(lf_text, spec.charset, spec.bom, spec.eol) {
        Ok(b) => (b, Vec::new()),
        Err(bad) if spec.allow_lossy => {
            let fixed: String = lf_text.chars().map(|c| if bad.contains(&c) { '?' } else { c }).collect();
            let b = encode_document(&fixed, spec.charset, spec.bom, spec.eol)
                .map_err(|chars| SaveError::Unmappable { charset: spec.charset, chars })?;
            (b, bad)
        }
        Err(chars) => return Err(SaveError::Unmappable { charset: spec.charset, chars }),
    };
    atomic_write(path, &bytes, spec.write).map_err(SaveError::Write)?;
    Ok(SaveOutcome { bytes_written: bytes.len(), lossy: !replaced.is_empty(), replaced })
}

/// 文件的 "指纹": 用来判断是否被外部改动。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FileStamp {
    pub exists: bool,
    pub mtime_ms: i64,
    pub size: u64,
    pub readonly: bool,
}

impl FileStamp {
    pub const MISSING: FileStamp = FileStamp { exists: false, mtime_ms: 0, size: 0, readonly: false };
}

pub fn file_stamp(path: &Path) -> FileStamp {
    match std::fs::metadata(path) {
        Ok(m) if !m.is_dir() => {
            let mtime_ms = m.modified().ok().map(|t| match t.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => d.as_millis() as i64,
                Err(e) => -(e.duration().as_millis() as i64),
            }).unwrap_or(0);
            FileStamp { exists: true, mtime_ms, size: m.len(), readonly: m.permissions().readonly() }
        }
        _ => FileStamp::MISSING,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExternalChange {
    Unchanged,
    /// mtime 或大小变了
    Modified,
    /// 记住时存在, 现在没了
    Deleted,
    /// 记住时不存在 (新文档), 现在出现了
    Created,
}

/// 纯函数: 记住的指纹 vs 现在的指纹。只读属性变化不算内容被改动。
pub fn external_change(remembered: &FileStamp, current: &FileStamp) -> ExternalChange {
    match (remembered.exists, current.exists) {
        (false, false) => ExternalChange::Unchanged,
        (false, true) => ExternalChange::Created,
        (true, false) => ExternalChange::Deleted,
        (true, true) => if remembered.mtime_ms != current.mtime_ms || remembered.size != current.size {
            ExternalChange::Modified
        } else { ExternalChange::Unchanged },
    }
}

/// 便捷: 外部是否改过 (Modified / Deleted / Created 都算)。
pub fn changed_externally(remembered: &FileStamp, current: &FileStamp) -> bool {
    external_change(remembered, current) != ExternalChange::Unchanged
}

#[cfg(test)]
mod safe_save_tests {
    use super::*;

    fn tdir(tag: &str) -> PathBuf {
        let n = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("docio-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn leftovers(d: &Path) -> usize {
        std::fs::read_dir(d).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).count()
    }
    fn spec(cs: Charset, lossy: bool) -> SaveSpec {
        SaveSpec { charset: cs, bom: Bom::None, eol: Eol::Lf, allow_lossy: lossy, write: AtomicWriteOpts::default() }
    }

    #[test]
    fn atomic_write_creates_and_replaces() {
        let d = tdir("rep");
        let p = d.join("a.txt");
        atomic_write(&p, b"one", AtomicWriteOpts::default()).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"one");
        atomic_write(&p, b"two-two", AtomicWriteOpts { sync: true, ..Default::default() }).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two-two");
        assert_eq!(leftovers(&d), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn failed_write_leaves_original_intact_and_cleans_temp() {
        let d = tdir("fail");
        let p = d.join("a.txt");
        std::fs::write(&p, b"original").unwrap();
        let e = atomic_write_inner(&p, b"new content", AtomicWriteOpts::default(), true).unwrap_err();
        assert!(matches!(e, AtomicWriteError::Io { stage: "write", .. }));
        assert_eq!(std::fs::read(&p).unwrap(), b"original");
        assert_eq!(leftovers(&d), 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn directory_target_and_missing_parent_are_typed_errors() {
        let d = tdir("dir");
        let target = d.join("sub");
        std::fs::create_dir_all(&target).unwrap();
        let e = atomic_write(&target, b"x", AtomicWriteOpts::default()).unwrap_err();
        assert!(matches!(e, AtomicWriteError::IsDirectory(_)));
        assert_eq!(leftovers(&d), 0);
        let missing = d.join("nodir").join("f.txt");
        let e = atomic_write(&missing, b"x", AtomicWriteOpts::default()).unwrap_err();
        assert!(matches!(e, AtomicWriteError::Io { stage: "create-temp", .. }));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn readonly_is_refused_or_cleared_explicitly() {
        let d = tdir("ro");
        let p = d.join("ro.txt");
        std::fs::write(&p, b"keep").unwrap();
        let mut perm = std::fs::metadata(&p).unwrap().permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&p, perm).unwrap();
        assert!(file_stamp(&p).readonly);

        let e = atomic_write(&p, b"new", AtomicWriteOpts::default()).unwrap_err();
        assert!(matches!(e, AtomicWriteError::ReadOnly(_)));
        assert!(e.to_string().starts_with("READONLY:"));
        assert_eq!(std::fs::read(&p).unwrap(), b"keep");
        assert_eq!(leftovers(&d), 0);

        atomic_write(&p, b"new", AtomicWriteOpts { clear_readonly: true, sync: false }).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"new");
        assert!(!file_stamp(&p).readonly, "清除后结果文件可写");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn save_document_uses_encoding_bom_and_eol() {
        let d = tdir("enc");
        let p = d.join("sj.txt");
        let s = SaveSpec { charset: Charset::ShiftJis, bom: Bom::None, eol: Eol::Crlf, allow_lossy: false, write: Default::default() };
        let o = save_document(&p, "日本語\nabc\n", &s).unwrap();
        assert!(!o.lossy && o.replaced.is_empty());
        let raw = std::fs::read(&p).unwrap();
        assert_eq!(o.bytes_written, raw.len());
        let back = decode_document(&raw, None);
        assert_eq!((back.content.as_str(), back.eol, back.charset), ("日本語\nabc\n", Eol::Crlf, Charset::ShiftJis));

        let u8s = SaveSpec { charset: Charset::Utf8, bom: Bom::Utf8, eol: Eol::Lf, allow_lossy: false, write: Default::default() };
        save_document(&p, "x", &u8s).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), vec![0xEF, 0xBB, 0xBF, b'x']);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unmappable_blocks_save_unless_lossy_allowed() {
        let d = tdir("lossy");
        let p = d.join("f.txt");
        std::fs::write(&p, b"old").unwrap();
        match save_document(&p, "hi 😀 ok", &spec(Charset::ShiftJis, false)) {
            Err(SaveError::Unmappable { chars, charset }) => { assert_eq!(chars, vec!['😀']); assert_eq!(charset, Charset::ShiftJis); }
            other => panic!("expected Unmappable, got {other:?}"),
        }
        assert_eq!(std::fs::read(&p).unwrap(), b"old", "被拒绝时文件不动");
        let o = save_document(&p, "hi 😀 ok", &spec(Charset::ShiftJis, true)).unwrap();
        assert!(o.lossy);
        assert_eq!(o.replaced, vec!['😀']);
        assert_eq!(std::fs::read(&p).unwrap(), b"hi ? ok");
        assert!(!save_document(&p, "😀", &spec(Charset::Utf16Le, false)).unwrap().lossy);
        let e = save_document(&p, "😀", &spec(Charset::Gbk, false)).unwrap_err();
        assert!(e.to_string().starts_with("UNMAPPABLE:"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn stamp_and_external_change() {
        let d = tdir("stamp");
        let p = d.join("s.txt");
        let none = file_stamp(&p);
        assert_eq!(none, FileStamp::MISSING);
        std::fs::write(&p, b"abc").unwrap();
        let a = file_stamp(&p);
        assert!(a.exists && a.size == 3 && a.mtime_ms > 0 && !a.readonly);
        assert_eq!(external_change(&a, &file_stamp(&p)), ExternalChange::Unchanged);
        assert_eq!(external_change(&none, &a), ExternalChange::Created);
        let grown = FileStamp { size: 4, ..a };
        assert_eq!(external_change(&a, &grown), ExternalChange::Modified);
        let touched = FileStamp { mtime_ms: a.mtime_ms + 1500, ..a };
        assert!(changed_externally(&a, &touched));
        let ro = FileStamp { readonly: true, ..a };
        assert!(!changed_externally(&a, &ro), "只读属性变化不算内容改动");
        std::fs::remove_file(&p).unwrap();
        assert_eq!(external_change(&a, &file_stamp(&p)), ExternalChange::Deleted);
        assert_eq!(external_change(&none, &none), ExternalChange::Unchanged);
        assert_eq!(file_stamp(&d), FileStamp::MISSING, "目录不算文件");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn own_save_then_restamp_is_not_external() {
        let d = tdir("own");
        let p = d.join("o.txt");
        save_document(&p, "v1", &spec(Charset::Utf8, false)).unwrap();
        let remembered = file_stamp(&p);
        save_document(&p, "version two", &spec(Charset::Utf8, false)).unwrap();
        assert!(changed_externally(&remembered, &file_stamp(&p)), "保存后不更新指纹就会被当成外部改动");
        let remembered = file_stamp(&p);
        assert!(!changed_externally(&remembered, &file_stamp(&p)));
        let _ = std::fs::remove_dir_all(&d);
    }
}
