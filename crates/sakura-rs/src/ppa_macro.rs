// ppa_macro.rs — PPA マクロ (basic) (sakura 9.2)
//
// PPA (Poor-Pascal) は Delphi/Pascal 系マクロ言語.
// 完全実装はしないが, basic なテキスト処理コマンドを実装.

/// PPA 風コマンド
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PpaCmd {
    /// S_InsertText 相当 — テキスト挿入
    InsertText(String),
    /// S_DeleteText — 範囲削除
    DeleteText { start: usize, length: usize },
    /// S_MoveCursor — カーソル移動
    MoveCursor { line: usize, col: usize },
    /// S_ReplaceText — 置換
    ReplaceText { find: String, replace: String },
    /// S_GetLineCount — 行数取得 (実行時)
    GetLineCount,
}

/// PPA 風マクロ
#[derive(Debug, Default, Clone)]
pub struct PpaMacro {
    name: String,
    cmds: Vec<PpaCmd>,
}

impl PpaMacro {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cmds: Vec::new(),
        }
    }

    pub fn add(&mut self, cmd: PpaCmd) {
        self.cmds.push(cmd);
    }

    pub fn commands(&self) -> &[PpaCmd] {
        &self.cmds
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// 簡易実行 (テキスト変換)
    pub fn execute(&self, input: &str) -> String {
        let mut out = input.to_string();
        for cmd in &self.cmds {
            match cmd {
                PpaCmd::InsertText(t) => {
                    out.push_str(t);
                }
                PpaCmd::DeleteText { start, length } => {
                    if *start < out.len() {
                        let end = (*start + *length).min(out.len());
                        out.replace_range(*start..end, "");
                    }
                }
                PpaCmd::ReplaceText { find, replace } => {
                    if !find.is_empty() {
                        out = out.replace(find, replace);
                    }
                }
                PpaCmd::MoveCursor { .. } | PpaCmd::GetLineCount => {
                    // 副作用なし
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_text() {
        let mut m = PpaMacro::new("add_foo");
        m.add(PpaCmd::InsertText("foo".to_string()));
        assert_eq!(m.execute(""), "foo");
    }

    #[test]
    fn test_delete_text() {
        let mut m = PpaMacro::new("del");
        m.add(PpaCmd::DeleteText { start: 0, length: 3 });
        assert_eq!(m.execute("hello world"), "lo world");
    }

    #[test]
    fn test_replace_text() {
        let mut m = PpaMacro::new("rep");
        m.add(PpaCmd::ReplaceText { find: "foo".to_string(), replace: "bar".to_string() });
        assert_eq!(m.execute("foo baz foo"), "bar baz bar");
    }

    #[test]
    fn test_chained_commands() {
        let mut m = PpaMacro::new("chain");
        m.add(PpaCmd::InsertText(" [edited]".to_string()));
        m.add(PpaCmd::ReplaceText { find: "x".to_string(), replace: "y".to_string() });
        let r = m.execute("xx");
        assert_eq!(r, "yy [edited]");
    }

    #[test]
    fn test_empty_find() {
        let mut m = PpaMacro::new("safe");
        m.add(PpaCmd::ReplaceText { find: "".to_string(), replace: "x".to_string() });
        // 入力は変更なし (空 find は skip)
        assert_eq!(m.execute("hello"), "hello");
    }
}
