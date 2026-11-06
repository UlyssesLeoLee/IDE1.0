//! `nema-codec` — IDE1.0 × IME1.0 共享的 NEMA v1.5 编解码器、所有权仲裁状态机与渲染视图引擎。
//!
//! 严格对应 `E:\.NEMA\codec\core.py` 与 `E:\.NEMA\codec\transport.py` 的全部语法不变量、
//! 字典候选增益算法、RLE 上下界 (`4..=100_000`)、错误码及测试向量。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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
}

/// 解析后的权威 `NemaDocument`（保留原始 span、column 与每列 `Vec<Vec<SemanticChunk>>`）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl EncodeMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::DictionaryAndRle => "dictionary+rle",
            Self::DictionaryOnly => "dictionary",
            Self::RleOnly => "rle",
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
        if item.opaque_dict_index.is_some() {
            out.push(item.text);
        } else if contains_cjk(&item.text) {
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

/// 解析 `\t(\|?|>)[1-9][0-9]*(?: [1-9][0-9]*)?` 域头
pub fn parse_domain_header(line: &str) -> Option<(DomainMode, String, u64)> {
    let rest = line.strip_prefix('\t')?;
    let mut chars = rest.chars();
    let mode_ch = chars.next()?;
    let mode = match mode_ch {
        '\\' => DomainMode::Table,
        '?' => DomainMode::FiveWOneH,
        '>' => DomainMode::Graph,
        _ => return None,
    };
    let tail = &rest[mode_ch.len_utf8()..];
    if let Some((id_part, rev_part)) = tail.split_once(' ') {
        if !is_positive_digits(id_part) || !is_positive_digits(rev_part) {
            return None;
        }
        let rev: u64 = rev_part.parse().ok()?;
        Some((mode, id_part.to_string(), rev))
    } else {
        if !is_positive_digits(tail) {
            return None;
        }
        Some((mode, tail.to_string(), 1))
    }
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
            chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
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
    for (row_idx, &line) in lines.iter().enumerate().skip(i) {
        let line_start = line_byte_offsets[row_idx];
        let line_no = row_idx + 1;
        if let Some((mode, id, revision)) = parse_domain_header(line) {
            if !seen_domain_ids.insert(id.clone()) {
                return Err(NemaError::DuplicateDomainId);
            }
            let columns = match mode {
                DomainMode::Table => Vec::new(),
                DomainMode::FiveWOneH => AXES_5W1H.iter().map(|s| (*s).to_string()).collect(),
                DomainMode::Graph => COLUMNS_GRAPH.iter().map(|s| (*s).to_string()).collect(),
            };
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
            });
            continue;
        }

        if line.contains('\t') {
            return Err(NemaError::TabInBody);
        }
        let Some(current) = doc.domains.last_mut() else {
            return Err(NemaError::BodyBeforeDomain);
        };

        // Table 首行精确 `\ ` 表头判断
        if current.mode == DomainMode::Table
            && current.records.is_empty()
            && !current.explicit_header
            && line.starts_with("\\ ")
        {
            let names: Vec<&str> = line[2..].split(' ').collect();
            if names.is_empty() || names.iter().any(|x| !is_valid_table_header_ident(x)) {
                return Err(NemaError::InvalidTableHeaderName);
            }
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
            continue;
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

    // Graph 域完整性校验：节点与边 ID 不可重复，边引用端点必须已声明为节点
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
        for (src, dst) in endpoints {
            if !nodes.contains(&src) || !nodes.contains(&dst) {
                return Err(NemaError::UndefinedGraphNode(src, dst));
            }
        }
    }

    Ok(doc)
}

