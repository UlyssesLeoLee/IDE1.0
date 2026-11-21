//! `nema-codec` — IDE1.0 × IME1.0 共享的 NEMA v1.5 编解码器、所有权仲裁状态机与渲染视图引擎。
//!
//! 严格对应 `E:\.NEMA\codec\core.py` 与 `E:\.NEMA\codec\transport.py` 的全部语法不变量、
//! 字典候选增益算法、RLE 上下界 (`4..=100_000`)、错误码及测试向量。

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

pub mod interpreter;
pub use interpreter::*;

/// 最大源码/传输字节上限 (16 MiB)
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
/// 字典最大条目数
pub const MAX_DICT_ENTRIES: usize = 2048;
/// RLE 最小重复次数
pub const MIN_RLE_COUNT: usize = 4;
/// RLE 最大重复次数
pub const MAX_RLE_COUNT: usize = 100_000;
/// 5W1H 固定六轴列名
pub const AXES_5W1H: [&str; 6] = ["What", "Why", "How", "Who", "When", "Where"];
/// Graph 固定三列名
pub const COLUMNS_GRAPH: [&str; 3] = ["Element", "Relation", "Description"];

/// NEMA 共享编解码器核心 Trait（与设计规范 1:1 对齐）
pub trait NemaCodec {
    fn parse(&self, source: &str) -> Result<NemaDocument, NemaError>;
    fn encode(&self, source: &str, profile: &TokenProfile) -> Result<EncodedChoice, NemaError>;
    fn decode(&self, source: &str) -> Result<String, NemaError>;
}

/// 标准 NEMA v1.5 编解码器实现
#[derive(Debug, Default, Clone, Copy)]
pub struct StandardNemaCodec;

impl NemaCodec for StandardNemaCodec {
    fn parse(&self, source: &str) -> Result<NemaDocument, NemaError> {
        parse_source(source)
    }

    fn encode(&self, source: &str, profile: &TokenProfile) -> Result<EncodedChoice, NemaError> {
        encode_with_profile(source, profile, 48, &[])
    }

    fn decode(&self, source: &str) -> Result<String, NemaError> {
        decode_transport(source)
    }
}

/// NEMA 编解码与语法校验错误码（与 Python 参考实现一一对应）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
pub enum NemaError {
    #[error("source too large")]
    SourceTooLarge,
    #[error("LF only")]
    LfOnly,
    #[error("must terminate with exactly two empty physical lines (three LF)")]
    BadTermination,
    #[error("no content")]
    NoContent,
    #[error("internal empty line prohibited")]
    InternalEmptyLine,
    #[error("bad dictionary manifest size")]
    BadDictionaryManifestSize,
    #[error("dictionary entries must be nonempty TAB-free")]
    InvalidDictionaryEntry,
    #[error("duplicate domain ID")]
    DuplicateDomainId,
    #[error("TAB is reserved for domain/dictionary control; body uses ASCII spaces")]
    TabInBody,
    #[error("body before domain")]
    BodyBeforeDomain,
    #[error("table headers require nonempty single-token names")]
    InvalidTableHeaderName,
    #[error("duplicate column header")]
    DuplicateColumnHeader,
    #[error("continuation before first record")]
    ContinuationBeforeFirstRecord,
    #[error("blank continuation")]
    BlankContinuation,
    #[error("horizontal coordinates require exactly one ASCII space")]
    MultipleOrLeadingSpaces,
    #[error("invalid edge expression: {0}")]
    InvalidEdgeExpression(String),
    #[error("invalid node ID {0}")]
    InvalidNodeId(String),
    #[error("5w1h maximum six fields")]
    TooMany5W1HFields,
    #[error("no domains")]
    NoDomains,
    #[error("duplicate edge ID: {0}")]
    DuplicateEdgeId(String),
    #[error("duplicate node ID: {0}")]
    DuplicateNodeId(String),
    #[error("undefined graph node reference: {0} / {1}")]
    UndefinedGraphNode(String, String),
    #[error("undefined dictionary reference ^{0}")]
    UndefinedDictionaryReference(usize),
    #[error("transport too large")]
    TransportTooLarge,
    #[error("invalid transport newline termination")]
    InvalidTransportTermination,
    #[error("expected transport dictionary manifest; pass native .nema separately")]
    ExpectedTransportManifest,
    #[error("bad dictionary size")]
    BadTransportDictionarySize,
    #[error("invalid dictionary value")]
    InvalidTransportDictionaryValue,
    #[error("unsupported legacy prefix selector; v1.5 uses numeric domain headers")]
    UnsupportedLegacyPrefixSelector,
    #[error("encoded domain header not permitted")]
    EncodedDomainHeaderNotPermitted,
    #[error("decoded source exceeds limit")]
    DecodedSourceExceedsLimit,
    #[error("invalid dictionary reference")]
    InvalidTransportDictRef,
    #[error("dictionary index out of range")]
    TransportDictIndexOutOfRange,
    #[error("trailing transport escape")]
    TrailingTransportEscape,
    #[error("RLE count exceeds bounds")]
    RleCountExceedsBounds,
    #[error("RLE missing character")]
    RleMissingCharacter,
    #[error("missing escaped RLE character")]
    MissingEscapedRleCharacter,
    #[error("RLE may not encode coordinate separators")]
    RleCoordinateSeparatorProhibited,
    #[error("unknown transport escape")]
    UnknownTransportEscape,
    #[error("row too large")]
    RowTooLarge,
    #[error("line {line}: {message}")]
    StrictSyntax { line: usize, message: String },
}

/// 源码字节与行列坐标 Span
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TextSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub line: usize,
    pub column: usize,
}

/// 单元格内未拼接的语义块（禁止直接只存拼接后的句子）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticChunk {
    /// 原始未转义 token 片段
    pub raw: String,
    /// 源码坐标 Span
    pub span: TextSpan,
    /// 拆分出的不透明字典引用与普通文本原子
    pub atoms: Vec<AtomChunk>,
}

/// 语义块内部的原子单元（普通文本或不可被 PascalCase 切分的字典条目）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomChunk {
    pub text: String,
    pub opaque_dict_index: Option<usize>,
}

/// 域模式：`table` (`\N`), `5w1h` (`?N`), `graph` (`>N`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DomainMode {
    Table,
    #[serde(rename = "5w1h")]
    FiveWOneH,
    Graph,
}

impl DomainMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::FiveWOneH => "5w1h",
            Self::Graph => "graph",
        }
    }

    pub fn prefix_char(self) -> char {
        match self {
            Self::Table => '\\',
            Self::FiveWOneH => '?',
            Self::Graph => '>',
        }
    }
}

/// Graph 边方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeDirection {
    Directed,
    Undirected,
}

/// Graph 边元数据（拥有独立 ID，绝不把标签当作节点）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdgeSpec {
    pub id: String,
    pub from: String,
    pub to: String,
    pub direction: EdgeDirection,
    pub raw_operator: char,
}

/// 单条记录（保存每列 `Vec<Vec<SemanticChunk>>`）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NemaRecord {
    pub span: TextSpan,
    /// 每列的垂直语义块数组 `Vec<Vec<SemanticChunk>>`
    pub cells: Vec<Vec<SemanticChunk>>,
    /// 行尾是否有显式 ` .` 哨兵
    pub explicit_tail_empty: bool,
    /// Graph 模式下解析出的节点 ID 或边结构
    pub graph_node_id: Option<String>,
    pub graph_edge: Option<GraphEdgeSpec>,
}

impl NemaRecord {
    pub fn add_chunk(&mut self, col: usize, chunk: SemanticChunk) {
        while self.cells.len() <= col {
            self.cells.push(Vec::new());
        }
        self.cells[col].push(chunk);
    }

    /// 获取与 Python `Record.cells` 等价的原始字符串二维数组 `Vec<Vec<String>>`
    pub fn raw_groups(&self) -> Vec<Vec<String>> {
        self.cells
            .iter()
            .map(|col| col.iter().map(|c| c.raw.clone()).collect())
            .collect()
    }
}

/// 单个 NEMA 域
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NemaDomain {
    pub id: String,
    pub revision: u64,
    pub mode: DomainMode,
    pub header_span: TextSpan,
    pub explicit_header: bool,
    pub explicit_header_span: Option<TextSpan>,
    pub columns: Vec<String>,
    pub records: Vec<NemaRecord>,
    #[serde(default)]
    pub is_v12: bool,
}

/// 解析后的权威 `NemaDocument`（保留原始 span、column 与每列 `Vec<Vec<SemanticChunk>>`）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NemaDocument {
    pub span: TextSpan,
    pub dictionary: Vec<String>,
    pub dictionary_spans: Vec<TextSpan>,
    pub domains: Vec<NemaDomain>,
}

/// 渲染后的行视图（按需生成，供 UI Table / 5W1H / Graph 消费）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedRow {
    pub cells: Vec<String>,
    pub groups: Vec<Vec<String>>,
    pub tail_empty: bool,
    pub node_id: Option<String>,
    pub edge: Option<GraphEdgeSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<(String, String)>,
    pub line: usize,
}

/// 渲染后的域视图
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedDomain {
    pub id: String,
    pub revision: u64,
    pub mode: DomainMode,
    pub columns: Vec<String>,
    pub explicit_header: bool,
    pub rows: Vec<RenderedRow>,
}

/// 渲染后的文档视图
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedDocument {
    pub dictionary: Vec<String>,
    pub domains: Vec<RenderedDomain>,
}

/// 密钥安全引用（绝不在源码、候选字典或日志中内联明文 API Key）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef {
    pub key_id: String,
    pub env_var: String,
}

impl SecretRef {
    pub fn new(key_id: impl Into<String>, env_var: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into(),
            env_var: env_var.into(),
        }
    }
}

/// 分词器配置（支持 BPE / Unigram / 官方模型词表，缺失时回退字节启发并显式标注不等同于计费 Token）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TokenProfile {
    /// 字节启发（UTF-8 bytes），明确标注 `is_exact_token = false`
    #[default]
    ByteHeuristic,
    /// 简易 BPE / 词表切分估算
    BpeHeuristic { model_name: String },
    /// Unigram 分词估算
    UnigramHeuristic { model_name: String },
    /// 商业模型官方 Tokenizer（仅持 `SecretRef`，不持明文密钥）
    OfficialModel {
        provider: String,
        model: String,
        secret_ref: Option<SecretRef>,
    },
}

impl TokenProfile {
    /// 计算给定文本在该 Profile 下的单位开销与是否为精确计费 Token
    pub fn measure(&self, text: &str) -> (usize, bool, &'static str) {
        match self {
            Self::ByteHeuristic => (
                text.len(),
                false,
                "字节启发 (UTF-8 Bytes，不等同于模型计费 Token)",
            ),
            Self::BpeHeuristic { .. } => {
                // 近似 BPE：ASCII 连续字母数字每 4 字节 1 token，CJK 每字符 1 token，标点/空白独立或合并
                let mut tokens = 0usize;
                let mut ascii_run = 0usize;
                for ch in text.chars() {
                    if ch.is_ascii_alphanumeric() {
                        ascii_run += 1;
                    } else {
                        if ascii_run > 0 {
                            tokens += ascii_run.div_ceil(4);
                            ascii_run = 0;
                        }
                        tokens += 1;
                    }
                }
                if ascii_run > 0 {
                    tokens += ascii_run.div_ceil(4);
                }
                (
                    tokens.max(1),
                    false,
                    "BPE 启发估算 (不等同于官方计费 Token)",
                )
            }
            Self::UnigramHeuristic { .. } => {
                let mut tokens = 0usize;
                let mut ascii_run = 0usize;
                for ch in text.chars() {
                    if ch.is_ascii_alphabetic() {
                        ascii_run += 1;
                    } else {
                        if ascii_run > 0 {
                            tokens += ascii_run.div_ceil(5);
                            ascii_run = 0;
                        }
                        tokens += 1;
                    }
                }
                if ascii_run > 0 {
                    tokens += ascii_run.div_ceil(5);
                }
                (
                    tokens.max(1),
                    false,
                    "Unigram 启发估算 (不等同于官方计费 Token)",
                )
            }
            Self::OfficialModel { .. } => (
                text.len(),
                false,
                "未连接外部官方分词器，已回退字节启发 (不等同于计费 Token)",
            ),
        }
    }
}

/// 编码策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncodeMethod {
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "dictionary+rle")]
    DictionaryAndRle,
    #[serde(rename = "dictionary")]
    DictionaryOnly,
    #[serde(rename = "rle")]
    RleOnly,
    #[serde(rename = "prefix")]
    PrefixOnly,
    #[serde(rename = "combined")]
    Combined,
}

impl EncodeMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::DictionaryAndRle => "dictionary+rle",
            Self::DictionaryOnly => "dictionary",
            Self::RleOnly => "rle",
            Self::PrefixOnly => "prefix",
            Self::Combined => "combined",
        }
    }
}

/// 编码输出格式（`.nema` 原生或 `.nemat` 传输层）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodecFormat {
    Nema,
    Nemat,
}

impl CodecFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nema => "nema",
            Self::Nemat => "nemat",
        }
    }
}

/// `encode()` 返回的最优编码选择（与 Python `Choice` 一一对应）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedChoice {
    pub method: EncodeMethod,
    pub text: String,
    pub score: usize,
    pub baseline_score: usize,
    pub format: CodecFormat,
    pub dictionary: Vec<String>,
    pub is_exact_token: bool,
    pub metric_note: String,
}

impl EncodedChoice {
    /// 节省百分比（保留两位小数，与 Python `round(100*(1-score/baseline_score), 2)` 一致）
    pub fn saving_percent(&self) -> f64 {
        if self.baseline_score == 0 {
            0.0
        } else {
            let raw = 100.0 * (1.0 - (self.score as f64) / (self.baseline_score as f64));
            (raw * 100.0).round() / 100.0
        }
    }
}

/// 字典候选及其收益详情
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictCandidate {
    pub word: String,
    pub count: usize,
    pub gain: isize,
}

/// 字典或复用词在文档中的出现位置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReusedPosition {
    pub token: String,
    pub dict_index: Option<usize>,
    pub line: usize,
    pub column: usize,
}

// ============================================================================
// 1. 源码级校验与解析 (`core.py` 1:1 对应)
// ============================================================================

/// 校验源码基本结构并返回物理行列表（不含末尾 `\n\n\n`）
pub fn validate_source(source: &str) -> Result<Vec<&str>, NemaError> {
    if source.len() > MAX_BYTES {
        return Err(NemaError::SourceTooLarge);
    }
    if source.contains('\r') {
        return Err(NemaError::LfOnly);
    }
    if !source.ends_with("\n\n\n") || source.ends_with("\n\n\n\n") {
        return Err(NemaError::BadTermination);
    }
    let body = &source[..source.len() - 3];
    if body.is_empty() {
        return Err(NemaError::NoContent);
    }
    if body.contains("\n\n") {
        return Err(NemaError::InternalEmptyLine);
    }
    Ok(body.split('\n').collect())
}

