//! Sakura 键盘宏 (`.mac`) 解析器 + 动作规划器 (WBS-02 包 4.1, 库层).
//!
//! 流程: `.mac` 文本 --[`parse`]--> [`Statement`] 列表 --[`plan`]--> [`MacroStep`] 列表.
//! 前端把每个 [`MacroStep`] 的 [`Action`] 依次交给已有的命令分发器执行 (逐步, 可随时中止).
//!
//! 语法 (与 Sakura 记录出的宏一致, 稍宽松):
//! - 语句 `S_Name(arg, ...);`, 行尾的 `;` 可省略; 函数名区分大小写且必须以 `S_` 开头
//! - 参数: 十进制/`0x` 十六进制整数 (可带负号), `"..."` 或 `'...'` 字符串
//! - 字符串转义: `\\ \" \' \r \n \t \0`; 其它 `\x` 原样保留反斜杠 (正则里的 `\w` 要保持原样)
//! - 注释: `// 到行尾`, `/* ... */`
//!
//! `SAKURA_COMMANDS` 表由一次性脚本从 `tests/mock/fixtures/sakura-map.mjs` 生成:
//! `(S_ 名(不含 S_), F_ id, 本项目命令名, 本项目默认键)`, 按名字排序以便二分查找.
//!
//! 不在范围: PPA 变量 / 控制流 / `S_Get*` 返回值 (WBS 4.4).

use serde::Serialize;
use std::fmt;

/// 宏调用参数.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MacroArg {
    Int(i64),
    Str(String),
}

/// 解析后的一条语句 (行列均从 1 开始).
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    /// 含 `S_` 前缀的完整函数名.
    pub name: String,
    pub args: Vec<MacroArg>,
    pub line: usize,
    pub col: usize,
}

/// 解析 / 规划错误, 带位置, 未知函数时 `func` 为函数名.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MacroError {
    pub line: usize,
    pub col: usize,
    pub func: Option<String>,
    pub message: String,
}

impl fmt::Display for MacroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}:{}: {}", self.line, self.col, self.message)
    }
}
impl std::error::Error for MacroError {}

/// 规划出的原子动作.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// 走命令分发器: 有 `cmd` 用 cmd, 否则用 `key` 模拟按键. `args` 是宏里带的(多为占位的 0).
    Command { id: String, cmd: Option<String>, key: Option<String>, args: Vec<MacroArg> },
    /// `S_InsText("x")`: 在光标处插入文本 (有选区则替换).
    InsText { text: String },
    /// `S_Char(code)`: 键入一个字符 (13 = 回车), 走键入路径.
    Char { code: u32 },
    /// `S_MoveCursor(line, col, flag)`: 逻辑位置, 这里已转成 0 起; `select` = flag 的 bit0.
    MoveCursor { row: usize, col: usize, select: bool },
    /// `S_MoveCursorLayout(line, col, flag)`: 同上, 但是折行后的物理位置.
    MoveCursorLayout { row: usize, col: usize, select: bool },
    /// `S_Jump(line, flag)`: 跳到行号 (1 起, 原样保留; 超出文件由前端夹到末行).
    Jump { line: i64, flag: i64 },
    SearchNext { text: String, flags: u32 },
    SearchPrev { text: String, flags: u32 },
    ReplaceAll { search: String, replace: String, flags: u32 },
    ReplaceNext { search: String, replace: String, flags: u32 },
}

/// 一步: 源码行 + 函数名 + 动作.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MacroStep {
    pub line: usize,
    pub name: String,
    pub action: Action,
}

/// 检索 / 置换的 flags 位 (`S_SearchNext` / `S_ReplaceAll` 的最后一个参数).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct SearchFlags {
    pub word: bool,           // 0x01
    pub case_sensitive: bool, // 0x02
    pub regex: bool,          // 0x04
    pub wrap: bool,           // 0x20 从头(尾)再检索
    pub in_selection: bool,   // 0x80 仅选区内 (置换)
}

impl SearchFlags {
    pub fn from_bits(b: u32) -> Self {
        SearchFlags {
            word: b & 0x01 != 0,
            case_sensitive: b & 0x02 != 0,
            regex: b & 0x04 != 0,
            wrap: b & 0x20 != 0,
            in_selection: b & 0x80 != 0,
        }
    }
}

// ---------------------------------------------------------------- 解析

