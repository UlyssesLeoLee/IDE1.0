//! 按块到达的字节流 -> 文本 (终端输出用)。
//!
//! 一个 UTF-8 字符可能被切在两个数据块之间: 对每块单独 `from_utf8_lossy` 会把它变成两个 U+FFFD。
//! 这里把块尾不完整的字节留到下一块再解; 真正非法的字节才换成 U+FFFD。

#[derive(Default)]
pub struct Utf8Chunker {
    pending: Vec<u8>,
}

impl Utf8Chunker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂入一块字节, 返回其中已经能完整解码的文本。
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    out.push_str(s);
                    self.pending.clear();
                    break;
                }
                Err(e) => {
                    let good = e.valid_up_to();
                    // SAFETY-free: 前 good 个字节已被验证为合法 UTF-8
                    out.push_str(&String::from_utf8_lossy(&self.pending[..good]));
                    match e.error_len() {
                        // 块尾被截断的字符: 留着等下一块
                        None => {
                            self.pending.drain(..good);
                            break;
                        }
                        // 非法字节: 替换后继续
                        Some(n) => {
                            out.push('\u{FFFD}');
                            self.pending.drain(..good + n);
                        }
                    }
                }
            }
        }
        out
    }

    /// 流结束: 把残留的不完整字节按替换符吐出。
    pub fn finish(&mut self) -> String {
        if self.pending.is_empty() {
            String::new()
        } else {
            self.pending.clear();
            "\u{FFFD}".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_multibyte_char_across_chunks() {
        let bytes = "日本語".as_bytes();
        let mut c = Utf8Chunker::new();
        // 在第一个字的中间切开
        let a = c.push(&bytes[..2]);
        let b = c.push(&bytes[2..]);
        assert_eq!((a.as_str(), b.as_str()), ("", "日本語"));
        assert!(!format!("{a}{b}").contains('\u{FFFD}'));
    }

    #[test]
    fn ascii_passes_through_and_invalid_byte_is_replaced_once() {
        let mut c = Utf8Chunker::new();
        assert_eq!(c.push(b"PS E:\\> "), "PS E:\\> ");
        assert_eq!(c.push(&[b'a', 0xFF, b'b']), "a\u{FFFD}b");
    }

    #[test]
    fn dangling_tail_is_flushed_on_finish() {
        let mut c = Utf8Chunker::new();
        assert_eq!(c.push(&"あ".as_bytes()[..2]), "");
        assert_eq!(c.finish(), "\u{FFFD}");
        assert_eq!(c.finish(), "");
    }
}