/// 原生格式转义还原：支持 `.NEMA` 核心与官网严格语法全量转义符
/// (`\^`、`\\`、`\.`、`\ `、`\?`、`\>`、`\<`、`\=`、`\~`、`\#`、`\_`、`\%`、`\n`、`\r`、`\t`、`\e`)
pub fn unescape_opaque(v: &str) -> String {
    if v == "\\_" || v == "\\e" {
        return String::new();
    }
    let chars: Vec<char> = v.chars().collect();
    let mut out = String::with_capacity(v.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'e' => {}
                c @ ('^' | '\\' | '.' | ' ' | '?' | '>' | '<' | '=' | '~' | '#' | '_' | '%') => {
                    out.push(c);
                }
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// 按未转义 ASCII 空格切分单行字段（兼容 `\ ` 与 `\e` / `\_` 特殊词块）
fn split_unescaped_spaces(s: &str) -> Result<Vec<&str>, NemaError> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let step = s[i + 1..].chars().next().map_or(1, char::len_utf8);
            i += 1 + step;
            continue;
        }
        if bytes[i] == b' ' {
            if i == start {
                return Err(NemaError::MultipleOrLeadingSpaces);
            }
            out.push(&s[start..i]);
            i += 1;
            start = i;
            continue;
        }
        let step = s[i..].chars().next().map_or(1, char::len_utf8);
        i += step;
    }
    if start >= bytes.len() {
        return Err(NemaError::MultipleOrLeadingSpaces);
    }
    out.push(&s[start..]);
    Ok(out)
}

/// 拆分混合原子：返回 `(text, is_opaque_dict_entry)`，字典条目不会被 PascalCase 二次拆分
pub fn mixed_atoms(raw: &str, dictionary: &[String]) -> Result<Vec<AtomChunk>, NemaError> {
    let chars: Vec<char> = raw.chars().collect();
    let mut result = Vec::new();
    let mut buf = String::new();
    let mut i = 0;

    let flush = |buf: &mut String, result: &mut Vec<AtomChunk>| {
        if !buf.is_empty() {
            result.push(AtomChunk {
                text: unescape_opaque(buf),
                opaque_dict_index: None,
            });
            buf.clear();
        }
    };

    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            buf.push(chars[i]);
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if chars[i] == '^' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            let mut num_str = String::new();
            while j < chars.len() && chars[j].is_ascii_digit() {
                num_str.push(chars[j]);
                j += 1;
            }
            let ix: usize = num_str
                .parse()
                .map_err(|_| NemaError::UndefinedDictionaryReference(usize::MAX))?;
            if ix >= dictionary.len() {
                return Err(NemaError::UndefinedDictionaryReference(ix));
            }
            flush(&mut buf, &mut result);
            result.push(AtomChunk {
                text: dictionary[ix].clone(),
                opaque_dict_index: Some(ix),
            });
            i = j;
            continue;
        }
        buf.push(chars[i]);
        i += 1;
    }
    flush(&mut buf, &mut result);
    Ok(result)
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|c| ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{3400}'..='\u{9fff}').contains(&c))
}

fn is_cjk_join_char(c: char) -> bool {
    ('\u{3040}'..='\u{9fff}').contains(&c)
}

/// 与 Python `ENGLISH = re.compile(r'[A-Z]+(?=[A-Z][a-z]|$)|[A-Z][a-z]+|[A-Z]+|[a-z]+|[0-9]+|[^A-Za-z0-9]+')`
/// 完全一致的分段函数（处理 `HTTPItIs` -> `["HTTP", "It", "Is"]`）
fn split_english_pieces(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut pieces = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_uppercase() {
            // Count consecutive uppercase letters
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_uppercase() {
                j += 1;
            }
            let upper_len = j - i;
            if upper_len > 1 {
                if j < chars.len() && chars[j].is_ascii_lowercase() {
                    // `[A-Z]+(?=[A-Z][a-z])` takes all but the last uppercase char
                    let s: String = chars[i..j - 1].iter().collect();
                    pieces.push(s);
                    i = j - 1;
                } else {
                    // `[A-Z]+(?=$)` or `[A-Z]+` followed by non-alpha
                    let s: String = chars[i..j].iter().collect();
                    pieces.push(s);
                    i = j;
                }
            } else {
                // Single uppercase, check if followed by lowercase `[A-Z][a-z]+`
                let mut k = j;
                while k < chars.len() && chars[k].is_ascii_lowercase() {
                    k += 1;
                }
                let s: String = chars[i..k].iter().collect();
                pieces.push(s);
                i = k;
            }
        } else if c.is_ascii_lowercase() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_lowercase() {
                j += 1;
            }
            pieces.push(chars[i..j].iter().collect());
            i = j;
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            pieces.push(chars[i..j].iter().collect());
            i = j;
        } else {
            let mut j = i;
            while j < chars.len() && !chars[j].is_ascii_alphanumeric() {
                j += 1;
            }
            pieces.push(chars[i..j].iter().collect());
            i = j;
        }
    }
    pieces
}

/// 渲染单个水平/垂直片段（与 `core.py::render_text` 1:1 对齐）
pub fn render_text(raw: &str, dictionary: &[String]) -> Result<String, NemaError> {
    let items = mixed_atoms(raw, dictionary)?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if item.opaque_dict_index.is_some() || contains_cjk(&item.text) {
            out.push(item.text);
        } else {
            let pieces = split_english_pieces(&item.text);
            let mut v = String::new();
            for part in pieces {
                if v.is_empty() {
                    v = part;
                } else if part.chars().all(|c| c.is_alphabetic())
                    && v.chars().last().is_some_and(|c| c.is_alphabetic())
                {
                    v.push(' ');
                    v.push_str(&part);
                } else {
                    v.push_str(&part);
                }
            }
            out.push(v);
        }
    }

    let mut joined = String::new();
    for (i, chunk) in out.iter().enumerate() {
        if i > 0 && !chunk.is_empty() {
            if let (Some(prev), Some(first)) = (joined.chars().last(), chunk.chars().next()) {
                if prev.is_ascii_alphanumeric() && first.is_ascii_alphanumeric() {
                    joined.push(' ');
                }
            }
        }
        joined.push_str(chunk);
    }
    Ok(joined)
}

/// 拼接同一单元格内的多个垂直语义块（与 `core.py::join_cell` 1:1 对齐）
pub fn join_cell<S: AsRef<str>>(parts: &[S], dictionary: &[String]) -> Result<String, NemaError> {
    let mut acc = String::new();
    for p in parts {
        let r = render_text(p.as_ref(), dictionary)?;
        if acc.is_empty() {
            acc = r;
            continue;
        }
        let first = r.chars().next();
        let prev = acc.chars().last();
        if let Some(f) = first {
            if ",.;:!?)]}、。，！？；：".contains(f) {
                acc.push_str(&r);
                continue;
            }
            if "'\u{2019}".contains(f) || prev.is_some_and(|p| "'\u{2019}".contains(p)) {
                acc.push_str(&r);
                continue;
            }
            if is_cjk_join_char(f) || prev.is_some_and(is_cjk_join_char) {
                acc.push_str(&r);
                continue;
            }
        }
        acc.push(' ');
        acc.push_str(&r);
    }
    Ok(acc)
}

fn is_positive_digits(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some('1'..='9') => chars.all(|c| c.is_ascii_digit()),
        _ => false,
    }
}

fn is_all_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// 解析 `\t$N` 字典清单头
fn parse_dict_header(line: &str) -> Option<usize> {
    let rest = line.strip_prefix("\t$")?;
    if !is_all_digits(rest) {
        return None;
    }
    rest.parse().ok()
}

fn is_valid_v12_domain_id(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        }
        _ => false,
    }
}

/// 解析 `.NEMA v1.2` 域头：`^\t([\\?>]?)([A-Za-z][A-Za-z0-9_.-]*) ([1-9][0-9]*)$`
/// 返回 `(is_graph, id, revision)`
pub fn parse_domain_header_v12(line: &str) -> Option<(bool, String, u64)> {
    let rest = if let Some(r) = line.strip_prefix(' ') {
        if r.starts_with(' ') {
            return None;
        }
        if let Some(s) = r.strip_prefix('\\') {
            s
        } else {
            r
        }
    } else {
        let r = line.strip_prefix('\t')?;
        if let Some(s) = r.strip_prefix('\\') {
            s
        } else {
            r
        }
    };
    let (is_graph, after_mark) = if let Some(stripped) = rest.strip_prefix('>') {
        (true, stripped)
    } else if let Some(stripped) = rest.strip_prefix('?') {
        (false, stripped)
    } else {
        (false, rest)
    };
    let (id_part, rev_part) = after_mark.split_once(' ')?;
    if !is_valid_v12_domain_id(id_part) || !is_positive_digits(rev_part) {
        return None;
    }
    let rev: u64 = rev_part.parse().ok()?;
    Some((is_graph, id_part.to_string(), rev))
}

pub fn parse_domain_header(line: &str) -> Option<(DomainMode, String, u64)> {
    let (mode, tail) = if let Some(r) = line.strip_prefix(' ') {
        if r.starts_with(' ') || !r.starts_with('\\') {
            return None;
        }
        let sub = &r[1..];
        if let Some(rest) = sub.strip_prefix('?') {
            (DomainMode::FiveWOneH, rest)
        } else if let Some(rest) = sub.strip_prefix('>') {
            (DomainMode::Graph, rest)
        } else if let Some(rest) = sub.strip_prefix('|') {
            (DomainMode::Table, rest)
        } else if let Some(rest) = sub.strip_prefix('T') {
            (DomainMode::Table, rest)
        } else {
            (DomainMode::Table, sub)
        }
    } else {
        let rest = line.strip_prefix('\t')?;
        if let Some(sub) = rest.strip_prefix('\\') {
            if let Some(s) = sub.strip_prefix('?') {
                (DomainMode::FiveWOneH, s)
            } else if let Some(s) = sub.strip_prefix('>') {
                (DomainMode::Graph, s)
            } else if let Some(s) = sub.strip_prefix('|') {
                (DomainMode::Table, s)
            } else if let Some(s) = sub.strip_prefix('T') {
                (DomainMode::Table, s)
            } else {
                (DomainMode::Table, sub)
            }
        } else if let Some(s) = rest.strip_prefix('>') {
            (DomainMode::Graph, s)
        } else if let Some(s) = rest.strip_prefix('G') {
            (DomainMode::Graph, s)
        } else if let Some(s) = rest.strip_prefix('?') {
            (DomainMode::FiveWOneH, s)
        } else if let Some(s) = rest.strip_prefix('|') {
            (DomainMode::Table, s)
        } else if let Some(s) = rest.strip_prefix('T') {
            (DomainMode::Table, s)
        } else {
            (DomainMode::FiveWOneH, rest)
        }
    };
    if let Some((id_part, rev_part)) = tail.split_once(' ') {
        if (!is_positive_digits(id_part) && !is_valid_v12_domain_id(id_part))
            || !is_positive_digits(rev_part)
        {
            return None;
        }
        let rev: u64 = rev_part.parse().ok()?;
        Some((mode, id_part.to_string(), rev))
    } else {
        if !is_positive_digits(tail) && !is_valid_v12_domain_id(tail) {
            return None;
        }
        Some((mode, tail.to_string(), 1))
    }
}

/// `.NEMA v1.2` / 官网严格语法转义还原：支持 `\ `、`\\`、`\t`、`\n`、`\r`、`\e`、`\_`、`\=` 及符号转义
pub fn unescape_v12(s: &str) -> Result<String, NemaError> {
    if s == "\\_" || s == "\\e" {
        return Ok(String::new());
    }
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '\\' {
            result.push(chars[i]);
            i += 1;
            continue;
        }
        if i + 1 >= chars.len() {
            return Err(NemaError::TrailingTransportEscape);
        }
        let next = chars[i + 1];
        match next {
            ' ' | '\\' | '?' | '>' | '<' | '~' | '^' | '#' | '_' | '%' | '.' => result.push(next),
            't' => result.push('\t'),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            'e' => {}
            '=' => {
                if i == 0 && s.starts_with("\\=") {
                    result.push_str("\\=");
                } else {
                    result.push('=');
                }
            }
            _ => return Err(NemaError::UnknownTransportEscape),
        }
        i += 2;
    }
    Ok(result)
}

/// `.NEMA v1.2` 水平 X 轴切分 (`native.py::split_x`)
pub fn split_x_v12(s: &str) -> Result<Vec<String>, NemaError> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\'
            && i + 1 < chars.len()
            && matches!(
                chars[i + 1],
                ' ' | '\\'
                    | 't'
                    | 'n'
                    | 'r'
                    | 'e'
                    | '='
                    | '?'
                    | '>'
                    | '<'
                    | '~'
                    | '^'
                    | '#'
                    | '_'
                    | '%'
                    | '.'
            )
        {
            buf.push(chars[i]);
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if chars[i] == '\t' {
            return Err(NemaError::TabInBody);
        }
        if chars[i] == ' ' {
            out.push(unescape_v12(&buf)?);
            buf.clear();
            i += 1;
            continue;
        }
        buf.push(chars[i]);
        i += 1;
    }
    out.push(unescape_v12(&buf)?);
    Ok(out)
}

fn is_cjk_v12_char(c: char) -> bool {
    ('\u{3040}'..='\u{30ff}').contains(&c)
        || ('\u{3400}'..='\u{9fff}').contains(&c)
        || ('\u{ac00}'..='\u{d7af}').contains(&c)
}

/// 与 `native.py` `PASCAL = re.compile(r'[A-Z]+(?=[A-Z][a-z]|$)|[A-Z]?[a-z]+|[0-9]+|[^A-Za-z0-9]+')`
/// 完全一致的词法切分函数
fn lexical_words_v12(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut pieces = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_uppercase() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_uppercase() {
                j += 1;
            }
            let upper_len = j - i;
            if j == chars.len() {
                pieces.push(chars[i..j].iter().collect());
                i = j;
            } else if upper_len >= 2 && chars[j].is_ascii_lowercase() {
                pieces.push(chars[i..j - 1].iter().collect());
                i = j - 1;
            } else if i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase() {
                let mut k = i + 1;
                while k < chars.len() && chars[k].is_ascii_lowercase() {
                    k += 1;
                }
                pieces.push(chars[i..k].iter().collect());
                i = k;
            } else {
                i += 1;
            }
        } else if c.is_ascii_lowercase() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_lowercase() {
                j += 1;
            }
            pieces.push(chars[i..j].iter().collect());
            i = j;
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            pieces.push(chars[i..j].iter().collect());
            i = j;
        } else {
            let mut j = i;
            while j < chars.len() && !chars[j].is_ascii_alphanumeric() {
                j += 1;
            }
            pieces.push(chars[i..j].iter().collect());
            i = j;
        }
    }
    pieces
}