struct Lexer {
    c: Vec<char>,
    i: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    fn err(&self, msg: impl Into<String>) -> MacroError {
        MacroError { line: self.line, col: self.col, func: None, message: msg.into() }
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn peek2(&self) -> Option<char> {
        self.c.get(self.i + 1).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.c.get(self.i).copied()?;
        self.i += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }
    /// 跳过空白与注释 (含换行).
    fn skip_ws(&mut self) -> Result<(), MacroError> {
        loop {
            match (self.peek(), self.peek2()) {
                (Some(ch), _) if ch.is_whitespace() || ch == '\u{feff}' => {
                    self.bump();
                }
                (Some('/'), Some('/')) => {
                    while let Some(ch) = self.peek() {
                        if ch == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                (Some('/'), Some('*')) => {
                    let (l, c) = (self.line, self.col);
                    self.bump();
                    self.bump();
                    loop {
                        match (self.peek(), self.peek2()) {
                            (Some('*'), Some('/')) => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            (Some(_), _) => {
                                self.bump();
                            }
                            (None, _) => {
                                return Err(MacroError { line: l, col: c, func: None, message: "unterminated /* comment".into() })
                            }
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }
    fn string(&mut self, q: char) -> Result<String, MacroError> {
        let (l, c) = (self.line, self.col);
        self.bump();
        let mut s = String::new();
        loop {
            match self.bump() {
                None | Some('\n') => {
                    return Err(MacroError { line: l, col: c, func: None, message: "unterminated string literal".into() })
                }
                Some(ch) if ch == q => return Ok(s),
                Some('\\') => match self.bump() {
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('\'') => s.push('\''),
                    Some('r') => s.push('\r'),
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('0') => s.push('\0'),
                    Some(o) if o != '\n' => {
                        s.push('\\');
                        s.push(o);
                    }
                    _ => return Err(MacroError { line: l, col: c, func: None, message: "unterminated string literal".into() }),
                },
                Some(ch) => s.push(ch),
            }
        }
    }
    fn int(&mut self) -> Result<i64, MacroError> {
        let neg = self.peek() == Some('-');
        if neg {
            self.bump();
        }
        let mut digits = String::new();
        let hex = self.peek() == Some('0') && matches!(self.peek2(), Some('x') | Some('X'));
        if hex {
            self.bump();
            self.bump();
        }
        while let Some(ch) = self.peek() {
            if (hex && ch.is_ascii_hexdigit()) || ch.is_ascii_digit() {
                digits.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        let radix = if hex { 16 } else { 10 };
        let v = i64::from_str_radix(&digits, radix).map_err(|_| self.err("invalid integer literal"))?;
        Ok(if neg { -v } else { v })
    }
}

/// 把 `.mac` 文本解析为语句列表. 不检查函数是否存在 (那是 [`plan`] 的事).
pub fn parse(src: &str) -> Result<Vec<Statement>, MacroError> {
    let mut lx = Lexer { c: src.chars().collect(), i: 0, line: 1, col: 1 };
    let mut out = Vec::new();
    loop {
        lx.skip_ws()?;
        let Some(first) = lx.peek() else { break };
        let (line, col) = (lx.line, lx.col);
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(lx.err(format!("unexpected character '{first}'")));
        }
        let mut name = String::new();
        while let Some(ch) = lx.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                name.push(ch);
                lx.bump();
            } else {
                break;
            }
        }
        let with_func = |mut e: MacroError| {
            e.func = Some(name.clone());
            e
        };
        if !name.starts_with("S_") {
            return Err(MacroError { line, col, func: Some(name.clone()), message: format!("'{name}' is not an S_ function") });
        }
        lx.skip_ws()?;
        if lx.peek() != Some('(') {
            return Err(with_func(lx.err(format!("expected '(' after {name}"))));
        }
        lx.bump();
        let mut args = Vec::new();
        lx.skip_ws()?;
        if lx.peek() == Some(')') {
            lx.bump();
        } else {
            loop {
                lx.skip_ws()?;
                match lx.peek() {
                    Some(q @ ('"' | '\'')) => args.push(MacroArg::Str(lx.string(q).map_err(&with_func)?)),
                    Some(ch) if ch.is_ascii_digit() || ch == '-' => args.push(MacroArg::Int(lx.int().map_err(&with_func)?)),
                    _ => return Err(with_func(lx.err("expected an argument"))),
                }
                lx.skip_ws()?;
                match lx.bump() {
                    Some(',') => {}
                    Some(')') => break,
                    _ => return Err(with_func(lx.err("expected ',' or ')'"))),
                }
            }
        }
        while matches!(lx.peek(), Some(' ' | '\t' | '\r')) {
            lx.bump();
        }
        match (lx.peek(), lx.peek2()) {
            (Some(';'), _) => {
                lx.bump();
            }
            (None, _) | (Some('\n'), _) | (Some('/'), Some('/' | '*')) => {}
            _ => return Err(with_func(lx.err("missing ';'"))),
        }
        out.push(Statement { name, args, line, col });
    }
    Ok(out)
}

// ---------------------------------------------------------------- 规划

/// 查 `S_` 函数 (传入不含 `S_` 的名字) 对应的命令: `(F_ id, 命令名, 默认键)`.
pub fn lookup_command(name: &str) -> Option<(&'static str, Option<&'static str>, Option<&'static str>)> {
    SAKURA_COMMANDS
        .binary_search_by(|e| e.0.cmp(name))
        .ok()
        .map(|i| (SAKURA_COMMANDS[i].1, SAKURA_COMMANDS[i].2, SAKURA_COMMANDS[i].3))
}

fn bad(st: &Statement, msg: impl Into<String>) -> MacroError {
    MacroError { line: st.line, col: st.col, func: Some(st.name.clone()), message: format!("{}: {}", st.name, msg.into()) }
}
fn int_at(st: &Statement, i: usize) -> Result<i64, MacroError> {
    match st.args.get(i) {
        Some(MacroArg::Int(v)) => Ok(*v),
        Some(_) => Err(bad(st, format!("argument {} must be an integer", i + 1))),
        None => Err(bad(st, format!("missing argument {}", i + 1))),
    }
}
fn str_at(st: &Statement, i: usize) -> Result<String, MacroError> {
    match st.args.get(i) {
        Some(MacroArg::Str(s)) => Ok(s.clone()),
        Some(_) => Err(bad(st, format!("argument {} must be a string", i + 1))),
        None => Err(bad(st, format!("missing argument {}", i + 1))),
    }
}
fn count(st: &Statement, lo: usize, hi: usize) -> Result<(), MacroError> {
    let n = st.args.len();
    if n < lo || n > hi {
        let want = if lo == hi { lo.to_string() } else { format!("{lo}..{hi}") };
        return Err(bad(st, format!("expects {want} argument(s), got {n}")));
    }
    Ok(())
}
fn flags_at(st: &Statement, i: usize) -> Result<u32, MacroError> {
    let v = int_at(st, i)?;
    u32::try_from(v).map_err(|_| bad(st, "flags out of range"))
}

fn plan_one(st: &Statement) -> Result<Action, MacroError> {
    let short = &st.name[2..];
    Ok(match short {
        "Char" => {
            count(st, 1, 1)?;
            let v = int_at(st, 0)?;
            let code = u32::try_from(v).ok().filter(|c| *c <= 0x10FFFF).ok_or_else(|| bad(st, "character code out of range"))?;
            Action::Char { code }
        }
        "InsText" => {
            count(st, 1, 1)?;
            Action::InsText { text: str_at(st, 0)? }
        }
        "MoveCursor" | "MoveCursorLayout" => {
            count(st, 3, 3)?;
            let row = (int_at(st, 0)?.max(1) - 1) as usize;
            let col = (int_at(st, 1)?.max(1) - 1) as usize;
            let select = int_at(st, 2)? & 1 != 0;
            if short == "MoveCursor" {
                Action::MoveCursor { row, col, select }
            } else {
                Action::MoveCursorLayout { row, col, select }
            }
        }
        "Jump" => {
            count(st, 1, 2)?;
            let flag = if st.args.len() > 1 { int_at(st, 1)? } else { 0 };
            Action::Jump { line: int_at(st, 0)?, flag }
        }
        "SearchNext" | "SearchPrev" if !st.args.is_empty() => {
            count(st, 2, 2)?;
            let (text, flags) = (str_at(st, 0)?, flags_at(st, 1)?);
            if short == "SearchNext" {
                Action::SearchNext { text, flags }
            } else {
                Action::SearchPrev { text, flags }
            }
        }
        "ReplaceAll" | "ReplaceNext" => {
            count(st, 3, 3)?;
            let (search, replace, flags) = (str_at(st, 0)?, str_at(st, 1)?, flags_at(st, 2)?);
            if short == "ReplaceAll" {
                Action::ReplaceAll { search, replace, flags }
            } else {
                Action::ReplaceNext { search, replace, flags }
            }
        }
        _ => match lookup_command(short) {
            Some((id, cmd, key)) => Action::Command {
                id: id.to_string(),
                cmd: cmd.map(str::to_string),
                key: key.map(str::to_string),
                args: st.args.clone(),
            },
            None => {
                return Err(MacroError {
                    line: st.line,
                    col: st.col,
                    func: Some(st.name.clone()),
                    message: format!("unknown Sakura macro function {} (line {})", st.name, st.line),
                })
            }
        },
    })
}

/// 解析并规划: `.mac` 文本 -> 逐步动作. 任何语法错误 / 未知函数 / 参数不符都整体失败 (不执行一半).
pub fn plan(src: &str) -> Result<Vec<MacroStep>, MacroError> {
    parse(src)?
        .iter()
        .map(|st| plan_one(st).map(|action| MacroStep { line: st.line, name: st.name.clone(), action }))
        .collect()
}

/// `(S_ 名(不含 S_), F_ id, 命令名, 默认键)`, 按名字字节序排序. 由 sakura-map.mjs 生成.
#[rustfmt::skip]
pub const SAKURA_COMMANDS: &[(&str, &str, Option<&str>, Option<&str>)] = &[
    ("1PageDown", "F_1PageDown", None, Some("PageDown")),
    ("1PageDown_BoxSel", "F_1PageDown_BOX", None, Some("Alt+PageDown")),
    ("1PageDown_Sel", "F_1PageDown_Sel", None, Some("Shift+PageDown")),
    ("1PageUp", "F_1PageUp", None, Some("PageUp")),
    ("1PageUp_BoxSel", "F_1PageUp_BOX", None, Some("Alt+PageUp")),
    ("1PageUp_Sel", "F_1PageUp_Sel", None, Some("Shift+PageUp")),
    ("About", "F_ABOUT", None, Some("Control+Shift+F1")),
    ("ActivateSQLPlus", "F_ACTIVATE_SQLPLUS", None, Some("Shift+F11")),
    ("Base64Decode", "F_BASE64DECODE", None, Some("Alt+F6")),
    ("BeginBoxSelect", "F_BEGIN_BOX", None, Some("Shift+F6")),
    ("BeginSelect", "F_BEGIN_SEL", None, Some("F6")),
    ("BindWindow", "F_BIND_WINDOW", Some("bind-window"), None),
    ("BookmarkNext", "F_BOOKMARK_NEXT", None, Some("F2")),
    ("BookmarkPrev", "F_BOOKMARK_PREV", None, Some("Shift+F2")),
    ("BookmarkReset", "F_BOOKMARK_RESET", None, Some("Control+Shift+F2")),
    ("BookmarkSet", "F_BOOKMARK_SET", None, Some("Control+F2")),
    ("BookmarkView", "F_BOOKMARK_VIEW", None, Some("Alt+F2")),
    ("BracketPair", "F_BRACKETPAIR", None, Some("Control+[")),
    ("Browse", "F_BROWSE", None, Some("Control+b")),
    ("CancelMode", "F_CANCEL_MODE", None, Some("Escape")),
    ("Cascade", "F_CASCADE", None, Some("Control+Alt+e")),
    ("ChgCharset", "F_CHG_CHARSET", Some("charset"), None),
    ("ChgmodEOLCR", "F_CHGMOD_EOL_CR", Some("eol-cr"), None),
    ("ChgmodEOLCRLF", "F_CHGMOD_EOL_CRLF", Some("eol-crlf"), None),
    ("ChgmodEOLLF", "F_CHGMOD_EOL_LF", Some("eol-lf"), None),
    ("ChgmodINS", "F_CHGMOD_INS", None, Some("Insert")),
    ("CodeCnvAUTOtoSJIS", "F_CODECNV_AUTO2SJIS", None, Some("Alt+F12")),
    ("CodeCnvEMAIL", "F_CODECNV_EMAIL", None, Some("Alt+F11")),
    ("CodeCnvEUCtoSJIS", "F_CODECNV_EUC2SJIS", None, Some("Alt+F10")),
    ("CodeCnvSJIStoEUC", "F_CODECNV_SJIS2EUC", None, Some("Shift+Alt+F10")),
    ("CodeCnvSJIStoJIS", "F_CODECNV_SJIS2JIS", None, Some("Shift+Alt+F11")),
    ("CodeCnvSJIStoUTF7", "F_CODECNV_SJIS2UTF7", None, Some("Shift+Alt+F7")),
    ("CodeCnvSJIStoUTF8", "F_CODECNV_SJIS2UTF8", None, Some("Shift+Alt+F8")),
    ("CodeCnvUNICODEBEtoSJIS", "F_CODECNV_UNICODEBE2SJIS", Some("codecnv-unicodebe2sjis"), None),
    ("CodeCnvUNICODEtoSJIS", "F_CODECNV_UNICODE2SJIS", None, Some("Alt+F9")),
    ("CodeCnvUTF7toSJIS", "F_CODECNV_UTF72SJIS", None, Some("Alt+F7")),
    ("CodeCnvUTF8toSJIS", "F_CODECNV_UTF82SJIS", None, Some("Alt+F8")),
    ("Compare", "F_COMPARE", None, Some("Control+Enter")),
    ("Copy", "F_COPY", None, Some("F8")),
    ("CopyAddCRLF", "F_COPY_ADDCRLF", Some("copy-addcrlf"), None),
    ("CopyCRLF", "F_COPY_CRLF", None, Some("Shift+F8")),
    ("CopyColorHtml", "F_COPY_COLOR_HTML", Some("copy-html"), None),
    ("CopyColorHtmlLineNumber", "F_COPY_COLOR_HTML_LINENUMBER", Some("copy-html-ln"), None),
    ("CopyFilename", "F_COPYFNAME", None, Some("Control+-")),
    ("CopyLines", "F_COPYLINES", Some("copy-lines"), None),
    ("CopyLinesAsPassage", "F_COPYLINESASPASSAGE", None, Some("Control+.")),
    ("CopyLinesWithLineNumber", "F_COPYLINESWITHLINENUMBER", Some("copy-linenum"), None),
    ("CopyPath", "F_COPYPATH", None, Some("Control+\\")),
    ("CopyTag", "F_COPYTAG", Some("copy-tag"), None),
    ("CreateKeyBindList", "F_CREATEKEYBINDLIST", None, Some("Control+q")),
    ("CtrlCodeDialog", "F_CTRL_CODE_DIALOG", Some("ctrl-code"), None),
    ("CurLineCenter", "F_CURLINECENTER", None, Some("Control+h")),
    ("Cut", "F_CUT", None, Some("F7")),
    ("CutLine", "F_CUT_LINE", None, Some("Control+e")),
    ("Delete", "F_DELETE", None, Some("Delete")),
    ("DeleteBack", "F_DELETE_BACK", None, Some("Backspace")),
    ("DeleteLine", "F_DELETE_LINE", None, Some("Control+Shift+e")),
    ("DiffDialog", "F_DIFF_DIALOG", Some("diff-list"), None),
    ("DiffNext", "F_DIFF_NEXT", Some("diff-next"), None),
    ("DiffPrev", "F_DIFF_PREV", Some("diff-prev"), None),
    ("DiffReset", "F_DIFF_RESET", Some("diff-clear"), None),
    ("DirectTagJump", "F_DIRECT_TAGJUMP", Some("tag-jump"), None),
    ("DlgWinList", "F_DLGWINLIST", Some("window-list"), None),
    ("Down", "F_DOWN", None, Some("ArrowDown")),
    ("Down2", "F_DOWN2", Some("cursor-down-2"), None),
    ("Down2_BoxSel", "F_DOWN2_BOX", None, Some("Control+Alt+ArrowDown")),
    ("Down2_Sel", "F_DOWN2_SEL", None, Some("Control+Shift+ArrowDown")),
    ("Down_BoxSel", "F_DOWN_BOX", None, Some("Alt+ArrowDown")),
    ("Down_Sel", "F_DOWN_SEL", None, Some("Shift+ArrowDown")),
    ("DuplicateLine", "F_DUPLICATELINE", None, Some("Control+i")),
    ("ExecCommandDialog", "F_EXECMD_DIALOG", None, Some("Control+F5")),
    ("ExecExtMacro", "F_EXECEXTMACRO", Some("js-macro"), None),
    ("ExecKeyMacro", "F_EXECKEYMACRO", None, Some("Control+Shift+l")),
    ("ExitAll", "F_EXITALL", None, Some("Control+Alt+F4")),
    ("ExitAllEditors", "F_EXITALLEDITORS", None, Some("Shift+Alt+F4")),
    ("ExtHelp1", "F_EXTHELP1", None, Some("Control+F1")),
    ("ExtHtmlHelp", "F_EXTHTMLHELP", None, Some("F1")),
    ("Favorite", "F_FAVORITE", Some("history-manager"), None),
    ("FileClose", "F_FILECLOSE", None, Some("Control+F4")),
    ("FileCloseOpen", "F_FILECLOSE_OPEN", None, Some("Control+Shift+F4")),
    ("FileNew", "F_FILENEW", None, Some("Control+n")),
    ("FileNewNewWindow", "F_FILENEW_NEWWINDOW", Some("new-window"), None),
    ("FileOpen", "F_FILEOPEN", None, Some("Control+o")),
    ("FileReopen", "F_FILE_REOPEN", Some("charset"), None),
    ("FileReopenCESU8", "F_FILE_REOPEN_CESU8", Some("reopen-CESU-8"), None),
    ("FileReopenEUC", "F_FILE_REOPEN_EUC", None, Some("Control+Alt+F10")),
    ("FileReopenJIS", "F_FILE_REOPEN_JIS", None, Some("Control+Alt+F11")),
    ("FileReopenLatin1", "F_FILE_REOPEN_LATIN1", Some("reopen-Latin1"), None),
    ("FileReopenSJIS", "F_FILE_REOPEN_SJIS", None, Some("Control+Alt+F12")),
    ("FileReopenUNICODE", "F_FILE_REOPEN_UNICODE", None, Some("Control+Alt+F9")),
    ("FileReopenUNICODEBE", "F_FILE_REOPEN_UNICODEBE", Some("reopen-UnicodeBE"), None),
    ("FileReopenUTF7", "F_FILE_REOPEN_UTF7", None, Some("Control+Alt+F7")),
    ("FileReopenUTF8", "F_FILE_REOPEN_UTF8", None, Some("Control+Alt+F8")),
    ("FileSave", "F_FILESAVE", None, Some("Control+s")),
    ("FileSaveAll", "F_FILESAVEALL", Some("save-all"), None),
    ("FileSaveAs", "F_FILESAVEAS_DIALOG", None, Some("Control+Shift+s")),
    ("FileSaveClose", "F_FILESAVECLOSE", Some("save-close"), None),
    ("FileTree", "F_FILETREE", Some("file-tree"), None),
    ("FileUsedRecently", "F_FILE_USED_RECENTLY", Some("recent"), None),
    ("Font", "F_FONT", None, Some("Control+7")),
    ("FuncListNext", "F_FUNCLIST_NEXT", Some("funclist-next"), None),
    ("FuncListPrev", "F_FUNCLIST_PREV", Some("funclist-prev"), None),
    ("GoFileEnd", "F_GOFILEEND", None, Some("Control+End")),
    ("GoFileEnd_BoxSel", "F_GOFILEEND_BOX", None, Some("Control+Alt+End")),
    ("GoFileEnd_Sel", "F_GOFILEEND_SEL", None, Some("Control+Shift+End")),
    ("GoFileTop", "F_GOFILETOP", None, Some("Control+Home")),
    ("GoFileTop_BoxSel", "F_GOFILETOP_BOX", None, Some("Control+Alt+Home")),
    ("GoFileTop_Sel", "F_GOFILETOP_SEL", None, Some("Control+Shift+Home")),
    ("GoLineEnd", "F_GOLINEEND", None, Some("End")),
    ("GoLineEnd_BoxSel", "F_GOLINEEND_BOX", None, Some("Alt+End")),
    ("GoLineEnd_Sel", "F_GOLINEEND_SEL", None, Some("Shift+End")),
    ("GoLineTop", "F_GOLINETOP", None, Some("Home")),
    ("GoLineTop_BoxSel", "F_GOLINETOP_BOX", None, Some("Alt+Home")),
    ("GoLineTop_Sel", "F_GOLINETOP_SEL", None, Some("Shift+Home")),
    ("GoLogicalLineTop_BoxSel", "F_GOLOGICALLINETOP_BOX", Some("box-line-top"), None),
    ("GoNextParagraph", "F_GONEXTPARAGRAPH", Some("cursor-para-next"), None),
    ("GoNextParagraph_Sel", "F_GONEXTPARAGRAPH_SEL", Some("sel-para-next"), None),
    ("GoPrevParagraph", "F_GOPREVPARAGRAPH", Some("cursor-para-prev"), None),
    ("GoPrevParagraph_Sel", "F_GOPREVPARAGRAPH_SEL", Some("sel-para-prev"), None),
    ("GrepDialog", "F_GREP_DIALOG", None, Some("Control+g")),
    ("GrepReplaceDialog", "F_GREP_REPLACE_DLG", Some("grep-replace"), None),
    ("GroupClose", "F_GROUPCLOSE", Some("group-close"), None),
    ("HalfPageDown", "F_HalfPageDown", None, Some("Control+PageDown")),
    ("HalfPageDown_BoxSel", "F_HalfPageDown_BOX", None, Some("Control+Alt+PageDown")),
    ("HalfPageDown_Sel", "F_HalfPageDown_Sel", None, Some("Control+Shift+PageDown")),
    ("HalfPageUp", "F_HalfPageUp", None, Some("Control+PageUp")),
    ("HalfPageUp_BoxSel", "F_HalfPageUp_BOX", None, Some("Control+Alt+PageUp")),
    ("HalfPageUp_Sel", "F_HalfPageUp_Sel", None, Some("Control+Shift+PageUp")),
    ("HanKataToZenHira", "F_HANKATATOZENHIRA", None, Some("Control+F12")),
    ("HanKataToZenKata", "F_HANKATATOZENKATA", None, Some("Control+F11")),
    ("HelpContents", "F_HELP_CONTENTS", None, Some("Alt+F1")),
    ("HelpSearch", "F_HELP_SEARCH", None, Some("Shift+Alt+F1")),
    ("Hokan", "F_HOKAN", None, Some("Control+Space")),
    ("ISearchMigemoNext", "F_ISEARCH_MIGEMO_NEXT", Some("isearch-migemo-fwd"), None),
    ("ISearchMigemoPrev", "F_ISEARCH_MIGEMO_PREV", Some("isearch-migemo-back"), None),
    ("ISearchNext", "F_ISEARCH_NEXT", Some("isearch-fwd"), None),
    ("ISearchPrev", "F_ISEARCH_PREV", Some("isearch-back"), None),
    ("ISearchRegexpNext", "F_ISEARCH_REGEXP_NEXT", Some("isearch-regex-fwd"), None),
    ("ISearchRegexpPrev", "F_ISEARCH_REGEXP_PREV", Some("isearch-regex-back"), None),
    ("IndentSpace", "F_INDENT_SPACE", Some("indent-space"), None),
    ("IndentTab", "F_INDENT_TAB", None, Some("Tab")),
    ("InsDate", "F_INS_DATE", None, Some("Alt+;")),
    ("InsFileUsedRecently", "F_INS_FILE_USED_RECENTLY", Some("ins-recent-file"), None),
    ("InsFolderUsedRecently", "F_INS_FOLDER_USED_RECENTLY", Some("ins-recent-folder"), None),
    ("InsTime", "F_INS_TIME", Some("insert-time"), None),
    ("JumpDialog", "F_JUMP_DIALOG", None, Some("Control+j")),
    ("JumpHistNext", "F_JUMPHIST_NEXT", None, Some("Alt+n")),
    ("JumpHistPrev", "F_JUMPHIST_PREV", None, Some("Alt+p")),
    ("JumpHistSet", "F_JUMPHIST_SET", Some("jumphist-set"), None),
    ("JumpSrchStartPos", "F_JUMP_SRCHSTARTPOS", None, Some("Control+Shift+F3")),
    ("LTrim", "F_LTRIM", None, Some("Alt+l")),
    ("Left", "F_LEFT", None, Some("ArrowLeft")),
    ("Left_BoxSel", "F_LEFT_BOX", None, Some("Alt+ArrowLeft")),
    ("Left_Sel", "F_LEFT_SEL", None, Some("Shift+ArrowLeft")),
    ("LineCutToEnd", "F_LineCutToEnd", None, Some("Control+k")),
    ("LineCutToStart", "F_LineCutToStart", None, Some("Control+u")),
    ("LineDeleteToEnd", "F_LineDeleteToEnd", None, Some("Control+Shift+k")),
    ("LineDeleteToStart", "F_LineDeleteToStart", None, Some("Control+Shift+u")),
    ("LoadKeyMacro", "F_LOADKEYMACRO", None, Some("Control+l")),
    ("MaximizeH", "F_MAXIMIZE_H", None, Some("Control+Shift+Alt+ArrowRight")),
    ("MaximizeV", "F_MAXIMIZE_V", None, Some("Control+Shift+Alt+ArrowUp")),
    ("MenuAllFunc", "F_MENU_ALLFUNC", None, Some("Shift+F1")),
    ("Merge", "F_MERGE", None, Some("Alt+m")),
    ("MinimizeAll", "F_MINIMIZE_ALL", None, Some("Control+Shift+Alt+ArrowDown")),
    ("ModifyLineNext", "F_MODIFYLINE_NEXT", Some("modify-next"), None),
    ("ModifyLineNext_Sel", "F_MODIFYLINE_NEXT_SEL", Some("modify-next-sel"), None),
    ("ModifyLinePrev", "F_MODIFYLINE_PREV", Some("modify-prev"), None),
    ("ModifyLinePrev_Sel", "F_MODIFYLINE_PREV_SEL", Some("modify-prev-sel"), None),
    ("NextGroup", "F_NEXTGROUP", Some("next-group"), None),
    ("NextWindow", "F_NEXTWINDOW", None, Some("Control+Tab")),
    ("OpenHfromtoC", "F_OPEN_HfromtoC", None, Some("Control+Shift+c")),
    ("Option", "F_OPTION", None, Some("Control+6")),
    ("OptionType", "F_OPTION_TYPE", None, Some("Control+5")),
    ("Outline", "F_OUTLINE", Some("outline"), None),
    ("PLSQLCompileOnSQLPlus", "F_PLSQL_COMPILE_ON_SQLPLUS", Some("sqlplus-run"), None),
    ("Paste", "F_PASTE", None, Some("F9")),
    ("PasteBox", "F_PASTEBOX", None, Some("Shift+F9")),
    ("PrevGroup", "F_PREVGROUP", Some("prev-group"), None),
    ("PrevWindow", "F_PREVWINDOW", None, Some("Control+Shift+Tab")),
    ("Print", "F_PRINT", None, Some("Control+p")),
    ("PrintPageSetup", "F_PRINT_PAGESETUP", None, Some("Control+Alt+p")),
    ("PrintPreview", "F_PRINT_PREVIEW", None, Some("Control+Shift+p")),
    ("PropertyFile", "F_PROPERTY_FILE", None, Some("Alt+Enter")),
    ("RTrim", "F_RTRIM", None, Some("Alt+r")),
    ("RecKeyMacro", "F_RECKEYMACRO", None, Some("Control+Shift+m")),
    ("Reconvert", "F_RECONVERT", Some("reconvert"), None),
    ("Redo", "F_REDO", None, Some("Control+y")),
    ("Redraw", "F_REDRAW", None, Some("F5")),
    ("ReplaceDialog", "F_REPLACE_DIALOG", None, Some("Control+r")),
    ("Right", "F_RIGHT", None, Some("ArrowRight")),
    ("Right_BoxSel", "F_RIGHT_BOX", None, Some("Alt+ArrowRight")),
    ("Right_Sel", "F_RIGHT_SEL", None, Some("Shift+ArrowRight")),
    ("SPACEToTAB", "F_SPACETOTAB", None, Some("Control+Shift+Alt+F5")),
    ("SaveKeyMacro", "F_SAVEKEYMACRO", None, Some("Control+m")),
    ("SearchClearMark", "F_SEARCH_CLEARMARK", None, Some("Control+F3")),
    ("SearchDialog", "F_SEARCH_DIALOG", None, Some("Control+f")),
    ("SearchNext", "F_SEARCH_NEXT", None, Some("F3")),
    ("SearchPrev", "F_SEARCH_PREV", None, Some("Shift+F3")),
    ("SelectAll", "F_SELECTALL", None, Some("Control+a")),
    ("SelectCountMode", "F_SELECT_COUNT_MODE", Some("select-count-mode"), None),
    ("SelectWord", "F_SELECTWORD", None, Some("Control+w")),
    ("ShowFuncKey", "F_SHOWFUNCKEY", None, Some("Control+2")),
    ("ShowMiniMap", "F_SHOWMINIMAP", Some("toggle-minimap"), None),
    ("ShowStatusbar", "F_SHOWSTATUSBAR", None, Some("Control+3")),
    ("ShowTab", "F_SHOWTAB", Some("toggle-tabs"), None),
    ("ShowToolbar", "F_SHOWTOOLBAR", None, Some("Control+1")),
    ("SortAsc", "F_SORT_ASC", None, Some("Alt+a")),
    ("SortDesc", "F_SORT_DESC", None, Some("Alt+d")),
    ("SplitH", "F_SPLIT_H", None, Some("Shift+F4")),
    ("SplitV", "F_SPLIT_V", None, Some("F4")),
    ("SplitVH", "F_SPLIT_VH", None, Some("Control+Shift+;")),
    ("TABToSPACE", "F_TABTOSPACE", None, Some("Control+Alt+F5")),
    ("TabCloseLeft", "F_TAB_CLOSELEFT", Some("close-left"), None),
    ("TabCloseOther", "F_TAB_CLOSEOTHER", Some("close-others"), None),
    ("TabCloseRight", "F_TAB_CLOSERIGHT", Some("close-right"), None),
    ("TabJointNext", "F_TAB_JOINTNEXT", Some("tab-joint-next"), None),
    ("TabJointPrev", "F_TAB_JOINTPREV", Some("tab-joint-prev"), None),
    ("TabMoveLeft", "F_TAB_MOVELEFT", Some("tab-move-left"), None),
    ("TabMoveRight", "F_TAB_MOVERIGHT", Some("tab-move-right"), None),
    ("TabSeparate", "F_TAB_SEPARATE", Some("tab-separate"), None),
    ("TagJump", "F_TAGJUMP", None, Some("F12")),
    ("TagJumpBack", "F_TAGJUMPBACK", None, Some("Shift+F12")),
    ("TagJumpKeyword", "F_TAGJUMP_KEYWORD", Some("tag-jump-keyword"), None),
    ("TagsMake", "F_TAGS_MAKE", Some("tags-make"), None),
    ("TileH", "F_TILE_H", None, Some("Control+Alt+t")),
    ("TileV", "F_TILE_V", None, Some("Control+Alt+h")),
    ("TmpWrapNoWrap", "F_TMPWRAPNOWRAP", None, Some("Control+Alt+x")),
    ("TmpWrapSetting", "F_TMPWRAPSETTING", None, Some("Control+Alt+s")),
    ("TmpWrapWindow", "F_TMPWRAPWINDOW", None, Some("Control+Alt+w")),
    ("ToHanEi", "F_TOHANEI", Some("to-hanei"), None),
    ("ToHankaku", "F_TOHANKAKU", None, Some("Control+F8")),
    ("ToHankata", "F_TOHANKATA", Some("to-hankata"), None),
    ("ToLower", "F_TOLOWER", None, Some("Control+F6")),
    ("ToUpper", "F_TOUPPER", None, Some("Control+F7")),
    ("ToZenEi", "F_TOZENEI", Some("to-zenei"), None),
    ("ToZenHira", "F_TOZENKAKUHIRA", None, Some("Control+F10")),
    ("ToZenKata", "F_TOZENKAKUKATA", None, Some("Control+F9")),
    ("ToggleKeySearch", "F_TOGGLE_KEY_SEARCH", Some("toggle-key-search"), None),
    ("TopMost", "F_TOPMOST", Some("topmost"), None),
    ("TypeList", "F_TYPE_LIST", None, Some("Control+4")),
    ("UUDecode", "F_UUDECODE", None, Some("Alt+F5")),
    ("Undo", "F_UNDO", None, Some("Control+z")),
    ("UnindentSpace", "F_UNINDENT_SPACE", Some("unindent-space"), None),
    ("UnindentTab", "F_UNINDENT_TAB", None, Some("Shift+Tab")),
    ("Up", "F_UP", None, Some("ArrowUp")),
    ("Up2", "F_UP2", Some("cursor-up-2"), None),
    ("Up2_BoxSel", "F_UP2_BOX", None, Some("Control+Alt+ArrowUp")),
    ("Up2_Sel", "F_UP2_SEL", None, Some("Control+Shift+ArrowUp")),
    ("Up_BoxSel", "F_UP_BOX", None, Some("Alt+ArrowUp")),
    ("Up_Sel", "F_UP_SEL", None, Some("Shift+ArrowUp")),
    ("ViewMode", "F_VIEWMODE", Some("toggle-readonly"), None),
    ("WinClose", "F_WINCLOSE", Some("close-tab"), None),
    ("WinCloseAll", "F_WIN_CLOSEALL", Some("close-all"), None),
    ("WinList", "F_WINLIST", Some("window-list"), None),
    ("WinOutput", "F_WIN_OUTPUT", Some("show-output"), None),
    ("WndScrollDown", "F_WndScrollDown", None, Some("Control+ArrowUp")),
    ("WndScrollUp", "F_WndScrollUp", None, Some("Control+ArrowDown")),
    ("WordCut", "F_WordCut", None, Some("Control+d")),
    ("WordDelete", "F_WordDelete", None, Some("Control+Shift+d")),
    ("WordDeleteToEnd", "F_WordDeleteToEnd", None, Some("Control+Delete")),
    ("WordDeleteToStart", "F_WordDeleteToStart", None, Some("Control+Backspace")),
    ("WordLeft", "F_WORDLEFT", None, Some("Control+ArrowLeft")),
    ("WordLeft_BoxSel", "F_WORDLEFT_BOX", None, Some("Control+Alt+ArrowLeft")),
    ("WordLeft_Sel", "F_WORDLEFT_SEL", None, Some("Control+Shift+ArrowLeft")),
    ("WordRight", "F_WORDRIGHT", None, Some("Control+ArrowRight")),
    ("WordRight_BoxSel", "F_WORDRIGHT_BOX", None, Some("Control+Alt+ArrowRight")),
    ("WordRight_Sel", "F_WORDRIGHT_SEL", None, Some("Control+Shift+ArrowRight")),
    ("WrapWindowWidth", "F_WRAPWINDOWWIDTH", None, Some("Control+Alt+u")),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> Action {
        let mut p = plan(src).unwrap();
        assert_eq!(p.len(), 1, "{src}");
        p.remove(0).action
    }
    fn cmd_id(src: &str) -> String {
        match one(src) {
            Action::Command { id, .. } => id,
            a => panic!("not a command: {a:?}"),
        }
    }

    #[test]
    fn table_is_sorted_unique_and_complete() {
        assert!(SAKURA_COMMANDS.windows(2).all(|w| w[0].0 < w[1].0), "must be strictly sorted");
        assert_eq!(SAKURA_COMMANDS.len(), 268);
        assert!(SAKURA_COMMANDS.iter().all(|e| e.2.is_some() || e.3.is_some()), "every entry is dispatchable");
        assert!(SAKURA_COMMANDS.iter().all(|e| e.1.starts_with("F_")));
    }

    #[test]
    fn simple_commands() {
        assert_eq!(cmd_id("S_Copy();"), "F_COPY");
        assert_eq!(cmd_id("S_Left_Sel();"), "F_LEFT_SEL");
        assert_eq!(cmd_id("S_Left_BoxSel();"), "F_LEFT_BOX");
        assert_eq!(cmd_id("S_ToZenKata();"), "F_TOZENKAKUKATA");
        assert_eq!(cmd_id("S_1PageUp();"), "F_1PageUp");
        match one("S_IndentSpace();") {
            Action::Command { cmd, key, .. } => {
                assert_eq!(cmd.as_deref(), Some("indent-space"));
                assert_eq!(key, None);
            }
            _ => panic!(),
        }
        match one("S_FileSave(0);") {
            Action::Command { args, key, .. } => {
                assert_eq!(args, vec![MacroArg::Int(0)]);
                assert_eq!(key.as_deref(), Some("Control+s"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn primitives() {
        assert_eq!(one("S_Char(81);"), Action::Char { code: 81 });
        assert_eq!(one("S_Char(13);"), Action::Char { code: 13 });
        assert_eq!(one("S_InsText(\"x\");"), Action::InsText { text: "x".into() });
        assert_eq!(one("S_MoveCursor(3, 5, 0);"), Action::MoveCursor { row: 2, col: 4, select: false });
        assert_eq!(one("S_MoveCursor(1,1,1);"), Action::MoveCursor { row: 0, col: 0, select: true });
        assert_eq!(one("S_MoveCursorLayout(2, 2, 0);"), Action::MoveCursorLayout { row: 1, col: 1, select: false });
        assert_eq!(one("S_Jump(99, 1);"), Action::Jump { line: 99, flag: 1 });
        assert_eq!(one("S_Jump(3);"), Action::Jump { line: 3, flag: 0 });
        assert_eq!(one("S_SearchNext(\"foo\", 32);"), Action::SearchNext { text: "foo".into(), flags: 32 });
        assert_eq!(one("S_SearchPrev(\"foo\", 2);"), Action::SearchPrev { text: "foo".into(), flags: 2 });
        assert_eq!(
            one("S_ReplaceAll(\"(f)(o+)\", \"[$2$1]\", 4);"),
            Action::ReplaceAll { search: "(f)(o+)".into(), replace: "[$2$1]".into(), flags: 4 }
        );
        assert_eq!(cmd_id("S_SearchNext();"), "F_SEARCH_NEXT");
    }

    #[test]
    fn search_flags_bits() {
        let f = SearchFlags::from_bits(0x02 | 0x04 | 0x20);
        assert!(f.case_sensitive && f.regex && f.wrap && !f.word && !f.in_selection);
        assert!(SearchFlags::from_bits(128).in_selection);
    }

    #[test]
    fn string_escapes() {
        assert_eq!(one(r#"S_InsText("a\tb\r\nc\\d\"e\'f\0");"#), Action::InsText { text: "a\tb\r\nc\\d\"e'f\0".into() });
        assert_eq!(one(r#"S_InsText('it\'s "q"');"#), Action::InsText { text: "it's \"q\"".into() });
        // 未知转义保留反斜杠 (正则)
        assert_eq!(one(r#"S_SearchNext("fo+\w*", 4);"#), Action::SearchNext { text: "fo+\\w*".into(), flags: 4 });
        assert_eq!(one("S_InsText(\"日本語\");"), Action::InsText { text: "日本語".into() });
    }

    #[test]
    fn numbers() {
        assert_eq!(one("S_Jump(-5, 0x10);"), Action::Jump { line: -5, flag: 16 });
        assert!(plan("S_Char(-1);").is_err());
        assert!(plan("S_Char(1114112);").is_err());
        assert!(plan("S_Char(abc);").is_err());
    }

    #[test]
    fn comments_whitespace_and_semicolons() {
        let src = "\u{feff}// header\r\n/* block\n comment */ S_Copy ( ) ;\r\nS_Paste() // no semicolon\nS_Undo()\n  S_Redo(); /* tail */ S_Cut();";
        let p = plan(src).unwrap();
        let ids: Vec<_> = p.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(ids, ["S_Copy", "S_Paste", "S_Undo", "S_Redo", "S_Cut"]);
        assert_eq!(p[0].line, 3);
        assert_eq!(p[1].line, 4);
        assert_eq!(p[3].line, 6);
        assert!(plan("").unwrap().is_empty());
        assert!(plan("  // only a comment").unwrap().is_empty());
        assert_eq!(plan("S_Copy()").unwrap().len(), 1);
        // 多行参数
        assert_eq!(one("S_MoveCursor(\n 2,\n 3, // c\n 0\n);"), Action::MoveCursor { row: 1, col: 2, select: false });
        // 注释里的字符串/分号不干扰
        assert_eq!(plan("S_InsText(\"// not a comment; /* nor this */\");").unwrap().len(), 1);
    }

    #[test]
    fn syntax_errors_have_positions() {
        let e = plan("S_Copy();\nS_Paste(;").unwrap_err();
        assert_eq!(e.line, 2);
        assert_eq!(e.func.as_deref(), Some("S_Paste"));
        let e = plan("S_Copy() S_Paste();").unwrap_err();
        assert!(e.message.contains("';'"), "{e}");
        assert!(plan("S_InsText(\"abc);").is_err());
        assert!(plan("S_InsText(\"a\nb\");").is_err());
        assert!(plan("/* never closed").is_err());
        assert!(plan("S_Copy").is_err());
        assert!(plan("S_Char(1,);").is_err());
        assert!(plan("Copy();").is_err());
        assert!(plan("S_Copy(;").is_err());
        assert!(plan("@").is_err());
    }

    #[test]
    fn unknown_function_names_function_and_line() {
        let e = plan("S_Copy();\n\nS_NoSuchThing(1);").unwrap_err();
        assert_eq!(e.line, 3);
        assert_eq!(e.func.as_deref(), Some("S_NoSuchThing"));
        assert!(e.message.contains("S_NoSuchThing") && e.message.contains("line 3"), "{e}");
        // 区分大小写
        assert!(plan("S_copy();").is_err());
        assert!(plan("s_Copy();").is_err());
    }

    #[test]
    fn argument_validation() {
        assert!(plan("S_InsText();").is_err());
        assert!(plan("S_InsText(1);").is_err());
        assert!(plan("S_Char(\"a\");").is_err());
        assert!(plan("S_MoveCursor(1, 2);").is_err());
        assert!(plan("S_ReplaceAll(\"a\", 1, 0);").is_err());
        assert!(plan("S_SearchNext(\"a\");").is_err());
        assert!(plan("S_SearchNext(\"a\", -1);").is_err());
        let e = plan("S_Char(\"a\");").unwrap_err();
        assert_eq!(e.func.as_deref(), Some("S_Char"));
    }

    #[test]
    fn plan_serializes() {
        let p = plan("S_Char(81);\nS_Copy();").unwrap();
        let j = serde_json::to_value(&p).unwrap();
        assert_eq!(j[0]["action"]["type"], "char");
        assert_eq!(j[0]["action"]["code"], 81);
        assert_eq!(j[1]["action"]["type"], "command");
        assert_eq!(j[1]["action"]["id"], "F_COPY");
        assert_eq!(j[1]["line"], 2);
        let e = serde_json::to_value(plan("S_Nope();").unwrap_err()).unwrap();
        assert_eq!(e["func"], "S_Nope");
    }

    #[test]
    fn mixed_realistic_macro() {
        let src = "S_GoFileTop();\nS_MoveCursor(3, 1, 0);\nS_InsText(\"hello\");\nS_Char(13);\nS_Left_Sel();\nS_Copy();\nS_FileSave(0);\nS_ExitAll(0);\n";
        let p = plan(src).unwrap();
        assert_eq!(p.len(), 8);
        assert_eq!(p[7].line, 8);
    }

    /// 取出 sakura-diff 用例里所有字面 `S_xxx(...);` 语句 (模板插值 `${..}` 换成 1, JS 的 `\\` 还原).
    fn diff_snippets() -> Vec<String> {
        let files = [
            include_str!("../../../tests/sakura-diff/cases.mjs"),
            include_str!("../../../tests/sakura-diff/cases2.mjs"),
            include_str!("../../../tests/sakura-diff/cases3.mjs"),
            include_str!("../../../tests/sakura-diff/cases4.mjs"),
            include_str!("../../../tests/sakura-diff/cases5.mjs"),
            include_str!("../../../tests/sakura-diff/cases6.mjs"),
            include_str!("../../../tests/sakura-diff/cases7.mjs"),
            include_str!("../../../tests/sakura-diff/cases8.mjs"),
            include_str!("../../../tests/sakura-diff/cases9.mjs"),
            include_str!("../../../tests/sakura-diff/cases10.mjs"),
        ];
        let mut out = Vec::new();
        for f in files {
            let mut text = String::new();
            let mut rest = f;
            while let Some(a) = rest.find("${") {
                text.push_str(&rest[..a]);
                match rest[a..].find('}') {
                    Some(b) => {
                        text.push('1');
                        rest = &rest[a + b + 1..];
                    }
                    None => {
                        rest = "";
                    }
                }
            }
            text.push_str(rest);
            let mut s = text.as_str();
            while let Some(a) = s.find("S_") {
                let prev_ok = a == 0 || !s[..a].chars().last().unwrap().is_ascii_alphanumeric();
                let tail = &s[a..];
                if let (true, Some(end)) = (prev_ok, tail.find(");")) {
                    let stmt = tail[..end + 2].replace("\\\\", "\\");
                    if !stmt.contains("Xxx") && !stmt.contains('\n') {
                        out.push(stmt);
                    }
                }
                s = &s[a + 2..];
            }
        }
        out
    }

    #[test]
    fn every_sakura_diff_snippet_plans() {
        let sn = diff_snippets();
        assert!(sn.len() > 100, "found only {}", sn.len());
        for s in &sn {
            if let Err(e) = plan(s) {
                panic!("snippet {s:?} failed: {e}");
            }
        }
        // 整体拼成一个宏也可以
        assert_eq!(plan(&sn.join("\n")).unwrap().len(), sn.len());
    }

    #[test]
    fn sakura_diff_names_all_resolve() {
        let mut names: Vec<String> = diff_snippets().iter().map(|s| s.split('(').next().unwrap().to_string()).collect();
        names.sort();
        names.dedup();
        assert!(names.len() > 70);
        for n in names {
            let short = &n[2..];
            let primitive = ["Char", "InsText", "Jump", "MoveCursor", "ReplaceAll", "SearchNext", "SearchPrev"].contains(&short);
            assert!(primitive || lookup_command(short).is_some(), "{n}");
        }
    }
}
