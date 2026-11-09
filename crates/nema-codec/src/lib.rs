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
                (tokens.max(1), false, "BPE 启发估算 (不等同于官方计费 Token)")
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

/// 原生格式不透明转义还原：仅 `\^`、`\\`、`\.` 被还原
pub fn unescape_opaque(v: &str) -> String {
    let chars: Vec<char> = v.chars().collect();
    let mut out = String::with_capacity(v.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && matches!(chars[i + 1], '^' | '\\' | '.') {
            out.push(chars[i + 1]);
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
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

/// `.NEMA v1.2` 原生转义还原 (`native.py::unescape`)：仅允许 `\ `、`\\`、`\t`、`\=`
pub fn unescape_v12(s: &str) -> Result<String, NemaError> {
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
            ' ' | '\\' => result.push(next),
            't' => result.push('\t'),
            '=' => result.push_str("\\="),
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
        if chars[i] == '\\' && i + 1 < chars.len() && matches!(chars[i + 1], ' ' | '\\' | 't' | '=')
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
        let has_special = body.chars().any(|c| matches!(c, '.' | '/' | ':' | '#' | '@'));
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
    }

    let mut seen_domain_ids = HashSet::new();
    let mut pending_v12_mode = false;
    let mut current_domain_has_raw_rows = false;

    for (row_idx, &line) in lines.iter().enumerate().skip(i) {
        let line_start = line_byte_offsets[row_idx];
        let line_no = row_idx + 1;
        if let Some((is_graph, id, revision)) = parse_domain_header_v12(line) {
            if let Some(prev) = doc.domains.last() {
                if !current_domain_has_raw_rows || prev.records.is_empty() {
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
                if !current_domain_has_raw_rows || prev.records.is_empty() {
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
                let candidate_header = if let Some(rest) = line.strip_prefix('\t') {
                    if rest.starts_with('\t') {
                        return Err(NemaError::TabInBody);
                    }
                    Some(rest)
                } else if let Some(rest) = line.strip_prefix(' ') {
                    if let Some(stripped) = rest.strip_prefix('\\') {
                        Some(stripped.trim_start())
                    } else {
                        Some(rest)
                    }
                } else {
                    line.strip_prefix('\\').map(str::trim_start)
                };

                let mut is_table_header = false;
                if let Some(raw_header) = candidate_header {
                    if !raw_header.is_empty() {
                        if let Ok(labels) = split_x_v12(raw_header) {
                            if !labels.is_empty()
                                && labels.iter().all(|v| !v.is_empty() && is_valid_table_header_ident(v))
                            {
                                let unique: HashSet<&str> = labels.iter().map(String::as_str).collect();
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
                } else {
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
                        append_v12_cols(rec, cells, &doc.dictionary, line_start, line.len(), line_no)?;
                    } else if let Some(after_space) = line.strip_prefix(' ') {
                        let cells = split_x_v12(after_space)?;
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
                                column: 1,
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

        // Table 首行精确表头判断
        if current.mode == DomainMode::Table
            && current.records.is_empty()
            && !current.explicit_header
        {
            let raw_hdr = if let Some(stripped) = line.strip_prefix("\\# ") {
                Some(stripped)
            } else if let Some(stripped) = line.strip_prefix("\\ ") {
                Some(stripped)
            } else if let Some(stripped) = line.strip_prefix(" \\# ") {
                Some(stripped)
            } else if let Some(stripped) = line.strip_prefix(" \\ ") {
                Some(stripped)
            } else {
                line.strip_prefix('\t')
            };
            if let Some(raw_names) = raw_hdr {
                let names: Vec<&str> = raw_names.split(' ').collect();
                if !names.is_empty() && names.iter().all(|x| is_valid_table_header_ident(x)) {
                    let unique: HashSet<&str> = names.iter().copied().collect();
                    if unique.len() != names.len() {
                        return Err(NemaError::DuplicateColumnHeader);
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

        if line.starts_with(' ') {
            if current.records.is_empty() {
                return Err(NemaError::ContinuationBeforeFirstRecord);
            }
            let n = line.len() - line.trim_start_matches(' ').len();
            let part = &line[n..];
            if part.is_empty() {
                return Err(NemaError::BlankContinuation);
            }
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
            let rec = current.records.last_mut().expect("checked non-empty");
            rec.span.end_byte = line_start + line.len();
            rec.add_chunk(n - 1, chunk);
        } else {
            let empty_sentinel = line.ends_with(" .") && !line.ends_with(" \\.");
            let effective = if empty_sentinel {
                &line[..line.len() - 2]
            } else {
                line
            };
            let cells: Vec<&str> = effective.split(' ').collect();
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
            let e = &last_rec.cells[0][0].raw;
            if e.contains(['>', '<', '=']) {
                let Some(edge_spec) = parse_edge_spec(e) else {
                    return Err(NemaError::InvalidEdgeExpression(e.clone()));
                };
                last_rec.graph_edge = Some(edge_spec);
            } else if !is_valid_graph_ident(e) {
                return Err(NemaError::InvalidNodeId(e.clone()));
            } else {
                last_rec.graph_node_id = Some(e.clone());
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
        if domain.records.is_empty() {
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
    let lines: Vec<&str> = normalized
        .trim_end_matches('\n')
        .split('\n')
        .collect();
    let cands = candidate_dictionary(&lines, 48);
    let mut positions = Vec::new();

    for (line_idx, &line) in lines.iter().enumerate() {
        if line.starts_with('\t') || parse_domain_header(line).is_some() || parse_domain_header_v12(line).is_some() {
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
            in_target_domain =
                !is_graph && target_domain_id.map_or(true, |target| target == id);
            is_v12_domain = true;
            continue;
        }
        if let Some((mode, id, _)) = parse_domain_header(line) {
            in_target_domain = mode == DomainMode::Table
                && target_domain_id.map_or(true, |target| target == id);
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
                if let Some(stripped) = line.strip_prefix("\\ ") {
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
                    *line = format!("\\ {}", parts.join(" "));
                }
            }
            break;
        }
    }
    let mut out = lines.join("\n");
    out.push_str("\n\n\n");
    out
}

// ============================================================================
// 3.5 同语义对照格式生成器与 `.NEMA v1.2 (R7)` 实测 Benchmark 数据
// ============================================================================

/// 将 `RenderedDocument` 转换为与 `native.py::view` 完全一致的 `serde_json::Value` 数组
pub fn render_to_json_value(doc: &RenderedDocument) -> serde_json::Value {
    let mut arr = Vec::with_capacity(doc.domains.len());
    for d in &doc.domains {
        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), serde_json::Value::String(d.id.clone()));
        obj.insert(
            "revision".to_string(),
            serde_json::Value::Number(serde_json::Number::from(d.revision)),
        );
        obj.insert(
            "mode".to_string(),
            serde_json::Value::String(d.mode.as_str().to_string()),
        );
        match d.mode {
            DomainMode::FiveWOneH => {
                let mut fields = serde_json::Map::new();
                let first_row = d.rows.first();
                for (i, &axis) in AXES_5W1H.iter().enumerate() {
                    let val = first_row
                        .and_then(|r| r.cells.get(i))
                        .cloned()
                        .unwrap_or_default();
                    fields.insert(axis.to_string(), serde_json::Value::String(val));
                }
                obj.insert("fields".to_string(), serde_json::Value::Object(fields));
            }
            DomainMode::Table => {
                obj.insert(
                    "columns".to_string(),
                    serde_json::Value::Array(
                        d.columns
                            .iter()
                            .cloned()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
                let mut records = Vec::with_capacity(d.rows.len());
                for r in &d.rows {
                    let mut rec_map = serde_json::Map::new();
                    for (i, col_name) in d.columns.iter().enumerate() {
                        let val = r.cells.get(i).cloned().unwrap_or_default();
                        rec_map.insert(col_name.clone(), serde_json::Value::String(val));
                    }
                    records.push(serde_json::Value::Object(rec_map));
                }
                obj.insert("records".to_string(), serde_json::Value::Array(records));
            }
            DomainMode::Graph => {
                let mut nodes = Vec::new();
                let mut edges = Vec::new();
                for r in &d.rows {
                    if let Some(nid) = &r.node_id {
                        let mut n_map = serde_json::Map::new();
                        n_map.insert("id".to_string(), serde_json::Value::String(nid.clone()));
                        n_map.insert(
                            "name".to_string(),
                            serde_json::Value::String(
                                r.cells.get(1).cloned().unwrap_or_default(),
                            ),
                        );
                        n_map.insert(
                            "description".to_string(),
                            serde_json::Value::String(
                                r.cells.get(2).cloned().unwrap_or_default(),
                            ),
                        );
                        if !r.extensions.is_empty() {
                            let mut ext_map = serde_json::Map::new();
                            for (k, v) in &r.extensions {
                                ext_map.insert(k.clone(), serde_json::Value::String(v.clone()));
                            }
                            n_map.insert(
                                "extensions".to_string(),
                                serde_json::Value::Object(ext_map),
                            );
                        }
                        nodes.push(serde_json::Value::Object(n_map));
                    } else if let Some(edge) = &r.edge {
                        let mut e_map = serde_json::Map::new();
                        e_map.insert(
                            "id".to_string(),
                            serde_json::Value::String(edge.id.clone()),
                        );
                        e_map.insert(
                            "source".to_string(),
                            serde_json::Value::String(edge.from.clone()),
                        );
                        e_map.insert(
                            "target".to_string(),
                            serde_json::Value::String(edge.to.clone()),
                        );
                        e_map.insert(
                            "direction".to_string(),
                            serde_json::Value::String(edge.raw_operator.to_string()),
                        );
                        e_map.insert(
                            "name".to_string(),
                            serde_json::Value::String(
                                r.cells.get(1).cloned().unwrap_or_default(),
                            ),
                        );
                        e_map.insert(
                            "description".to_string(),
                            serde_json::Value::String(
                                r.cells.get(2).cloned().unwrap_or_default(),
                            ),
                        );
                        if !r.extensions.is_empty() {
                            let mut ext_map = serde_json::Map::new();
                            for (k, v) in &r.extensions {
                                ext_map.insert(k.clone(), serde_json::Value::String(v.clone()));
                            }
                            e_map.insert(
                                "extensions".to_string(),
                                serde_json::Value::Object(ext_map),
                            );
                        }
                        edges.push(serde_json::Value::Object(e_map));
                    }
                }
                obj.insert("nodes".to_string(), serde_json::Value::Array(nodes));
                obj.insert("edges".to_string(), serde_json::Value::Array(edges));
            }
        }
        arr.push(serde_json::Value::Object(obj));
    }
    serde_json::Value::Array(arr)
}

fn domain_to_positional_json_value(d: &RenderedDomain, tagged: bool) -> serde_json::Value {
    let mut row = Vec::with_capacity(if tagged { 5 } else { 4 });
    if tagged {
        let tag = match d.mode {
            DomainMode::FiveWOneH => "w",
            DomainMode::Table => "t",
            DomainMode::Graph => "g",
        };
        row.push(serde_json::Value::String(tag.to_string()));
    }
    row.push(serde_json::Value::String(d.id.clone()));
    row.push(serde_json::Value::Number(serde_json::Number::from(
        d.revision,
    )));
    match d.mode {
        DomainMode::FiveWOneH => {
            let first = d.rows.first();
            let fields: Vec<serde_json::Value> = (0..6)
                .map(|i| {
                    serde_json::Value::String(
                        first
                            .and_then(|r| r.cells.get(i))
                            .cloned()
                            .unwrap_or_default(),
                    )
                })
                .collect();
            row.push(serde_json::Value::Array(fields));
        }
        DomainMode::Table => {
            row.push(serde_json::Value::Array(
                d.columns
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ));
            let recs: Vec<serde_json::Value> = d
                .rows
                .iter()
                .map(|r| {
                    serde_json::Value::Array(
                        (0..d.columns.len())
                            .map(|i| {
                                serde_json::Value::String(
                                    r.cells.get(i).cloned().unwrap_or_default(),
                                )
                            })
                            .collect(),
                    )
                })
                .collect();
            row.push(serde_json::Value::Array(recs));
        }
        DomainMode::Graph => {
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            for r in &d.rows {
                if let Some(nid) = &r.node_id {
                    let mut n_arr = vec![
                        serde_json::Value::String(nid.clone()),
                        serde_json::Value::String(r.cells.get(1).cloned().unwrap_or_default()),
                        serde_json::Value::String(r.cells.get(2).cloned().unwrap_or_default()),
                    ];
                    for (_, ext_v) in &r.extensions {
                        n_arr.push(serde_json::Value::String(ext_v.clone()));
                    }
                    nodes.push(serde_json::Value::Array(n_arr));
                } else if let Some(edge) = &r.edge {
                    let mut e_arr = vec![
                        serde_json::Value::String(edge.id.clone()),
                        serde_json::Value::String(edge.from.clone()),
                        serde_json::Value::String(edge.to.clone()),
                        serde_json::Value::String(edge.raw_operator.to_string()),
                        serde_json::Value::String(r.cells.get(1).cloned().unwrap_or_default()),
                        serde_json::Value::String(r.cells.get(2).cloned().unwrap_or_default()),
                    ];
                    for (_, ext_v) in &r.extensions {
                        e_arr.push(serde_json::Value::String(ext_v.clone()));
                    }
                    edges.push(serde_json::Value::Array(e_arr));
                }
            }
            row.push(serde_json::Value::Array(nodes));
            row.push(serde_json::Value::Array(edges));
        }
    }
    serde_json::Value::Array(row)
}

/// 生成同语义 `JSON objects` 紧凑字符串
pub fn to_equivalent_json_objects(doc: &RenderedDocument) -> String {
    serde_json::to_string(&render_to_json_value(doc)).unwrap_or_default()
}

/// 生成同语义 `JSON positional` 紧凑字符串
pub fn to_equivalent_json_positional(doc: &RenderedDocument) -> String {
    let arr: Vec<serde_json::Value> = doc
        .domains
        .iter()
        .map(|d| domain_to_positional_json_value(d, false))
        .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

/// 生成同语义 `JSON positional (tagged w/t/g)` 紧凑字符串（自包含模式标签，无需外部模式注册表）
pub fn to_equivalent_json_tagged(doc: &RenderedDocument) -> String {
    let arr: Vec<serde_json::Value> = doc
        .domains
        .iter()
        .map(|d| domain_to_positional_json_value(d, true))
        .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

/// 生成同语义 `YAML positional` 紧凑字符串
pub fn to_equivalent_yaml_positional(doc: &RenderedDocument) -> String {
    doc.domains
        .iter()
        .map(|d| {
            let line = serde_json::to_string(&domain_to_positional_json_value(d, false))
                .unwrap_or_default();
            format!("- {line}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 单文档多格式同语义 Token / 字节实时对照行
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatComparisonRow {
    pub format_name: &'static str,
    pub bytes: usize,
    pub bpe_tokens: usize,
    pub unigram_tokens: usize,
    pub saving_vs_json_bpe_pct: f64,
    pub saving_vs_pos_json_bpe_pct: f64,
    pub sample_preview: String,
}

fn round_pct(base: usize, cur: usize) -> f64 {
    if base == 0 {
        0.0
    } else {
        let raw = ((base as f64 - cur as f64) * 100.0) / (base as f64);
        (raw * 100.0).round() / 100.0
    }
}

/// 计算当前 `.nema` 源码与同语义 `.nemat`、`JSON objects`、`JSON positional`、`JSON tagged (w/t/g)`、`YAML positional` 的实时对照
pub fn compare_document_formats(source: &str) -> Result<Vec<FormatComparisonRow>, NemaError> {
    let normalized = normalize_nema_newlines(source);
    let rendered = render_source(&normalized)?;
    let bpe_prof = TokenProfile::BpeHeuristic {
        model_name: "local-bpe".to_string(),
    };
    let uni_prof = TokenProfile::UnigramHeuristic {
        model_name: "local-unigram".to_string(),
    };
    let nemat_choice = encode_with_profile(&normalized, &bpe_prof, 12, &[])?;
    let json_obj = to_equivalent_json_objects(&rendered);
    let json_pos = to_equivalent_json_positional(&rendered);
    let json_tag = to_equivalent_json_tagged(&rendered);
    let yaml_pos = to_equivalent_yaml_positional(&rendered);

    let json_bpe = bpe_prof.measure(&json_obj).0;
    let pos_bpe = bpe_prof.measure(&json_pos).0;

    let formats: [(&'static str, String); 6] = [
        (".nema", normalized),
        (".nemat", nemat_choice.text),
        ("JSON objects", json_obj),
        ("JSON positional", json_pos),
        ("JSON tagged (w/t/g)", json_tag),
        ("YAML positional", yaml_pos),
    ];

    Ok(formats
        .into_iter()
        .map(|(format_name, text)| {
            let bytes = text.len();
            let bpe_tokens = bpe_prof.measure(&text).0;
            let unigram_tokens = uni_prof.measure(&text).0;
            let preview: String = text.chars().take(240).collect();
            FormatComparisonRow {
                format_name,
                bytes,
                bpe_tokens,
                unigram_tokens,
                saving_vs_json_bpe_pct: round_pct(json_bpe, bpe_tokens),
                saving_vs_pos_json_bpe_pct: round_pct(pos_bpe, bpe_tokens),
                sample_preview: preview,
            }
        })
        .collect())
}

/// 官方 `.NEMA v1.2 (2026-11-07)` 单个测试场景实测数据 (`latest_results.json` + `tagged_json_audit_r7.json`)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OfficialBenchmarkScenario {
    pub scenario: &'static str,
    pub label_zh: &'static str,
    pub domains: usize,
    pub raw_bytes_nema: usize,
    pub raw_bytes_json: usize,
    pub raw_bytes_json_positional: usize,
    pub raw_bytes_json_tagged: usize,
    pub raw_bytes_yaml_positional: usize,
    pub bpe_nema: usize,
    pub bpe_nemat: usize,
    pub bpe_json: usize,
    pub bpe_json_positional: usize,
    pub bpe_json_tagged: usize,
    pub bpe_yaml_positional: usize,
    pub nemat_mode: &'static str,
    pub bpe_nema_vs_json_pct: f64,
    pub bpe_nema_vs_pos_json_pct: f64,
    pub bpe_nema_vs_tagged_json_pct: f64,
    pub unigram_nema: usize,
    pub unigram_nemat: usize,
    pub unigram_json: usize,
    pub unigram_json_positional: usize,
    pub unigram_json_tagged: usize,
    pub unigram_yaml_positional: usize,
}

/// 官方 `.NEMA v1.2` 全部 8 组基准场景实测表（含高熵低收益场景，与 `https://nema-sigma.vercel.app/website/index.html#benchmark` 1:1 对齐）
pub const OFFICIAL_BENCHMARK_V12: &[OfficialBenchmarkScenario] = &[
    OfficialBenchmarkScenario {
        scenario: "english_60",
        label_zh: "英文 60 域 (5W1H)",
        domains: 60,
        raw_bytes_nema: 4842,
        raw_bytes_json: 10912,
        raw_bytes_json_positional: 6172,
        raw_bytes_json_tagged: 6412,
        raw_bytes_yaml_positional: 6290,
        bpe_nema: 3012,
        bpe_nemat: 1953,
        bpe_json: 6502,
        bpe_json_positional: 3851,
        bpe_json_tagged: 3971,
        bpe_yaml_positional: 3969,
        nemat_mode: "combined",
        bpe_nema_vs_json_pct: 53.68,
        bpe_nema_vs_pos_json_pct: 21.79,
        bpe_nema_vs_tagged_json_pct: 24.15,
        unigram_nema: 3898,
        unigram_nemat: 2455,
        unigram_json: 8198,
        unigram_json_positional: 4778,
        unigram_json_tagged: 4898,
        unigram_yaml_positional: 4896,
    },
    OfficialBenchmarkScenario {
        scenario: "chinese_60",
        label_zh: "中文 60 域 (5W1H)",
        domains: 60,
        raw_bytes_nema: 5732,
        raw_bytes_json: 11492,
        raw_bytes_json_positional: 6752,
        raw_bytes_json_tagged: 6992,
        raw_bytes_yaml_positional: 6870,
        bpe_nema: 5532,
        bpe_nemat: 3647,
        bpe_json: 8662,
        bpe_json_positional: 6011,
        bpe_json_tagged: 6131,
        bpe_yaml_positional: 6129,
        nemat_mode: "combined",
        bpe_nema_vs_json_pct: 36.13,
        bpe_nema_vs_pos_json_pct: 7.97,
        bpe_nema_vs_tagged_json_pct: 9.77,
        unigram_nema: 5588,
        unigram_nemat: 3703,
        unigram_json: 9488,
        unigram_json_positional: 6068,
        unigram_json_tagged: 6188,
        unigram_yaml_positional: 6186,
    },
    OfficialBenchmarkScenario {
        scenario: "japanese_60",
        label_zh: "日文 60 域 (5W1H)",
        domains: 60,
        raw_bytes_nema: 8432,
        raw_bytes_json: 14192,
        raw_bytes_json_positional: 9452,
        raw_bytes_json_tagged: 9692,
        raw_bytes_yaml_positional: 9570,
        bpe_nema: 7062,
        bpe_nemat: 4088,
        bpe_json: 10192,
        bpe_json_positional: 7541,
        bpe_json_tagged: 7661,
        bpe_yaml_positional: 7659,
        nemat_mode: "combined",
        bpe_nema_vs_json_pct: 30.71,
        bpe_nema_vs_pos_json_pct: 6.35,
        bpe_nema_vs_tagged_json_pct: 7.82,
        unigram_nema: 7248,
        unigram_nemat: 4220,
        unigram_json: 11148,
        unigram_json_positional: 7728,
        unigram_json_tagged: 7848,
        unigram_yaml_positional: 7846,
    },
    OfficialBenchmarkScenario {
        scenario: "table_200",
        label_zh: "表格 200 行 (Table)",
        domains: 1,
        raw_bytes_nema: 6671,
        raw_bytes_json: 13283,
        raw_bytes_json_positional: 8432,
        raw_bytes_json_tagged: 8436,
        raw_bytes_yaml_positional: 8432,
        bpe_nema: 4899,
        bpe_nemat: 2105,
        bpe_json: 7920,
        bpe_json_positional: 5890,
        bpe_json_tagged: 5892,
        bpe_yaml_positional: 5890,
        nemat_mode: "dictionary",
        bpe_nema_vs_json_pct: 38.14,
        bpe_nema_vs_pos_json_pct: 16.83,
        bpe_nema_vs_tagged_json_pct: 16.85,
        unigram_nema: 5208,
        unigram_nemat: 2153,
        unigram_json: 9067,
        unigram_json_positional: 6226,
        unigram_json_tagged: 6228,
        unigram_yaml_positional: 6226,
    },
    OfficialBenchmarkScenario {
        scenario: "graph_40n_160e",
        label_zh: "图谱 40 节点 / 160 边 (Graph)",
        domains: 1,
        raw_bytes_nema: 8488,
        raw_bytes_json: 21312,
        raw_bytes_json_positional: 11265,
        raw_bytes_json_tagged: 11269,
        raw_bytes_yaml_positional: 11265,
        bpe_nema: 6952,
        bpe_nemat: 3265,
        bpe_json: 13778,
        bpe_json_positional: 7551,
        bpe_json_tagged: 7553,
        bpe_yaml_positional: 7551,
        nemat_mode: "dictionary",
        bpe_nema_vs_json_pct: 49.54,
        bpe_nema_vs_pos_json_pct: 7.93,
        bpe_nema_vs_tagged_json_pct: 7.96,
        unigram_nema: 7152,
        unigram_nemat: 3306,
        unigram_json: 15484,
        unigram_json_positional: 8170,
        unigram_json_tagged: 8172,
        unigram_yaml_positional: 8170,
    },
    OfficialBenchmarkScenario {
        scenario: "high_entropy_60",
        label_zh: "高熵随机 60 域 (低复用对照)",
        domains: 60,
        raw_bytes_nema: 7502,
        raw_bytes_json: 15085,
        raw_bytes_json_positional: 10345,
        raw_bytes_json_tagged: 10585,
        raw_bytes_yaml_positional: 10463,
        bpe_nema: 6358,
        bpe_nemat: 6181,
        bpe_json: 9878,
        bpe_json_positional: 7228,
        bpe_json_tagged: 7348,
        bpe_yaml_positional: 7346,
        nemat_mode: "prefix",
        bpe_nema_vs_json_pct: 35.63,
        bpe_nema_vs_pos_json_pct: 12.04,
        bpe_nema_vs_tagged_json_pct: 13.47,
        unigram_nema: 6687,
        unigram_nemat: 6451,
        unigram_json: 11917,
        unigram_json_positional: 8497,
        unigram_json_tagged: 8617,
        unigram_yaml_positional: 8615,
    },
    OfficialBenchmarkScenario {
        scenario: "short_what",
        label_zh: "极短单域 (What HTTPItIs)",
        domains: 1,
        raw_bytes_nema: 22,
        raw_bytes_json: 125,
        raw_bytes_json_positional: 46,
        raw_bytes_json_tagged: 50,
        raw_bytes_yaml_positional: 46,
        bpe_nema: 16,
        bpe_nemat: 16,
        bpe_json: 73,
        bpe_json_positional: 29,
        bpe_json_tagged: 31,
        bpe_yaml_positional: 29,
        nemat_mode: "native",
        bpe_nema_vs_json_pct: 78.08,
        bpe_nema_vs_pos_json_pct: 44.83,
        bpe_nema_vs_tagged_json_pct: 48.39,
        unigram_nema: 20,
        unigram_nemat: 20,
        unigram_json: 91,
        unigram_json_positional: 34,
        unigram_json_tagged: 36,
        unigram_yaml_positional: 34,
    },
    OfficialBenchmarkScenario {
        scenario: "graph_3n_4e",
        label_zh: "小型认证图 (3 节点 / 4 边)",
        domains: 1,
        raw_bytes_nema: 240,
        raw_bytes_json: 689,
        raw_bytes_json_positional: 340,
        raw_bytes_json_tagged: 344,
        raw_bytes_yaml_positional: 340,
        bpe_nema: 144,
        bpe_nemat: 144,
        bpe_json: 417,
        bpe_json_positional: 205,
        bpe_json_tagged: 207,
        bpe_yaml_positional: 205,
        nemat_mode: "native",
        bpe_nema_vs_json_pct: 65.47,
        bpe_nema_vs_pos_json_pct: 29.76,
        bpe_nema_vs_tagged_json_pct: 30.43,
        unigram_nema: 186,
        unigram_nemat: 186,
        unigram_json: 491,
        unigram_json_positional: 239,
        unigram_json_tagged: 241,
        unigram_yaml_positional: 239,
    },
];

/// `.NEMA v1.2` Graph 单空格节点语法局部标记优化实测 (`old_graph_markup` vs `new_graph_markup`)
pub const OFFICIAL_SYNTAX_BEFORE_AFTER: (&str, &str, usize, usize, usize, usize, usize, usize) = (
    "\t>Gauth1 1\nA \\=LoginAPI Authentication\nB \\=TokenStore SessionStorage\nA>X>B Reads VerifySession",
    "\t>Gauth1 1\n A LoginAPI Authentication\n B TokenStore SessionStorage\nA>X>B Reads VerifySession",
    53, // old BPE
    45, // new BPE (-15.09%)
    74, // old Unigram
    72, // new Unigram (-2.70%)
    97, // old bytes
    95, // new bytes
);

// ============================================================================
// 4. IDE1.0 × IME1.0 所有权仲裁状态机 (Observer + Lease + Epoch)
// ============================================================================

/// 所有权角色：默认 `Observer`（只读 IME 最新版本，绝不按键后抢转换）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OwnershipRole {
    Observer,
    LeaseRequested {
        source_hash: u64,
        revision: u64,
        epoch: u64,
    },
    ImePausedSameEpoch {
        source_hash: u64,
        revision: u64,
        epoch: u64,
    },
    Committed {
        source_hash: u64,
        revision: u64,
        epoch: u64,
    },
}

/// 幂等转换缓存记录
#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedCommit {
    source_hash: u64,
    revision: u64,
    epoch: u64,
    choice: EncodedChoice,
}

/// IDE1.0 × IME1.0 租约仲裁器
#[derive(Debug, Clone)]
pub struct NemaLeaseArbitrator {
    role: OwnershipRole,
    current_epoch: u64,
    last_commit: Option<CachedCommit>,
}

impl Default for NemaLeaseArbitrator {
    fn default() -> Self {
        Self {
            role: OwnershipRole::Observer,
            current_epoch: 0,
            last_commit: None,
        }
    }
}

impl NemaLeaseArbitrator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn role(&self) -> &OwnershipRole {
        &self.role
    }

    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// 计算源码 FNV-1a 64-bit 哈希
    pub fn hash_source(source: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for b in source.as_bytes() {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// 1. `request_lease(source_hash, revision)`
    ///
    /// 若同一 `(source_hash, revision)` 已提交过，直接返回缓存的幂等结果。
    pub fn request_lease(&mut self, source_hash: u64, revision: u64) -> (u64, Option<EncodedChoice>) {
        if let Some(cached) = &self.last_commit {
            if cached.source_hash == source_hash && cached.revision == revision {
                return (cached.epoch, Some(cached.choice.clone()));
            }
        }
        self.current_epoch += 1;
        let epoch = self.current_epoch;
        self.role = OwnershipRole::LeaseRequested {
            source_hash,
            revision,
            epoch,
        };
        (epoch, None)
    }

    /// 2. `pause_ime_same_epoch(epoch)`：确认同一 epoch 下暂停 IME 自动转换；过时 epoch 丢弃
    pub fn pause_ime_same_epoch(&mut self, epoch: u64) -> bool {
        if epoch != self.current_epoch {
            return false;
        }
        if let OwnershipRole::LeaseRequested {
            source_hash,
            revision,
            epoch: ep,
        } = self.role
        {
            self.role = OwnershipRole::ImePausedSameEpoch {
                source_hash,
                revision,
                epoch: ep,
            };
            true
        } else {
            false
        }
    }

    /// 3. `atomic_commit(epoch, choice)`：仅当前活跃 epoch 允许原子提交，过时 epoch 的计算结果全部丢弃
    pub fn atomic_commit(&mut self, epoch: u64, choice: EncodedChoice) -> bool {
        if epoch != self.current_epoch {
            return false;
        }
        if let OwnershipRole::ImePausedSameEpoch {
            source_hash,
            revision,
            epoch: ep,
        } = self.role
        {
            self.last_commit = Some(CachedCommit {
                source_hash,
                revision,
                epoch: ep,
                choice,
            });
            self.role = OwnershipRole::Committed {
                source_hash,
                revision,
                epoch: ep,
            };
            true
        } else {
            false
        }
    }

    /// 4. `release_lease(epoch)`：释放租约并回到 `Observer`
    pub fn release_lease(&mut self, epoch: u64) -> bool {
        if epoch != self.current_epoch {
            return false;
        }
        self.role = OwnershipRole::Observer;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> String {
        format!("{s}\n\n\n")
    }

    #[test]
    fn test_numeric_headers_and_default_revision() {
        let cases = [
            ("\t\\1", DomainMode::Table, 1u64),
            ("\t?2", DomainMode::FiveWOneH, 1u64),
            ("\t>3", DomainMode::Graph, 1u64),
            ("\t\\4 7", DomainMode::Table, 7u64),
        ];
        for (head, mode, rev) in cases {
            let body = if mode == DomainMode::Graph {
                "A Node"
            } else {
                "Hello"
            };
            let d = parse_source(&doc(&format!("{head}\n{body}")))
                .unwrap()
                .domains
                .remove(0);
            assert_eq!((d.mode, d.revision), (mode, rev));
        }
    }

    #[test]
    fn test_header_vs_continuation() {
        let d = render_source(&doc("\t\\1\nA B\n  Continued\n\t\\2\nC D"))
            .unwrap()
            .domains;
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].rows[0].cells[1], "B Continued");
    }

    #[test]
    fn test_reject_missing_graph_node_and_duplicate_edge() {
        let err1 = parse_source(&doc("\t>1\nA Node\nA>X>B Reads")).unwrap_err();
        assert!(matches!(err1, NemaError::UndefinedGraphNode(_, _)));

        let err2 = parse_source(&doc("\t>1\nA Node\nB Node\nA>X>B Reads\nB<X<A Writes")).unwrap_err();
        assert!(matches!(err2, NemaError::DuplicateEdgeId(_)));
    }

    #[test]
    fn test_headerless_and_explicit_header_table() {
        let d1 = render_source(&doc("\t\\1\nClaude Ready HTTPItIs"))
            .unwrap()
            .domains
            .remove(0);
        assert!(!d1.explicit_header);
        assert_eq!(d1.columns, vec!["column1", "column2", "column3"]);
        assert_eq!(d1.rows[0].cells[2], "HTTP It Is");

        let d2 = render_source(&doc("\t\\1\n\\ Agent Status Note\nClaude Ready CheckSession"))
            .unwrap()
            .domains
            .remove(0);
        assert!(d2.explicit_header);
        assert_eq!(d2.columns, vec!["Agent", "Status", "Note"]);
        assert_eq!(d2.rows.len(), 1);
    }

    #[test]
    fn test_expand_column_and_backslash_caret_not_header() {
        let d1 = render_source(&doc("\t\\1\nA B\nC D E\n    Fourth"))
            .unwrap()
            .domains
            .remove(0);
        assert_eq!(d1.columns, vec!["column1", "column2", "column3", "column4"]);
        assert_eq!(d1.rows.last().unwrap().cells[3], "Fourth");

        let d2 = render_source(&doc("\t\\1\n\\^0 A"))
            .unwrap()
            .domains
            .remove(0);
        assert!(!d2.explicit_header);
        assert_eq!(d2.rows[0].cells[0], "^0");
    }

    #[test]
    fn test_5w1h_and_dictionary_atoms_and_cjk_join() {
        let d = render_source(&doc("\t?1\nBuildProject MaintainQuality Rust ."))
            .unwrap()
            .domains
            .remove(0);
        assert_eq!(d.columns, AXES_5W1H);
        assert!(d.rows[0].tail_empty);
        assert_eq!(&d.rows[0].cells[3..6], &["", "", ""]);

        let en_src = doc("\t$1\nAPIKey\n\t?1\nValidate^0BeforeAccess");
        assert_eq!(
            render_source(&en_src).unwrap().domains[0].rows[0].cells[0],
            "Validate APIKey Before Access"
        );

        let zh_src = doc("\t$1\nAPIKey\n\t?1\n请检查^0是否有效");
        assert_eq!(
            render_source(&zh_src).unwrap().domains[0].rows[0].cells[0],
            "请检查APIKey是否有效"
        );

        let ja_src = doc("\t?1\n私は\n 学生\n です。");
        assert_eq!(
            render_source(&ja_src).unwrap().domains[0].rows[0].cells[0],
            "私は学生です。"
        );
    }

    #[test]
    fn test_graph_directions_and_vertical_edge() {
        let fwd = render_source(&doc("\t>1\nA Node\nB Node\nA>X>B Reads"))
            .unwrap()
            .domains
            .remove(0);
        let e1 = fwd.rows[2].edge.as_ref().unwrap();
        assert_eq!((e1.from.as_str(), e1.to.as_str()), ("A", "B"));

        let rev = render_source(&doc("\t>1\nA Node\nB Node\nA<X<B Reads"))
            .unwrap()
            .domains
            .remove(0);
        let e2 = rev.rows[2].edge.as_ref().unwrap();
        assert_eq!((e2.from.as_str(), e2.to.as_str()), ("B", "A"));

        let und = render_source(&doc("\t>1\nA Node\nB Node\nA=X=B Shares"))
            .unwrap()
            .domains
            .remove(0);
        let e3 = und.rows[2].edge.as_ref().unwrap();
        assert_eq!(e3.direction, EdgeDirection::Undirected);

        let vert = render_source(&doc(
            "\t>1\nA Node\nB Node\nA>X>B Reads CheckSession\n   RetryOnce",
        ))
        .unwrap()
        .domains
        .remove(0);
        assert_eq!(vert.rows.last().unwrap().cells[2], "Check Session Retry Once");
    }

    #[test]
    fn test_transport_roundtrips_and_rle() {
        let codec = StandardNemaCodec;
        let profile = TokenProfile::ByteHeuristic;

        let s1 = doc("\t$1\nAPIKey\n\t?1\nValidate^0BeforeAccess\n\t?2\nValidate^0BeforeAccess");
        let ch1 = codec.encode(&s1, &profile).unwrap();
        let dec1 = if ch1.format == CodecFormat::Nemat {
            codec.decode(&ch1.text).unwrap()
        } else {
            ch1.text.clone()
        };
        assert_eq!(dec1, s1);

        let s2 = doc(&format!("\t\\1\n{} AAAAAAAA", "哈".repeat(200)));
        let ch2 = codec.encode(&s2, &profile).unwrap();
        assert_eq!(ch2.format, CodecFormat::Nemat);
        assert_eq!(codec.decode(&ch2.text).unwrap(), s2);

        let s3 = doc("\t\\1\n\\^ \\^7 \\8A \\\\ \\.");
        let ch3 = codec.encode(&s3, &profile).unwrap();
        let dec3 = if ch3.format == CodecFormat::Nemat {
            codec.decode(&ch3.text).unwrap()
        } else {
            ch3.text.clone()
        };
        assert_eq!(dec3, s3);

        // Bounds check on RLE
        assert_eq!(
            codec.decode(&doc("\t$0\n\t\\1\n\\9999999A")).unwrap_err(),
            NemaError::RleCountExceedsBounds
        );
        assert_eq!(
            codec.decode(&doc("\t$0\n\t0\n\t\\1\nA")).unwrap_err(),
            NemaError::UnsupportedLegacyPrefixSelector
        );
    }

    #[test]
    fn test_reference_examples_if_present() {
        let examples_dir = std::path::Path::new("E:/.NEMA/examples");
        if !examples_dir.exists() {
            return;
        }
        let codec = StandardNemaCodec;
        let profile = TokenProfile::ByteHeuristic;
        for entry in std::fs::read_dir(examples_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) == Some("nema") {
                let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if file_name.contains("_v13") || file_name.contains("_v15") {
                    continue;
                }
                let raw = normalize_nema_newlines(&std::fs::read_to_string(&path).unwrap());
                let parsed = codec.parse(&raw).unwrap();
                assert!(!parsed.domains.is_empty());
                let ch = codec.encode(&raw, &profile).unwrap();
                if path.file_name().and_then(|s| s.to_str()) == Some("bench_high_entropy_120.nema") {
                    assert_eq!(ch.format, CodecFormat::Nema);
                }
                let roundtrip = if ch.format == CodecFormat::Nemat {
                    codec.decode(&ch.text).unwrap()
                } else {
                    ch.text
                };
                assert_eq!(roundtrip, raw, "roundtrip mismatch on {:?}", path);
            }
        }
    }

    #[test]
    fn test_lease_arbitrator_epoch_and_idempotency() {
        let mut arb = NemaLeaseArbitrator::new();
        assert_eq!(*arb.role(), OwnershipRole::Observer);

        let src = doc("\t\\1\nHello World");
        let h = NemaLeaseArbitrator::hash_source(&src);
        let (ep1, cached) = arb.request_lease(h, 1);
        assert!(cached.is_none());
        assert!(arb.pause_ime_same_epoch(ep1));

        // Stale epoch must be rejected
        assert!(!arb.pause_ime_same_epoch(ep1 + 99));

        let choice = StandardNemaCodec
            .encode(&src, &TokenProfile::ByteHeuristic)
            .unwrap();
        assert!(arb.atomic_commit(ep1, choice.clone()));
        assert!(arb.release_lease(ep1));
        assert_eq!(*arb.role(), OwnershipRole::Observer);

        // Duplicate (source_hash, revision) returns idempotent cached result without bumping epoch
        let (ep_dup, cached_dup) = arb.request_lease(h, 1);
        assert_eq!(ep_dup, ep1);
        assert_eq!(cached_dup.unwrap(), choice);
    }

    #[test]
    fn test_v12_and_r7_extensions_and_equivalent_json() {
        let src = "\t>Gext1 1\n A LoginAPI Authentication APIKey\n B TokenStore SessionStorage\nA>X>B Reads VerifySession HTTP401\n   RetryOnTimeout\n\n\n";
        let rendered = render_source(src).unwrap();
        let g = &rendered.domains[0];
        assert_eq!(g.mode, DomainMode::Graph);
        assert_eq!(g.rows[0].node_id.as_deref(), Some("A"));
        assert_eq!(g.rows[0].cells[1], "LoginAPI");
        assert_eq!(g.rows[0].cells[2], "Authentication");
        assert_eq!(
            g.rows[0].extensions,
            vec![("X3".to_string(), "APIKey".to_string())]
        );
        assert!(g.rows[1].extensions.is_empty());
        assert_eq!(g.rows[2].cells[1], "Reads");
        assert_eq!(g.rows[2].cells[2], "Verify session");
        assert_eq!(
            g.rows[2].extensions,
            vec![("X3".to_string(), "HTTP401 RetryOnTimeout".to_string())]
        );

        // Dangling edge without declared node is rejected
        assert!(parse_source(&doc("\t>G1 1\nA>X>B Reads")).is_err());
        assert!(parse_source(&doc("\t>G1 1\nA LoginAPI\nB TokenStore\nA>X>B Reads")).is_ok());

        // Compare formats on COMPLETE_V12_SAMPLE
        let rows = compare_document_formats(COMPLETE_V12_SAMPLE).unwrap();
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].format_name, ".nema");
        assert_eq!(rows[2].format_name, "JSON objects");
        assert_eq!(rows[3].format_name, "JSON positional");
        assert_eq!(rows[4].format_name, "JSON tagged (w/t/g)");
        assert_eq!(OFFICIAL_BENCHMARK_V12.len(), 8);

        // If E:/.NEMA/examples exists, verify 1:1 semantic JSON equality across all 8 benchmark scenarios
        let examples_dir = std::path::Path::new("E:/.NEMA/examples");
        if examples_dir.exists() {
            for name in [
                "english_60",
                "chinese_60",
                "japanese_60",
                "table_200",
                "graph_40n_160e",
                "high_entropy_60",
                "short_what",
                "graph_3n_4e",
            ] {
                let nema_path = examples_dir.join(format!("{name}.nema"));
                let json_path = examples_dir.join(format!("{name}.json"));
                if nema_path.exists() && json_path.exists() {
                    let raw = std::fs::read_to_string(&nema_path)
                        .unwrap()
                        .replace("\r\n", "\n");
                    let expected: serde_json::Value =
                        serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap())
                            .unwrap();
                    let actual = render_to_json_value(&render_source(&raw).unwrap());
                    assert_eq!(actual, expected, "semantic JSON mismatch on {name}");
                }
            }
        }
    }
    #[test]
    fn test_space_domain_header_v12() {
        // v1.2 字母域名头支持空格前缀与 \ 标记
        assert_eq!(parse_domain_header_v12(r" \?task1 1"), Some((false, "task1".into(), 1)));
        assert_eq!(parse_domain_header_v12(r" \>auth1 1"), Some((true, "auth1".into(), 1)));
        assert_eq!(parse_domain_header_v12(r" >auth1 1"), Some((true, "auth1".into(), 1)));
        assert_eq!(parse_domain_header_v12(r" ?task1 1"), Some((false, "task1".into(), 1)));
        assert_eq!(parse_domain_header_v12("\t\\>auth1 1"), Some((true, "auth1".into(), 1)));
        assert_eq!(parse_domain_header_v12("\t>Gauth1 1"), Some((true, "Gauth1".into(), 1)));
        assert!(parse_domain_header_v12(r"  \>auth1 1").is_none(), "double space must be rejected as continuation line");

        // 全局域头解析支持单空格数字域头
        assert_eq!(parse_domain_header(r" \1 1"), Some((DomainMode::Table, "1".into(), 1)));
        assert_eq!(parse_domain_header(r" \?2 1"), Some((DomainMode::FiveWOneH, "2".into(), 1)));
        assert_eq!(parse_domain_header(r" \>3 1"), Some((DomainMode::Graph, "3".into(), 1)));

        // 空格前缀的 Table 领域头及表头识别测试
        let src_space_table = " Ttable1 1\n Agent Status Note\nClaude Ready 认证完成\n\n\n";
        let doc_space = render_source(src_space_table).expect("space table parse");
        assert_eq!(doc_space.domains[0].mode, DomainMode::Table);
        assert_eq!(doc_space.domains[0].columns, vec!["Agent", "Status", "Note"]);

        let src_slash_table = " \\table1 1\n \\ Agent Status Note\nClaude Ready 认证完成\n\n\n";
        let doc_slash = render_source(src_slash_table).expect("slash table parse");
        assert_eq!(doc_slash.domains[0].mode, DomainMode::Table);
        assert_eq!(doc_slash.domains[0].columns, vec!["Agent", "Status", "Note"]);
    }
}