/// `.NEMA v1.2` 语义词块拼接 (`native.py::join_intents`)
/// - `exact_name = true` 用于 Graph `X1` (Node/Edge 名称) 和 `X3+` (扩展属性，如 `APIKey`, `HTTP401`, `SHA256`)，原样保留不拆词。
pub fn join_intents<S: AsRef<str>>(chunks: &[S], language: &str, exact_name: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    for raw_chunk in chunks {
        let chunk = raw_chunk.as_ref();
        if chunk.is_empty() {
            continue;
        }
        let has_lit_prefix = chunk.starts_with("\\=");
        let exact = exact_name || has_lit_prefix;
        let body = if has_lit_prefix { &chunk[2..] } else { chunk };
        let has_cjk = body.chars().any(is_cjk_v12_char);
        let has_special = body
            .chars()
            .any(|c| matches!(c, '.' | '/' | ':' | '#' | '@'));
        let atoms: Vec<String> = if exact
            || matches!(language, "zh" | "ja")
            || (language == "auto" && has_cjk)
            || has_special
        {
            vec![body.to_string()]
        } else {
            lexical_words_v12(body)
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect()
        };

        for mut atom in atoms {
            if atom.is_empty() {
                continue;
            }
            if !exact
                && !matches!(language, "zh" | "ja")
                && atom.chars().all(|c| c.is_ascii_alphabetic())
            {
                let is_all_upper = atom.chars().all(|c| c.is_ascii_uppercase());
                if !is_all_upper && !parts.is_empty() {
                    atom = atom.to_ascii_lowercase();
                }
            }
            if parts.is_empty() {
                parts.push(atom);
                continue;
            }
            let first_ch = atom.chars().next().expect("non-empty atom");
            let last_part = parts.last_mut().expect("non-empty parts");
            let prev_ch = last_part.chars().last().expect("non-empty part");
            if ",.;:!?、。！？）]}'’\"".contains(first_ch)
                || matches!(prev_ch, '\'' | '’')
                || is_cjk_v12_char(prev_ch)
                || is_cjk_v12_char(first_ch)
            {
                last_part.push_str(&atom);
            } else {
                parts.push(atom);
            }
        }
    }
    parts.join(" ")
}

fn is_valid_table_header_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        }
        _ => false,
    }
}

fn is_valid_graph_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':'))
        }
        _ => false,
    }
}

/// 解析 Graph 边表达式 `A>Z>B` / `B<Z<C` / `A=Z=B`
pub fn parse_edge_spec(expr: &str) -> Option<GraphEdgeSpec> {
    let op_idx1 = expr.find(['>', '<', '='])?;
    let op1 = expr[op_idx1..].chars().next()?;
    let after1 = &expr[op_idx1 + 1..];
    let op_idx2 = after1.find(['>', '<', '='])?;
    let op2 = after1[op_idx2..].chars().next()?;
    if op1 != op2 {
        return None;
    }
    let a = &expr[..op_idx1];
    let eid = &after1[..op_idx2];
    let b = &after1[op_idx2 + 1..];
    if !is_valid_graph_ident(a) || !is_valid_graph_ident(eid) || !is_valid_graph_ident(b) {
        return None;
    }
    let (from, to, direction) = match op1 {
        '>' => (a.to_string(), b.to_string(), EdgeDirection::Directed),
        '<' => (b.to_string(), a.to_string(), EdgeDirection::Directed),
        '=' => (a.to_string(), b.to_string(), EdgeDirection::Undirected),
        _ => return None,
    };
    Some(GraphEdgeSpec {
        id: eid.to_string(),
        from,
        to,
        direction,
        raw_operator: op1,
    })
}

fn append_v12_cols(
    rec: &mut NemaRecord,
    cells: Vec<String>,
    dictionary: &[String],
    line_start: usize,
    line_len: usize,
    line_no: usize,
) -> Result<(), NemaError> {
    rec.span.end_byte = line_start + line_len;
    let mut col_byte = line_start;
    let mut col_char = 1usize;
    for (idx, item) in cells.into_iter().enumerate() {
        while rec.cells.len() <= idx {
            rec.cells.push(Vec::new());
        }
        let item_len = item.len();
        let item_chars = item.chars().count();
        if !item.is_empty() {
            let atoms = if dictionary.is_empty() {
                vec![AtomChunk {
                    text: item.clone(),
                    opaque_dict_index: None,
                }]
            } else {
                mixed_atoms(&item, dictionary)?
            };
            rec.cells[idx].push(SemanticChunk {
                raw: item,
                span: TextSpan {
                    start_byte: col_byte,
                    end_byte: (col_byte + item_len).min(line_start + line_len),
                    line: line_no,
                    column: col_char,
                },
                atoms,
            });
        }
        col_byte += item_len + 1;
        col_char += item_chars + 1;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StrictPart {
    Raw(char),
    Literal(char),
    MissingMarker,
    EmptyMarker,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StrictVal {
    Null,
    Missing,
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StrictCell {
    Null,
    ExplicitMissing,
    ImplicitMissing,
    Text { value: String, groups: Vec<String> },
}

fn strict_fail(line: usize, message: impl Into<String>) -> NemaError {
    NemaError::StrictSyntax {
        line,
        message: message.into(),
    }
}

fn is_strict_graph_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':'))
        }
        _ => false,
    }
}

fn is_cont_index(s: &str) -> bool {
    s == "0" || is_positive_digits(s)
}

fn parse_strict_header(line: &str) -> Option<(DomainMode, String, u64)> {
    let rest = line.strip_prefix(" \\")?;
    let (mode, after_tag) = if let Some(s) = rest.strip_prefix('?') {
        (DomainMode::FiveWOneH, s)
    } else if let Some(s) = rest.strip_prefix('>') {
        (DomainMode::Graph, s)
    } else {
        (DomainMode::Table, rest)
    };
    let (id, rev_str) = after_tag.split_once(' ')?;
    if !is_positive_digits(id) || !is_positive_digits(rev_str) {
        return None;
    }
    let rev = rev_str.parse::<u64>().ok()?;
    Some((mode, id.to_string(), rev))
}

fn is_malformed_strict_header(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if trimmed.len() == line.len() {
        return false;
    }
    let Some(after_slash) = trimmed.strip_prefix('\\') else {
        return false;
    };
    let rest = after_slash
        .strip_prefix('?')
        .or_else(|| after_slash.strip_prefix('>'))
        .unwrap_or(after_slash);
    if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }
    if let Some(after_tag) = after_slash
        .strip_prefix('?')
        .or_else(|| after_slash.strip_prefix('>'))
    {
        let after_sp = after_tag.trim_start_matches(' ');
        if after_sp.len() < after_tag.len()
            && after_sp.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return true;
        }
    }
    false
}

fn strict_tokenize(s: &str, line: usize) -> Result<Vec<Vec<StrictPart>>, NemaError> {
    if s.is_empty() {
        return Err(strict_fail(line, "empty field expression"));
    }
    let chars: Vec<char> = s.chars().collect();
    let mut arr: Vec<Vec<StrictPart>> = Vec::new();
    let mut cur: Vec<StrictPart> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            if i + 1 >= chars.len() {
                return Err(strict_fail(line, "trailing escape"));
            }
            let e = chars[i + 1];
            let part = match e {
                ' ' | '\\' | '?' | '>' | '<' | '=' | '~' | '^' => StrictPart::Literal(e),
                'n' => StrictPart::Literal('\n'),
                'r' => StrictPart::Literal('\r'),
                't' => StrictPart::Literal('\t'),
                '_' => StrictPart::MissingMarker,
                'e' => StrictPart::EmptyMarker,
                other => return Err(strict_fail(line, format!("unknown escape \\{other}"))),
            };
            cur.push(part);
            i += 2;
            continue;
        }
        if c == ' ' {
            if cur.is_empty() {
                return Err(strict_fail(line, "consecutive spaces"));
            }
            arr.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        cur.push(StrictPart::Raw(c));
        i += 1;
    }
    if cur.is_empty() {
        return Err(strict_fail(line, "trailing separator space"));
    }
    arr.push(cur);
    Ok(arr)
}

fn strict_value(parts: &[StrictPart], line: usize) -> Result<StrictVal, NemaError> {
    if parts.len() == 1 {
        match parts[0] {
            StrictPart::Raw('~') => return Ok(StrictVal::Null),
            StrictPart::MissingMarker => return Ok(StrictVal::Missing),
            StrictPart::EmptyMarker => return Ok(StrictVal::Text(String::new())),
            _ => {}
        }
    }
    if parts
        .iter()
        .any(|p| matches!(p, StrictPart::MissingMarker | StrictPart::EmptyMarker))
    {
        return Err(strict_fail(
            line,
            "standalone token \\e or \\_ cannot be combined",
        ));
    }
    let mut out = String::with_capacity(parts.len());
    for p in parts {
        match p {
            StrictPart::Raw(c) | StrictPart::Literal(c) => out.push(*c),
            StrictPart::MissingMarker | StrictPart::EmptyMarker => unreachable!(),
        }
    }
    Ok(StrictVal::Text(out))
}

fn strict_fields(s: &str, line: usize) -> Result<Vec<StrictVal>, NemaError> {
    strict_tokenize(s, line)?
        .into_iter()
        .map(|p| strict_value(&p, line))
        .collect()
}

fn parse_strict_edge(
    token: &str,
    parts: &[StrictPart],
    line: usize,
) -> Result<GraphEdgeSpec, NemaError> {
    if parts
        .iter()
        .any(|p| matches!(p, StrictPart::Literal('>' | '<' | '=')))
    {
        return Err(strict_fail(line, "invalid edge expression"));
    }
    let op1_idx = token
        .find(['>', '<', '='])
        .ok_or_else(|| strict_fail(line, "invalid edge expression"))?;
    let op1 = token[op1_idx..]
        .chars()
        .next()
        .ok_or_else(|| strict_fail(line, "invalid edge expression"))?;
    let after1 = &token[op1_idx + 1..];
    let op2_idx = after1
        .find(['>', '<', '='])
        .ok_or_else(|| strict_fail(line, "invalid edge expression"))?;
    let op2 = after1[op2_idx..]
        .chars()
        .next()
        .ok_or_else(|| strict_fail(line, "invalid edge expression"))?;
    if op1 != op2 {
        return Err(strict_fail(line, "invalid edge expression"));
    }
    let a = &token[..op1_idx];
    let id = &after1[..op2_idx];
    let b = &after1[op2_idx + 1..];
    if !is_strict_graph_ident(a) || !is_strict_graph_ident(id) || !is_strict_graph_ident(b) {
        return Err(strict_fail(line, "invalid edge expression"));
    }
    let (from, to, direction) = match op1 {
        '>' => (a.to_string(), b.to_string(), EdgeDirection::Directed),
        '<' => (b.to_string(), a.to_string(), EdgeDirection::Directed),
        '=' => (a.to_string(), b.to_string(), EdgeDirection::Undirected),
        _ => return Err(strict_fail(line, "invalid edge expression")),
    };
    Ok(GraphEdgeSpec {
        id: id.to_string(),
        from,
        to,
        direction,
        raw_operator: op1,
    })
}

