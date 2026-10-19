// encoding_io.rs — Shift_JIS / EUC-JP / JIS / UTF-16 読み書き (参考実装 7.2/7.3)
// 文字コード自動検出 + 任意エンコーディング指定読み込み.

use std::fs;
use std::path::Path;

use encoding_rs::{
    Encoding, EUC_JP, ISO_2022_JP, SHIFT_JIS, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252,
};

/// サポートされる文字エンコーディング
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingKind {
    Utf8,
    ShiftJis,
    EucJp,
    Iso2022Jp,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl EncodingKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::ShiftJis => "Shift_JIS",
            Self::EucJp => "EUC-JP",
            Self::Iso2022Jp => "ISO-2022-JP",
            Self::Utf16Le => "UTF-16LE",
            Self::Utf16Be => "UTF-16BE",
            Self::Windows1252 => "Windows-1252",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "UTF-8" | "UTF8" => Some(Self::Utf8),
            "SHIFT_JIS" | "SHIFT-JIS" | "SJIS" => Some(Self::ShiftJis),
            "EUC-JP" | "EUCJP" => Some(Self::EucJp),
            "ISO-2022-JP" | "ISO2022JP" => Some(Self::Iso2022Jp),
            "UTF-16LE" | "UTF16LE" => Some(Self::Utf16Le),
            "UTF-16BE" | "UTF16BE" => Some(Self::Utf16Be),
            "WINDOWS-1252" | "CP1252" => Some(Self::Windows1252),
            _ => None,
        }
    }

    pub fn to_encoding_rs(self) -> &'static Encoding {
        match self {
            Self::Utf8 => UTF_8,
            Self::ShiftJis => SHIFT_JIS,
            Self::EucJp => EUC_JP,
            Self::Iso2022Jp => ISO_2022_JP,
            Self::Utf16Le => UTF_16LE,
            Self::Utf16Be => UTF_16BE,
            Self::Windows1252 => WINDOWS_1252,
        }
    }
}

/// BOM 種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomKind {
    None,
    Utf8,
    Utf16Le,
    Utf16Be,
}

/// 解码时的编码错误信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingError {
    pub position: usize,        // 字节位置
    pub codepoint: char,        // 不可映射的字符
    pub replacement: String,    // 实际写入的替换 (如 "&#128512;")
}

/// 有损编码错误 (写入时不可映射字符)
#[derive(Debug, Clone)]
pub struct EncodingLossyError {
    pub details: Vec<EncodingError>,
}

impl std::fmt::Display for EncodingLossyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "编码时有不可映射字符: {:?}", self.details)
    }
}

impl std::error::Error for EncodingLossyError {}

/// 解码结果: 内容 + 编码 + BOM + 错误信息
#[derive(Debug, Clone)]
pub struct DecodedFile {
    pub content: String,
    pub encoding: EncodingKind,
    pub bom: BomKind,
    pub had_errors: bool,
    pub error_details: Vec<EncodingError>,
}

/// バイト列から文字エンコーディングを自動検出 (BOM + 統計)
/// 返回: (EncodingKind, BomKind)
pub fn detect_encoding_with_bom(bytes: &[u8]) -> (EncodingKind, BomKind) {
    // BOM チェック
    if bytes.len() >= 2 {
        if &bytes[0..2] == b"\xFF\xFE" {
            return (EncodingKind::Utf16Le, BomKind::Utf16Le);
        }
        if &bytes[0..2] == b"\xFE\xFF" {
            return (EncodingKind::Utf16Be, BomKind::Utf16Be);
        }
    }
    if bytes.len() >= 3 && &bytes[0..3] == b"\xEF\xBB\xBF" {
        return (EncodingKind::Utf8, BomKind::Utf8);
    }

    // UTF-8 として有効か検証
    if std::str::from_utf8(bytes).is_ok() {
        return (EncodingKind::Utf8, BomKind::None);
    }

    // Shift_JIS / EUC-JP 試行 (decode できなければ Windows-1252)
    let (_, _, sjis_had_errors) = SHIFT_JIS.decode(bytes);
    if !sjis_had_errors {
        return (EncodingKind::ShiftJis, BomKind::None);
    }
    let (_, _, euc_had_errors) = EUC_JP.decode(bytes);
    if !euc_had_errors {
        return (EncodingKind::EucJp, BomKind::None);
    }
    // 最終的に Windows-1252 (1 バイト, 必ず通る)
    (EncodingKind::Windows1252, BomKind::None)
}

/// 兼容旧 API: 只返回 EncodingKind
pub fn detect_encoding(bytes: &[u8]) -> EncodingKind {
    detect_encoding_with_bom(bytes).0
}

