//! ppa_engine.rs — 完整 PPA (Pascal-like) 宏脚本语言词法、语法解析器与执行引擎.
//!
//! 对标 Sakura 编辑器经典的 PPA (Poor-Pascal for Application) 宏引擎:
//! - 词法解析器 (Lexer): 关键字不区分大小写、十六进制 `$1A` / `0x1A`、浮点数、Pascal 赋值 `:=`、单引号字符串与 `''` 转义;
//! - 语法解析器 (Parser) & AST: `PpaProgram`, 全局/局部变量声明, `procedure` / `function`, `if/then/else`, `while`, `for ... to/downto`, `begin ... end`;
//! - 表达式求值: 算术 (+, -, *, div, mod), 逻辑 (and, or, not), 关系比较 (=, <>, <, <=, >, >=), 字符串拼接与类型自动升级;
//! - 执行引擎 (`PpaInterpreter`): 作用域环境栈, 递归调用, `Result` / 函数名赋值返回值, 内置标准库, 以及可注入的 `MacroContext` 编辑器分派接口.

use std::collections::HashMap;
use std::fmt;

// ============================================================================
// 1. 错误类型定义 (PpaError)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PpaError {
    LexError {
        line: usize,
        col: usize,
        message: String,
    },
    ParseError {
        line: usize,
        col: usize,
        message: String,
    },
    RuntimeError {
        message: String,
    },
    DivisionByZero,
    UndefinedVariable(String),
    UndefinedFunction(String),
    TypeMismatch(String),
    InvalidArgCount {
        expected: usize,
        got: usize,
    },
    Custom(String),
}

impl fmt::Display for PpaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PpaError::LexError { line, col, message } => {
                write!(f, "PPA Lex error at {}:{}: {}", line, col, message)
            }
            PpaError::ParseError { line, col, message } => {
                write!(f, "PPA Parse error at {}:{}: {}", line, col, message)
            }
            PpaError::RuntimeError { message } => {
                write!(f, "PPA Runtime error: {}", message)
            }
            PpaError::DivisionByZero => write!(f, "PPA Error: Division by zero"),
            PpaError::UndefinedVariable(name) => {
                write!(f, "PPA Error: Undefined variable '{}'", name)
            }
            PpaError::UndefinedFunction(name) => {
                write!(f, "PPA Error: Undefined function or procedure '{}'", name)
            }
            PpaError::TypeMismatch(msg) => write!(f, "PPA Type mismatch: {}", msg),
            PpaError::InvalidArgCount { expected, got } => {
                write!(
                    f,
                    "PPA Error: Invalid argument count, expected {}, got {}",
                    expected, got
                )
            }
            PpaError::Custom(msg) => write!(f, "PPA Error: {}", msg),
        }
    }
}

impl std::error::Error for PpaError {}

// ============================================================================
// 2. 动态值系统 (PpaValue)
// ============================================================================

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum PpaValue {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Void,
}

impl fmt::Display for PpaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PpaValue::Integer(i) => write!(f, "{}", i),
            PpaValue::Float(fl) => write!(f, "{}", fl),
            PpaValue::String(s) => write!(f, "{}", s),
            PpaValue::Boolean(b) => write!(f, "{}", if *b { "True" } else { "False" }),
            PpaValue::Void => write!(f, ""),
        }
    }
}

impl PpaValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            PpaValue::Boolean(b) => *b,
            PpaValue::Integer(i) => *i != 0,
            PpaValue::Float(f) => *f != 0.0,
            PpaValue::String(s) => !s.is_empty(),
            PpaValue::Void => false,
        }
    }

    pub fn as_int(&self) -> Result<i64, PpaError> {
        match self {
            PpaValue::Integer(i) => Ok(*i),
            PpaValue::Float(f) => Ok(*f as i64),
            PpaValue::Boolean(b) => Ok(if *b { 1 } else { 0 }),
            PpaValue::String(s) => s.trim().parse::<i64>().map_err(|_| {
                PpaError::TypeMismatch(format!("Cannot convert string '{}' to integer", s))
            }),
            PpaValue::Void => Ok(0),
        }
    }

    pub fn as_float(&self) -> Result<f64, PpaError> {
        match self {
            PpaValue::Float(f) => Ok(*f),
            PpaValue::Integer(i) => Ok(*i as f64),
            PpaValue::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
            PpaValue::String(s) => s.trim().parse::<f64>().map_err(|_| {
                PpaError::TypeMismatch(format!("Cannot convert string '{}' to float", s))
            }),
            PpaValue::Void => Ok(0.0),
        }
    }

    pub fn as_bool(&self) -> Result<bool, PpaError> {
        match self {
            PpaValue::Boolean(b) => Ok(*b),
            _ => Ok(self.is_truthy()),
        }
    }

    pub fn as_str(&self) -> Result<&str, PpaError> {
        match self {
            PpaValue::String(s) => Ok(s.as_str()),
            _ => Err(PpaError::TypeMismatch(format!(
                "Expected string, found {:?}",
                self
            ))),
        }
    }

    pub fn to_ppa_string(&self) -> String {
        self.to_string()
    }
}

impl From<i64> for PpaValue {
    fn from(v: i64) -> Self {
        PpaValue::Integer(v)
    }
}

impl From<i32> for PpaValue {
    fn from(v: i32) -> Self {
        PpaValue::Integer(v as i64)
    }
}

impl From<f64> for PpaValue {
    fn from(v: f64) -> Self {
        PpaValue::Float(v)
    }
}

impl From<String> for PpaValue {
    fn from(v: String) -> Self {
        PpaValue::String(v)
    }
}

impl From<&str> for PpaValue {
    fn from(v: &str) -> Self {
        PpaValue::String(v.to_string())
    }
}

impl From<bool> for PpaValue {
    fn from(v: bool) -> Self {
        PpaValue::Boolean(v)
    }
}