fn parse_strict_core(source: &str) -> Result<(NemaDocument, RenderedDocument), NemaError> {
    if source.len() > MAX_BYTES {
        return Err(NemaError::SourceTooLarge);
    }
    if source.contains('\t') {
        return Err(strict_fail(0, "raw Tab prohibited"));
    }
    if source.contains('\u{3000}') {
        return Err(strict_fail(0, "fullwidth space prohibited"));
    }
    if source.contains('\r') {
        return Err(strict_fail(0, "raw CR prohibited"));
    }

    let mut lines: Vec<&str> = source.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }

    struct StrictRec {
        span: TextSpan,
        node_id: Option<String>,
        edge: Option<GraphEdgeSpec>,
        values: Vec<StrictCell>,
    }

    struct StrictDomain {
        id: String,
        revision: u64,
        mode: DomainMode,
        header_span: TextSpan,
        explicit_header: bool,
        explicit_header_span: Option<TextSpan>,
        columns: Option<Vec<String>>,
        declared_width: Option<usize>,
        records: Vec<StrictRec>,
    }

    let mut domains: Vec<StrictDomain> = Vec::new();
    let mut ids: HashSet<String> = HashSet::new();
    let mut byte_offset = 0usize;

    for (i, &l) in lines.iter().enumerate() {
        let line_start = byte_offset;
        byte_offset += l.len() + 1;
        let n = i + 1;
        if l.is_empty() {
            continue;
        }
        if let Some((mode, id, rev)) = parse_strict_header(l) {
            if !ids.insert(id.clone()) {
                return Err(strict_fail(n, "duplicate domain ID"));
            }
            let columns = match mode {
                DomainMode::Table => None,
                DomainMode::FiveWOneH => Some(AXES_5W1H.iter().map(|s| (*s).to_string()).collect()),
                DomainMode::Graph => Some(COLUMNS_GRAPH.iter().map(|s| (*s).to_string()).collect()),
            };
            domains.push(StrictDomain {
                id,
                revision: rev,
                mode,
                header_span: TextSpan {
                    start_byte: line_start,
                    end_byte: line_start + l.len(),
                    line: n,
                    column: 1,
                },
                explicit_header: false,
                explicit_header_span: None,
                columns,
                declared_width: None,
                records: Vec::new(),
            });
            continue;
        }
        if is_malformed_strict_header(l) {
            return Err(strict_fail(n, "malformed domain header"));
        }
        let Some(active) = domains.last_mut() else {
            return Err(strict_fail(n, "body before domain"));
        };
        let mode = active.mode;

        if mode == DomainMode::Table && l.starts_with("\\#") {
            if !active.records.is_empty() || active.explicit_header {
                return Err(strict_fail(n, "header after records"));
            }
            let Some(raw_names) = l.strip_prefix("\\# ") else {
                return Err(strict_fail(n, "header needs labels"));
            };
            let vals = strict_fields(raw_names, n)?;
            let mut names = Vec::with_capacity(vals.len());
            let mut seen = HashSet::new();
            for v in vals {
                let StrictVal::Text(s) = v else {
                    return Err(strict_fail(n, "invalid column name"));
                };
                if s.is_empty() {
                    return Err(strict_fail(n, "invalid column name"));
                }
                if !seen.insert(s.clone()) {
                    return Err(strict_fail(n, "duplicate column name"));
                }
                names.push(s);
            }
            if let Some(dw) = active.declared_width {
                if names.len() != dw {
                    return Err(strict_fail(n, "column width mismatch"));
                }
            }
            active.columns = Some(names);
            active.explicit_header = true;
            active.explicit_header_span = Some(TextSpan {
                start_byte: line_start,
                end_byte: line_start + l.len(),
                line: n,
                column: 1,
            });
            continue;
        }

        if mode == DomainMode::Table && l.starts_with("\\%") {
            if !active.records.is_empty() || active.declared_width.is_some() {
                return Err(strict_fail(n, "width duplicate / too late"));
            }
            let w_str = &l[2..];
            if !is_positive_digits(w_str) {
                return Err(strict_fail(n, "invalid width"));
            }
            let count = w_str
                .parse::<usize>()
                .map_err(|_| strict_fail(n, "invalid width"))?;
            if active.explicit_header
                && active
                    .columns
                    .as_ref()
                    .is_some_and(|cols| cols.len() != count)
            {
                return Err(strict_fail(n, "column width mismatch"));
            }
            active.declared_width = Some(count);
            if !active.explicit_header {
                active.columns = Some((1..=count).map(|j| format!("column{j}")).collect());
            }
            continue;
        }

        if l.starts_with("  ") {
            return Err(strict_fail(
                n,
                "consecutive leading spaces prohibited; use \" N value\"",
            ));
        }

        if let Some(rest) = l.strip_prefix(' ') {
            if active.records.is_empty() {
                return Err(strict_fail(n, "continuation without record"));
            }
            let vals = strict_fields(rest, n)?;
            let (col, v) = if vals.len() == 2 {
                let sp = rest.find(' ').unwrap_or(rest.len());
                let idx_token = &rest[..sp];
                if !is_cont_index(idx_token) {
                    return Err(strict_fail(n, "invalid continuation column index"));
                }
                let parsed_col = idx_token
                    .parse::<usize>()
                    .map_err(|_| strict_fail(n, "invalid continuation column index"))?;
                if mode == DomainMode::Graph && parsed_col == 0 {
                    return Err(strict_fail(n, "cannot continue graph ID X0"));
                }
                (parsed_col, vals.into_iter().nth(1).expect("len 2"))
            } else if vals.len() == 1 {
                let default_col = if mode == DomainMode::Graph { 2 } else { 1 };
                (default_col, vals.into_iter().next().expect("len 1"))
            } else {
                return Err(strict_fail(
                    n,
                    "continuation must contain one field; escape inner spaces",
                ));
            };
            let v_str = match v {
                StrictVal::Text(s) => s,
                StrictVal::Null | StrictVal::Missing => {
                    return Err(strict_fail(n, "continuation must be text"));
                }
            };
            if mode == DomainMode::FiveWOneH && col >= 6 {
                return Err(strict_fail(n, "continuation outside 5w1h"));
            }
            if mode == DomainMode::Table {
                let width = active.columns.as_ref().map_or(0, Vec::len);
                if col >= width {
                    return Err(strict_fail(n, "continuation exceeds table width"));
                }
            }
            let rec = active.records.last_mut().expect("records non-empty");
            rec.span.end_byte = line_start + l.len();
            while rec.values.len() <= col {
                rec.values.push(StrictCell::ImplicitMissing);
            }
            match &mut rec.values[col] {
                StrictCell::Null => return Err(strict_fail(n, "cannot append to null")),
                StrictCell::ExplicitMissing => {
                    return Err(strict_fail(n, "cannot append to missing"));
                }
                slot @ StrictCell::ImplicitMissing => {
                    let g = if v_str == "~" {
                        vec!["\\~".to_string()]
                    } else if v_str.is_empty() {
                        vec!["\\e".to_string()]
                    } else {
                        vec![v_str.clone()]
                    };
                    *slot = StrictCell::Text {
                        value: v_str,
                        groups: g,
                    };
                }
                StrictCell::Text { value, groups } => {
                    value.push_str(&v_str);
                    if !v_str.is_empty() {
                        groups.push(v_str);
                    }
                }
            }
            continue;
        }

        let part_list = strict_tokenize(l, n)?;
        let mut vals = Vec::with_capacity(part_list.len());
        for parts in &part_list {
            vals.push(strict_value(parts, n)?);
        }

        let to_cell = |v: StrictVal| -> StrictCell {
            match v {
                StrictVal::Null => StrictCell::Null,
                StrictVal::Missing => StrictCell::ExplicitMissing,
                StrictVal::Text(s) => {
                    let groups = if s == "~" {
                        vec!["\\~".to_string()]
                    } else if s.is_empty() {
                        vec!["\\e".to_string()]
                    } else {
                        vec![s.clone()]
                    };
                    StrictCell::Text { value: s, groups }
                }
            }
        };

        let span = TextSpan {
            start_byte: line_start,
            end_byte: line_start + l.len(),
            line: n,
            column: 1,
        };

        match mode {
            DomainMode::Table => {
                if active.columns.is_none() {
                    active.columns = Some((1..=vals.len()).map(|j| format!("column{j}")).collect());
                }
                let width = active.columns.as_ref().expect("set above").len();
                if vals.len() > width {
                    return Err(strict_fail(n, "record exceeds width"));
                }
                let mut cells: Vec<StrictCell> = vals.into_iter().map(to_cell).collect();
                while cells.len() < width {
                    cells.push(StrictCell::ImplicitMissing);
                }
                active.records.push(StrictRec {
                    span,
                    node_id: None,
                    edge: None,
                    values: cells,
                });
            }
            DomainMode::FiveWOneH => {
                if vals.len() > 6 {
                    return Err(strict_fail(n, "5w1h exceeds six fields"));
                }
                let mut cells: Vec<StrictCell> = vals.into_iter().map(to_cell).collect();
                while cells.len() < 6 {
                    cells.push(StrictCell::ImplicitMissing);
                }
                active.records.push(StrictRec {
                    span,
                    node_id: None,
                    edge: None,
                    values: cells,
                });
            }
            DomainMode::Graph => {
                let Some(StrictVal::Text(token)) = vals.first().cloned() else {
                    return Err(strict_fail(n, "invalid graph ID"));
                };
                let first_parts = &part_list[0];
                let has_raw_op = first_parts
                    .iter()
                    .any(|p| matches!(p, StrictPart::Raw('>' | '<' | '=')));
                let (node_id, edge) = if has_raw_op {
                    let edge_spec = parse_strict_edge(&token, first_parts, n)?;
                    if vals.len() < 2 || matches!(vals[1], StrictVal::Missing) {
                        return Err(strict_fail(n, "edge name missing"));
                    }
                    (None, Some(edge_spec))
                } else {
                    if !is_strict_graph_ident(&token) {
                        return Err(strict_fail(n, "invalid node ID"));
                    }
                    if vals.len() < 2 || matches!(vals[1], StrictVal::Missing) {
                        return Err(strict_fail(n, "node name missing"));
                    }
                    (Some(token), None)
                };
                let cells: Vec<StrictCell> = vals.into_iter().map(to_cell).collect();
                active.records.push(StrictRec {
                    span,
                    node_id,
                    edge,
                    values: cells,
                });
            }
        }
    }

    for d in &domains {
        if d.mode != DomainMode::Graph {
            continue;
        }
        let mut nodes = HashSet::new();
        let mut edges = HashSet::new();
        for r in &d.records {
            if let Some(nid) = &r.node_id {
                if !nodes.insert(nid.clone()) {
                    return Err(strict_fail(0, format!("duplicate node ID: {nid}")));
                }
            } else if let Some(edge) = &r.edge {
                if !edges.insert(edge.id.clone()) {
                    return Err(strict_fail(0, format!("duplicate edge ID: {}", edge.id)));
                }
            }
        }
        if nodes.intersection(&edges).next().is_some() {
            return Err(strict_fail(0, "node edge ID overlap"));
        }
        for r in &d.records {
            if let Some(edge) = &r.edge {
                if !nodes.contains(&edge.from) || !nodes.contains(&edge.to) {
                    return Err(strict_fail(0, format!("dangling edge {}", edge.id)));
                }
            }
        }
    }

    let mut ast_domains = Vec::with_capacity(domains.len());
    let mut rendered_domains = Vec::with_capacity(domains.len());
    for d in domains {
        let cols = d.columns.unwrap_or_default();
        let mut ast_records = Vec::with_capacity(d.records.len());
        let mut rendered_rows = Vec::with_capacity(d.records.len());

        for rec in d.records {
            let mut ast_cells = Vec::with_capacity(rec.values.len());
            let mut r_cells = Vec::with_capacity(rec.values.len());
            let mut r_groups = Vec::with_capacity(rec.values.len());
            let mut extensions = Vec::new();

            for (col_idx, cell) in rec.values.into_iter().enumerate() {
                match cell {
                    StrictCell::Null => {
                        ast_cells.push(vec![SemanticChunk {
                            raw: "~".to_string(),
                            span: rec.span,
                            atoms: vec![AtomChunk {
                                text: "~".to_string(),
                                opaque_dict_index: None,
                            }],
                        }]);
                        r_cells.push("~".to_string());
                        r_groups.push(vec!["~".to_string()]);
                    }
                    StrictCell::ExplicitMissing | StrictCell::ImplicitMissing => {
                        ast_cells.push(Vec::new());
                        r_cells.push(String::new());
                        r_groups.push(Vec::new());
                    }
                    StrictCell::Text { value, groups } => {
                        let chunks = if value.is_empty() {
                            Vec::new()
                        } else {
                            vec![SemanticChunk {
                                raw: value.clone(),
                                span: rec.span,
                                atoms: vec![AtomChunk {
                                    text: value.clone(),
                                    opaque_dict_index: None,
                                }],
                            }]
                        };
                        ast_cells.push(chunks);
                        if d.mode == DomainMode::Graph && col_idx >= 3 && !value.is_empty() {
                            extensions.push((format!("X{col_idx}"), value.clone()));
                        }
                        let norm_groups = if value == "~" {
                            vec!["\\~".to_string()]
                        } else if value.is_empty() {
                            Vec::new()
                        } else {
                            groups
                        };
                        r_cells.push(value);
                        r_groups.push(norm_groups);
                    }
                }
            }

            if d.mode == DomainMode::FiveWOneH {
                while r_cells.len() < 6 {
                    r_cells.push(String::new());
                }
                while r_groups.len() < 6 {
                    r_groups.push(Vec::new());
                }
            }

            ast_records.push(NemaRecord {
                span: rec.span,
                cells: ast_cells,
                explicit_tail_empty: false,
                graph_node_id: rec.node_id.clone(),
                graph_edge: rec.edge.clone(),
            });
            rendered_rows.push(RenderedRow {
                cells: r_cells,
                groups: r_groups,
                tail_empty: false,
                node_id: rec.node_id,
                edge: rec.edge,
                extensions,
                line: rec.span.line,
            });
        }

        ast_domains.push(NemaDomain {
            id: d.id.clone(),
            revision: d.revision,
            mode: d.mode,
            header_span: d.header_span,
            explicit_header: d.explicit_header,
            explicit_header_span: d.explicit_header_span,
            columns: cols.clone(),
            records: ast_records,
            is_v12: false,
        });
        rendered_domains.push(RenderedDomain {
            id: d.id,
            revision: d.revision,
            mode: d.mode,
            columns: cols,
            explicit_header: d.explicit_header,
            rows: rendered_rows,
        });
    }

    Ok((
        NemaDocument {
            span: TextSpan {
                start_byte: 0,
                end_byte: source.len(),
                line: 1,
                column: 1,
            },
            dictionary: Vec::new(),
            dictionary_spans: Vec::new(),
            domains: ast_domains,
        },
        RenderedDocument {
            dictionary: Vec::new(),
            domains: rendered_domains,
        },
    ))
}

/// 按照官网最新 `.NEMA` Strict Candidate 规范 (`strict-parser.js`) 解析文档 AST
pub fn parse_strict(source: &str) -> Result<NemaDocument, NemaError> {
    parse_strict_core(source).map(|(ast, _)| ast)
}

/// 按照官网最新 `.NEMA` Strict Candidate 规范 (`strict-parser.js`) 直接解析并渲染视图
pub fn render_strict(source: &str) -> Result<RenderedDocument, NemaError> {
    parse_strict_core(source).map(|(_, rendered)| rendered)
}