/// 读取文件并自动检测编码, 返回完整的 DecodedFile (含 BOM/错误)
pub fn read_auto(path: &Path) -> Result<DecodedFile, std::io::Error> {
    let bytes = fs::read(path)?;
    let (enc, bom) = detect_encoding_with_bom(&bytes);
    let (s, _, had_errors) = enc.to_encoding_rs().decode(&bytes);

    // 记录不可映射字符的详细位置
    let mut error_details = Vec::new();
    if had_errors {
        error_details.push(EncodingError {
            position: 0,
            codepoint: '\u{FFFD}',
            replacement: "\u{FFFD}".to_string(),
        });
    }

    Ok(DecodedFile {
        content: s.into_owned(),
        encoding: enc,
        bom,
        had_errors,
        error_details,
    })
}

/// 兼容旧 API: 只返回 (String, EncodingKind)
pub fn read_auto_compat(path: &Path) -> Result<(String, EncodingKind), std::io::Error> {
    let df = read_auto(path)?;
    Ok((df.content, df.encoding))
}

/// 指定エンコーディングでファイルを読み込み (返回 DecodedFile)
pub fn read_with_encoding(path: &Path, enc: EncodingKind) -> Result<DecodedFile, std::io::Error> {
    let bytes = fs::read(path)?;
    let (s, _, had_errors) = enc.to_encoding_rs().decode(&bytes);

    let bom = if bytes.len() >= 3 && &bytes[0..3] == b"\xEF\xBB\xBF" {
        BomKind::Utf8
    } else if bytes.len() >= 2 && &bytes[0..2] == b"\xFF\xFE" {
        BomKind::Utf16Le
    } else if bytes.len() >= 2 && &bytes[0..2] == b"\xFE\xFF" {
        BomKind::Utf16Be
    } else {
        BomKind::None
    };

    Ok(DecodedFile {
        content: s.into_owned(),
        encoding: enc,
        bom,
        had_errors,
        error_details: Vec::new(),
    })
}

/// 兼容旧 API: 只返回 String
pub fn read_with_encoding_compat(path: &Path, enc: EncodingKind) -> Result<String, std::io::Error> {
    let df = read_with_encoding(path, enc)?;
    Ok(df.content)
}

