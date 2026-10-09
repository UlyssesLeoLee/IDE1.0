// js_macro.rs — JavaScript 風ミニマクロ (sakura 9.3)
//
// フル JS エンジン (rquickjs/boa) は依存が大きすぎるため,
// 専用の mini JS-like 構文でマクロ機能を実装.
//
// サポート:
//   editor.toUpperCase()
//   editor.toLowerCase()
//   editor.insertText("...")
//   editor.replace("find", "rep")
//   editor.setText("...")
//   editor.getText() → string
//
// 注: フル JavaScript ではない. 専用マクロ DSL.

/// ミニ JS 風マクロ実行器
pub struct JsMacroEngine {
    text: String,
}

impl JsMacroEngine {
    pub fn new() -> Self {
        Self { text: String::new() }
    }

    /// スクリプト実行
    pub fn execute_transform(&mut self, script: &str, input: &str) -> Result<String, JsError> {
        self.text = input.to_string();

        // スクリプトを行に分割
        for line in script.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            self.exec_line(trimmed)?;
        }

        Ok(self.text.clone())
    }

    fn exec_line(&mut self, line: &str) -> Result<(), JsError> {
        // editor.toUpperCase();
        if line == "editor.toUpperCase();" || line == "editor.toUpperCase()" {
            self.text = self.text.to_uppercase();
            return Ok(());
        }
        // editor.toLowerCase();
        if line == "editor.toLowerCase();" || line == "editor.toLowerCase()" {
            self.text = self.text.to_lowercase();
            return Ok(());
        }
        // editor.setText("...");
        if let Some(arg) = extract_call_arg(line, "editor.setText") {
            self.text = arg;
            return Ok(());
        }
        // editor.insertText("...");
        if let Some(arg) = extract_call_arg(line, "editor.insertText") {
            self.text.push_str(&arg);
            return Ok(());
        }
        // editor.replace("find", "rep");
        if line.starts_with("editor.replace(") {
            let args = extract_two_args(line, "editor.replace")?;
            self.text = self.text.replace(&args.0, &args.1);
            return Ok(());
        }
        // editor.deleteRange(start, len);
        if line.starts_with("editor.deleteRange(") {
            let args = extract_two_args(line, "editor.deleteRange")?;
            let start = args.0.parse::<usize>().unwrap_or(0);
            let len = args.1.parse::<usize>().unwrap_or(0);
            if start < self.text.len() {
                let end = (start + len).min(self.text.len());
                self.text.replace_range(start..end, "");
            }
            return Ok(());
        }
        // editor.getText() → 値を返すコマンド (read mode)
        if line == "editor.getText();" || line == "editor.getText()" {
            // 副作用なし
            return Ok(());
        }

        Err(JsError::UnknownCommand(line.to_string()))
    }

    /// 現在のテキスト取得
    pub fn get_text(&self) -> &str {
        &self.text
    }
}

/// `func("arg")` の "arg" 部分を取り出す
fn extract_call_arg(line: &str, func: &str) -> Option<String> {
    let after = line.strip_prefix(func)?;
    let after = after.trim_start_matches('(');
    let after = after.trim_end_matches(';').trim_end_matches(')');
    let s = after.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        Some(s[1..s.len() - 1].to_string())
    } else {
        None
    }
}

/// `func("a", "b")` の 2 引数を取り出す
fn extract_two_args(line: &str, func: &str) -> Result<(String, String), JsError> {
    let after = line
        .strip_prefix(func)
        .ok_or_else(|| JsError::Syntax(line.to_string()))?;
    let after = after.trim_start_matches('(').trim_end_matches(';').trim_end_matches(')');
    let after = after.trim();
    // "a", "b" を split
    let parts: Vec<&str> = after.split(',').map(|s| s.trim()).collect();
    if parts.len() != 2 {
        return Err(JsError::ArgCount(2, parts.len()));
    }
    let a = parts[0].trim_matches('"');
    let b = parts[1].trim_matches('"');
    Ok((a.to_string(), b.to_string()))
}

#[derive(Debug)]
pub enum JsError {
    UnknownCommand(String),
    Syntax(String),
    ArgCount(usize, usize),
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCommand(s) => write!(f, "Unknown command: {s}"),
            Self::Syntax(s) => write!(f, "Syntax error: {s}"),
            Self::ArgCount(exp, got) => write!(f, "Expected {exp} args, got {got}"),
        }
    }
}

impl std::error::Error for JsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_upper() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform("editor.toUpperCase();", "hello world").unwrap();
        assert_eq!(r, "HELLO WORLD");
    }

    #[test]
    fn test_to_lower() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform("editor.toLowerCase();", "HELLO").unwrap();
        assert_eq!(r, "hello");
    }

    #[test]
    fn test_insert() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform(r#"editor.insertText(" world");"#, "hello").unwrap();
        assert_eq!(r, "hello world");
    }

    #[test]
    fn test_set_text() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform(r#"editor.setText("REPLACED");"#, "original").unwrap();
        assert_eq!(r, "REPLACED");
    }

    #[test]
    fn test_replace() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform(
            r#"editor.replace("foo", "bar");"#,
            "foo baz foo"
        ).unwrap();
        assert_eq!(r, "bar baz bar");
    }

    #[test]
    fn test_delete_range() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform(
            r#"editor.deleteRange(0, 3);"#,
            "hello world"
        ).unwrap();
        assert_eq!(r, "lo world");
    }

    #[test]
    fn test_chained() {
        let mut e = JsMacroEngine::new();
        let script = r#"
            editor.toUpperCase();
            editor.insertText("!");
        "#;
        let r = e.execute_transform(script, "hello").unwrap();
        assert_eq!(r, "HELLO!");
    }

    #[test]
    fn test_unknown_command() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform("foo();", "x");
        assert!(r.is_err());
    }

    #[test]
    fn test_get_text_no_op() {
        let mut e = JsMacroEngine::new();
        let r = e.execute_transform("editor.getText();", "x").unwrap();
        assert_eq!(r, "x");
    }

    #[test]
    fn test_comments() {
        let mut e = JsMacroEngine::new();
        let script = r#"
            // comment
            editor.toUpperCase();
        "#;
        let r = e.execute_transform(script, "hi").unwrap();
        assert_eq!(r, "HI");
    }
}