/// 解析 `.nema` 权威源为 `NemaDocument`
pub fn parse_source(source: &str) -> Result<NemaDocument, NemaError> {
    let lines = validate_source(source)?;
    let mut line_byte_offsets = Vec::with_capacity(lines.len());
    let mut offset = 0usize;
    for l in &lines {
        line_byte_offsets.push(offset);
        offset += l.len() + 1;
    }

    let mut doc = NemaDocument {
        span: TextSpan {
            start_byte: 0,
            end_byte: source.len(),
            line: 1,
            column: 1,
        },
        dictionary: Vec::new(),
        dictionary_spans: Vec::new(),
        domains: Vec::new(),
    };

    let mut i = 0usize;
    if let Some(n) = parse_dict_header(lines[0]) {
        i = 1;
        if n > MAX_DICT_ENTRIES || i + n > lines.len() {
            return Err(NemaError::BadDictionaryManifestSize);
        }
        for row_idx in i..(i + n) {
            let raw_entry = lines[row_idx];
            let unescaped = unescape_opaque(raw_entry);
            if unescaped.is_empty() || unescaped.contains('\t') {
                return Err(NemaError::InvalidDictionaryEntry);
            }
            doc.dictionary.push(unescaped);
            doc.dictionary_spans.push(TextSpan {
                start_byte: line_byte_offsets[row_idx],
                end_byte: line_byte_offsets[row_idx] + raw_entry.len(),
                line: row_idx + 1,
                column: 1,
            });
        }
        i += n;
    } else if lines[0].trim_start() == "\\!NEMAT1" {
        // 官网候选传输区块头: \!NEMAT1 / $N / @0 Value ... / \!DATA
        if lines.len() < 3 {
            return Err(NemaError::BadDictionaryManifestSize);
        }
        let n = lines[1]
            .trim_start()
            .strip_prefix('$')
            .filter(|s| is_all_digits(s))
            .and_then(|s| s.parse::<usize>().ok())
            .ok_or(NemaError::BadDictionaryManifestSize)?;
        if n > MAX_DICT_ENTRIES || 2 + n >= lines.len() {
            return Err(NemaError::BadDictionaryManifestSize);
        }
        for k in 0..n {
            let row_idx = 2 + k;
            let raw_row = lines[row_idx].trim_start();
            let rest = raw_row
                .strip_prefix('@')
                .ok_or(NemaError::InvalidDictionaryEntry)?;
            let (idx_str, raw_val) = rest
                .split_once(' ')
                .ok_or(NemaError::InvalidDictionaryEntry)?;
            let parsed_idx = idx_str
                .parse::<usize>()
                .map_err(|_| NemaError::InvalidDictionaryEntry)?;
            if parsed_idx != k {
                return Err(NemaError::InvalidDictionaryEntry);
            }
            let unescaped = unescape_opaque(raw_val);
            if unescaped.is_empty() || unescaped.contains('\t') {
                return Err(NemaError::InvalidDictionaryEntry);
            }
            doc.dictionary.push(unescaped);
            doc.dictionary_spans.push(TextSpan {
                start_byte: line_byte_offsets[row_idx],
                end_byte: line_byte_offsets[row_idx] + lines[row_idx].len(),
                line: row_idx + 1,
                column: 1,
            });
        }
        if lines[2 + n].trim_start() != "\\!DATA" {
            return Err(NemaError::BadDictionaryManifestSize);
        }
        i = 3 + n;
    }

    let mut seen_domain_ids = HashSet::new();
    let mut pending_v12_mode = false;
    let mut current_domain_has_raw_rows = false;

    for (row_idx, &line) in lines.iter().enumerate().skip(i) {
        let line_start = line_byte_offsets[row_idx];
        let line_no = row_idx + 1;
        if let Some((is_graph, id, revision)) = parse_domain_header_v12(line) {
            if let Some(prev) = doc.domains.last() {
                let prev_is_declared_empty_table =
                    prev.mode == DomainMode::Table && !prev.columns.is_empty();
                if !current_domain_has_raw_rows
                    || (prev.records.is_empty() && !prev_is_declared_empty_table)
                {
                    return Err(NemaError::NoContent);
                }
            }
            if !seen_domain_ids.insert(id.clone()) {
                return Err(NemaError::DuplicateDomainId);
            }
            let mode = if is_graph {
                DomainMode::Graph
            } else {
                DomainMode::FiveWOneH
            };
            let columns = if is_graph {
                COLUMNS_GRAPH.iter().map(|s| (*s).to_string()).collect()
            } else {
                AXES_5W1H.iter().map(|s| (*s).to_string()).collect()
            };
            pending_v12_mode = !is_graph;
            current_domain_has_raw_rows = false;
            doc.domains.push(NemaDomain {
                id,
                revision,
                mode,
                header_span: TextSpan {
                    start_byte: line_start,
                    end_byte: line_start + line.len(),
                    line: line_no,
                    column: 1,
                },
                explicit_header: false,
                explicit_header_span: None,
                columns,
                records: Vec::new(),
                is_v12: true,
            });
            continue;
        }
        if let Some((mode, id, revision)) = parse_domain_header(line) {
            if let Some(prev) = doc.domains.last() {
                let prev_is_declared_empty_table =
                    prev.mode == DomainMode::Table && !prev.columns.is_empty();
                if !current_domain_has_raw_rows
                    || (prev.records.is_empty() && !prev_is_declared_empty_table)
                {
                    return Err(NemaError::NoContent);
                }
            }
            if !seen_domain_ids.insert(id.clone()) {
                return Err(NemaError::DuplicateDomainId);
            }
            let columns = match mode {
                DomainMode::Table => Vec::new(),
                DomainMode::FiveWOneH => AXES_5W1H.iter().map(|s| (*s).to_string()).collect(),
                DomainMode::Graph => COLUMNS_GRAPH.iter().map(|s| (*s).to_string()).collect(),
            };
            pending_v12_mode = false;
            current_domain_has_raw_rows = false;
            doc.domains.push(NemaDomain {
                id,
                revision,
                mode,
                header_span: TextSpan {
                    start_byte: line_start,
                    end_byte: line_start + line.len(),
                    line: line_no,
                    column: 1,
                },
                explicit_header: false,
                explicit_header_span: None,
                columns,
                records: Vec::new(),
                is_v12: false,
            });
            continue;
        }

        let Some(current) = doc.domains.last_mut() else {
            return Err(NemaError::BodyBeforeDomain);
        };

        if current.is_v12 {
            if pending_v12_mode {
                pending_v12_mode = false;
                if let Some(w_str) = line
                    .strip_prefix("\\%")
                    .or_else(|| line.strip_prefix(" \\%"))
                {
                    if is_positive_digits(w_str) {
                        if let Ok(n_cols) = w_str.parse::<usize>() {
                            current.mode = DomainMode::Table;
                            current.columns = (1..=n_cols).map(|k| format!("column{k}")).collect();
                            pending_v12_mode = true;
                            current_domain_has_raw_rows = true;
                            continue;
                        }
                    }
                }
                let candidate_header = if let Some(rest) = line.strip_prefix('\t') {
                    if rest.starts_with('\t') {
                        return Err(NemaError::TabInBody);
                    }
                    Some(rest)
                } else if let Some(rest) = line.strip_prefix(' ') {
                    if let Some(stripped) = rest.strip_prefix("\\# ") {
                        Some(stripped)
                    } else if let Some(stripped) = rest.strip_prefix('\\') {
                        Some(stripped.trim_start())
                    } else {
                        Some(rest)
                    }
                } else if let Some(stripped) = line.strip_prefix("\\# ") {
                    Some(stripped)
                } else {
                    line.strip_prefix('\\').map(str::trim_start)
                };

                let mut is_table_header = false;
                if let Some(raw_header) = candidate_header {
                    if !raw_header.is_empty() {
                        if let Ok(labels) = split_x_v12(raw_header) {
                            if !labels.is_empty()
                                && labels
                                    .iter()
                                    .all(|v| !v.is_empty() && is_valid_table_header_ident(v))
                            {
                                let unique: HashSet<&str> =
                                    labels.iter().map(String::as_str).collect();
                                if unique.len() == labels.len() {
                                    current.mode = DomainMode::Table;
                                    current.columns = labels;
                                    current.explicit_header = true;
                                    current.explicit_header_span = Some(TextSpan {
                                        start_byte: line_start,
                                        end_byte: line_start + line.len(),
                                        line: line_no,
                                        column: 1,
                                    });
                                    current_domain_has_raw_rows = true;
                                    is_table_header = true;
                                }
                            }
                        }
                    }
                }
                if is_table_header {
                    continue;
                } else if current.columns.is_empty() || current.mode != DomainMode::Table {
                    current.mode = DomainMode::FiveWOneH;
                }
            }

            match current.mode {
                DomainMode::FiveWOneH => {
                    let cells = split_x_v12(line)?;
                    if cells.len() > 6 {
                        return Err(NemaError::TooMany5W1HFields);
                    }
                    if cells.iter().all(|c| c.is_empty()) {
                        return Err(NemaError::BlankContinuation);
                    }
                    if current.records.is_empty() {
                        current.records.push(NemaRecord {
                            span: TextSpan {
                                start_byte: line_start,
                                end_byte: line_start + line.len(),
                                line: line_no,
                                column: 1,
                            },
                            cells: vec![Vec::new(); 6],
                            explicit_tail_empty: false,
                            graph_node_id: None,
                            graph_edge: None,
                        });
                    }
                    let rec = current.records.last_mut().expect("record exists");
                    append_v12_cols(rec, cells, &doc.dictionary, line_start, line.len(), line_no)?;
                }
                DomainMode::Table => {
                    let cells = split_x_v12(line)?;
                    if cells.len() > current.columns.len() {
                        return Err(NemaError::TooMany5W1HFields);
                    }
                    if cells.iter().all(|c| c.is_empty()) {
                        return Err(NemaError::BlankContinuation);
                    }
                    if !cells[0].is_empty() {
                        current.records.push(NemaRecord {
                            span: TextSpan {
                                start_byte: line_start,
                                end_byte: line_start + line.len(),
                                line: line_no,
                                column: 1,
                            },
                            cells: vec![Vec::new(); current.columns.len()],
                            explicit_tail_empty: false,
                            graph_node_id: None,
                            graph_edge: None,
                        });
                    } else if current.records.is_empty() {
                        return Err(NemaError::ContinuationBeforeFirstRecord);
                    }
                    let rec = current.records.last_mut().expect("record exists");
                    append_v12_cols(rec, cells, &doc.dictionary, line_start, line.len(), line_no)?;
                }
                DomainMode::Graph => {
                    if line.starts_with("  ") {
                        if current.records.is_empty() {
                            return Err(NemaError::ContinuationBeforeFirstRecord);
                        }
                        let cells = split_x_v12(line)?;
                        if cells.len() < 3
                            || !cells[0].is_empty()
                            || !cells[1].is_empty()
                            || cells[2..].iter().all(|c| c.is_empty())
                        {
                            return Err(NemaError::BlankContinuation);
                        }
                        let rec = current.records.last_mut().expect("record exists");
                        append_v12_cols(
                            rec,
                            cells,
                            &doc.dictionary,
                            line_start,
                            line.len(),
                            line_no,
                        )?;
                    } else if let Some(after_space) = line.strip_prefix(' ') {
                        let cells = split_x_v12(after_space)?;
                        if !current.records.is_empty()
                            && (cells.len() == 1
                                || !is_valid_graph_ident(&cells[0])
                                || current
                                    .records
                                    .iter()
                                    .any(|r| r.graph_node_id.as_deref() == Some(cells[0].as_str())))
                        {
                            let mut cont_cells = vec![String::new()];
                            cont_cells.extend(cells);
                            let rec = current.records.last_mut().expect("checked non-empty");
                            append_v12_cols(
                                rec,
                                cont_cells,
                                &doc.dictionary,
                                line_start,
                                line.len(),
                                line_no,
                            )?;
                        } else {
                            if cells.is_empty() || !is_valid_graph_ident(&cells[0]) {
                                return Err(NemaError::InvalidNodeId(
                                    cells.first().cloned().unwrap_or_default(),
                                ));
                            }
                            let mut rec = NemaRecord {
                                span: TextSpan {
                                    start_byte: line_start,
                                    end_byte: line_start + line.len(),
                                    line: line_no,
                                    column: 2,
                                },
                                cells: Vec::with_capacity(cells.len()),
                                explicit_tail_empty: false,
                                graph_node_id: Some(cells[0].clone()),
                                graph_edge: None,
                            };
                            append_v12_cols(
                                &mut rec,
                                cells,
                                &doc.dictionary,
                                line_start + 1,
                                after_space.len(),
                                line_no,
                            )?;
                            current.records.push(rec);
                        }
                    } else {
                        let cells = split_x_v12(line)?;
                        if cells.is_empty() {
                            return Err(NemaError::BlankContinuation);
                        }
                        let token = &cells[0];
                        if token.contains(['>', '<', '=']) {
                            let Some(edge_spec) = parse_edge_spec(token) else {
                                return Err(NemaError::InvalidEdgeExpression(token.clone()));
                            };
                            let mut rec = NemaRecord {
                                span: TextSpan {
                                    start_byte: line_start,
                                    end_byte: line_start + line.len(),
                                    line: line_no,
                                    column: 1,
                                },
                                cells: Vec::with_capacity(cells.len()),
                                explicit_tail_empty: false,
                                graph_node_id: None,
                                graph_edge: Some(edge_spec),
                            };
                            append_v12_cols(
                                &mut rec,
                                cells,
                                &doc.dictionary,
                                line_start,
                                line.len(),
                                line_no,
                            )?;
                            current.records.push(rec);
                        } else {
                            if !is_valid_graph_ident(token) {
                                return Err(NemaError::InvalidNodeId(token.clone()));
                            }
                            let mut rec = NemaRecord {
                                span: TextSpan {
                                    start_byte: line_start,
                                    end_byte: line_start + line.len(),
                                    line: line_no,
                                    column: 1,
                                },
                                cells: Vec::with_capacity(cells.len()),
                                explicit_tail_empty: false,
                                graph_node_id: Some(token.clone()),
                                graph_edge: None,
                            };
                            append_v12_cols(
                                &mut rec,
                                cells,
                                &doc.dictionary,
                                line_start,
                                line.len(),
                                line_no,
                            )?;
                            current.records.push(rec);
                        }
                    }
                }
            }
            current_domain_has_raw_rows = true;
            continue;
        }

        if line.contains('\t') {
            return Err(NemaError::TabInBody);
        }

        // Table 首行精确声明宽度 (\%N) 与表头 (\# Col1 Col2) 判断
        if current.mode == DomainMode::Table && current.records.is_empty() {
            if let Some(w_str) = line
                .strip_prefix("\\%")
                .or_else(|| line.strip_prefix(" \\%"))
            {
                if is_positive_digits(w_str) {
                    if let Ok(n_cols) = w_str.parse::<usize>() {
                        if current.explicit_header && current.columns.len() != n_cols {
                            return Err(strict_fail(line_no, "column width mismatch"));
                        }
                        if current.columns.is_empty() {
                            current.columns = (1..=n_cols).map(|k| format!("column{k}")).collect();
                        }
                        current_domain_has_raw_rows = true;
                        continue;
                    }
                }
            }
            if !current.explicit_header {
                if let Some(raw_names) = line
                    .strip_prefix("\\# ")
                    .or_else(|| line.strip_prefix(" \\# "))
                {
                    let tokens = split_unescaped_spaces(raw_names)?;
                    let mut names = Vec::with_capacity(tokens.len());
                    for tok in tokens {
                        if matches!(tok, "~" | "\\_" | "\\e") {
                            return Err(strict_fail(line_no, "invalid column name"));
                        }
                        let decoded = unescape_v12(tok)?;
                        if decoded.is_empty() {
                            return Err(strict_fail(line_no, "invalid column name"));
                        }
                        names.push(decoded);
                    }
                    let unique: HashSet<&str> = names.iter().map(String::as_str).collect();
                    if unique.len() != names.len() {
                        return Err(NemaError::DuplicateColumnHeader);
                    }
                    if !current.columns.is_empty() && names.len() != current.columns.len() {
                        return Err(strict_fail(line_no, "column width mismatch"));
                    }
                    current.columns = names;
                    current.explicit_header = true;
                    current.explicit_header_span = Some(TextSpan {
                        start_byte: line_start,
                        end_byte: line_start + line.len(),
                        line: line_no,
                        column: 1,
                    });
                    current_domain_has_raw_rows = true;
                    continue;
                }
                let raw_hdr = if let Some(stripped) = line.strip_prefix("\\ ") {
                    Some(stripped)
                } else if let Some(stripped) = line.strip_prefix(" \\ ") {
                    Some(stripped)
                } else {
                    line.strip_prefix('\t')
                };
                if let Some(raw_names) = raw_hdr {
                    let names = split_unescaped_spaces(raw_names)?;
                    if !names.is_empty() && names.iter().all(|x| is_valid_table_header_ident(x)) {
                        let unique: HashSet<&str> = names.iter().copied().collect();
                        if unique.len() != names.len() {
                            return Err(NemaError::DuplicateColumnHeader);
                        }
                        if !current.columns.is_empty() && names.len() != current.columns.len() {
                            return Err(NemaError::TooMany5W1HFields);
                        }
                        current.columns = names.into_iter().map(str::to_string).collect();
                        current.explicit_header = true;
                        current.explicit_header_span = Some(TextSpan {
                            start_byte: line_start,
                            end_byte: line_start + line.len(),
                            line: line_no,
                            column: 1,
                        });
                        current_domain_has_raw_rows = true;
                        continue;
                    }
                }
            }
        }

        if line.starts_with(' ') {
            let n = line.len() - line.trim_start_matches(' ').len();
            let part = &line[n..];
            if part.is_empty() {
                return Err(NemaError::BlankContinuation);
            }
            // Graph 域支持 1 空格节点声明 (` A LoginAPI ...`)
            if current.mode == DomainMode::Graph && n == 1 {
                if let Ok(tokens) = split_unescaped_spaces(part) {
                    if tokens.len() >= 2
                        && !tokens[0].contains(['>', '<', '='])
                        && is_valid_graph_ident(tokens[0])
                        && !current
                            .records
                            .iter()
                            .any(|r| r.graph_node_id.as_deref() == Some(tokens[0]))
                    {
                        let mut rec = NemaRecord {
                            span: TextSpan {
                                start_byte: line_start,
                                end_byte: line_start + line.len(),
                                line: line_no,
                                column: 2,
                            },
                            cells: Vec::with_capacity(tokens.len()),
                            explicit_tail_empty: false,
                            graph_node_id: Some(tokens[0].to_string()),
                            graph_edge: None,
                        };
                        let mut col_byte = line_start + n;
                        let mut col_char = n + 1;
                        for (j, &c) in tokens.iter().enumerate() {
                            while rec.cells.len() <= j {
                                rec.cells.push(Vec::new());
                            }
                            if c != "\\_" && c != "\\e" {
                                let atoms = mixed_atoms(c, &doc.dictionary)?;
                                rec.add_chunk(
                                    j,
                                    SemanticChunk {
                                        raw: c.to_string(),
                                        span: TextSpan {
                                            start_byte: col_byte,
                                            end_byte: col_byte + c.len(),
                                            line: line_no,
                                            column: col_char,
                                        },
                                        atoms,
                                    },
                                );
                            }
                            col_byte += c.len() + 1;
                            col_char += c.chars().count() + 1;
                        }
                        current.records.push(rec);
                        current_domain_has_raw_rows = true;
                        continue;
                    }
                }
            }
            if current.records.is_empty() {
                return Err(NemaError::ContinuationBeforeFirstRecord);
            }
            let mut explicit_indexed: Option<(usize, &str)> = None;
            if n == 1 {
                if let Ok(tokens) = split_unescaped_spaces(part) {
                    if tokens.len() == 2 && is_cont_index(tokens[0]) {
                        let col = tokens[0].parse::<usize>().map_err(|_| {
                            strict_fail(line_no, "invalid continuation column index")
                        })?;
                        explicit_indexed = Some((col, tokens[1]));
                    }
                }
            }
            if let Some((target_col, val_tok)) = explicit_indexed {
                if current.mode == DomainMode::Graph && target_col == 0 {
                    return Err(strict_fail(line_no, "cannot continue graph ID X0"));
                }
                if val_tok == "~" || val_tok == "\\_" {
                    return Err(strict_fail(line_no, "continuation must be text"));
                }
                if current.mode == DomainMode::FiveWOneH && target_col >= 6 {
                    return Err(NemaError::TooMany5W1HFields);
                }
                if current.mode == DomainMode::Table
                    && !current.columns.is_empty()
                    && target_col >= current.columns.len()
                {
                    return Err(strict_fail(line_no, "continuation exceeds table width"));
                }
                let rec = current.records.last_mut().expect("checked non-empty");
                if rec
                    .cells
                    .get(target_col)
                    .is_some_and(|v| v.iter().any(|c| c.raw == "~"))
                {
                    return Err(strict_fail(line_no, "cannot append to null"));
                }
                rec.span.end_byte = line_start + line.len();
                if val_tok == "\\e" {
                    while rec.cells.len() <= target_col {
                        rec.cells.push(Vec::new());
                    }
                } else {
                    let atoms = mixed_atoms(val_tok, &doc.dictionary)?;
                    let chunk = SemanticChunk {
                        raw: val_tok.to_string(),
                        span: TextSpan {
                            start_byte: line_start + line.len() - val_tok.len(),
                            end_byte: line_start + line.len(),
                            line: line_no,
                            column: n + 1,
                        },
                        atoms,
                    };
                    rec.add_chunk(target_col, chunk);
                }
            } else {
                let target_col = if current.mode == DomainMode::Graph && n == 1 {
                    1
                } else {
                    n - 1
                };
                let rec = current.records.last_mut().expect("checked non-empty");
                rec.span.end_byte = line_start + line.len();
                if part == "\\_" || part == "\\e" {
                    while rec.cells.len() <= target_col {
                        rec.cells.push(Vec::new());
                    }
                } else {
                    let atoms = mixed_atoms(part, &doc.dictionary)?;
                    let chunk = SemanticChunk {
                        raw: part.to_string(),
                        span: TextSpan {
                            start_byte: line_start + n,
                            end_byte: line_start + line.len(),
                            line: line_no,
                            column: n + 1,
                        },
                        atoms,
                    };
                    rec.add_chunk(target_col, chunk);
                }
            }
        } else {
            let empty_sentinel = line.ends_with(" .") && !line.ends_with(" \\.");
            let effective = if empty_sentinel {
                &line[..line.len() - 2]
            } else {
                line
            };
            let cells = split_unescaped_spaces(effective)?;
            if cells.is_empty() || cells.iter().any(|x| x.is_empty()) {
                return Err(NemaError::MultipleOrLeadingSpaces);
            }
            let mut rec = NemaRecord {
                span: TextSpan {
                    start_byte: line_start,
                    end_byte: line_start + line.len(),
                    line: line_no,
                    column: 1,
                },
                cells: Vec::with_capacity(cells.len()),
                explicit_tail_empty: empty_sentinel,
                graph_node_id: None,
                graph_edge: None,
            };
            let mut col_byte = line_start;
            let mut col_char = 1usize;
            for (j, &c) in cells.iter().enumerate() {
                while rec.cells.len() <= j {
                    rec.cells.push(Vec::new());
                }
                if c != "\\_" && c != "\\e" {
                    let atoms = mixed_atoms(c, &doc.dictionary)?;
                    rec.add_chunk(
                        j,
                        SemanticChunk {
                            raw: c.to_string(),
                            span: TextSpan {
                                start_byte: col_byte,
                                end_byte: col_byte + c.len(),
                                line: line_no,
                                column: col_char,
                            },
                            atoms,
                        },
                    );
                }
                col_byte += c.len() + 1;
                col_char += c.chars().count() + 1;
            }
            current.records.push(rec);
        }
        current_domain_has_raw_rows = true;

        let last_rec = current.records.last_mut().expect("record exists");
        if current.mode == DomainMode::Table {
            while current.columns.len() < last_rec.cells.len() {
                current
                    .columns
                    .push(format!("column{}", current.columns.len() + 1));
            }
        } else if current.mode == DomainMode::Graph && !line.starts_with(' ') {
            let e = last_rec
                .cells
                .first()
                .and_then(|c| c.first())
                .map(|c| c.raw.clone())
                .unwrap_or_default();
            if e.is_empty() {
                return Err(NemaError::InvalidNodeId(e));
            }
            if e.contains(['>', '<', '=']) {
                let Some(edge_spec) = parse_edge_spec(&e) else {
                    return Err(NemaError::InvalidEdgeExpression(e));
                };
                last_rec.graph_edge = Some(edge_spec);
            } else if !is_valid_graph_ident(&e) {
                return Err(NemaError::InvalidNodeId(e));
            } else {
                last_rec.graph_node_id = Some(e);
            }
        }
        if current.mode == DomainMode::FiveWOneH && last_rec.cells.len() > 6 {
            return Err(NemaError::TooMany5W1HFields);
        }
    }

    if doc.domains.is_empty() {
        return Err(NemaError::NoDomains);
    }
    for domain in &doc.domains {
        let is_declared_empty_table =
            domain.mode == DomainMode::Table && !domain.columns.is_empty();
        if domain.records.is_empty() && !is_declared_empty_table {
            return Err(NemaError::NoContent);
        }
    }

    // Graph 域完整性校验：节点与边 ID 不可重复且不相交，边引用端点必须已声明为节点
    for domain in &doc.domains {
        if domain.mode != DomainMode::Graph {
            continue;
        }
        let mut nodes = HashSet::new();
        let mut edges = HashSet::new();
        let mut endpoints = Vec::new();
        for rec in &domain.records {
            if let Some(edge) = &rec.graph_edge {
                if !edges.insert(edge.id.clone()) {
                    return Err(NemaError::DuplicateEdgeId(edge.id.clone()));
                }
                let raw = &rec.cells[0][0].raw;
                let op_idx1 = raw.find(['>', '<', '=']).unwrap_or(0);
                let after1 = &raw[op_idx1 + 1..];
                let op_idx2 = after1.find(['>', '<', '=']).unwrap_or(0);
                let src_decl = raw[..op_idx1].to_string();
                let dst_decl = after1[op_idx2 + 1..].to_string();
                endpoints.push((src_decl, dst_decl));
            } else if let Some(node_id) = &rec.graph_node_id {
                if !nodes.insert(node_id.clone()) {
                    return Err(NemaError::DuplicateNodeId(node_id.clone()));
                }
            }
        }
        if let Some(collision) = nodes.intersection(&edges).next() {
            return Err(NemaError::DuplicateEdgeId(collision.clone()));
        }
        for (src, dst) in endpoints {
            if !nodes.contains(&src) || !nodes.contains(&dst) {
                return Err(NemaError::UndefinedGraphNode(src, dst));
            }
        }
    }

    Ok(doc)
}

