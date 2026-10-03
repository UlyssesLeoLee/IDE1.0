//! 编辑缓冲 — 单行 buffer + cursor.
//!
//! 简化为 Vec<char> + cursor 索引。Stage 3.0 brief 范围内足够;
//! 后续 Stage 4 起可换 rope / 行集合。

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputBuffer {
    chars: Vec<char>,
    cursor: usize, // 字符 index, 范围 0..=chars.len()
}

impl InputBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_str(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, idx: usize) {
        self.cursor = idx.min(self.chars.len());
    }

    /// 在 cursor 处插入一个字符。cursor 后移 1。
    pub fn insert_char(&mut self, c: char) {
        if self.cursor > self.chars.len() {
            self.cursor = self.chars.len();
        }
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// 删除 cursor 左侧字符 (Backspace)。若 cursor==0 则 no-op。
    pub fn backspace(&mut self) -> Option<char> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        Some(self.chars.remove(self.cursor))
    }

    /// 删除 cursor 右侧字符 (Delete)。
    pub fn delete(&mut self) -> Option<char> {
        if self.cursor >= self.chars.len() {
            return None;
        }
        Some(self.chars.remove(self.cursor))
    }

    /// cursor 左移 1。
    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// cursor 右移 1。
    pub fn move_right(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }

    /// 清空。
    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_as_str() {
        let mut b = InputBuffer::new();
        b.insert_char('a');
        b.insert_char('b');
        b.insert_char('c');
        assert_eq!(b.as_str(), "abc");
        assert_eq!(b.cursor(), 3);
    }

    #[test]
    fn test_backspace() {
        let mut b = InputBuffer::new();
        b.insert_char('x');
        b.insert_char('y');
        assert_eq!(b.backspace(), Some('y'));
        assert_eq!(b.as_str(), "x");
        assert_eq!(b.cursor(), 1);
    }

    #[test]
    fn test_backspace_at_start_is_none() {
        let mut b = InputBuffer::new();
        assert_eq!(b.backspace(), None);
    }

    #[test]
    fn test_delete() {
        let mut b = InputBuffer::new();
        b.insert_char('a');
        b.insert_char('b');
        b.set_cursor(0);
        assert_eq!(b.delete(), Some('a'));
        assert_eq!(b.as_str(), "b");
    }

    #[test]
    fn test_move_left_right() {
        let mut b = InputBuffer::new();
        b.insert_char('a');
        b.insert_char('b');
        b.move_left();
        assert_eq!(b.cursor(), 1);
        b.insert_char('X');
        assert_eq!(b.as_str(), "aXb");
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn test_set_cursor_clamps() {
        let mut b = InputBuffer::new();
        b.insert_char('a');
        b.set_cursor(99);
        assert_eq!(b.cursor(), 1);
        b.set_cursor(0);
        assert_eq!(b.cursor(), 0);
    }

    #[test]
    fn test_clear() {
        let mut b = InputBuffer::new();
        b.insert_char('a');
        b.clear();
        assert!(b.is_empty());
        assert_eq!(b.cursor(), 0);
    }
}