/// 渲染 `NemaDocument` 为结构化视图（与 `core.py::render` 1:1 对应）
pub fn render_document(doc: &NemaDocument) -> Result<RenderedDocument, NemaError> {
    let mut out = Vec::with_capacity(doc.domains.len());
    for d in &doc.domains {
        let mut rows = Vec::with_capacity(d.records.len());
        for rec in &d.records {
            let mut vals = Vec::with_capacity(rec.cells.len().max(6));
            for col in &rec.cells {
                let raw_parts: Vec<&str> = col.iter().map(|c| c.raw.as_str()).collect();
                vals.push(join_cell(&raw_parts, &doc.dictionary)?);
            }
            if d.mode == DomainMode::FiveWOneH {
                while vals.len() < 6 {
                    vals.push(String::new());
                }
            }
            rows.push(RenderedRow {
                cells: vals,
                groups: rec.raw_groups(),
                tail_empty: rec.explicit_tail_empty,
                node_id: rec.graph_node_id.clone(),
                edge: rec.graph_edge.clone(),
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
        if row.starts_with('\t') && parse_domain_header(row).is_some() {
            lines.push(row.to_string());
        } else {
            lines.push(encode_row(row, words, use_rle));
        }
    }
    let mut joined = lines.join("\n");
    joined.push_str("\n\n\n");
    joined
}

/// 将 `.nema` 源码编码为最优 `.nemat` 或保留 `.nema`（与 `transport.py::encode` 1:1 对应）
pub fn encode_with_profile(
    source: &str,
    profile: &TokenProfile,
    max_entries: usize,
    locked_dictionary: &[String],
) -> Result<EncodedChoice, NemaError> {
    parse_source(source)?;
    let (baseline, is_exact_token, metric_note) = profile.measure(source);
    let rows: Vec<&str> = source[..source.len() - 3].split('\n').collect();

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

/// 将 `.nemat` 传输流解码并校验还原为权威 `.nema` 源码（与 `transport.py::decode` 1:1 对应）
pub fn decode_transport(text: &str) -> Result<String, NemaError> {
    if text.len() > MAX_BYTES {
        return Err(NemaError::TransportTooLarge);
    }
    if text.contains('\r') || !text.ends_with("\n\n\n") || text.ends_with("\n\n\n\n") {
        return Err(NemaError::InvalidTransportTermination);
    }
    let rows: Vec<&str> = text[..text.len() - 3].split('\n').collect();
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
        if v.starts_with('\t') && parse_domain_header(&v).is_some() {
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
        if line.starts_with('\t') {
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

/// 将任意换行规范化为合法 `.nema` 尾部 `\n\n\n`
pub fn normalize_nema_newlines(source: &str) -> String {
    let lf = source.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = lf.trim_end_matches('\n');
    if trimmed.is_empty() {
        "\t\\1\n\\ Column1 Column2\nItem1 Ready\n\n\n".to_string()
    } else {
        format!("{trimmed}\n\n\n")
    }
}

/// 在指定 Table 域（或第一个 Table 域）首行前切换 `\\ ` 表头前缀（支持字节级无损往返）
pub fn toggle_table_header_in_source(source: &str, target_domain_id: Option<&str>) -> String {
    let lf = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = lf
        .trim_end_matches('\n')
        .split('\n')
        .map(str::to_string)
        .collect();
    let mut in_target_table = false;
    for line in &mut lines {
        if let Some((mode, id, _)) = parse_domain_header(line) {
            in_target_table = mode == DomainMode::Table
                && target_domain_id.map_or(true, |target| target == id);
            continue;
        }
        if in_target_table && !line.starts_with('\t') && !line.trim().is_empty() {
            if let Some(stripped) = line.strip_prefix("\\ ") {
                *line = stripped.to_string();
            } else {
                // Sanitize tokens to valid header identifiers if needed
                let parts: Vec<String> = line
                    .split_whitespace()
                    .enumerate()
                    .map(|(idx, tok)| {
                        let clean: String = tok
                            .chars()
                            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
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
            break;
        }
    }
    let mut out = lines.join("\n");
    out.push_str("\n\n\n");
    out
}

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
                let raw = std::fs::read_to_string(&path)
                    .unwrap()
                    .replace("\r\n", "\n");
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
}