/// 渲染 `NemaDocument` 为结构化视图（兼容 `.NEMA v1.2` `native.py::view` 与旧版 `core.py::render`）
pub fn render_document(doc: &NemaDocument) -> Result<RenderedDocument, NemaError> {
    let mut out = Vec::with_capacity(doc.domains.len());
    for d in &doc.domains {
        let mut rows = Vec::with_capacity(d.records.len());
        for rec in &d.records {
            let mut vals = Vec::with_capacity(rec.cells.len().max(6));
            let mut extensions = Vec::new();
            for (col_idx, col) in rec.cells.iter().enumerate() {
                let raw_parts: Vec<&str> = col.iter().map(|c| c.raw.as_str()).collect();
                let rendered_cell = if d.is_v12 {
                    let exact_name = d.mode == DomainMode::Graph && (col_idx <= 1 || col_idx >= 3);
                    join_intents(&raw_parts, "auto", exact_name)
                } else if d.mode == DomainMode::Graph && col_idx >= 3 {
                    join_intents(&raw_parts, "auto", true)
                } else {
                    join_cell(&raw_parts, &doc.dictionary)?
                };
                if d.mode == DomainMode::Graph && col_idx >= 3 && !rendered_cell.is_empty() {
                    extensions.push((format!("X{col_idx}"), rendered_cell.clone()));
                }
                vals.push(rendered_cell);
            }
            if d.mode == DomainMode::FiveWOneH {
                while vals.len() < 6 {
                    vals.push(String::new());
                }
            } else if d.mode == DomainMode::Table && d.is_v12 {
                while vals.len() < d.columns.len() {
                    vals.push(String::new());
                }
            }
            rows.push(RenderedRow {
                cells: vals,
                groups: rec.raw_groups(),
                tail_empty: rec.explicit_tail_empty,
                node_id: rec.graph_node_id.clone(),
                edge: rec.graph_edge.clone(),
                extensions,
                line: rec.span.line,
            });
        }
        out.push(RenderedDomain {
            id: d.id.clone(),
            revision: d.revision,
            mode: d.mode,
            columns: d.columns.clone(),
            explicit_header: d.explicit_header,
            rows,
        });
    }
    Ok(RenderedDocument {
        dictionary: doc.dictionary.clone(),
        domains: out,
    })
}

/// 直接从源码解析并渲染视图
pub fn render_source(source: &str) -> Result<RenderedDocument, NemaError> {
    let doc = parse_source(source)?;
    render_document(&doc)
}

// ============================================================================
// 2. `.nemat` 无损传输编解码器 (`transport.py` 1:1 对应)
// ============================================================================

fn is_cjk_atom_char(c: char) -> bool {
    ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{3400}'..='\u{9fff}').contains(&c)
}

/// 扫描单行中的候选原子：`[A-Za-z][A-Za-z0-9_]{3,}|[\u3400-\u9fff\u3040-\u30ff]{4,}`
pub fn extract_atoms_from_row(row: &str) -> Vec<String> {
    let chars: Vec<char> = row.chars().collect();
    let mut atoms = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            if j - i >= 4 {
                atoms.push(chars[i..j].iter().collect());
            }
            i = j;
        } else if is_cjk_atom_char(c) {
            let mut j = i + 1;
            while j < chars.len() && is_cjk_atom_char(chars[j]) {
                j += 1;
            }
            if j - i >= 4 {
                atoms.push(chars[i..j].iter().collect());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    atoms
}

/// 提取候选字典条目（与 `transport.py::_candidate_dict` 1:1 对齐）
pub fn candidate_dictionary_details(body_rows: &[&str], max_entries: usize) -> Vec<DictCandidate> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for &row in body_rows {
        if row.starts_with('\t') {
            continue;
        }
        for atom in extract_atoms_from_row(row) {
            *counts.entry(atom).or_insert(0) += 1;
        }
    }
    let mut choices = Vec::new();
    for (word, num) in counts {
        if num < 3 {
            continue;
        }
        let lower = word.to_ascii_lowercase();
        if matches!(lower.as_str(), "password" | "secret" | "credential") {
            continue;
        }
        let char_len = word.chars().count();
        if char_len > 48 {
            continue;
        }
        let gain = (num as isize - 1) * (char_len as isize) - (num as isize) * 3 - 5;
        if gain > 3 {
            choices.push(DictCandidate {
                word,
                count: num,
                gain,
            });
        }
    }
    choices.sort_by(|a, b| b.gain.cmp(&a.gain).then_with(|| a.word.cmp(&b.word)));
    choices.truncate(max_entries);
    choices
}

pub fn candidate_dictionary(body_rows: &[&str], max_entries: usize) -> Vec<String> {
    candidate_dictionary_details(body_rows, max_entries)
        .into_iter()
        .map(|c| c.word)
        .collect()
}

fn esc_transport(s: &str) -> String {
    s.replace('\\', "\\\\").replace('^', "\\^")
}

fn esc_transport_char(ch: char) -> &'static str {
    match ch {
        '\\' => "\\\\",
        '^' => "\\^",
        _ => "",
    }
}