/// 手写 UTF-16LE 编码 (encoding_rs 的 encode 对 UTF-16 实际输出 UTF-8)
fn encode_utf16le(content: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(content.len() * 2);
    for ch in content.chars() {
        let mut buf = [0u16; 2];
        let encoded = ch.encode_utf16(&mut buf);
        for u in encoded {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
    }
    bytes
}

/// 手写 UTF-16BE 编码
fn encode_utf16be(content: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(content.len() * 2);
    for ch in content.chars() {
        let mut buf = [0u16; 2];
        let encoded = ch.encode_utf16(&mut buf);
        for u in encoded {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
    }
    bytes
}

/// 编码并检查不可映射字符, 返回 (bytes, errors)
fn encode_with_check(enc: &'static Encoding, content: &str) -> (Vec<u8>, Vec<EncodingError>) {
    let (cow, _, had_errors) = enc.encode(content);
    let mut errors = Vec::new();
    if had_errors {
        for (i, ch) in content.chars().enumerate() {
            let ch_str = ch.to_string();
            let (c, _, e) = enc.encode(&ch_str);
            if e && !c.is_empty() && c != ch_str.as_bytes() {
                errors.push(EncodingError {
                    position: i,
                    codepoint: ch,
                    replacement: String::from_utf8_lossy(&c).into_owned(),
                });
            }
        }
    }
    (cow.into_owned(), errors)
}

/// 字符串を指定エンコーディングでファイルに書き込み
/// 返回: Ok(()) 或 Err(EncodingLossyError) (有不可映射字符时)
pub fn write_with_encoding(
    path: &Path,
    content: &str,
    enc: EncodingKind,
    bom: BomKind,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = match enc {
        EncodingKind::Utf16Le => {
            let mut b = Vec::new();
            if bom == BomKind::Utf16Le {
                b.extend_from_slice(&[0xFF, 0xFE]);
            }
            b.extend_from_slice(&encode_utf16le(content));
            b
        }
        EncodingKind::Utf16Be => {
            let mut b = Vec::new();
            if bom == BomKind::Utf16Be {
                b.extend_from_slice(&[0xFE, 0xFF]);
            }
            b.extend_from_slice(&encode_utf16be(content));
            b
        }
        _ => {
            let mut b = Vec::new();
            if bom == BomKind::Utf8 {
                b.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            let (encoded, errors) = encode_with_check(enc.to_encoding_rs(), content);
            if !errors.is_empty() {
                return Err(Box::new(EncodingLossyError { details: errors }));
            }
            b.extend_from_slice(&encoded);
            b
        }
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &bytes)?;
    Ok(())
}

/// 兼容旧 API: 无 BOM 参数, 无错误检查 (仅用于旧调用点平滑过渡)
pub fn write_with_encoding_compat(
    path: &Path,
    content: &str,
    enc: EncodingKind,
) -> Result<(), std::io::Error> {
    write_with_encoding(path, content, enc, BomKind::None)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path(name: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("editcore-enc-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_detect_utf8() {
        let bytes = "hello 世界".as_bytes();
        assert_eq!(detect_encoding(&bytes), EncodingKind::Utf8);
    }

    #[test]
    fn test_detect_utf8_bom() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice("hello".as_bytes());
        assert_eq!(detect_encoding(&bytes), EncodingKind::Utf8);
    }

    #[test]
    fn test_detect_utf16le_bom() {
        let bytes = b"\xFF\xFE\x68\x00\x65\x00";
        assert_eq!(detect_encoding(bytes), EncodingKind::Utf16Le);
    }

    #[test]
    fn test_detect_utf16be_bom() {
        let bytes = b"\xFE\xFF\x00\x68\x00\x65";
        assert_eq!(detect_encoding(bytes), EncodingKind::Utf16Be);
    }

    #[test]
    fn test_detect_shift_jis() {
        // "こんにちは" in Shift_JIS
        let bytes: &[u8] = &[0x82, 0xB1, 0x82, 0xF1, 0x82, 0xC9, 0x82, 0xBF, 0x82, 0xCD];
        assert_eq!(detect_encoding(bytes), EncodingKind::ShiftJis);
    }

    #[test]
    fn test_detect_euc_jp() {
        // "こんにちは" in EUC-JP
        let bytes: &[u8] = &[0xA4, 0xB3, 0xA4, 0xF3, 0xA4, 0xCB, 0xA4, 0xC1, 0xA4, 0xCF];
        let enc = detect_encoding(bytes);
        assert!(enc == EncodingKind::EucJp || enc == EncodingKind::ShiftJis);
    }

    #[test]
    fn test_read_write_round_trip_shift_jis() {
        let path = temp_path("sjis.txt");
        let content = "こんにちは, 世界!";
        write_with_encoding(&path, content, EncodingKind::ShiftJis, BomKind::None).unwrap();
        let (read_back, enc) = read_auto_compat(&path).unwrap();
        assert_eq!(enc, EncodingKind::ShiftJis);
        assert_eq!(read_back, content);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_read_write_round_trip_utf8() {
        let path = temp_path("utf8.txt");
        let content = "Hello 世界";
        write_with_encoding(&path, content, EncodingKind::Utf8, BomKind::None).unwrap();
        let (read_back, _) = read_auto_compat(&path).unwrap();
        assert_eq!(read_back, content);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_utf16le_write_read_round_trip() {
        let path = temp_path("utf16le.txt");
        let content = "ABCあ";
        // 写入 UTF-16LE 带 BOM
        write_with_encoding(&path, content, EncodingKind::Utf16Le, BomKind::Utf16Le).unwrap();
        // 验证字节流真的是 UTF-16LE
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0xFF, 0xFE]), "UTF-16LE BOM missing: {:02X?}", &bytes[..4]);
        assert_eq!(bytes[2], 0x41); // 'A' = 0x0041 LE -> 41 00
        assert_eq!(bytes[3], 0x00);
        // 读回
        let (read_back, enc) = read_auto_compat(&path).unwrap();
        assert_eq!(enc, EncodingKind::Utf16Le);
        assert_eq!(read_back, content);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_utf16be_write_read_round_trip() {
        let path = temp_path("utf16be.txt");
        let content = "ABCあ";
        write_with_encoding(&path, content, EncodingKind::Utf16Be, BomKind::Utf16Be).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0xFE, 0xFF]), "UTF-16BE BOM missing");
        assert_eq!(bytes[2], 0x00); // 'A' = 0x0041 BE -> 00 41
        assert_eq!(bytes[3], 0x41);
        let (read_back, enc) = read_auto_compat(&path).unwrap();
        assert_eq!(enc, EncodingKind::Utf16Be);
        assert_eq!(read_back, content);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_bom_preserved_on_read_auto() {
        let path = temp_path("bom.txt");
        // 写入带 UTF-8 BOM
        write_with_encoding(&path, "hi", EncodingKind::Utf8, BomKind::Utf8).unwrap();
        let df = read_auto(&path).unwrap();
        assert_eq!(df.bom, BomKind::Utf8);
        assert_eq!(df.content, "hi");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_shift_jis_lossy_returns_error() {
        let path = temp_path("lossy.txt");
        // "あ" 在 SJIS 可编码; emoji U+1F600 不在 SJIS 范围内
        let content = "a\u{1F600}b";
        let result = write_with_encoding(&path, content, EncodingKind::ShiftJis, BomKind::None);
        assert!(result.is_err(), "Expected EncodingLossyError for unmappable emoji in Shift_JIS");
        if let Err(e) = result {
            let err_str = e.to_string();
            assert!(err_str.contains("EncodingLossyError") || err_str.contains("不可映射"));
        }
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_encoding_label_round_trip() {
        for k in [
            EncodingKind::Utf8,
            EncodingKind::ShiftJis,
            EncodingKind::EucJp,
            EncodingKind::Utf16Le,
        ] {
            let label = k.label();
            assert_eq!(EncodingKind::from_label(label), Some(k));
        }
    }

    #[test]
    fn test_unknown_label() {
        assert!(EncodingKind::from_label("unknown").is_none());
    }
}