// ============================================================================
// 3. 词法解析器 (Lexer & Tokens)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // 关键字 (Keywords)
    Var,
    Begin,
    End,
    If,
    Then,
    Else,
    While,
    Do,
    For,
    To,
    Downto,
    Procedure,
    Function,
    And,
    Or,
    Not,
    Div,
    Mod,
    Program,
    Exit,
    Return,

    // 字面量 (Literals)
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),

    // 标识符 (Identifier)
    Ident(String),

    // 运算符与标点符号 (Operators & Punctuation)
    Assign,    // :=
    Eq,        // =
    Ne,        // <>
    Lt,        // <
    Le,        // <=
    Gt,        // >
    Ge,        // >=
    Plus,      // +
    Minus,     // -
    Star,      // *
    Slash,     // /
    LParen,    // (
    RParen,    // )
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Semicolon, // ;
    Colon,     // :
    Dot,       // .
    DotDot,    // ..

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer {
    chars: Vec<char>,
    cursor: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            cursor: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }

    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.chars.get(self.cursor + n).copied()
    }

    fn advance(&mut self) -> Option<char> {
        if let Some(ch) = self.chars.get(self.cursor).copied() {
            self.cursor += 1;
            if ch == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            Some(ch)
        } else {
            None
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else if ch == '/' && self.peek_ahead(1) == Some('/') {
                // 单行注释 //
                self.advance();
                self.advance();
                while let Some(c) = self.peek() {
                    self.advance();
                    if c == '\n' {
                        break;
                    }
                }
            } else if ch == '/' && self.peek_ahead(1) == Some('*') {
                // C 风格块注释 /* ... */
                self.advance();
                self.advance();
                while let Some(c) = self.peek() {
                    if c == '*' && self.peek_ahead(1) == Some('/') {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
            } else if ch == '{' {
                // Pascal 风格花括号注释 { ... }
                self.advance();
                while let Some(c) = self.peek() {
                    self.advance();
                    if c == '}' {
                        break;
                    }
                }
            } else if ch == '(' && self.peek_ahead(1) == Some('*') {
                // Pascal (* ... *) 注释
                self.advance();
                self.advance();
                while let Some(c) = self.peek() {
                    if c == '*' && self.peek_ahead(1) == Some(')') {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, PpaError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();

            let line = self.line;
            let col = self.col;

            let Some(ch) = self.peek() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    line,
                    col,
                });
                break;
            };

            // 单引号字符串 (Pascal 风格, '' 代表单引号转义)
            if ch == '\'' {
                self.advance();
                let mut buf = String::new();
                let mut closed = false;
                while let Some(c) = self.peek() {
                    if c == '\'' {
                        self.advance();
                        if self.peek() == Some('\'') {
                            // 转义单引号 ''
                            self.advance();
                            buf.push('\'');
                        } else {
                            closed = true;
                            break;
                        }
                    } else {
                        buf.push(c);
                        self.advance();
                    }
                }
                if !closed {
                    return Err(PpaError::LexError {
                        line,
                        col,
                        message: "Unterminated string literal".into(),
                    });
                }
                tokens.push(Token {
                    kind: TokenKind::String(buf),
                    line,
                    col,
                });
                continue;
            }

            // 双引号字符串兼容
            if ch == '"' {
                self.advance();
                let mut buf = String::new();
                let mut closed = false;
                while let Some(c) = self.peek() {
                    if c == '"' {
                        self.advance();
                        closed = true;
                        break;
                    } else if c == '\\' {
                        self.advance();
                        if let Some(escaped) = self.peek() {
                            self.advance();
                            match escaped {
                                'n' => buf.push('\n'),
                                'r' => buf.push('\r'),
                                't' => buf.push('\t'),
                                '\\' => buf.push('\\'),
                                '"' => buf.push('"'),
                                _ => {
                                    buf.push('\\');
                                    buf.push(escaped);
                                }
                            }
                        }
                    } else {
                        buf.push(c);
                        self.advance();
                    }
                }
                if !closed {
                    return Err(PpaError::LexError {
                        line,
                        col,
                        message: "Unterminated double-quoted string literal".into(),
                    });
                }
                tokens.push(Token {
                    kind: TokenKind::String(buf),
                    line,
                    col,
                });
                continue;
            }

            // 十六进制整数: $1A / $FF 或 0x1A
            if ch == '$' {
                self.advance();
                let mut hex_buf = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_hexdigit() {
                        hex_buf.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                if hex_buf.is_empty() {
                    return Err(PpaError::LexError {
                        line,
                        col,
                        message: "Expected hex digits after '$'".into(),
                    });
                }
                let val = i64::from_str_radix(&hex_buf, 16).map_err(|_| PpaError::LexError {
                    line,
                    col,
                    message: format!("Invalid hex literal '${}'", hex_buf),
                })?;
                tokens.push(Token {
                    kind: TokenKind::Integer(val),
                    line,
                    col,
                });
                continue;
            }

            if ch == '0'
                && (self.peek_ahead(1) == Some('x') || self.peek_ahead(1) == Some('X'))
            {
                self.advance();
                self.advance();
                let mut hex_buf = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_hexdigit() {
                        hex_buf.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let val = i64::from_str_radix(&hex_buf, 16).map_err(|_| PpaError::LexError {
                    line,
                    col,
                    message: format!("Invalid hex literal '0x{}'", hex_buf),
                })?;
                tokens.push(Token {
                    kind: TokenKind::Integer(val),
                    line,
                    col,
                });
                continue;
            }

            // 数字字面量 (十进制整数或浮点数)
            if ch.is_ascii_digit() {
                let mut num_buf = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        num_buf.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }

                // 检查浮点数小数点 (注意: 排除 1..10 这种情况)
                let is_float = if self.peek() == Some('.')
                    && self.peek_ahead(1) != Some('.')
                    && self.peek_ahead(1).map_or(false, |c| c.is_ascii_digit())
                {
                    num_buf.push(self.advance().unwrap());
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            num_buf.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    true
                } else {
                    false
                };

                if is_float {
                    let val = num_buf.parse::<f64>().map_err(|_| PpaError::LexError {
                        line,
                        col,
                        message: format!("Invalid float literal '{}'", num_buf),
                    })?;
                    tokens.push(Token {
                        kind: TokenKind::Float(val),
                        line,
                        col,
                    });
                } else {
                    let val = num_buf.parse::<i64>().map_err(|_| PpaError::LexError {
                        line,
                        col,
                        message: format!("Invalid integer literal '{}'", num_buf),
                    })?;
                    tokens.push(Token {
                        kind: TokenKind::Integer(val),
                        line,
                        col,
                    });
                }
                continue;
            }

            // 标识符与关键字 (不区分大小写)
            if ch.is_alphabetic() || ch == '_' {
                let mut ident = String::new();
                while let Some(c) = self.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }

                let kind = match ident.to_ascii_lowercase().as_str() {
                    "var" => TokenKind::Var,
                    "begin" => TokenKind::Begin,
                    "end" => TokenKind::End,
                    "if" => TokenKind::If,
                    "then" => TokenKind::Then,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "do" => TokenKind::Do,
                    "for" => TokenKind::For,
                    "to" => TokenKind::To,
                    "downto" => TokenKind::Downto,
                    "procedure" => TokenKind::Procedure,
                    "function" => TokenKind::Function,
                    "and" => TokenKind::And,
                    "or" => TokenKind::Or,
                    "not" => TokenKind::Not,
                    "div" => TokenKind::Div,
                    "mod" => TokenKind::Mod,
                    "program" => TokenKind::Program,
                    "exit" => TokenKind::Exit,
                    "return" => TokenKind::Return,
                    "true" => TokenKind::Boolean(true),
                    "false" => TokenKind::Boolean(false),
                    _ => TokenKind::Ident(ident),
                };

                tokens.push(Token { kind, line, col });
                continue;
            }

            // 运算符与双字符标点
            self.advance();
            let kind = match ch {
                ':' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::Assign
                    } else {
                        TokenKind::Colon
                    }
                }
                '<' => {
                    if self.peek() == Some('>') {
                        self.advance();
                        TokenKind::Ne
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::Le
                    } else {
                        TokenKind::Lt
                    }
                }
                '>' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::Ge
                    } else {
                        TokenKind::Gt
                    }
                }
                '.' => {
                    if self.peek() == Some('.') {
                        self.advance();
                        TokenKind::DotDot
                    } else {
                        TokenKind::Dot
                    }
                }
                '=' => TokenKind::Eq,
                '+' => TokenKind::Plus,
                '-' => TokenKind::Minus,
                '*' => TokenKind::Star,
                '/' => TokenKind::Slash,
                '(' => TokenKind::LParen,
                ')' => TokenKind::RParen,
                '[' => TokenKind::LBracket,
                ']' => TokenKind::RBracket,
                ',' => TokenKind::Comma,
                ';' => TokenKind::Semicolon,
                _ => {
                    return Err(PpaError::LexError {
                        line,
                        col,
                        message: format!("Unexpected character '{}'", ch),
                    });
                }
            };

            tokens.push(Token { kind, line, col });
        }

        Ok(tokens)
    }
}