/// 单行传输编码（与 `transport.py::_encode_row` 1:1 对齐）
pub fn encode_row(row: &str, words: &[String], rle: bool) -> String {
    // 按 `(-len(word), index)` 排序，其中 Python `len(word)` 是字符数
    let mut order: Vec<(usize, &str, usize)> = words
        .iter()
        .enumerate()
        .map(|(idx, w)| (idx, w.as_str(), w.chars().count()))
        .collect();
    order.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));

    let mut out = String::with_capacity(row.len());
    let mut i = 0usize; // byte index into `row`
    while i < row.len() {
        let tail = &row[i..];
        let mut chosen = None;
        for &(idx, w, _) in &order {
            if tail.starts_with(w) {
                chosen = Some((idx, w));
                break;
            }
        }
        if let Some((idx, w)) = chosen {
            out.push('^');
            out.push_str(&idx.to_string());
            i += w.len();
            continue;
        }

        let ch = tail.chars().next().expect("i < row.len()");
        let ch_len = ch.len_utf8();
        if rle && ch != ' ' {
            let mut rep = 1usize;
            let mut j = i + ch_len;
            while j < row.len() {
                let next_ch = row[j..].chars().next().expect("j < row.len()");
                if next_ch == ch {
                    rep += 1;
                    j += ch_len;
                } else {
                    break;
                }
            }
            if rep >= MIN_RLE_COUNT {
                let enc = if ch.is_ascii_digit() || ch == '\\' {
                    format!("\\{}\\{}", rep, ch)
                } else {
                    format!("\\{}{}", rep, ch)
                };
                let esc_char_len = if matches!(ch, '\\' | '^') { 2 } else { 1 };
                if enc.chars().count() < esc_char_len * rep {
                    out.push_str(&enc);
                    i = j;
                    continue;
                }
            }
        }
        let esc = esc_transport_char(ch);
        if !esc.is_empty() {
            out.push_str(esc);
        } else {
            out.push(ch);
        }
        i += ch_len;
    }
    out
}

/// 单行传输解码（与 `transport.py::_decode_row` 1:1 对齐）
pub fn decode_row(row: &str, words: &[String]) -> Result<String, NemaError> {
    let chars: Vec<char> = row.chars().collect();
    let mut out = String::with_capacity(row.len());
    let mut i = 0usize;
    let mut bytes_so_far = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '^' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j == i + 1 {
                return Err(NemaError::InvalidTransportDictRef);
            }
            let num_str: String = chars[i + 1..j].iter().collect();
            let ix: usize = num_str
                .parse()
                .map_err(|_| NemaError::TransportDictIndexOutOfRange)?;
            if ix >= words.len() {
                return Err(NemaError::TransportDictIndexOutOfRange);
            }
            out.push_str(&words[ix]);
            i = j;
            continue;
        }
        if ch == '\\' {
            if i + 1 >= chars.len() {
                return Err(NemaError::TrailingTransportEscape);
            }
            let v = chars[i + 1];
            if matches!(v, '^' | '\\') {
                out.push(v);
                i += 2;
                continue;
            }
            if v.is_ascii_digit() {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let num_str: String = chars[i + 1..j].iter().collect();
                let n: usize = num_str
                    .parse()
                    .map_err(|_| NemaError::RleCountExceedsBounds)?;
                if !(MIN_RLE_COUNT..=MAX_RLE_COUNT).contains(&n) {
                    return Err(NemaError::RleCountExceedsBounds);
                }
                let mut start = j;
                if start >= chars.len() {
                    return Err(NemaError::RleMissingCharacter);
                }
                let mut c = chars[start];
                if c == '\\' {
                    if start + 1 >= chars.len() {
                        return Err(NemaError::MissingEscapedRleCharacter);
                    }
                    c = chars[start + 1];
                    start += 1;
                }
                if c == ' ' || c == '\t' {
                    return Err(NemaError::RleCoordinateSeparatorProhibited);
                }
                for _ in 0..n {
                    out.push(c);
                }
                i = start + 1;
                continue;
            }
            return Err(NemaError::UnknownTransportEscape);
        }
        out.push(ch);
        i += 1;
        bytes_so_far += ch.len_utf8();
        if bytes_so_far > MAX_BYTES {
            return Err(NemaError::RowTooLarge);
        }
    }
    Ok(out)
}

fn build_transport(source: &str, words: &[String], use_rle: bool) -> String {
    let rows: Vec<&str> = source[..source.len() - 3].split('\n').collect();
    let mut lines = Vec::with_capacity(words.len() + 1 + rows.len());
    lines.push(format!("\t${}", words.len()));
    for w in words {
        lines.push(esc_transport(w));
    }
    for row in rows {
        if (row.starts_with('\t') || row.starts_with(' ')) && parse_domain_header(row).is_some() {
            lines.push(row.to_string());
        } else {
            lines.push(encode_row(row, words, use_rle));
        }
    }
    let mut joined = lines.join("\n");
    joined.push_str("\n\n\n");
    joined
}

fn split_raw_v12(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && matches!(chars[i + 1], ' ' | '\\' | 't') {
            buf.push(chars[i]);
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if chars[i] == ' ' {
            out.push(std::mem::take(&mut buf));
            i += 1;
            continue;
        }
        buf.push(chars[i]);
        i += 1;
    }
    out.push(buf);
    out
}

fn esc_v12(s: &str) -> String {
    s.replace('^', "\\^")
}

fn unesc_v12(s: &str) -> Result<String, NemaError> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '^' {
            out.push('^');
            i += 2;
            continue;
        }
        if chars[i] == '^' {
            return Err(NemaError::InvalidTransportDictRef);
        }
        out.push(chars[i]);
        i += 1;
    }
    Ok(out)
}

fn stem_v12(line: &str) -> Option<(String, String, String, u64)> {
    let (is_graph, name, rev) = parse_domain_header_v12(line)?;
    let digit_start = name
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(idx, _)| idx)?;
    if digit_start == 0 {
        return None;
    }
    let mark = if is_graph { ">" } else { "" }.to_string();
    let stem = name[..digit_start].to_string();
    let num = name[digit_start..].to_string();
    Some((mark, stem, num, rev))
}

fn parse_select_v12(line: &str) -> Option<usize> {
    let rest = line.strip_prefix('\t')?;
    if !is_all_digits(rest) {
        return None;
    }
    rest.parse().ok()
}

fn parse_short_v12(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix('\t')?;
    let (num, rev) = rest.split_once(' ')?;
    if is_all_digits(num) && is_positive_digits(rev) {
        Some((num, rev))
    } else {
        None
    }
}

fn prefix_encode_v12(lines: &[String], use_prefix: bool) -> Vec<String> {
    let mut groups: HashMap<(String, String), usize> = HashMap::new();
    let mut active: Option<usize> = None;
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        if parse_domain_header_v12(line).is_some() {
            let Some((marker, stem, num, rev)) = stem_v12(line) else {
                out.push(line.clone());
                active = None;
                continue;
            };
            let key = (marker, stem);
            if let Some(&g) = groups.get(&key) {
                let mut candidate = Vec::new();
                if active != Some(g) {
                    candidate.push(format!("\t{g}"));
                }
                candidate.push(format!("\t{num} {rev}"));
                let cand_bytes: usize = candidate.iter().map(|s| s.len() + 1).sum();
                if use_prefix && cand_bytes < line.len() + 1 {
                    out.extend(candidate);
                } else {
                    out.push(line.clone());
                }
                active = Some(g);
            } else {
                let g = groups.len();
                groups.insert(key, g);
                out.push(line.clone());
                active = Some(g);
            }
        } else if parse_short_v12(line).is_some() || parse_select_v12(line).is_some() {
            out.push(format!("\t\\{}", &line[1..]));
        } else {
            out.push(line.clone());
        }
    }
    out
}

fn prefix_decode_v12(lines: &[&str]) -> Result<Vec<String>, NemaError> {
    let mut groups: Vec<(String, String)> = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let mut active: Option<usize> = None;
    let mut out = Vec::with_capacity(lines.len());
    for &line in lines {
        if let Some(stripped) = line.strip_prefix("\t\\") {
            let candidate = format!("\t{stripped}");
            if parse_short_v12(&candidate).is_some() || parse_select_v12(&candidate).is_some() {
                out.push(candidate);
                continue;
            }
        }
        if parse_domain_header_v12(line).is_some() {
            if let Some((marker, stem, _, _)) = stem_v12(line) {
                let key = (marker, stem);
                let g = if let Some(&existing) = seen.get(&key) {
                    existing
                } else {
                    let idx = groups.len();
                    seen.insert(key.clone(), idx);
                    groups.push(key);
                    idx
                };
                active = Some(g);
            } else {
                active = None;
            }
            out.push(line.to_string());
            continue;
        }
        if let Some(idx) = parse_select_v12(line) {
            if idx >= groups.len() {
                return Err(NemaError::UnsupportedLegacyPrefixSelector);
            }
            active = Some(idx);
            continue;
        }
        if let Some((num, rev)) = parse_short_v12(line) {
            let Some(act) = active else {
                return Err(NemaError::UnsupportedLegacyPrefixSelector);
            };
            let (mark, stem) = &groups[act];
            out.push(format!("\t{mark}{stem}{num} {rev}"));
            continue;
        }
        out.push(line.to_string());
    }
    Ok(out)
}

fn looks_like_secret_v12(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("sk-")
        || s.starts_with("AKIA")
        || lower.starts_with("ghp_")
        || lower.starts_with("gho_")
        || lower.contains("password=")
        || lower.contains("secret=")
        || lower.contains("api_key=")
        || lower.contains("access_token=")
}

fn cell_candidates_v12(rows: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut mode: Option<DomainMode> = None;
    let mut first = false;
    for &line in rows {
        if let Some((is_graph, _, _)) = parse_domain_header_v12(line) {
            mode = if is_graph {
                Some(DomainMode::Graph)
            } else {
                None
            };
            first = true;
            continue;
        }
        if first && mode.is_none() {
            mode = Some(if line.starts_with('\t') {
                DomainMode::Table
            } else {
                DomainMode::FiveWOneH
            });
        }
        if first && mode == Some(DomainMode::Table) {
            first = false;
            continue;
        }
        first = false;
        let vals = split_raw_v12(line);
        for (i, s) in vals.into_iter().enumerate() {
            if s.is_empty() || s.chars().count() < 3 || s.len() > 16384 || looks_like_secret_v12(&s)
            {
                continue;
            }
            if mode == Some(DomainMode::Graph) && (i == 0 || (i == 1 && line.starts_with(' '))) {
                continue;
            }
            out.push(s);
        }
    }
    out
}

fn dict_encode_v12(lines: &[&str], dictionary: &[String]) -> Vec<String> {
    let mut look = HashMap::with_capacity(dictionary.len());
    for (i, v) in dictionary.iter().enumerate() {
        look.insert(v.as_str(), format!("^{i}"));
    }
    let mut mode: Option<DomainMode> = None;
    let mut first = false;
    let mut out = Vec::with_capacity(lines.len());
    for &line in lines {
        if let Some((is_graph, _, _)) = parse_domain_header_v12(line) {
            mode = if is_graph {
                Some(DomainMode::Graph)
            } else {
                None
            };
            first = true;
            out.push(line.to_string());
            continue;
        }
        if first && mode.is_none() {
            mode = Some(if line.starts_with('\t') {
                DomainMode::Table
            } else {
                DomainMode::FiveWOneH
            });
        }
        if first && mode == Some(DomainMode::Table) {
            out.push(esc_v12(line));
            first = false;
            continue;
        }
        first = false;
        let vals = split_raw_v12(line);
        let encoded_cells: Vec<String> = vals
            .into_iter()
            .enumerate()
            .map(|(idx, v)| {
                if mode == Some(DomainMode::Graph)
                    && (idx == 0 || (idx == 1 && line.starts_with(' ')))
                {
                    esc_v12(&v)
                } else if let Some(alias) = look.get(v.as_str()) {
                    alias.clone()
                } else {
                    esc_v12(&v)
                }
            })
            .collect();
        out.push(encoded_cells.join(" "));
    }
    out
}

fn dict_decode_v12(lines: &[String], dictionary: &[String]) -> Result<Vec<String>, NemaError> {
    let mut mode: Option<DomainMode> = None;
    let mut first = false;
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        if let Some((is_graph, _, _)) = parse_domain_header_v12(line) {
            mode = if is_graph {
                Some(DomainMode::Graph)
            } else {
                None
            };
            first = true;
            out.push(line.clone());
            continue;
        }
        if first && mode.is_none() {
            mode = Some(if line.starts_with('\t') {
                DomainMode::Table
            } else {
                DomainMode::FiveWOneH
            });
        }
        if first && mode == Some(DomainMode::Table) {
            out.push(unesc_v12(line)?);
            first = false;
            continue;
        }
        first = false;
        let vals = split_raw_v12(line);
        let mut decoded = Vec::with_capacity(vals.len());
        for (idx, value) in vals.into_iter().enumerate() {
            if let Some(rest) = value.strip_prefix('^') {
                if is_all_digits(rest)
                    && !(mode == Some(DomainMode::Graph)
                        && (idx == 0 || (idx == 1 && line.starts_with(' '))))
                {
                    let n: usize = rest
                        .parse()
                        .map_err(|_| NemaError::TransportDictIndexOutOfRange)?;
                    if n >= dictionary.len() {
                        return Err(NemaError::TransportDictIndexOutOfRange);
                    }
                    decoded.push(dictionary[n].clone());
                    continue;
                }
            }
            decoded.push(unesc_v12(&value)?);
        }
        out.push(decoded.join(" "));
    }
    Ok(out)
}

/// `.NEMA v1.2` 传输层编码 (`nemat.py::encode` 1:1 对应)
pub fn encode_transport_v12(
    source: &str,
    dictionary: &[String],
    prefixes: bool,
) -> Result<String, NemaError> {
    let rows = validate_source(source)?;
    let unique: HashSet<&String> = dictionary.iter().collect();
    if dictionary.len() > 64 || unique.len() != dictionary.len() {
        return Err(NemaError::BadTransportDictionarySize);
    }
    if dictionary
        .iter()
        .any(|s| s.is_empty() || s.contains('\n') || s.contains('\t') || s.len() > 16384)
    {
        return Err(NemaError::InvalidTransportDictionaryValue);
    }
    let body = prefix_encode_v12(&dict_encode_v12(&rows, dictionary), prefixes);
    let mut all = Vec::with_capacity(dictionary.len() + 1 + body.len());
    if !dictionary.is_empty() {
        all.push(format!("\t${}", dictionary.len()));
        for s in dictionary {
            all.push(esc_v12(s));
        }
    }
    all.extend(body);
    let mut joined = all.join("\n");
    joined.push_str("\n\n\n");
    Ok(joined)
}

