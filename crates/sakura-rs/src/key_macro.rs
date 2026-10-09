// key_macro.rs — キーマクロ記録/再生 (sakura 9.1)
//
// キーストロークを記録し, 再生する.
// 実装: 簡略化された コマンド列 (Insert/Delete/Move) を記録.

/// マクロコマンド
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroCmd {
    /// 文字挿入
    Insert(char),
    /// 文字削除
    Delete,
    /// バックスペース
    Backspace,
    /// カーソル移動 (dx, dy)
    Move(i32, i32),
    /// Enter
    Newline,
    /// 任意テキスト
    Text(String),
}

/// キーマクロ
#[derive(Debug, Default, Clone)]
pub struct KeyMacro {
    name: String,
    cmds: Vec<MacroCmd>,
    recording: bool,
}

impl KeyMacro {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cmds: Vec::new(),
            recording: false,
        }
    }

    /// 記録開始
    pub fn start_record(&mut self) {
        self.cmds.clear();
        self.recording = true;
    }

    /// 記録停止
    pub fn stop_record(&mut self) {
        self.recording = false;
    }

    /// 記録中判定
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// コマンド記録
    pub fn record(&mut self, cmd: MacroCmd) {
        if self.recording {
            self.cmds.push(cmd);
        }
    }

    /// コマンド全件
    pub fn commands(&self) -> &[MacroCmd] {
        &self.cmds
    }

    /// 件数
    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    /// 名前
    pub fn name(&self) -> &str {
        &self.name
    }

    /// クリア
    pub fn clear(&mut self) {
        self.cmds.clear();
    }

    /// テキストとして保存
    pub fn to_script(&self) -> String {
        let mut s = format!("# macro: {}\n", self.name);
        for cmd in &self.cmds {
            let line = match cmd {
                MacroCmd::Insert(c) => format!("insert {:?}", c),
                MacroCmd::Delete => "delete".to_string(),
                MacroCmd::Backspace => "backspace".to_string(),
                MacroCmd::Move(dx, dy) => format!("move {} {}", dx, dy),
                MacroCmd::Newline => "newline".to_string(),
                MacroCmd::Text(t) => format!("text {:?}", t),
            };
            s.push_str(&line);
            s.push('\n');
        }
        s
    }
}

/// キーマクロ管理
#[derive(Debug, Default)]
pub struct MacroRegistry {
    macros: Vec<KeyMacro>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, m: KeyMacro) {
        self.macros.push(m);
    }

    pub fn get(&self, name: &str) -> Option<&KeyMacro> {
        self.macros.iter().find(|m| m.name() == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut KeyMacro> {
        self.macros.iter_mut().find(|m| m.name() == name)
    }

    pub fn names(&self) -> Vec<String> {
        self.macros.iter().map(|m| m.name().to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_commands() {
        let mut m = KeyMacro::new("test1");
        m.start_record();
        m.record(MacroCmd::Insert('h'));
        m.record(MacroCmd::Insert('i'));
        m.record(MacroCmd::Move(1, 0));
        m.stop_record();
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn test_no_record_when_stopped() {
        let mut m = KeyMacro::new("test2");
        m.record(MacroCmd::Insert('a')); // 記録前 → 無視
        m.start_record();
        m.record(MacroCmd::Insert('b'));
        m.stop_record();
        m.record(MacroCmd::Insert('c')); // 停止後 → 無視
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn test_to_script() {
        let mut m = KeyMacro::new("greet");
        m.start_record();
        m.record(MacroCmd::Insert('H'));
        m.record(MacroCmd::Insert('i'));
        m.stop_record();
        let s = m.to_script();
        assert!(s.contains("# macro: greet"));
        assert!(s.contains("insert 'H'"));
    }

    #[test]
    fn test_registry() {
        let mut r = MacroRegistry::new();
        r.register(KeyMacro::new("a"));
        r.register(KeyMacro::new("b"));
        assert_eq!(r.names(), vec!["a", "b"]);
        assert!(r.get("a").is_some());
    }

    #[test]
    fn test_clear() {
        let mut m = KeyMacro::new("c");
        m.start_record();
        m.record(MacroCmd::Insert('x'));
        m.stop_record();
        m.clear();
        assert!(m.is_empty());
    }
}
