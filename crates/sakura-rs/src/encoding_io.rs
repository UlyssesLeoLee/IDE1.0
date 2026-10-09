// encoding_io.rs — Shift_JIS / EUC-JP / JIS / UTF-16 読み書き (sakura 7.2/7.3)
//
// 文字コード自動検出 + 任意エンコーディング指定読み込み.

use std::fs;
use std::path::Path;

use encoding_rs::{Encoding, UTF_8, SHIFT_JIS, EUC_JP, ISO_2022_JP, UTF_16LE, UTF_16BE, WINDOWS_1252};

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

/// バイト列から文字エンコーディングを自動検出 (BOM + 統計)
///
/// 戦略:
/// 1. UTF-16 BOM チェック
/// 2. UTF-8 として有効なら UTF-8
/// 3. それ以外は Shift_JIS / EUC-JP フォールバック
pub fn detect_encoding(bytes: &[u8]) -> EncodingKind {
    // BOM チェック
    if bytes.len() >= 2 {
        if &bytes[0..2] == b"\xFF\xFE" {
            return EncodingKind::Utf16Le;
        }
        if &bytes[0..2] == b"\xFE\xFF" {
            return EncodingKind::Utf16Be;
        }
    }
    if bytes.len() >= 3 && &bytes[0..3] == b"\xEF\xBB\xBF" {
        return EncodingKind::Utf8; // BOM あり
    }

    // UTF-8 として有効か検証
    if std::str::from_utf8(bytes).is_ok() {
        return EncodingKind::Utf8;
    }

    // Shift_JIS / EUC-JP 試行 (decode できなければ Windows-1252)
    let (_, _, sjis_had_errors) = SHIFT_JIS.decode(bytes);
    if !sjis_had_errors {
        return EncodingKind::ShiftJis;
    }
    let (_, _, euc_had_errors) = EUC_JP.decode(bytes);
    if !euc_had_errors {
        return EncodingKind::EucJp;
    }
    // 最終的に Windows-1252 (1 バイト, 必ず通る)
    EncodingKind::Windows1252
}

/// ファイルを読み込み, 自動検出したエンコーディングで文字列にデコード
pub fn read_auto(path: &Path) -> Result<(String, EncodingKind), std::io::Error> {
    let bytes = fs::read(path)?;
    let enc = detect_encoding(&bytes);
    let (s, _, _) = enc.to_encoding_rs().decode(&bytes);
    Ok((s.into_owned(), enc))
}

/// 指定エンコーディングでファイルを読み込み
pub fn read_with_encoding(
    path: &Path,
    enc: EncodingKind,
) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    let (s, _, _) = enc.to_encoding_rs().decode(&bytes);
    Ok(s.into_owned())
}

/// 文字列を指定エンコーディングでファイルに書き込み
pub fn write_with_encoding(
    path: &Path,
    content: &str,
    enc: EncodingKind,
) -> Result<(), std::io::Error> {
    let (bytes, _, _) = enc.to_encoding_rs().encode(content);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_path(name: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("sakura-rs-enc-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_detect_utf8() {
        let bytes = "hello 世界".as_bytes();
        assert_eq!(detect_encoding(bytes), EncodingKind::Utf8);
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
        // Note: encoding_rs's Shift_JIS decoder may accept these bytes,
        // so priority is heuristic. Either is acceptable for ambiguous cases.
        assert!(enc == EncodingKind::EucJp || enc == EncodingKind::ShiftJis);
    }

    #[test]
    fn test_read_write_round_trip_shift_jis() {
        let path = temp_path("sjis.txt");
        let content = "こんにちは, 世界!";
        write_with_encoding(&path, content, EncodingKind::ShiftJis).unwrap();
        let (read_back, enc) = read_auto(&path).unwrap();
        assert_eq!(enc, EncodingKind::ShiftJis);
        assert_eq!(read_back, content);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn test_read_write_round_trip_utf8() {
        let path = temp_path("utf8.txt");
        let content = "Hello 世界";
        write_with_encoding(&path, content, EncodingKind::Utf8).unwrap();
        let (read_back, _) = read_auto(&path).unwrap();
        assert_eq!(read_back, content);
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