// ============================================================================
// 4. 抽象语法树 (AST)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct VarDecl {
    pub names: Vec<String>,
    pub var_type: Option<String>,
    pub init: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParamDecl {
    pub name: String,
    pub param_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcedureDecl {
    pub name: String,
    pub params: Vec<ParamDecl>,
    pub local_vars: Vec<VarDecl>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<ParamDecl>,
    pub return_type: Option<String>,
    pub local_vars: Vec<VarDecl>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PpaProgram {
    pub name: Option<String>,
    pub global_vars: Vec<VarDecl>,
    pub procedures: Vec<ProcedureDecl>,
    pub functions: Vec<FunctionDecl>,
    pub main_block: Vec<Stmt>,
}

impl PpaProgram {
    pub fn parse(source: &str) -> Result<Self, PpaError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        parser.parse_program()
    }

    pub fn body(&self) -> &[Stmt] {
        &self.main_block
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Assignment {
        target: String,
        expr: Expr,
    },
    IfThenElse {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    WhileLoop {
        condition: Expr,
        body: Box<Stmt>,
    },
    ForLoop {
        var_name: String,
        start: Expr,
        downto: bool,
        end: Expr,
        body: Box<Stmt>,
    },
    ProcCall {
        name: String,
        args: Vec<Expr>,
    },
    Block(Vec<Stmt>),
    Exit(Option<Expr>),
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(PpaValue),
    Variable(String),
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    Pos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,  // 实数除法
    Idiv, // div 整数整除
    Mod,  // mod 取模
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

// ============================================================================
// 5. 语法解析器 (Parser)
// ============================================================================

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current.min(self.tokens.len() - 1)]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek_ahead_kind(&self, n: usize) -> Option<&TokenKind> {
        self.tokens.get(self.current + n).map(|t| &t.kind)
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        &self.tokens[self.current - 1]
    }

    fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() {
            false
        } else {
            *self.peek_kind() == kind
        }
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: TokenKind, msg: &str) -> Result<&Token, PpaError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            Err(PpaError::ParseError {
                line: tok.line,
                col: tok.col,
                message: format!("{}, found {:?}", msg, tok.kind),
            })
        }
    }

    fn consume_ident(&mut self, msg: &str) -> Result<String, PpaError> {
        let tok = self.peek().clone();
        if let TokenKind::Ident(name) = tok.kind {
            self.advance();
            Ok(name)
        } else {
            Err(PpaError::ParseError {
                line: tok.line,
                col: tok.col,
                message: format!("{}, found {:?}", msg, tok.kind),
            })
        }
    }

    pub fn parse_program(&mut self) -> Result<PpaProgram, PpaError> {
        let mut prog_name = None;
        let mut global_vars = Vec::new();
        let mut procedures = Vec::new();
        let mut functions = Vec::new();
        let mut main_block = Vec::new();

        if self.match_token(TokenKind::Program) {
            prog_name = Some(self.consume_ident("Expected program name")?);
            self.match_token(TokenKind::Semicolon);
        }

        while !self.is_at_end() {
            match self.peek_kind() {
                TokenKind::Var => {
                    self.advance();
                    let vars = self.parse_var_section()?;
                    global_vars.extend(vars);
                }
                TokenKind::Procedure => {
                    let proc = self.parse_procedure()?;
                    procedures.push(proc);
                }
                TokenKind::Function => {
                    let func = self.parse_function()?;
                    functions.push(func);
                }
                TokenKind::Begin => {
                    self.advance();
                    let stmts = self.parse_block_stmts()?;
                    main_block.extend(stmts);
                    // 允许主块结束以 '.' 或 ';' 结尾
                    if self.match_token(TokenKind::Dot) {
                        break;
                    }
                    self.match_token(TokenKind::Semicolon);
                }
                _ => {
                    // 顶层散装语句 (支持不带 begin/end 包装的简单宏脚本)
                    if self.match_token(TokenKind::Semicolon) {
                        continue;
                    }
                    let stmt = self.parse_statement()?;
                    main_block.push(stmt);
                    self.match_token(TokenKind::Semicolon);
                }
            }
        }

        Ok(PpaProgram {
            name: prog_name,
            global_vars,
            procedures,
            functions,
            main_block,
        })
    }

    fn parse_var_section(&mut self) -> Result<Vec<VarDecl>, PpaError> {
        let mut decls = Vec::new();

        while let TokenKind::Ident(_) = self.peek_kind() {
            let mut names = Vec::new();
            names.push(self.consume_ident("Expected variable name")?);

            while self.match_token(TokenKind::Comma) {
                names.push(self.consume_ident("Expected variable name after comma")?);
            }

            let var_type = if self.match_token(TokenKind::Colon) {
                Some(self.consume_ident("Expected type identifier")?)
            } else {
                None
            };

            let init = if self.match_token(TokenKind::Assign) || self.match_token(TokenKind::Eq) {
                Some(self.parse_expr()?)
            } else {
                None
            };

            self.match_token(TokenKind::Semicolon);

            decls.push(VarDecl {
                names,
                var_type,
                init,
            });

            // 遇到过程、函数、begin 或下一个 var 则结束当前 var 段
            if matches!(
                self.peek_kind(),
                TokenKind::Begin
                    | TokenKind::Procedure
                    | TokenKind::Function
                    | TokenKind::Var
                    | TokenKind::Eof
            ) {
                break;
            }
        }

        Ok(decls)
    }

    fn parse_procedure(&mut self) -> Result<ProcedureDecl, PpaError> {
        self.consume(TokenKind::Procedure, "Expected 'procedure'")?;
        let name = self.consume_ident("Expected procedure name")?;

        let params = if self.match_token(TokenKind::LParen) {
            self.parse_params()?
        } else {
            Vec::new()
        };

        self.match_token(TokenKind::Semicolon);

        let local_vars = if self.match_token(TokenKind::Var) {
            self.parse_var_section()?
        } else {
            Vec::new()
        };

        self.consume(TokenKind::Begin, "Expected 'begin' for procedure body")?;
        let body = self.parse_block_stmts()?;
        self.match_token(TokenKind::Semicolon);

        Ok(ProcedureDecl {
            name,
            params,
            local_vars,
            body,
        })
    }

    fn parse_function(&mut self) -> Result<FunctionDecl, PpaError> {
        self.consume(TokenKind::Function, "Expected 'function'")?;
        let name = self.consume_ident("Expected function name")?;

        let params = if self.match_token(TokenKind::LParen) {
            self.parse_params()?
        } else {
            Vec::new()
        };

        let return_type = if self.match_token(TokenKind::Colon) {
            Some(self.consume_ident("Expected function return type")?)
        } else {
            None
        };

        self.match_token(TokenKind::Semicolon);

        let local_vars = if self.match_token(TokenKind::Var) {
            self.parse_var_section()?
        } else {
            Vec::new()
        };

        self.consume(TokenKind::Begin, "Expected 'begin' for function body")?;
        let body = self.parse_block_stmts()?;
        self.match_token(TokenKind::Semicolon);

        Ok(FunctionDecl {
            name,
            params,
            return_type,
            local_vars,
            body,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<ParamDecl>, PpaError> {
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                let mut group_names = Vec::new();
                group_names.push(self.consume_ident("Expected parameter name")?);
                while self.match_token(TokenKind::Comma) {
                    group_names.push(self.consume_ident("Expected parameter name")?);
                }

                let param_type = if self.match_token(TokenKind::Colon) {
                    Some(self.consume_ident("Expected parameter type")?)
                } else {
                    None
                };

                for n in group_names {
                    params.push(ParamDecl {
                        name: n,
                        param_type: param_type.clone(),
                    });
                }

                if !self.match_token(TokenKind::Semicolon) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, "Expected ')' after parameter list")?;
        Ok(params)
    }

    fn parse_block_stmts(&mut self) -> Result<Vec<Stmt>, PpaError> {
        let mut stmts = Vec::new();
        while !self.check(TokenKind::End) && !self.is_at_end() {
            if self.match_token(TokenKind::Semicolon) {
                continue;
            }
            stmts.push(self.parse_statement()?);
            self.match_token(TokenKind::Semicolon);
        }
        self.consume(TokenKind::End, "Expected 'end'")?;
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Stmt, PpaError> {
        if self.match_token(TokenKind::Begin) {
            let stmts = self.parse_block_stmts()?;
            return Ok(Stmt::Block(stmts));
        }

        if self.match_token(TokenKind::If) {
            return self.parse_if();
        }

        if self.match_token(TokenKind::While) {
            return self.parse_while();
        }

        if self.match_token(TokenKind::For) {
            return self.parse_for();
        }

        if self.match_token(TokenKind::Exit) || self.match_token(TokenKind::Return) {
            let expr = if self.match_token(TokenKind::LParen) {
                let e = self.parse_expr()?;
                self.consume(TokenKind::RParen, "Expected ')' after return expression")?;
                Some(e)
            } else if !self.check(TokenKind::Semicolon)
                && !self.check(TokenKind::End)
                && !self.is_at_end()
            {
                Some(self.parse_expr()?)
            } else {
                None
            };
            return Ok(Stmt::Exit(expr));
        }

        if self.match_token(TokenKind::Semicolon) {
            return Ok(Stmt::Empty);
        }

        // 赋值或过程调用
        if let TokenKind::Ident(name) = self.peek_kind().clone() {
            self.advance();
            if self.match_token(TokenKind::Assign) {
                let expr = self.parse_expr()?;
                return Ok(Stmt::Assignment { target: name, expr });
            }

            if self.match_token(TokenKind::LParen) {
                let args = self.parse_arguments()?;
                return Ok(Stmt::ProcCall { name, args });
            }

            // 无括号过程调用 (例如 MyProc; 或 S_InsertText;)
            return Ok(Stmt::ProcCall {
                name,
                args: Vec::new(),
            });
        }

        let tok = self.peek();
        Err(PpaError::ParseError {
            line: tok.line,
            col: tok.col,
            message: format!("Unexpected token in statement: {:?}", tok.kind),
        })
    }

    fn parse_if(&mut self) -> Result<Stmt, PpaError> {
        let condition = self.parse_expr()?;
        self.consume(TokenKind::Then, "Expected 'then' after if condition")?;
        let then_branch = Box::new(self.parse_statement()?);

        // 宽容处理: 允许 then 语句后带分号的 else (Pascal 原生要求无分号)
        let mut else_branch = None;
        if self.match_token(TokenKind::Else) {
            else_branch = Some(Box::new(self.parse_statement()?));
        } else if self.check(TokenKind::Semicolon)
            && self.peek_ahead_kind(1) == Some(&TokenKind::Else)
        {
            self.advance(); // 跳过分号
            self.advance(); // 跳过 else
            else_branch = Some(Box::new(self.parse_statement()?));
        }

        Ok(Stmt::IfThenElse {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt, PpaError> {
        let condition = self.parse_expr()?;
        self.consume(TokenKind::Do, "Expected 'do' after while condition")?;
        let body = Box::new(self.parse_statement()?);
        Ok(Stmt::WhileLoop { condition, body })
    }

    fn parse_for(&mut self) -> Result<Stmt, PpaError> {
        let var_name = self.consume_ident("Expected loop variable name")?;
        self.consume(TokenKind::Assign, "Expected ':=' in for loop")?;
        let start = self.parse_expr()?;

        let downto = if self.match_token(TokenKind::To) {
            false
        } else if self.match_token(TokenKind::Downto) {
            true
        } else {
            let tok = self.peek();
            return Err(PpaError::ParseError {
                line: tok.line,
                col: tok.col,
                message: "Expected 'to' or 'downto' in for loop".into(),
            });
        };

        let end = self.parse_expr()?;
        self.consume(TokenKind::Do, "Expected 'do' after for loop limits")?;
        let body = Box::new(self.parse_statement()?);

        Ok(Stmt::ForLoop {
            var_name,
            start,
            downto,
            end,
            body,
        })
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr>, PpaError> {
        let mut args = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, "Expected ')' after arguments")?;
        Ok(args)
    }

    // --- 表达式解析 (优先级爬升) ---

    pub fn parse_expr(&mut self) -> Result<Expr, PpaError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, PpaError> {
        let mut left = self.parse_and()?;
        while self.match_token(TokenKind::Or) {
            let right = self.parse_and()?;
            left = Expr::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, PpaError> {
        let mut left = self.parse_comparison()?;
        while self.match_token(TokenKind::And) {
            let right = self.parse_comparison()?;
            left = Expr::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, PpaError> {
        let mut left = self.parse_additive()?;
        loop {
            let op = if self.match_token(TokenKind::Eq) {
                BinaryOp::Eq
            } else if self.match_token(TokenKind::Ne) {
                BinaryOp::Ne
            } else if self.match_token(TokenKind::Lt) {
                BinaryOp::Lt
            } else if self.match_token(TokenKind::Le) {
                BinaryOp::Le
            } else if self.match_token(TokenKind::Gt) {
                BinaryOp::Gt
            } else if self.match_token(TokenKind::Ge) {
                BinaryOp::Ge
            } else {
                break;
            };
            let right = self.parse_additive()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, PpaError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = if self.match_token(TokenKind::Plus) {
                BinaryOp::Add
            } else if self.match_token(TokenKind::Minus) {
                BinaryOp::Sub
            } else {
                break;
            };
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, PpaError> {
        let mut left = self.parse_unary()?;
        loop {
            let op = if self.match_token(TokenKind::Star) {
                BinaryOp::Mul
            } else if self.match_token(TokenKind::Slash) {
                BinaryOp::Div
            } else if self.match_token(TokenKind::Div) {
                BinaryOp::Idiv
            } else if self.match_token(TokenKind::Mod) {
                BinaryOp::Mod
            } else {
                break;
            };
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, PpaError> {
        if self.match_token(TokenKind::Not) {
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.match_token(TokenKind::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
            });
        }
        if self.match_token(TokenKind::Plus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Pos,
                expr: Box::new(expr),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, PpaError> {
        if let TokenKind::Integer(i) = self.peek_kind() {
            let val = *i;
            self.advance();
            return Ok(Expr::Literal(PpaValue::Integer(val)));
        }
        if let TokenKind::Float(f) = self.peek_kind() {
            let val = *f;
            self.advance();
            return Ok(Expr::Literal(PpaValue::Float(val)));
        }
        if let TokenKind::String(s) = self.peek_kind() {
            let val = s.clone();
            self.advance();
            return Ok(Expr::Literal(PpaValue::String(val)));
        }
        if let TokenKind::Boolean(b) = self.peek_kind() {
            let val = *b;
            self.advance();
            return Ok(Expr::Literal(PpaValue::Boolean(val)));
        }
        if self.match_token(TokenKind::LParen) {
            let inner = self.parse_expr()?;
            self.consume(TokenKind::RParen, "Expected ')' after parenthesized expr")?;
            return Ok(inner);
        }
        if let TokenKind::Ident(name) = self.peek_kind().clone() {
            self.advance();
            if self.match_token(TokenKind::LParen) {
                let args = self.parse_arguments()?;
                return Ok(Expr::Call { name, args });
            }
            return Ok(Expr::Variable(name));
        }

        let tok = self.peek();
        Err(PpaError::ParseError {
            line: tok.line,
            col: tok.col,
            message: format!("Unexpected token in expression: {:?}", tok.kind),
        })
    }
}

// ============================================================================
// 6. 环境作用域与编辑器分派接口 (Environment & MacroContext)
// ============================================================================

#[derive(Debug, Clone)]
pub struct Environment {
    scopes: Vec<HashMap<String, PpaValue>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()], // 全局作用域
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn get(&self, name: &str) -> Option<PpaValue> {
        let lower = name.to_ascii_lowercase();
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(&lower) {
                return Some(val.clone());
            }
        }
        None
    }

    pub fn set(&mut self, name: &str, value: PpaValue) {
        let lower = name.to_ascii_lowercase();
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(&lower) {
                scope.insert(lower, value);
                return;
            }
        }
        // 如果各层都不存在，写入当前最顶层作用域
        if let Some(top) = self.scopes.last_mut() {
            top.insert(lower, value);
        }
    }

    pub fn define(&mut self, name: &str, value: PpaValue) {
        let lower = name.to_ascii_lowercase();
        if let Some(top) = self.scopes.last_mut() {
            top.insert(lower, value);
        }
    }

    pub fn globals(&self) -> &HashMap<String, PpaValue> {
        self.scopes.first().unwrap()
    }
}

/// 宏回调上下文接口 (支持用户扩展 Sakura S_* 函数与宿主交互)
pub trait MacroContext {
    fn call(&mut self, name: &str, args: &[PpaValue]) -> Result<PpaValue, PpaError>;
}

impl<F> MacroContext for F
where
    F: FnMut(&str, &[PpaValue]) -> Result<PpaValue, PpaError>,
{
    fn call(&mut self, name: &str, args: &[PpaValue]) -> Result<PpaValue, PpaError> {
        (self)(name, args)
    }
}

/// 内存模拟编辑器环境 (支持 S_InsertText, S_GetLineStr, S_MoveCursor 等)
#[derive(Debug, Clone)]
pub struct DefaultMacroContext {
    pub lines: Vec<String>,
    pub cursor_line: usize, // 1-based 行号
    pub cursor_col: usize,  // 1-based 列号
    pub selected_text: String,
    pub call_log: Vec<(String, Vec<PpaValue>)>,
}

impl Default for DefaultMacroContext {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl DefaultMacroContext {
    pub fn new(lines: Vec<String>) -> Self {
        let non_empty = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Self {
            lines: non_empty,
            cursor_line: 1,
            cursor_col: 1,
            selected_text: String::new(),
            call_log: Vec::new(),
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn set_text(&mut self, text: &str) {
        let split: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        self.lines = if split.is_empty() {
            vec![String::new()]
        } else {
            split
        };
    }

    /// 在当前光标处插入文本 (支持多行换行分割)
    pub fn insert_text(&mut self, text: &str) {
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        let row = if self.cursor_line == 0 {
            0
        } else {
            self.cursor_line - 1
        };
        while self.lines.len() <= row {
            self.lines.push(String::new());
        }
        let col = if self.cursor_col == 0 {
            0
        } else {
            self.cursor_col - 1
        };

        let cur_line = &self.lines[row];
        let char_indices: Vec<(usize, char)> = cur_line.char_indices().collect();
        let byte_offset = if col >= char_indices.len() {
            cur_line.len()
        } else {
            char_indices[col].0
        };

        let prefix = cur_line[..byte_offset].to_string();
        let suffix = cur_line[byte_offset..].to_string();

        let normalized = text.replace("\r\n", "\n");
        let parts: Vec<&str> = normalized.split('\n').collect();

        if parts.len() == 1 {
            let new_line = format!("{}{}{}", prefix, parts[0], suffix);
            let inserted_len = parts[0].chars().count();
            self.lines[row] = new_line;
            self.cursor_col += inserted_len;
        } else {
            let mut new_lines = Vec::new();
            new_lines.push(format!("{}{}", prefix, parts[0]));
            for p in &parts[1..parts.len() - 1] {
                new_lines.push(p.to_string());
            }
            let last_chars = parts[parts.len() - 1].chars().count();
            new_lines.push(format!("{}{}", parts[parts.len() - 1], suffix));

            self.lines.splice(row..=row, new_lines);
            self.cursor_line = row + parts.len();
            self.cursor_col = 1 + last_chars;
        }
    }
}

impl MacroContext for DefaultMacroContext {
    fn call(&mut self, name: &str, args: &[PpaValue]) -> Result<PpaValue, PpaError> {
        self.call_log.push((name.to_string(), args.to_vec()));
        match name.to_ascii_lowercase().as_str() {
            "s_inserttext" | "instext" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                let t = args[0].to_ppa_string();
                self.insert_text(&t);
                Ok(PpaValue::Void)
            }
            "s_getlinestr" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                let line_num = args[0].as_int()?;
                if line_num >= 1 && (line_num as usize) <= self.lines.len() {
                    Ok(PpaValue::String(self.lines[(line_num - 1) as usize].clone()))
                } else {
                    Ok(PpaValue::String(String::new()))
                }
            }
            "s_getselectedstring" => Ok(PpaValue::String(self.selected_text.clone())),
            "s_movecursor" => {
                if args.len() < 2 {
                    return Err(PpaError::InvalidArgCount {
                        expected: 2,
                        got: args.len(),
                    });
                }
                let line = args[0].as_int()?.max(1) as usize;
                let col = args[1].as_int()?.max(1) as usize;
                self.cursor_line = line;
                self.cursor_col = col;
                Ok(PpaValue::Void)
            }
            "s_getlinecount" => Ok(PpaValue::Integer(self.lines.len() as i64)),
            _ if name.to_ascii_lowercase().starts_with("s_") => Ok(PpaValue::Void),
            _ => Err(PpaError::UndefinedFunction(name.to_string())),
        }
    }
}

// ============================================================================
// 7. 执行信号与解释器核心 (PpaInterpreter)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum ExecSignal {
    Normal,
    Exit(Option<PpaValue>),
}

pub struct PpaInterpreter {
    env: Environment,
    context: Box<dyn MacroContext>,
    current_program: Option<PpaProgram>,
}

impl Default for PpaInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl PpaInterpreter {
    pub fn new() -> Self {
        Self::with_context(Box::new(DefaultMacroContext::default()))
    }

    pub fn with_context(context: Box<dyn MacroContext>) -> Self {
        Self {
            env: Environment::new(),
            context,
            current_program: None,
        }
    }

    pub fn with_editor(editor: DefaultMacroContext) -> Self {
        Self::with_context(Box::new(editor))
    }

    pub fn set_context(&mut self, context: Box<dyn MacroContext>) {
        self.context = context;
    }

    pub fn set_callback<F>(&mut self, f: F)
    where
        F: FnMut(&str, &[PpaValue]) -> Result<PpaValue, PpaError> + 'static,
    {
        self.context = Box::new(f);
    }

    pub fn env(&self) -> &Environment {
        &self.env
    }

    pub fn env_mut(&mut self) -> &mut Environment {
        &mut self.env
    }

    pub fn get_var(&self, name: &str) -> Option<PpaValue> {
        self.env.get(name)
    }

    pub fn set_var(&mut self, name: &str, value: PpaValue) {
        self.env.set(name, value);
    }

    pub fn eval(&mut self, source: &str) -> Result<PpaValue, PpaError> {
        let program = PpaProgram::parse(source)?;
        self.execute(&program)
    }

    pub fn eval_with<C: MacroContext>(
        &mut self,
        source: &str,
        ctx: &mut C,
    ) -> Result<PpaValue, PpaError> {
        let program = PpaProgram::parse(source)?;
        self.execute_with(&program, ctx)
    }

    pub fn execute(&mut self, program: &PpaProgram) -> Result<PpaValue, PpaError> {
        let mut ctx = std::mem::replace(
            &mut self.context,
            Box::new(DefaultMacroContext::default()),
        );
        let res = self.execute_dyn(program, ctx.as_mut());
        self.context = ctx;
        res
    }

    pub fn execute_with<C: MacroContext>(
        &mut self,
        program: &PpaProgram,
        ctx: &mut C,
    ) -> Result<PpaValue, PpaError> {
        self.execute_dyn(program, ctx)
    }

    fn execute_dyn(
        &mut self,
        program: &PpaProgram,
        ctx: &mut dyn MacroContext,
    ) -> Result<PpaValue, PpaError> {
        self.current_program = Some(program.clone());

        // 初始化全局变量声明
        for decl in &program.global_vars {
            let init_val = if let Some(ref init_expr) = decl.init {
                self.eval_expr(init_expr, ctx)?
            } else {
                PpaValue::Void
            };
            for name in &decl.names {
                self.env.define(name, init_val.clone());
            }
        }

        // 执行主代码块
        let mut last_val = PpaValue::Void;
        for stmt in &program.main_block {
            let signal = self.execute_stmt(stmt, ctx)?;
            if let ExecSignal::Exit(opt_val) = signal {
                return Ok(opt_val.unwrap_or(PpaValue::Void));
            }
        }

        // 如果主代码块最后一条语句为表达式求值赋值，或全局有结果则返回
        if let Some(res) = self.env.get("result") {
            last_val = res;
        }

        Ok(last_val)
    }

    fn execute_stmt(
        &mut self,
        stmt: &Stmt,
        ctx: &mut dyn MacroContext,
    ) -> Result<ExecSignal, PpaError> {
        match stmt {
            Stmt::Empty => Ok(ExecSignal::Normal),

            Stmt::Assignment { target, expr } => {
                let val = self.eval_expr(expr, ctx)?;
                self.env.set(target, val);
                Ok(ExecSignal::Normal)
            }

            Stmt::IfThenElse {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.eval_expr(condition, ctx)?;
                if cond_val.is_truthy() {
                    self.execute_stmt(then_branch, ctx)
                } else if let Some(else_branch) = else_branch {
                    self.execute_stmt(else_branch, ctx)
                } else {
                    Ok(ExecSignal::Normal)
                }
            }

            Stmt::WhileLoop { condition, body } => {
                while self.eval_expr(condition, ctx)?.is_truthy() {
                    let signal = self.execute_stmt(body, ctx)?;
                    if let ExecSignal::Exit(_) = signal {
                        return Ok(signal);
                    }
                }
                Ok(ExecSignal::Normal)
            }

            Stmt::ForLoop {
                var_name,
                start,
                downto,
                end,
                body,
            } => {
                let start_val = self.eval_expr(start, ctx)?.as_int()?;
                let end_val = self.eval_expr(end, ctx)?.as_int()?;

                let mut current = start_val;
                if !*downto {
                    while current <= end_val {
                        self.env.set(var_name, PpaValue::Integer(current));
                        let signal = self.execute_stmt(body, ctx)?;
                        if let ExecSignal::Exit(_) = signal {
                            return Ok(signal);
                        }
                        current += 1;
                    }
                } else {
                    while current >= end_val {
                        self.env.set(var_name, PpaValue::Integer(current));
                        let signal = self.execute_stmt(body, ctx)?;
                        if let ExecSignal::Exit(_) = signal {
                            return Ok(signal);
                        }
                        current -= 1;
                    }
                }
                // 循环结束设置终值
                self.env.set(var_name, PpaValue::Integer(current));
                Ok(ExecSignal::Normal)
            }

            Stmt::ProcCall { name, args } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(self.eval_expr(a, ctx)?);
                }
                self.call_proc_or_func(name, &evaluated_args, ctx)?;
                Ok(ExecSignal::Normal)
            }

            Stmt::Block(stmts) => {
                for s in stmts {
                    let signal = self.execute_stmt(s, ctx)?;
                    if let ExecSignal::Exit(_) = signal {
                        return Ok(signal);
                    }
                }
                Ok(ExecSignal::Normal)
            }

            Stmt::Exit(opt_expr) => {
                let val = if let Some(e) = opt_expr {
                    Some(self.eval_expr(e, ctx)?)
                } else {
                    None
                };
                Ok(ExecSignal::Exit(val))
            }
        }
    }

    fn eval_expr(
        &mut self,
        expr: &Expr,
        ctx: &mut dyn MacroContext,
    ) -> Result<PpaValue, PpaError> {
        match expr {
            Expr::Literal(val) => Ok(val.clone()),

            Expr::Variable(name) => {
                // 优先查找作用域变量
                if let Some(val) = self.env.get(name) {
                    return Ok(val);
                }
                // 支持 Pascal 无参函数调用语法 (无需带括号)
                if self.is_known_function(name) {
                    return self.call_proc_or_func(name, &[], ctx);
                }
                // 仅当是以 S_ 开头或内置标准库函数时尝试无参派发
                let lower = name.to_ascii_lowercase();
                if lower.starts_with("s_")
                    || matches!(
                        lower.as_str(),
                        "inttostr"
                            | "strtoint"
                            | "strtointdef"
                            | "length"
                            | "copy"
                            | "pos"
                            | "lowercase"
                            | "uppercase"
                            | "trim"
                            | "chr"
                            | "ord"
                    )
                {
                    if let Ok(val) = self.call_builtin_or_context(name, &[], ctx) {
                        return Ok(val);
                    }
                }
                Err(PpaError::UndefinedVariable(name.clone()))
            }

            Expr::UnaryOp { op, expr } => {
                let val = self.eval_expr(expr, ctx)?;
                match op {
                    UnaryOp::Not => {
                        if let PpaValue::Boolean(b) = val {
                            Ok(PpaValue::Boolean(!b))
                        } else if let PpaValue::Integer(i) = val {
                            Ok(PpaValue::Integer(!i))
                        } else {
                            Ok(PpaValue::Boolean(!val.is_truthy()))
                        }
                    }
                    UnaryOp::Neg => match val {
                        PpaValue::Integer(i) => Ok(PpaValue::Integer(-i)),
                        PpaValue::Float(f) => Ok(PpaValue::Float(-f)),
                        _ => Err(PpaError::TypeMismatch(
                            "Unary minus requires numeric operand".into(),
                        )),
                    },
                    UnaryOp::Pos => match val {
                        PpaValue::Integer(i) => Ok(PpaValue::Integer(i)),
                        PpaValue::Float(f) => Ok(PpaValue::Float(f)),
                        _ => Err(PpaError::TypeMismatch(
                            "Unary plus requires numeric operand".into(),
                        )),
                    },
                }
            }

            Expr::BinaryOp { op, left, right } => {
                let l = self.eval_expr(left, ctx)?;
                let r = self.eval_expr(right, ctx)?;
                self.eval_binary_op(*op, l, r)
            }

            Expr::Call { name, args } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(self.eval_expr(a, ctx)?);
                }
                self.call_proc_or_func(name, &evaluated_args, ctx)
            }
        }
    }

    fn eval_binary_op(
        &self,
        op: BinaryOp,
        left: PpaValue,
        right: PpaValue,
    ) -> Result<PpaValue, PpaError> {
        match op {
            BinaryOp::Add => {
                // 字符串拼接自动升级
                match (&left, &right) {
                    (PpaValue::String(s1), _) => {
                        Ok(PpaValue::String(format!("{}{}", s1, right.to_ppa_string())))
                    }
                    (_, PpaValue::String(s2)) => {
                        Ok(PpaValue::String(format!("{}{}", left.to_ppa_string(), s2)))
                    }
                    (PpaValue::Integer(i1), PpaValue::Integer(i2)) => {
                        Ok(PpaValue::Integer(i1 + i2))
                    }
                    (PpaValue::Float(f1), PpaValue::Float(f2)) => Ok(PpaValue::Float(f1 + f2)),
                    (PpaValue::Integer(i1), PpaValue::Float(f2)) => {
                        Ok(PpaValue::Float(*i1 as f64 + f2))
                    }
                    (PpaValue::Float(f1), PpaValue::Integer(i2)) => {
                        Ok(PpaValue::Float(f1 + *i2 as f64))
                    }
                    _ => Err(PpaError::TypeMismatch(format!(
                        "Cannot add {:?} and {:?}",
                        left, right
                    ))),
                }
            }

            BinaryOp::Sub => match (&left, &right) {
                (PpaValue::Integer(i1), PpaValue::Integer(i2)) => Ok(PpaValue::Integer(i1 - i2)),
                (PpaValue::Float(f1), PpaValue::Float(f2)) => Ok(PpaValue::Float(f1 - f2)),
                (PpaValue::Integer(i1), PpaValue::Float(f2)) => {
                    Ok(PpaValue::Float(*i1 as f64 - f2))
                }
                (PpaValue::Float(f1), PpaValue::Integer(i2)) => {
                    Ok(PpaValue::Float(f1 - *i2 as f64))
                }
                _ => Err(PpaError::TypeMismatch(format!(
                    "Cannot subtract {:?} and {:?}",
                    left, right
                ))),
            },

            BinaryOp::Mul => match (&left, &right) {
                (PpaValue::Integer(i1), PpaValue::Integer(i2)) => Ok(PpaValue::Integer(i1 * i2)),
                (PpaValue::Float(f1), PpaValue::Float(f2)) => Ok(PpaValue::Float(f1 * f2)),
                (PpaValue::Integer(i1), PpaValue::Float(f2)) => {
                    Ok(PpaValue::Float(*i1 as f64 * f2))
                }
                (PpaValue::Float(f1), PpaValue::Integer(i2)) => {
                    Ok(PpaValue::Float(f1 * *i2 as f64))
                }
                _ => Err(PpaError::TypeMismatch(format!(
                    "Cannot multiply {:?} and {:?}",
                    left, right
                ))),
            },

            BinaryOp::Div => {
                let f1 = left.as_float()?;
                let f2 = right.as_float()?;
                if f2 == 0.0 {
                    return Err(PpaError::DivisionByZero);
                }
                Ok(PpaValue::Float(f1 / f2))
            }

            BinaryOp::Idiv => {
                let i1 = left.as_int()?;
                let i2 = right.as_int()?;
                if i2 == 0 {
                    return Err(PpaError::DivisionByZero);
                }
                Ok(PpaValue::Integer(i1 / i2))
            }

            BinaryOp::Mod => {
                let i1 = left.as_int()?;
                let i2 = right.as_int()?;
                if i2 == 0 {
                    return Err(PpaError::DivisionByZero);
                }
                Ok(PpaValue::Integer(i1 % i2))
            }

            BinaryOp::And => match (&left, &right) {
                (PpaValue::Boolean(b1), PpaValue::Boolean(b2)) => {
                    Ok(PpaValue::Boolean(*b1 && *b2))
                }
                (PpaValue::Integer(i1), PpaValue::Integer(i2)) => {
                    Ok(PpaValue::Integer(i1 & i2))
                }
                _ => Ok(PpaValue::Boolean(left.is_truthy() && right.is_truthy())),
            },

            BinaryOp::Or => match (&left, &right) {
                (PpaValue::Boolean(b1), PpaValue::Boolean(b2)) => {
                    Ok(PpaValue::Boolean(*b1 || *b2))
                }
                (PpaValue::Integer(i1), PpaValue::Integer(i2)) => {
                    Ok(PpaValue::Integer(i1 | i2))
                }
                _ => Ok(PpaValue::Boolean(left.is_truthy() || right.is_truthy())),
            },

            BinaryOp::Eq => Ok(PpaValue::Boolean(self.values_equal(&left, &right))),
            BinaryOp::Ne => Ok(PpaValue::Boolean(!self.values_equal(&left, &right))),

            BinaryOp::Lt => Ok(PpaValue::Boolean(self.compare_values(&left, &right)? < 0)),
            BinaryOp::Le => {
                Ok(PpaValue::Boolean(self.compare_values(&left, &right)? <= 0))
            }
            BinaryOp::Gt => Ok(PpaValue::Boolean(self.compare_values(&left, &right)? > 0)),
            BinaryOp::Ge => {
                Ok(PpaValue::Boolean(self.compare_values(&left, &right)? >= 0))
            }
        }
    }

    fn values_equal(&self, left: &PpaValue, right: &PpaValue) -> bool {
        match (left, right) {
            (PpaValue::Integer(i1), PpaValue::Integer(i2)) => i1 == i2,
            (PpaValue::Float(f1), PpaValue::Float(f2)) => (f1 - f2).abs() < f64::EPSILON,
            (PpaValue::Integer(i1), PpaValue::Float(f2)) => (*i1 as f64 - f2).abs() < f64::EPSILON,
            (PpaValue::Float(f1), PpaValue::Integer(i2)) => (f1 - *i2 as f64).abs() < f64::EPSILON,
            (PpaValue::String(s1), PpaValue::String(s2)) => s1 == s2,
            (PpaValue::Boolean(b1), PpaValue::Boolean(b2)) => b1 == b2,
            (PpaValue::Void, PpaValue::Void) => true,
            _ => false,
        }
    }

    fn compare_values(&self, left: &PpaValue, right: &PpaValue) -> Result<i8, PpaError> {
        match (left, right) {
            (PpaValue::Integer(i1), PpaValue::Integer(i2)) => Ok(i1.cmp(i2) as i8),
            (PpaValue::Float(f1), PpaValue::Float(f2)) => {
                Ok(f1.partial_cmp(f2).unwrap_or(std::cmp::Ordering::Equal) as i8)
            }
            (PpaValue::Integer(i1), PpaValue::Float(f2)) => {
                Ok((*i1 as f64).partial_cmp(f2).unwrap_or(std::cmp::Ordering::Equal) as i8)
            }
            (PpaValue::Float(f1), PpaValue::Integer(i2)) => {
                Ok(f1.partial_cmp(&(*i2 as f64)).unwrap_or(std::cmp::Ordering::Equal) as i8)
            }
            (PpaValue::String(s1), PpaValue::String(s2)) => Ok(s1.cmp(s2) as i8),
            (PpaValue::Boolean(b1), PpaValue::Boolean(b2)) => Ok(b1.cmp(b2) as i8),
            _ => Err(PpaError::TypeMismatch(format!(
                "Cannot compare {:?} and {:?}",
                left, right
            ))),
        }
    }

    fn is_known_function(&self, name: &str) -> bool {
        if let Some(ref prog) = self.current_program {
            if prog
                .functions
                .iter()
                .any(|f| f.name.eq_ignore_ascii_case(name))
            {
                return true;
            }
            if prog
                .procedures
                .iter()
                .any(|p| p.name.eq_ignore_ascii_case(name))
            {
                return true;
            }
        }
        false
    }

    fn call_proc_or_func(
        &mut self,
        name: &str,
        args: &[PpaValue],
        ctx: &mut dyn MacroContext,
    ) -> Result<PpaValue, PpaError> {
        // 1. 查找用户自定义函数 (Function)
        if let Some(ref prog) = self.current_program.clone() {
            if let Some(func) = prog
                .functions
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(name))
                .cloned()
            {
                if args.len() != func.params.len() {
                    return Err(PpaError::InvalidArgCount {
                        expected: func.params.len(),
                        got: args.len(),
                    });
                }

                self.env.push_scope();

                // 绑定形参
                for (param, val) in func.params.iter().zip(args.iter()) {
                    self.env.define(&param.name, val.clone());
                }

                // 初始化 Pascal 函数名本身以及 Result 变量作为返回值寄存器
                self.env.define("result", PpaValue::Void);
                self.env.define(&func.name, PpaValue::Void);

                // 初始化局部变量
                for decl in &func.local_vars {
                    let init_val = if let Some(ref ie) = decl.init {
                        self.eval_expr(ie, ctx)?
                    } else {
                        PpaValue::Void
                    };
                    for n in &decl.names {
                        self.env.define(n, init_val.clone());
                    }
                }

                let mut ret_val = PpaValue::Void;
                for stmt in &func.body {
                    let signal = self.execute_stmt(stmt, ctx)?;
                    if let ExecSignal::Exit(opt_v) = signal {
                        if let Some(v) = opt_v {
                            ret_val = v;
                        }
                        break;
                    }
                }

                // 优先检查显式 exit(val)，次之 result 变量，再次之函数名变量
                if ret_val == PpaValue::Void {
                    if let Some(r) = self.env.get("result") {
                        if r != PpaValue::Void {
                            ret_val = r;
                        }
                    }
                }
                if ret_val == PpaValue::Void {
                    if let Some(fn_var) = self.env.get(&func.name) {
                        if fn_var != PpaValue::Void {
                            ret_val = fn_var;
                        }
                    }
                }

                self.env.pop_scope();
                return Ok(ret_val);
            }

            // 2. 查找用户自定义过程 (Procedure)
            if let Some(proc) = prog
                .procedures
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(name))
                .cloned()
            {
                if args.len() != proc.params.len() {
                    return Err(PpaError::InvalidArgCount {
                        expected: proc.params.len(),
                        got: args.len(),
                    });
                }

                self.env.push_scope();

                for (param, val) in proc.params.iter().zip(args.iter()) {
                    self.env.define(&param.name, val.clone());
                }

                for decl in &proc.local_vars {
                    let init_val = if let Some(ref ie) = decl.init {
                        self.eval_expr(ie, ctx)?
                    } else {
                        PpaValue::Void
                    };
                    for n in &decl.names {
                        self.env.define(n, init_val.clone());
                    }
                }

                for stmt in &proc.body {
                    let signal = self.execute_stmt(stmt, ctx)?;
                    if let ExecSignal::Exit(_) = signal {
                        break;
                    }
                }

                self.env.pop_scope();
                return Ok(PpaValue::Void);
            }
        }

        // 3. 派发给标准内置库或编辑器 MacroContext
        self.call_builtin_or_context(name, args, ctx)
    }

    fn call_builtin_or_context(
        &mut self,
        name: &str,
        args: &[PpaValue],
        ctx: &mut dyn MacroContext,
    ) -> Result<PpaValue, PpaError> {
        match name.to_ascii_lowercase().as_str() {
            "inttostr" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(PpaValue::String(args[0].to_ppa_string()))
            }

            "strtoint" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                let i = args[0].as_int()?;
                Ok(PpaValue::Integer(i))
            }

            "strtointdef" => {
                if args.len() < 2 {
                    return Err(PpaError::InvalidArgCount {
                        expected: 2,
                        got: args.len(),
                    });
                }
                match args[0].as_int() {
                    Ok(val) => Ok(PpaValue::Integer(val)),
                    Err(_) => Ok(args[1].clone()),
                }
            }

            "length" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                let len = match &args[0] {
                    PpaValue::String(s) => s.chars().count() as i64,
                    _ => 0,
                };
                Ok(PpaValue::Integer(len))
            }

            // Copy(S, Index, Count) — Pascal 1-based 下标
            "copy" => {
                if args.len() < 3 {
                    return Err(PpaError::InvalidArgCount {
                        expected: 3,
                        got: args.len(),
                    });
                }
                let s = args[0].to_ppa_string();
                let index = args[1].as_int()?;
                let count = args[2].as_int()?;

                if count <= 0 {
                    return Ok(PpaValue::String(String::new()));
                }

                let chars: Vec<char> = s.chars().collect();
                let start = (index - 1).max(0) as usize;
                if start >= chars.len() {
                    return Ok(PpaValue::String(String::new()));
                }

                let end = (start + count as usize).min(chars.len());
                let sub: String = chars[start..end].iter().collect();
                Ok(PpaValue::String(sub))
            }

            // Pos(SubStr, Str) — Pascal 1-based 下标, 找不到返回 0
            "pos" => {
                if args.len() < 2 {
                    return Err(PpaError::InvalidArgCount {
                        expected: 2,
                        got: args.len(),
                    });
                }
                let substr = args[0].to_ppa_string();
                let target = args[1].to_ppa_string();

                if substr.is_empty() {
                    return Ok(PpaValue::Integer(0));
                }

                if let Some(byte_idx) = target.find(&substr) {
                    let char_count = target[..byte_idx].chars().count() as i64;
                    Ok(PpaValue::Integer(char_count + 1))
                } else {
                    Ok(PpaValue::Integer(0))
                }
            }

            "lowercase" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(PpaValue::String(args[0].to_ppa_string().to_lowercase()))
            }

            "uppercase" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(PpaValue::String(args[0].to_ppa_string().to_uppercase()))
            }

            "trim" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(PpaValue::String(
                    args[0].to_ppa_string().trim().to_string(),
                ))
            }

            "chr" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                let code = args[0].as_int()? as u32;
                let c = std::char::from_u32(code).unwrap_or('?');
                Ok(PpaValue::String(c.to_string()))
            }

            "ord" => {
                if args.is_empty() {
                    return Err(PpaError::InvalidArgCount {
                        expected: 1,
                        got: 0,
                    });
                }
                let s = args[0].to_ppa_string();
                let code = s.chars().next().map(|c| c as i64).unwrap_or(0);
                Ok(PpaValue::Integer(code))
            }

            _ => {
                // 委托给 MacroContext (例如 S_InsertText, S_GetLineStr 等)
                ctx.call(name, args)
            }
        }
    }
}