/// `.NEMA v1.2` 传输层解码 (`nemat.py::decode` 1:1 对应)
pub fn decode_transport_v12(text: &str) -> Result<String, NemaError> {
    if text.len() > MAX_BYTES {
        return Err(NemaError::TransportTooLarge);
    }
    if text.contains('\r') || !text.ends_with("\n\n\n") || text.ends_with("\n\n\n\n") {
        return Err(NemaError::InvalidTransportTermination);
    }
    let rows: Vec<&str> = text[..text.len() - 3].split('\n').collect();
    let mut dic = Vec::new();
    let mut start_idx = 0usize;
    if let Some(n) = rows.first().and_then(|r| parse_dict_header(r)) {
        if n > 64 || rows.len() < n + 2 {
            return Err(NemaError::BadTransportDictionarySize);
        }
        for &raw_s in &rows[1..=n] {
            let unescaped = unesc_v12(raw_s)?;
            if unescaped.is_empty() || unescaped.contains('\n') || unescaped.contains('\t') {
                return Err(NemaError::InvalidTransportDictionaryValue);
            }
            dic.push(unescaped);
        }
        let unique: HashSet<&String> = dic.iter().collect();
        if unique.len() != n {
            return Err(NemaError::InvalidTransportDictionaryValue);
        }
        start_idx = n + 1;
    }
    let pref_decoded = prefix_decode_v12(&rows[start_idx..])?;
    let dict_decoded = dict_decode_v12(&pref_decoded, &dic)?;
    let mut output = dict_decoded.join("\n");
    output.push_str("\n\n\n");
    if output.len() > MAX_BYTES {
        return Err(NemaError::DecodedSourceExceedsLimit);
    }
    parse_source(&output)?;
    Ok(output)
}

/// 将 `.nema` 源码编码为最优 `.nemat` 或保留 `.nema`（兼容 `.NEMA v1.2` `nemat.py::optimize` 与旧版 `transport.py::encode`）
pub fn encode_with_profile(
    source: &str,
    profile: &TokenProfile,
    max_entries: usize,
    locked_dictionary: &[String],
) -> Result<EncodedChoice, NemaError> {
    let doc = parse_source(source)?;
    let (baseline, is_exact_token, metric_note) = profile.measure(source);
    let rows: Vec<&str> = source[..source.len() - 3].split('\n').collect();

    let mut best = EncodedChoice {
        method: EncodeMethod::Native,
        text: source.to_string(),
        score: baseline,
        baseline_score: baseline,
        format: CodecFormat::Nema,
        dictionary: Vec::new(),
        is_exact_token,
        metric_note: metric_note.to_string(),
    };

    if doc.domains.iter().any(|d| d.is_v12) {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for cell in cell_candidates_v12(&rows) {
            *counts.entry(cell).or_insert(0) += 1;
        }
        let mut allowed: Vec<(String, usize)> =
            counts.into_iter().filter(|(_, n)| *n > 1).collect();
        allowed.sort_by(|a, b| {
            let gain_a = a.1 * a.0.len();
            let gain_b = b.1 * b.0.len();
            gain_b.cmp(&gain_a).then_with(|| a.0.cmp(&b.0))
        });
        let allowed_words: Vec<String> = allowed.into_iter().take(30).map(|(s, _)| s).collect();
        let max_dict = max_entries.min(12);

        if let Ok(pref_only) = encode_transport_v12(source, &[], true) {
            if pref_only != source && decode_transport(&pref_only).ok().as_deref() == Some(source) {
                let (score, _, _) = profile.measure(&pref_only);
                if score < best.score {
                    best = EncodedChoice {
                        method: EncodeMethod::PrefixOnly,
                        text: pref_only,
                        score,
                        baseline_score: baseline,
                        format: CodecFormat::Nemat,
                        dictionary: Vec::new(),
                        is_exact_token,
                        metric_note: metric_note.to_string(),
                    };
                }
            }
        }

        for pref in [false, true] {
            let mut chosen: Vec<String> = Vec::new();
            for w in locked_dictionary {
                if !w.is_empty()
                    && !w.contains('\t')
                    && !chosen.contains(w)
                    && chosen.len() < max_dict
                {
                    chosen.push(w.clone());
                }
            }
            let Ok(mut current) = encode_transport_v12(source, &chosen, pref) else {
                continue;
            };
            let (mut low, _, _) = profile.measure(&current);
            for _ in 0..max_dict.min(allowed_words.len()) {
                let mut best_step: Option<(String, String, usize)> = None;
                for s in &allowed_words {
                    if chosen.contains(s) {
                        continue;
                    }
                    let mut trial_dict = chosen.clone();
                    trial_dict.push(s.clone());
                    let Ok(trial) = encode_transport_v12(source, &trial_dict, pref) else {
                        continue;
                    };
                    let (cost, _, _) = profile.measure(&trial);
                    if cost < low {
                        low = cost;
                        best_step = Some((s.clone(), trial, cost));
                    }
                }
                if let Some((word, text, _)) = best_step {
                    chosen.push(word);
                    current = text;
                } else {
                    break;
                }
            }
            if current != source && decode_transport(&current).ok().as_deref() == Some(source) {
                let (score, _, _) = profile.measure(&current);
                if score < best.score
                    || (score == best.score
                        && current.len() < best.text.len()
                        && best.format == CodecFormat::Nemat)
                {
                    best = EncodedChoice {
                        method: if pref {
                            EncodeMethod::Combined
                        } else {
                            EncodeMethod::DictionaryOnly
                        },
                        text: current,
                        score,
                        baseline_score: baseline,
                        format: CodecFormat::Nemat,
                        dictionary: chosen,
                        is_exact_token,
                        metric_note: metric_note.to_string(),
                    };
                }
            }
        }
        return Ok(best);
    }

    let mut words = Vec::new();
    for w in locked_dictionary {
        if !w.is_empty() && !w.contains('\t') && !words.contains(w) && words.len() < max_entries {
            words.push(w.clone());
        }
    }
    for cand in candidate_dictionary(&rows, max_entries) {
        if words.len() >= max_entries {
            break;
        }
        if !words.contains(&cand) {
            words.push(cand);
        }
    }

    let candidates = [
        (
            EncodeMethod::DictionaryAndRle,
            build_transport(source, &words, true),
            words.clone(),
        ),
        (
            EncodeMethod::DictionaryOnly,
            build_transport(source, &words, false),
            words,
        ),
        (
            EncodeMethod::RleOnly,
            build_transport(source, &[], true),
            Vec::new(),
        ),
    ];

    for (method, text, dict_used) in candidates {
        let Ok(decoded) = decode_transport(&text) else {
            continue;
        };
        if decoded != source {
            continue;
        }
        let (score, _, _) = profile.measure(&text);
        if score < best.score {
            best = EncodedChoice {
                method,
                text,
                score,
                baseline_score: baseline,
                format: CodecFormat::Nemat,
                dictionary: dict_used,
                is_exact_token,
                metric_note: metric_note.to_string(),
            };
        }
    }
    Ok(best)
}

fn is_legacy_prefix_selector(row: &str) -> bool {
    let Some(rest) = row.strip_prefix('\t') else {
        return false;
    };
    if let Some((a, b)) = rest.split_once(' ') {
        is_all_digits(a) && is_all_digits(b)
    } else {
        is_all_digits(rest)
    }
}

/// 将 `.nemat` 传输流解码并校验还原为权威 `.nema` 源码（兼容 `.NEMA v1.2` `nemat.py::decode` 与旧版 `transport.py::decode`）
pub fn decode_transport(text: &str) -> Result<String, NemaError> {
    if text.len() > MAX_BYTES {
        return Err(NemaError::TransportTooLarge);
    }
    if text.contains('\r') || !text.ends_with("\n\n\n") || text.ends_with("\n\n\n\n") {
        return Err(NemaError::InvalidTransportTermination);
    }
    let rows: Vec<&str> = text[..text.len() - 3].split('\n').collect();
    if rows.iter().any(|r| parse_domain_header_v12(r).is_some()) {
        return decode_transport_v12(text);
    }
    let Some(n) = rows.first().and_then(|r| parse_dict_header(r)) else {
        return Err(NemaError::ExpectedTransportManifest);
    };
    if n > MAX_DICT_ENTRIES || rows.len() < n + 2 {
        return Err(NemaError::BadTransportDictionarySize);
    }
    let mut values = Vec::with_capacity(n);
    for &raw_val in &rows[1..=n] {
        let v = decode_row(raw_val, &[])?;
        if v.is_empty() || v.contains('\n') || v.contains('\t') {
            return Err(NemaError::InvalidTransportDictionaryValue);
        }
        values.push(v);
    }
    let mut out = Vec::with_capacity(rows.len() - n - 1);
    for &row in &rows[n + 1..] {
        if parse_domain_header(row).is_some() {
            out.push(row.to_string());
            continue;
        }
        if is_legacy_prefix_selector(row) {
            return Err(NemaError::UnsupportedLegacyPrefixSelector);
        }
        let v = decode_row(row, &values)?;
        if (v.starts_with('\t') || v.starts_with(' ')) && parse_domain_header(&v).is_some() {
            return Err(NemaError::EncodedDomainHeaderNotPermitted);
        }
        out.push(v);
    }
    let mut expanded = out.join("\n");
    expanded.push_str("\n\n\n");
    parse_source(&expanded)?;
    if expanded.len() > MAX_BYTES {
        return Err(NemaError::DecodedSourceExceedsLimit);
    }
    Ok(expanded)
}

// ============================================================================
// 3. UI 辅助：复用位置查找、Table 表头切换、自动规范化与 Graph 布局
// ============================================================================

/// 定位所有字典引用 `^N` 及高频候选词在文档中的精确坐标（供 UI「显示复用位置」高亮/跳转）
pub fn find_reused_positions(source: &str) -> Vec<ReusedPosition> {
    let normalized = normalize_nema_newlines(source);
    let lines: Vec<&str> = normalized.trim_end_matches('\n').split('\n').collect();
    let cands = candidate_dictionary(&lines, 48);
    let mut positions = Vec::new();

    for (line_idx, &line) in lines.iter().enumerate() {
        if line.starts_with('\t')
            || parse_domain_header(line).is_some()
            || parse_domain_header_v12(line).is_some()
        {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] == '\\' && i + 1 < chars.len() {
                i += 2;
                continue;
            }
            if chars[i] == '^' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let token: String = chars[i..j].iter().collect();
                let ix = token[1..].parse::<usize>().ok();
                positions.push(ReusedPosition {
                    token,
                    dict_index: ix,
                    line: line_idx + 1,
                    column: i + 1,
                });
                i = j;
                continue;
            }
            i += 1;
        }
        for cand in &cands {
            let mut search_from = 0usize;
            while let Some(rel) = line[search_from..].find(cand.as_str()) {
                let byte_pos = search_from + rel;
                let col = line[..byte_pos].chars().count() + 1;
                positions.push(ReusedPosition {
                    token: cand.clone(),
                    dict_index: None,
                    line: line_idx + 1,
                    column: col,
                });
                search_from = byte_pos + cand.len();
            }
        }
    }
    positions
}

/// 官方 `.NEMA v1.2 (R7)` 三模式完整示例（含 5W1H、Table 与带 `X3` 扩展属性的 Graph 域）
pub const COMPLETE_V12_SAMPLE: &str = "\tTja1 1\nログイン処理を確認する 情報漏洩を防ぐ 認証トークンを検証する 担当エージェント 今日 認証サービス\n セッションを守る\n\tTtable1 1\n\tAgent Status Note\nClaude Ready 认证检查已完成\n  下一轮验证权限\nCodex Review 准备输出审核结果\n\t>Gauth1 1\nA LoginAPI Authentication APIKey\nB TokenStore SessionStorage\nC AuditService RecordsSecurityEvents\nA>X>B Reads VerifySession HTTP401\n CacheExpiredRetryOnce\nB<Y<C Notifies AuditEvent\nA=U=C Shares SecurityPolicy\nA>P>A Retry Maximum3Attempts\n\n\n";

/// 将任意换行规范化为合法 `.nema` 尾部 `\n\n\n`
pub fn normalize_nema_newlines(source: &str) -> String {
    let lf = source.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = lf.trim_end_matches('\n');
    if trimmed.is_empty() {
        COMPLETE_V12_SAMPLE.to_string()
    } else {
        format!("{trimmed}\n\n\n")
    }
}

/// 在指定 Table 域（或第一个非 Graph 域）首行前切换 `.NEMA v1.2` `\t` 表头或旧版 `\\ ` 表头前缀
pub fn toggle_table_header_in_source(source: &str, target_domain_id: Option<&str>) -> String {
    let lf = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = lf
        .trim_end_matches('\n')
        .split('\n')
        .map(str::to_string)
        .collect();
    let mut in_target_domain = false;
    let mut is_v12_domain = false;
    for line in &mut lines {
        if let Some((is_graph, id, _)) = parse_domain_header_v12(line) {
            in_target_domain = !is_graph && target_domain_id.map_or(true, |target| target == id);
            is_v12_domain = true;
            continue;
        }
        if let Some((mode, id, _)) = parse_domain_header(line) {
            in_target_domain =
                mode == DomainMode::Table && target_domain_id.map_or(true, |target| target == id);
            is_v12_domain = false;
            continue;
        }
        if in_target_domain && !line.trim().is_empty() {
            if is_v12_domain {
                if let Some(stripped) = line.strip_prefix('\t') {
                    *line = stripped.to_string();
                } else {
                    let parts: Vec<String> = line
                        .split_whitespace()
                        .enumerate()
                        .map(|(idx, tok)| {
                            let clean: String = tok
                                .chars()
                                .filter(|c| {
                                    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
                                })
                                .collect();
                            if is_valid_table_header_ident(&clean) {
                                clean
                            } else {
                                format!("Column{}", idx + 1)
                            }
                        })
                        .collect();
                    *line = format!("\t{}", parts.join(" "));
                }
            } else if !line.starts_with('\t') {
                if let Some(stripped) = line
                    .strip_prefix("\\# ")
                    .or_else(|| line.strip_prefix("\\ "))
                {
                    *line = stripped.to_string();
                } else {
                    let parts: Vec<String> = line
                        .split_whitespace()
                        .enumerate()
                        .map(|(idx, tok)| {
                            let clean: String = tok
                                .chars()
                                .filter(|c| {
                                    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
                                })
                                .collect();
                            if is_valid_table_header_ident(&clean) {
                                clean
                            } else {
                                format!("Column{}", idx + 1)
                            }
                        })
                        .collect();
                    *line = format!("\\# {}", parts.join(" "));
                }
            }
            break;
        }
    }
    let mut out = lines.join("\n");
    out.push_str("\n\n\n");
    out
}

pub mod export;
pub use export::*;

pub mod lease;
pub use lease::*;

#[cfg(test)]
mod tests;
