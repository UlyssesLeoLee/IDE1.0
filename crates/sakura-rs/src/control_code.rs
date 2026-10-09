// control_code.rs — コントロールコード表示 (sakura 7.5)
//
// 不可視文字 (\0 \1 ... \31, \127) を可視化.
// 出力: 表示用文字列 (各コントロールコードを `<0x00>` 形式に置換)

/// コントロールコード表示設定
#[derive(Debug, Clone, Copy)]
pub struct ControlCodeDisplay {
    /// タブを ^I として表示
    pub show_tab: bool,
    /// 改行 (CR/LF) を可視化
    pub show_newline: bool,
    /// 空白を中点で表示
    pub show_space: bool,
    /// その他の制御文字を <0xNN> で表示
    pub show_other: bool,
}

impl Default for ControlCodeDisplay {
    fn default() -> Self {
        Self {
            show_tab: true,
            show_newline: true,
            show_space: false,
            show_other: true,
        }
    }
}

/// 文字を可視化
pub fn visualize_char(c: char, opts: ControlCodeDisplay) -> String {
    match c {
        '\t' if opts.show_tab => "^I".to_string(),
        '\t' => "\t".to_string(), // 表示オフ → そのまま
        '\n' if opts.show_newline => "\\n".to_string(),
        '\n' => "\n".to_string(),
        '\r' if opts.show_newline => "\\r".to_string(),
        '\r' => "\r".to_string(),
        ' ' if opts.show_space => "·".to_string(),
        c if (c as u32) < 0x20 && opts.show_other => format!("<0x{:02x}>", c as u32),
        '\x7f' if opts.show_other => "<0x7f>".to_string(),
        c => c.to_string(),
    }
}

/// 文字列全体を可視化
pub fn visualize(text: &str, opts: ControlCodeDisplay) -> String {
    text.chars().map(|c| visualize_char(c, opts)).collect()
}

/// コントロールコード判定
pub fn is_control(c: char) -> bool {
    let code = c as u32;
    code < 0x20 || code == 0x7f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab() {
        let s = visualize("\t", ControlCodeDisplay::default());
        assert_eq!(s, "^I");
    }

    #[test]
    fn test_newline() {
        let s = visualize("\n", ControlCodeDisplay::default());
        assert_eq!(s, "\\n");
    }

    #[test]
    fn test_control_zero() {
        let s = visualize("\0", ControlCodeDisplay::default());
        assert_eq!(s, "<0x00>");
    }

    #[test]
    fn test_normal_passthrough() {
        let s = visualize("hello", ControlCodeDisplay::default());
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_disable_tab() {
        let opts = ControlCodeDisplay { show_tab: false, ..Default::default() };
        assert_eq!(visualize("\t", opts), "\t");
    }

    #[test]
    fn test_is_control() {
        assert!(is_control('\0'));
        assert!(is_control('\n'));
        assert!(is_control('\t'));
        assert!(is_control('\x7f'));
        assert!(!is_control('A'));
        assert!(!is_control(' '));
    }

    #[test]
    fn test_mixed() {
        let input = "a\tb\nc";
        let out = visualize(input, ControlCodeDisplay::default());
        assert_eq!(out, "a^Ib\\nc");
    }
}
