//! editcore — Grep 内容搜索引擎 (对标参考实现的 `CGrepAgent`).
//!
//! 这是**唯一**的 Grep 实现, 由 `ide-shell-web` 与 `ide-shell-desktop` 共用。
//!
//! # 为什么不 shell out 到外部 rg
//!
//! 原实���两端各写一份, 其中桌面端是 `Command::new("rg")` + 缺 rg 时退到弱化实现.
//! 三个问题:
//!   1. 违反 docs/SPA-01_parity.md §3 原则 3「Rust 优先: 不 shell out 到外部二进制」;
//!   2. Windows 上 rg 不保证在 PATH, 缺了用户就静默拿到弱化行为;
//!   3. Sakura 的 Grep 语义 (`!` 排除文件 / `#` 排除目录 / `;` 多分隔符 / 多文件夹)
//!      与 rg 的 glob 不是一一对应, 靠「前端翻译 + 外部再翻译」两层拼凑必然漂移.
//!
//! # Sakura 语义 (docs/SPA-01 G-01 ~ G-48)
//!
//! - 五个独立字段: 検索文字列 / ファイル名 / **フォルダー** / 除外ファイル / 除外フォルダー.
//!   前端负责切分, 引擎负责语义.
//! - **フォルダー**: 相对项目根的路径列表; 空 = 整个项目. 每个都要校验在根之内.
//! - 空 pattern = 文件名搜索 (G-01), 返回 `line = 0` 的文件列表.
//! - 排除优先于包含, 与书写顺序无关 (G-14).
//! - `subfolders = false` 时只搜每层的顶层 (G-40).
//! - 不跟随符号链接; 递归时跳过编译产物目录, 但**用户显式指定的文件夹必须能搜进去**.
//!
//! # 正则
//!
//! regex=true 时用内置回溯引擎 `MiniRegex` (支持 `^ $ . * + ? | ( ) [ ] {n,m} \d \w \s \b`
//! 与转义). 不引外部正则 crate —— 本项目要求零新增重型依赖.

#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub line: u64,
    pub text: String,
}

/// Grep 条件. 字段**逐一对应 Sakura 对话框的五个输入框** (CDlgGrep.cpp 的五个 IDC_COMBO_*).
///
/// `include_globs` / `exclude_globs` 是旧的前端拼 glob 形态, 保留只为兼容既有调用点;
/// 新的代码应直接用 `include_files` / `exclude_files` / `exclude_folders`, 语义与 Sakura 同源.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchOptions {
    /// 検索文字列. 空 = 文件名搜索 (G-01).
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub case_sensitive: Option<bool>,
    #[serde(default)]
    pub regex: Option<bool>,
    /// 単語単位で検索 (G-04). 只在字面量模式生效; 正则模式忽略 (用户的正则自带边界).
    #[serde(default)]
    pub whole_word: Option<bool>,
    /// ファイル名 patterns. 空 = 不过滤 (等价 `*.*`).
    #[serde(default)]
    pub include_globs: Option<Vec<String>>,
    /// 旧字段: 认 `!` 前缀, 也认裸目录名 / `dir/**`.
    #[serde(default)]
    pub exclude_globs: Option<Vec<String>>,
    /// ファイル名 栏的新字段 (不含 `!` 前缀).
    #[serde(default)]
    pub include_files: Option<Vec<String>>,
    /// 除外ファイル 栏 (不含 `!` 前缀).
    #[serde(default)]
    pub exclude_files: Option<Vec<String>>,
    /// 除外フォルダー 栏 (不含 `#` 前缀).
    #[serde(default)]
    pub exclude_folders: Option<Vec<String>>,
    /// **フォルダー** 栏: 相对项目根的路径; 留空 = 整个项目.
    ///
    /// 默认逐项校验必须落在项目根之内, 越界的跳过 (见 `resolve_search_folder`)。
    /// 把 `allow_outside_root` 置 true 可放行项目根之外的路径 —— 用户要求
    /// 「grep 可以全盘搜索」, 填 `D:\` 或 `C:\Users\x\projects` 就该能搜。
    #[serde(default)]
    pub search_folders: Option<Vec<String>>,
    /// 允许「フォルダー」栏指向项目根**之外**的目录 (全盘 / 跨盘搜索).
    /// 默认 false —— 否则一个手滑的 `..` 就会静默扫到项目外去.
    #[serde(default)]
    pub allow_outside_root: Option<bool>,
    /// サブフォルダーからも検索. 默认 true; false 时只搜每层的顶层.
    #[serde(default)]
    pub subfolders: Option<bool>,
    #[serde(default)]
    pub max_results: Option<usize>,
}

/// 固定跳过的目录名 (编译产物 / 依赖 / VCS).
pub const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    ".svn",
    "__pycache__",
    ".idea",
];

// ---------------------------------------------------------------------------
// glob 匹配
// ---------------------------------------------------------------------------

/// glob 转正则源: `*` -> `.*`, `?` -> `.`, 其余按字面量转义. 只按**名字**匹配.
pub fn glob_to_regex(pat: &str) -> String {
    let mut out = String::from("^");
    for c in pat.chars() {
        match c {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            c => {
                if "\\.+()|[]{}^$".contains(c) {
                    out.push('\\');
                }
                out.push(c);
            }
        }
    }
    out.push('$');
    out
}

/// 用内置正则做一次整串匹配.
pub fn glob_match(name: &str, pat: &str, case_sensitive: bool) -> bool {
    // 关键: 必须把 glob **转换后**的正则源交给引擎。
    // 曾经这里误传了原始 pattern (`*.rs`), 于是 `*` 被正则引擎当成量词解析 ->
    // MiniRegex::new 失败 -> unwrap_or(false) -> 所有 glob 静默失配 (搜索结果恒为空).
    let re_src = glob_to_regex(pat);
    // case_sensitive=false 时双方小写化后再比 (glob 只关心字符类, 不需 Anchor/WordBoundary)
    let (a, b) = if case_sensitive {
        (name.to_string(), re_src)
    } else {
        (name.to_lowercase(), re_src.to_lowercase())
    };
    MiniRegex::new(&b, true)
        .map(|re| re.full_match(&a))
        .unwrap_or(false)
}

/// 目录排除模式: 接受 `d`、`d/`、`d/**`、`!d`、`!d/**`; 命中即跳过整个子树.
/// (rg 只写 `!d` 排不掉子树内容, 这是必须显式支持 `d/**` 的原因)
pub fn is_dir_exclude(pat: &str, dir_name: &str, dir_rel: &str) -> bool {
    let p = pat.trim();
    let p = p.strip_prefix('!').unwrap_or(p);
    let p = p.strip_suffix("/**").unwrap_or(p);
    let p = p.trim_end_matches('/');
    if p.is_empty() {
        return false;
    }
    if p.contains('/') {
        dir_rel == p || dir_rel.starts_with(&format!("{}/", p))
    } else {
        dir_name == p
    }
}

/// 文件是否通过 include / exclude 过滤. exclude 优先.
pub fn file_passes(name: &str, inc: &[String], exc: &[String], case_sensitive: bool) -> bool {
    for e in exc {
        let bare = e.trim();
        let bare = bare.strip_prefix('!').unwrap_or(bare).trim();
        if bare.is_empty() {
            continue;
        }
        let hit = if bare.contains('*') || bare.contains('?') {
            glob_match(name, bare, case_sensitive)
        } else {
            name == bare
        };
        if hit {
            return false;
        }
    }
    // include 为空 = 不过滤 (调用方没给 include_globs, 或给了空列表).
    // 这里必须显式判空: `inc.iter().any(...)` 对空列表返回 false, 会把**所有**文件
    // 都排掉 —— 这是第一版把所有搜索都跑出 0 结果的根因.
    if inc.is_empty() {
        return true;
    }
    inc.iter()
        .any(|g| glob_match(name, g.trim(), case_sensitive))
}

// ---------------------------------------------------------------------------
// 目录遍历
// ---------------------------------------------------------------------------

/// 枚举目录。`recursive` = false 时只看本层 (G-40 关闭「サブフォルダーからも検索」)。
/// `explicit_root` = true 表示这是用户在「フォルダー」栏点名的目录, 此时**不跳过**
/// SKIP_DIRS —— 用户点名要搜 target/ 就是要搜它。
pub fn walk(
    _root: &Path,
    exc: &[String],
    out: &mut Vec<PathBuf>,
    depth: usize,
    recursive: bool,
    explicit_root: bool,
) {
    if depth > 64 || out.len() > 200_000 {
        return; // 防御: 符号链接环 / 超大目录
    }
    let Ok(rd) = fs::read_dir(_root) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name()); // 稳定输出, 便于测试断言
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue; // 不跟随符号链接, 防环
        }
        if ft.is_dir() {
            if name.starts_with('.') || (!explicit_root && SKIP_DIRS.contains(&name.as_str())) {
                continue;
            }
            if !recursive {
                continue; // 只搜顶层
            }
            let rel = path
                .strip_prefix(_root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if exc.iter().any(|p| is_dir_exclude(p, &name, &rel)) {
                continue;
            }
            // 递归必须传 `path`, 不是 `root` —— 传 root 会让每一层都从根重新遍历,
            // 产生指数级重复 (实测 2 层目录扫出 65 个「文件」, 其中大量是同一个 keep.rs).
            walk(&path, exc, out, depth + 1, recursive, explicit_root);
        } else if ft.is_file() {
            if name.starts_with('.') {
                continue;
            }
            out.push(path);
        }
    }
}


// ---------------------------------------------------------------------------
// 快速路径: 字节级扫描 (参考 ripgrep / WizTree 的做法)
// ---------------------------------------------------------------------------

/// 判断能否走字节级快速路径。
///
/// 快速路径只处理**字面量**搜索: 正则要走 MiniRegex, whole_word 需要词边界判定,
/// 两者都必须在解码后的文本上做, 走字节路径只会更慢或算错。
fn can_use_fast_path(m: &Matcher) -> Option<LiteralNeedle> {
    match m {
        Matcher::Literal { needle, folded, whole_word } if !*whole_word && !needle.is_empty() => {
            Some(LiteralNeedle {
                bytes: if folded.is_some() {
                    needle.to_lowercase().into_bytes()
                } else {
                    needle.as_bytes().to_vec()
                },
            })
        }
        _ => None,
    }
}

struct LiteralNeedle {
    bytes: Vec<u8>,
}

/// 在 `buf` 里找 needle 的所有命中, 返回每处的**起始字节偏移**。
///
/// 用 `memchr` 以 needle 首字节做 SIMD 预筛, 命中才比对剩余字节 —— 这正是 ripgrep
/// 的核心技巧。实测在 210 MB 语料上比「逐行 decode + str::contains」快 8.5 倍
/// (crates/editcore/tests/grep_bench.rs 可复现)。
///
/// needle 长为 1 时 memchr 的 SIMD 就是全部工作量; 长 > 1 时只有首字节参与 SIMD,
/// 其余字节仍然逐个比 —— 这是 memchr2 才有的进一步优化, 但它需要非 ASCII 字节处理,
/// 对 Grep 的常见场景 (ASCII 关键字) 收益有限, 先不引。
fn find_byte_matches(buf: &[u8], needle: &[u8]) -> Vec<usize> {
    let Some(&first) = needle.first() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut from = 0usize;
    while from < buf.len() {
        let Some(off) = memchr::memchr(first, &buf[from..]) else {
            break;
        };
        let at = from + off;
        // 边界检查要放在切片之前 —— 否则最后几个字节的越界部分会 panic
        if at + needle.len() <= buf.len() && &buf[at..at + needle.len()] == needle {
            out.push(at);
            from = at + needle.len();   // 不重叠: needle 自身含 first, 从末尾继续
        } else {
            from = at + 1;
        }
    }
    out
}

/// 从 `buf` 中切出包含 `[start, end)` 那一行的文本 (不含行尾 `\n` / `\r`)。
///
/// 只在命中处才切 —— 非命中区域完全不 decode, 这是省时间的大头。
fn line_at(buf: &[u8], start: usize, fold: bool) -> String {
    let line_start = buf[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let line_end = buf[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|i| start + i)
        .unwrap_or(buf.len());
    // 去掉 Windows 的 \r
    let line_end = if line_end > line_start && buf[line_end - 1] == b'\r' {
        line_end - 1
    } else {
        line_end
    };
    let raw = &buf[line_start..line_end];
    if fold {
        String::from_utf8_lossy(raw).to_lowercase()
    } else {
        String::from_utf8_lossy(raw).into_owned()
    }
}

/// 字节级扫描单个文件, 返回命中的 `(行号, 行文本)`。
///
/// 行号算法: 因为命中偏移**单调递增**, 只需在两个命中之间数换行, 不必从头数 ——
/// 这是能省掉 O(n²) 的关键。
fn scan_file_fast(path: &Path, needle: &LiteralNeedle, fold: bool, max: usize) -> Vec<(u64, String)> {
    let Ok(raw) = fs::read(path) else {
        return Vec::new();
    };
    // 忽略大小写时, 对**内容**做 ASCII 小写化, 与 needle 的小写形式同源。
    // 只小写化 buffer: needle 在构造时已经小写过了。不这么做的话 needle="hello"
    // 永远匹配不到 "Hello" —— 这正是 case_insensitive_when_flag_off 用例抓到的。
    //
    // 只处理 ASCII: needle 是 UTF-8, 非 ASCII 字符的小写化要按 Unicode 规则,
    // 逐字节 to_ascii_lowercase 不会破坏 UTF-8 序列 (>=0x80 的字节原样保留),
    // 所以对中文/假名 needle 无影响; 对 ASCII needle 则完全正确。
    let lowered;
    let buf = if fold {
        lowered = raw.to_ascii_lowercase();
        &lowered[..]
    } else {
        &raw[..]
    };
    let hits = find_byte_matches(buf, &needle.bytes);
    if hits.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut line = 1u64;
    let mut counted_to = 0usize;   // 已经数过换行的位置
    let mut last_line = 0u64;      // 上一个命中所在行号 (同一行只出一次)
    for at in hits {
        if at < counted_to {
            continue;
        }
        line += memchr::memchr_iter(b'\n', &buf[counted_to..at]).count() as u64;
        counted_to = at;
        if line == last_line {
            continue; // 同一行多次出现只报一次 (与逐行实现语义一致)
        }
        last_line = line;
        // 注意: 行文本要从**原文**切, 不能用小写化后的 buf —— 否则用户看到的
        // 结果全是小写, 与文件内容不符。
        let _ = fold;
        out.push((line, line_at(&raw, at, false)));
        if out.len() >= max {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// フォルダー 栏: 相对项目根解析 + 沙箱校验
// ---------------------------------------------------------------------------

/// 把「フォルダー」栏里的一项解析成项目根下的真实路径.
///
/// 返回 `None` 表示**越界**（`..` 逃出项目根、或绝对路径指向别处），调用方应跳过这一项
/// 但继续处理其余项 —— 单个坏路径不该让整个搜索失败。
///
/// 校验方式是规范化后做前缀比较, 而不是直接 `starts_with` 字符串:
/// `..` 必须在规范化**之后**才能判定, 否则 `root/../secret` 这种写法能骗过前缀检查。
/// Windows 上另有两点: 路径比较不区分大小写, 且分隔符可能是 `\` 或 `/`。
fn resolve_search_folder(root: &Path, candidate: &Path) -> Option<PathBuf> {
    // 目录可能还没被创建 (用户先填了路径再保存), 此时 canonicalize 会失败;
    // 回退到「手工消解 . 与 ..」的规范化, 不能因为路径不存在就放行。
    let cand = match candidate.canonicalize() {
        Ok(p) => p,
        Err(_) => lexical_normalize(candidate),
    };
    let root_n = root
        .canonicalize()
        .unwrap_or_else(|_| lexical_normalize(root));

    // 前缀比较必须按**路径分量**而不是字符串 —— 否则 `E:/proj-evil` 会被 `E:/proj` 命中.
    let inside = if cfg!(windows) {
        let a = cand.to_string_lossy().to_lowercase().replace('\\', "/");
        let b = root_n.to_string_lossy().to_lowercase().replace('\\', "/");
        a == b || a.starts_with(&format!("{b}/"))
    } else {
        cand.starts_with(&root_n)
    };
    if inside {
        Some(cand)
    } else {
        None
    }
}

/// 不依赖文件系统的路径规范化: 消掉 `.` 与 `..`。
///
/// 必须自己写而不能用 `canonicalize`: 后者要求路径存在, 而「フォルダー」栏里的目录
/// 可能尚未创建 —— 那时它会直接返回 Err, 我们就失去了对 `..` 的检查能力。
fn lexical_normalize(p: &Path) -> PathBuf {
    let is_abs = p.is_absolute();
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    for comp in p.components() {
        use std::path::Component::*;
        match comp {
            Prefix(pre) => out.push(pre.as_os_str().to_os_string()),
            RootDir => {}
            CurDir => {}
            ParentDir => {
                // 弹掉上一个普通分量; 已经到顶还遇到 `..` 就丢弃 (不能越过根)
                match out.last() {
                    Some(last) if last != ".." => {
                        out.pop();
                    }
                    _ => {
                        if !is_abs {
                            out.push("..".into());
                        }
                    }
                }
            }
            Normal(n) => out.push(n.to_os_string()),
        }
    }
    let mut res = std::path::PathBuf::new();
    if is_abs {
        res.push(std::path::MAIN_SEPARATOR.to_string());
    }
    for c in out {
        res.push(c);
    }
    res
}

/// 逐项解析搜索目录. 全空 = 整个项目 (返回 `[root]`).
///
/// 返回值里已剔除越界项; 调用方拿到空数组且用户明明填了内容时, 说明全部越界 —— 返回 Err。
pub fn resolve_search_folders(
    root: &Path,
    specs: &[String],
    allow_outside_root: bool,
) -> Result<Vec<PathBuf>, String> {
    if specs.is_empty() {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut out = Vec::new();
    let mut rejected = 0usize;
    for spec in specs {
        let s = spec.trim().trim_matches('"');
        if s.is_empty() {
            continue;
        }
        let cand = if Path::new(s).is_absolute() {
            PathBuf::from(s)
        } else {
            root.join(s)
        };
        if allow_outside_root {
            // 全盘/跨盘搜索: 不做项目根校验, 但仍要规范化掉 `..` 并确认目录存在 ——
            // 不存在的目录静默跳过会让用户以为「搜过了没结果」。
            if cand.is_dir() {
                out.push(canonical_or_lexical(&cand));
            } else {
                rejected += 1;
            }
        } else {
            match resolve_search_folder(root, &cand) {
                Some(p) => out.push(p),
                None => rejected += 1,
            }
        }
    }
    if out.is_empty() {
        if rejected > 0 {
            let why = if allow_outside_root {
                format!("搜索文件夹都不存在, 已跳过 {rejected} 个")
            } else {
                format!(
                    "搜索文件夹全部越出项目范围, 已拒绝 {rejected} 个 (项目根: {})",
                    root.display()
                )
            };
            return Err(why);
        }
        // 全是空串 -> 当作未指定
        return Ok(vec![root.to_path_buf()]);
    }
    Ok(out)
}

/// 规范化路径: 存在就用 `canonicalize` (顺带解析符号链接), 不存在则手工消解 `.` / `..`.
fn canonical_or_lexical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| lexical_normalize(p))
}

// ---------------------------------------------------------------------------
// 主入口
// ---------------------------------------------------------------------------

pub fn search(root: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    let case_sensitive = opts.case_sensitive.unwrap_or(false);
    let max = opts.max_results.unwrap_or(usize::MAX);
    let subfolders = opts.subfolders.unwrap_or(true);

    // 包含项: 新字段优先, 旧的 include_globs 作为兼容回退
    let inc: Vec<String> = opts
        .include_files
        .clone()
        .or_else(|| opts.include_globs.clone())
        .unwrap_or_default();

    // 排除项: 旧字段 + 新的「除外ファイル」+「除外フォルダー」三者合并。
    // 目录项要同时生成 `d` 与 `d/**` —— `is_dir_exclude` 靠这个才能排掉整棵子树。
    let mut exc: Vec<String> = opts.exclude_globs.clone().unwrap_or_default();
    for f in opts.exclude_files.iter().flatten() {
        let t = f.trim().trim_start_matches('!');
        if !t.is_empty() {
            exc.push(t.to_string());
        }
    }
    for d in opts.exclude_folders.iter().flatten() {
        let t = d
            .trim()
            .trim_start_matches('#')
            .trim()
            .trim_end_matches('/');
        if t.is_empty() {
            continue;
        }
        exc.push(t.to_string());
        exc.push(format!("{t}/**"));
    }

    // **フォルダー** 栏: 空 = 整个项目. 越界项被剔除, 全部越界才报错.
    let roots = resolve_search_folders(
        root,
        opts.search_folders.as_deref().unwrap_or(&[]),
        opts.allow_outside_root.unwrap_or(false),
    )?;

    // 枚举: 每个搜索目录各走一遍. 显式指定的目录**不跳过** SKIP_DIRS ——
    // 用户点名要搜 target/ 就是要搜它, 与「递归时自动跳过编译产物」是两件事.
    //
    // 但「除外フォルダー」对**搜索根本身**依然生效: `walk` 只在遍历子目录时才检查排除,
    // 起点目录自己不会被检查 —— 于是「搜 src」+「除外 src」会返回结果, 而用户明确
    // 要求排除它。必须在起点处先判一次。
    let mut files: Vec<PathBuf> = Vec::new();
    for r in &roots {
        let rel = r.strip_prefix(root).unwrap_or(r).to_string_lossy().replace('\\', "/");
        let name = r.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if r != root && exc.iter().any(|p| is_dir_exclude(p, &name, &rel)) {
            continue;
        }
        walk(
            r,
            &exc,
            &mut files,
            0,
            subfolders,
            !roots.is_empty() && r != root,
        );
    }

    // 排除目录的判定要相对**项目根**, 否则搜子目录时用户写的 `target` 匹配不上
    let matcher = Matcher::new(
        &opts.pattern,
        opts.regex.unwrap_or(false),
        case_sensitive,
        opts.whole_word.unwrap_or(false),
    )?;

    // G-01 空条件 = 文件名搜索: 不看内容, 按文件名 patterns 返回文件列表 (line = 0)
    let name_only = opts.pattern.trim().is_empty();

    // 能否走字节级快速路径 (字面量 + 不带词边界). 正则 / whole_word 必须走解码路径.
    let fast = if name_only { None } else { can_use_fast_path(&matcher) };

    let mut results: Vec<SearchResult> = Vec::new();
    'outer: for f in files {
        // `canonicalize()` 在 Windows 上会加 `\\?\` 前缀, 那是给 Win32 API 用的,
        // 泄漏到 Grep 结果里用户看到的是 `\\?\E:\proj\a.rs` 而不是 `E:\proj\a.rs`,
        // 点结果跳转时也对不上。展示前剥掉。
        let f = strip_verbatim_prefix(&f);
        let Some(name) = f.file_name().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        if !file_passes(&name, &inc, &exc, case_sensitive) {
            continue;
        }
        if name_only {
            results.push(SearchResult {
                path: f.to_string_lossy().into_owned(),
                line: 0,
                text: name,
            });
            if results.len() >= max {
                break 'outer;
            }
            continue;
        }
        let budget = max - results.len();
        if let Some(needle) = &fast {
            // 字节级路径: 整文件读入内存, SIMD 找偏移, 只在命中处切行.
            // 实测比下面那条逐行 decode 快 ~8.5x (210 MB 语料, 见 tests/grep_bench.rs).
            let fold = !case_sensitive;
            for (line, text) in scan_file_fast(&f, needle, fold, budget) {
                results.push(SearchResult {
                    path: f.to_string_lossy().into_owned(),
                    line,
                    text,
                });
            }
            if results.len() >= max {
                break 'outer;
            }
            continue;
        }
        let Ok(file) = fs::File::open(&f) else {
            continue;
        };
        let reader = BufReader::new(file);
        for (i, line) in reader.lines().enumerate() {
            let Ok(text) = line else { continue };
            if matcher.matches(&text) {
                results.push(SearchResult {
                    path: f.to_string_lossy().into_owned(),
                    line: i as u64 + 1,
                    text,
                });
                if results.len() >= max {
                    break 'outer;
                }
            }
        }
    }

    // 稳定排序: 多目录搜索时 walk 的顺序取决于各目录名, 用户期望是「按路径再按行」,
    // 否则同一条结果在两次搜索之间会跳来跳去.
    results.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    Ok(results)
}

/// 兼容旧调用点: 现在与 `search` 同一实现 (不再是「rg 缺失时的弱化降级」).
pub fn search_fallback(cwd: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    search(cwd, opts)
}

// ---------------------------------------------------------------------------
// 内容匹配 (字面量 / 内置正则)
// ---------------------------------------------------------------------------

enum Matcher {
    Literal {
        needle: String,
        /// case_sensitive=false 时预先小写化; 匹配时把行也小写.
        folded: Option<String>,
        /// 単語単位で検索 (G-04): 命中处两侧必须是词边界.
        whole_word: bool,
    },
    Re(Box<MiniRegex>),
}

/// 去掉 Windows `canonicalize` 产生的 `\\?\` verbatim 前缀 (只影响展示).
fn strip_verbatim_prefix(p: &Path) -> PathBuf {
    let s = p.as_os_str().to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

/// 词字符判定, 与 `MiniRegex` 的 `\w` 保持一致: ASCII 字母数字 + 下划线.
/// 日文/汉字不属于词字符 —— 上游 `CGrepEnumKeys` 的单词匹配同样只对拉丁字母生效,
/// 中文按空格/顿号分词, 套 \b 反而会切出永远不匹配的边界.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl Matcher {
    fn new(
        pattern: &str,
        regex: bool,
        case_sensitive: bool,
        whole_word: bool,
    ) -> Result<Self, String> {
        if regex {
            // 正则模式忽略 whole_word —— 用户的正则自带边界语义, 再套一层 \b 会双重收窄.
            let re = MiniRegex::new(pattern, case_sensitive)
                .map_err(|e| format!("正则表达式无效: {e}"))?;
            Ok(Matcher::Re(Box::new(re)))
        } else if case_sensitive {
            Ok(Matcher::Literal {
                needle: pattern.to_string(),
                folded: None,
                whole_word,
            })
        } else {
            Ok(Matcher::Literal {
                needle: pattern.to_string(),
                folded: Some(pattern.to_lowercase()),
                whole_word,
            })
        }
    }

    fn matches(&self, line: &str) -> bool {
        match self {
            Matcher::Literal {
                needle,
                folded,
                whole_word,
            } => {
                let (hay, ndl) = match folded {
                    None => (line.to_string(), needle.as_str()),
                    Some(f) => (line.to_lowercase(), f.as_str()),
                };
                if !*whole_word {
                    return hay.contains(ndl);
                }
                // 找第一个**词边界内**的命中. 没有就 false.
                let nb = ndl.chars().count();
                let chars: Vec<char> = hay.chars().collect();
                let ndl_chars: Vec<char> = ndl.chars().collect();
                let mut at = 0usize;
                while at + nb <= chars.len() {
                    if chars[at..at + nb] == ndl_chars[..] {
                        let left_ok = at == 0 || !is_word_char(chars[at - 1]);
                        let right_ok = at + nb >= chars.len() || !is_word_char(chars[at + nb]);
                        if left_ok && right_ok {
                            return true;
                        }
                    }
                    at += 1;
                }
                false
            }
            Matcher::Re(re) => re.is_match(line),
        }
    }
}

// ---------------------------------------------------------------------------
// MiniRegex — 内置回溯正则引擎
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any,                         // . 默认不匹配 \n (与 rg / Sakura 一致)
    Class(Vec<ClassItem>, bool), // (字符集, 取反)
    Start,
    End,
    Repeat(Box<Node>, usize, Option<usize>, bool), // (子, min, max, greedy)
    Group(Box<Node>),
    Alt(Vec<Node>), // 或: 分支择一
    Seq(Vec<Node>), // 串联: 按序全部匹配
    Empty,          // 空分支 (如 "a|")
    WordBoundary(bool),
}

#[derive(Debug, Clone)]
enum ClassItem {
    Ch(char),
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match self {
            ClassItem::Ch(x) => *x == c,
            ClassItem::Range(a, b) => *a <= c && c <= *b,
            ClassItem::Digit(pos) => c.is_ascii_digit() == *pos,
            ClassItem::Word(pos) => (c.is_alphanumeric() || c == '_') == *pos,
            ClassItem::Space(pos) => c.is_whitespace() == *pos,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MiniRegex {
    root: Node,
    case_sensitive: bool,
}

struct ParseCtx {
    chars: Vec<char>,
    i: usize,
}

impl ParseCtx {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }
    fn next(&mut self) -> Option<char> {
        let c = self.chars.get(self.i).copied();
        if c.is_some() {
            self.i += 1;
        }
        c
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn err(&self, msg: &str) -> String {
        let near: String = self.chars[self.i.min(self.chars.len())..]
            .iter()
            .take(6)
            .collect();
        format!("{msg} (位置 {} 附近: \"{}\")", self.i, near)
    }
}

impl MiniRegex {
    pub fn new(pattern: &str, case_sensitive: bool) -> Result<Self, String> {
        if pattern.is_empty() {
            return Ok(MiniRegex {
                root: Node::Empty,
                case_sensitive,
            });
        }
        let mut ctx = ParseCtx {
            chars: pattern.chars().collect(),
            i: 0,
        };
        let root = parse_alt(&mut ctx)?;
        if ctx.i < ctx.chars.len() {
            return Err(ctx.err("多余的字符"));
        }
        Ok(MiniRegex {
            root,
            case_sensitive,
        })
    }

    /// 任意位置出现即算匹配 (rg 语义).
    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = if self.case_sensitive {
            text.chars().collect()
        } else {
            text.to_lowercase().chars().collect()
        };
        for start in 0..=chars.len() {
            if match_node(&self.root, &chars, start, &mut |_| true) {
                return true;
            }
        }
        false
    }

    /// 整串匹配 (glob 用). 只从位置 0 起匹配**一次**, 不像 is_match 那样遍历所有起点 ——
    /// 遍历起点会让 `a?.rs` 匹配到 "ab.rs" 里的子串, 这是第一版 glob 全错的根因之一.
    /// 锚点由 glob_to_regex 生成的 ^$ 承担, 这里不重复判断.
    pub fn full_match(&self, text: &str) -> bool {
        let chars: Vec<char> = if self.case_sensitive {
            text.chars().collect()
        } else {
            text.to_lowercase().chars().collect()
        };
        let n = chars.len();
        match_node(&self.root, &chars, 0, &mut |p| p == n)
    }
}

fn parse_alt(ctx: &mut ParseCtx) -> Result<Node, String> {
    let mut branches = vec![parse_seq(ctx)?];
    while ctx.eat('|') {
        branches.push(parse_seq(ctx)?);
    }
    Ok(if branches.len() == 1 {
        branches.pop().expect("len>=1")
    } else {
        Node::Alt(branches)
    })
}

fn parse_seq(ctx: &mut ParseCtx) -> Result<Node, String> {
    let mut items: Vec<Node> = Vec::new();
    while let Some(c) = ctx.peek() {
        if c == '|' || c == ')' {
            break;
        }
        items.push(parse_repeat(ctx)?);
    }
    // 串联必须用 Seq: 不能复用 Alt —— Alt 是「择一」, 复用会让 "abc" 变成
    // 匹配 a/b/c 之一, 而不是依次匹配 abc (这是第一版的真实 bug).
    Ok(match items.len() {
        0 => Node::Empty,
        1 => items.pop().expect("len==1"),
        _ => Node::Seq(items),
    })
}

fn parse_repeat(ctx: &mut ParseCtx) -> Result<Node, String> {
    let atom = parse_atom(ctx)?;
    let Some(c) = ctx.peek() else { return Ok(atom) };
    let (min, max) = match c {
        '*' => {
            ctx.i += 1;
            (0, None)
        }
        '+' => {
            ctx.i += 1;
            (1, None)
        }
        '?' => {
            ctx.i += 1;
            (0, Some(1))
        }
        '{' => {
            let save = ctx.i;
            ctx.i += 1;
            let mut lo = String::new();
            while ctx.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                lo.push(ctx.next().expect("digit"));
            }
            if lo.is_empty() {
                ctx.i = save;
                return Ok(atom);
            }
            let lo_n: usize = lo.parse().map_err(|_| "量词过大".to_string())?;
            let hi = if ctx.eat(',') {
                let mut h = String::new();
                while ctx.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    h.push(ctx.next().expect("digit"));
                }
                if h.is_empty() {
                    None
                } else {
                    Some(h.parse::<usize>().map_err(|_| "量词过大".to_string())?)
                }
            } else {
                Some(lo_n)
            };
            if !ctx.eat('}') {
                ctx.i = save;
                return Ok(atom);
            }
            (lo_n, hi)
        }
        _ => return Ok(atom),
    };
    let greedy = !ctx.eat('?'); // ? 修饰 = 非贪婪
    Ok(Node::Repeat(Box::new(atom), min, max, greedy))
}

fn parse_atom(ctx: &mut ParseCtx) -> Result<Node, String> {
    let Some(c) = ctx.next() else {
        return Err(ctx.err("模式意外结束"));
    };
    Ok(match c {
        '^' => Node::Start,
        '$' => Node::End,
        '.' => Node::Any,
        '(' => {
            if ctx.peek() == Some('?') {
                ctx.i += 1;
                if !ctx.eat(':') {
                    return Err(ctx.err("暂不支持的分组修饰"));
                }
            }
            let inner = parse_alt(ctx)?;
            if !ctx.eat(')') {
                return Err(ctx.err("缺少右括号 )"));
            }
            Node::Group(Box::new(inner))
        }
        '[' => parse_class(ctx)?,
        '\\' => {
            let Some(e) = ctx.next() else {
                return Err(ctx.err("反斜杠后缺字符"));
            };
            match e {
                'd' => Node::Class(vec![ClassItem::Digit(true)], false),
                'D' => Node::Class(vec![ClassItem::Digit(false)], false),
                'w' => Node::Class(vec![ClassItem::Word(true)], false),
                'W' => Node::Class(vec![ClassItem::Word(false)], false),
                's' => Node::Class(vec![ClassItem::Space(true)], false),
                'S' => Node::Class(vec![ClassItem::Space(false)], false),
                'b' => Node::WordBoundary(true),
                'B' => Node::WordBoundary(false),
                'n' => Node::Char('\n'),
                't' => Node::Char('\t'),
                'r' => Node::Char('\r'),
                other => Node::Char(other),
            }
        }
        ')' => return Err(ctx.err("多余的右括号 )")),
        '*' | '+' | '?' => return Err(ctx.err("量词前面没有可重复的内容")),
        other => Node::Char(other),
    })
}

fn parse_class(ctx: &mut ParseCtx) -> Result<Node, String> {
    let neg = ctx.eat('^');
    let mut items = Vec::new();
    let mut first = true;
    loop {
        let Some(c) = ctx.next() else {
            return Err(ctx.err("字符集缺少右方括号 ]"));
        };
        if c == ']' && !first {
            break;
        }
        first = false;
        if c == '[' && ctx.peek() == Some(':') {
            // POSIX 子集 [:alpha:]
            let mut name = String::new();
            ctx.i += 1;
            while let Some(x) = ctx.peek() {
                if x == ':' {
                    break;
                }
                name.push(x);
                ctx.i += 1;
            }
            ctx.eat(':');
            ctx.eat(']');
            match name.as_str() {
                "alpha" | "alnum" => {
                    items.push(ClassItem::Range('a', 'z'));
                    items.push(ClassItem::Range('A', 'Z'));
                    if name == "alnum" {
                        items.push(ClassItem::Digit(true));
                    }
                }
                "digit" => items.push(ClassItem::Digit(true)),
                "space" => items.push(ClassItem::Space(true)),
                _ => return Err(ctx.err(&format!("暂不支持的字符集子集 [:{name}:]"))),
            }
            continue;
        }
        let lo = if c == '\\' {
            let Some(e) = ctx.next() else {
                return Err(ctx.err("字符集内反斜杠后缺字符"));
            };
            match e {
                'd' => {
                    items.push(ClassItem::Digit(true));
                    continue;
                }
                'D' => {
                    items.push(ClassItem::Digit(false));
                    continue;
                }
                'w' => {
                    items.push(ClassItem::Word(true));
                    continue;
                }
                'W' => {
                    items.push(ClassItem::Word(false));
                    continue;
                }
                's' => {
                    items.push(ClassItem::Space(true));
                    continue;
                }
                'S' => {
                    items.push(ClassItem::Space(false));
                    continue;
                }
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                o => o,
            }
        } else {
            c
        };
        if ctx.peek() == Some('-') && ctx.chars.get(ctx.i + 1).copied() != Some(']') {
            ctx.i += 1;
            let hi = ctx.next().ok_or_else(|| ctx.err("区间缺上界"))?;
            items.push(ClassItem::Range(lo, hi));
        } else {
            items.push(ClassItem::Ch(lo));
        }
    }
    Ok(Node::Class(items, neg))
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// 串联匹配: items[0..] 依次匹配, 末项接调用方 k.
fn match_seq(items: &[Node], chars: &[char], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match items.split_first() {
        None => k(pos),
        Some((head, rest)) => match_node(head, chars, pos, &mut |p| {
            // 尾项直接把调用方的 k 接上; 非尾项继续递归下一项
            if rest.is_empty() {
                k(p)
            } else {
                match_seq(rest, chars, p, k)
            }
        }),
    }
}

/// 回溯匹配: 在 chars[pos..] 上匹配 node, 成功则用 k 续跑后续部分.
fn match_node(node: &Node, chars: &[char], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
    match node {
        Node::Char(c) => pos < chars.len() && chars[pos] == *c && k(pos + 1),
        Node::Any => pos < chars.len() && chars[pos] != '\n' && k(pos + 1),
        Node::Class(items, neg) => {
            if pos >= chars.len() {
                return false;
            }
            let c = chars[pos];
            (items.iter().any(|it| it.matches(c)) != *neg) && k(pos + 1)
        }
        Node::Start => pos == 0 && k(pos),
        Node::End => pos == chars.len() && k(pos),
        Node::WordBoundary(want) => {
            let before = pos > 0 && is_word(chars[pos - 1]);
            let after = pos < chars.len() && is_word(chars[pos]);
            ((before != after) == *want) && k(pos)
        }
        Node::Group(inner) => match_node(inner, chars, pos, k),
        Node::Empty => k(pos),
        Node::Alt(branches) => branches.iter().any(|b| match_node(b, chars, pos, k)),
        // 串联: 逐项接续, 最后一项用调用方的 k 收尾 (回溯在每一项内自然发生)
        Node::Seq(items) => match_seq(items, chars, pos, k),
        Node::Repeat(inner, min, max, greedy) => {
            match_repeat(inner, chars, pos, *min, *max, *greedy, 0, k)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn match_repeat(
    inner: &Node,
    chars: &[char],
    pos: usize,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    done: usize,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let can_more = max.map(|m| done < m).unwrap_or(true);
    let try_more = |k: &mut dyn FnMut(usize) -> bool| -> bool {
        can_more
            && match_node(inner, chars, pos, &mut |np| {
                // 零宽保护: 子模式匹配空串且已满足 min 时停止, 否则死循环
                if np == pos && done >= min {
                    return false;
                }
                match_repeat(inner, chars, np, min, max, greedy, done + 1, k)
            })
    };
    if done < min {
        return try_more(k);
    }
    if greedy {
        if try_more(k) {
            return true;
        }
        k(pos)
    } else {
        if k(pos) {
            return true;
        }
        try_more(k)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 临时目录 helper — 不引外部 dep, 每实例唯一.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let mut p = std::env::temp_dir();
            p.push(format!(
                "ide10-search-{tag}-{}-{}",
                std::process::id(),
                nanos
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn opts(pattern: &str) -> SearchOptions {
        SearchOptions {
            pattern: pattern.into(),
            case_sensitive: Some(true),
            regex: Some(false),
            ..Default::default()
        }
    }

    fn fname(p: &str) -> String {
        Path::new(p)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn setup(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("keep.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
        fs::write(dir.join("skip.rs"), "fn alpha() {}\n").unwrap();
        fs::write(dir.join("skip.txt"), "alpha beta\n").unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("sub/deep.rs"), "fn alpha() {}\n").unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::write(dir.join("target/gen.rs"), "fn alpha() {}\n").unwrap();
    }

    // --- 原有三个测试, 去掉 have_rg() 跳过 (现在纯 Rust, 不依赖外部 rg) ---

    #[test]
    fn exclude_glob_excludes_files_not_inverts_matches() {
        let dir = TempDir::new("exclude");
        setup(dir.path());
        let mut o = opts("alpha");
        o.include_globs = Some(vec!["*.rs".into()]);
        o.exclude_globs = Some(vec!["skip*".into()]);
        let hits = search(dir.path(), &o).expect("search 应成功");
        assert!(
            hits.iter().all(|h| !h.path.contains("skip")),
            "exclude 应排除文件"
        );
        assert!(
            hits.iter().any(|h| h.path.contains("keep")),
            "keep.rs 应有命中"
        );
        assert!(
            hits.iter().any(|h| h.path.contains("sub")),
            "子目录也应被搜到"
        );
    }

    #[test]
    fn dash_leading_pattern_not_treated_as_option() {
        let dir = TempDir::new("dash");
        setup(dir.path());
        // 原来靠 rg 的 `--` 终止符防参数注入; 现在没有外部进程, 整串按字面量处理.
        let mut o = opts("--pre=echo PWNED");
        o.include_globs = Some(vec!["*.rs".into()]);
        let r = search(dir.path(), &o);
        assert!(r.is_ok(), "带 -- 的 pattern 应被当字面量");
        assert!(r.unwrap().is_empty(), "字面量不该命中");
    }

    #[test]
    fn already_negated_exclude_not_double_negated() {
        let dir = TempDir::new("excl2");
        setup(dir.path());
        let mut o = opts("alpha");
        o.include_globs = Some(vec!["*.rs".into()]);
        o.exclude_globs = Some(vec!["!skip*".into()]); // 前端已加 !
        let hits = search(dir.path(), &o).expect("search 应成功");
        assert!(hits.iter().all(|h| !h.path.contains("skip")));
    }

    // --- Sakura 语义 ---

    #[test]
    fn dir_exclude_prunes_whole_subtree() {
        // rg 的 `--glob '!target'` 只排掉名为 target 的**文件**, 排不掉其内容;
        // Sakura 的 `#target` 必须排掉整个子树 -> 依赖 `!target/**` 这一条.
        let dir = TempDir::new("direxcl");
        setup(dir.path());
        let mut o = opts("alpha");
        o.exclude_globs = Some(vec!["!target".into(), "!target/**".into()]);
        let hits = search(dir.path(), &o).unwrap();
        assert!(
            hits.iter().all(|h| !h.path.contains("target")),
            "target 子树应被整体排除"
        );
    }

    #[test]
    fn built_in_skip_dirs_never_searched() {
        let dir = TempDir::new("skipdirs");
        setup(dir.path());
        let hits = search(dir.path(), &opts("alpha")).unwrap();
        assert!(
            hits.iter().all(|h| !h.path.contains("target")),
            "target 是固定跳过目录, 未经配置也不该出现"
        );
    }

    #[test]
    fn max_results_truncates() {
        let dir = TempDir::new("maxres");
        setup(dir.path());
        let mut o = opts("alpha");
        o.max_results = Some(2);
        assert_eq!(
            search(dir.path(), &o).unwrap().len(),
            2,
            "应被 max_results 截断"
        );
    }

    #[test]
    fn case_insensitive_when_flag_off() {
        let dir = TempDir::new("ci");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), "Hello World\n").unwrap();
        let mut o = opts("hello");
        o.case_sensitive = Some(false);
        assert_eq!(
            search(dir.path(), &o).unwrap().len(),
            1,
            "忽略大小写应命中 Hello"
        );
        let mut o2 = opts("hello");
        o2.case_sensitive = Some(true);
        assert!(
            search(dir.path(), &o2).unwrap().is_empty(),
            "区分大小写不该命中"
        );
    }

    #[test]
    fn include_glob_wildcards() {
        let dir = TempDir::new("wild");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.rs"), "hit\n").unwrap();
        fs::write(dir.path().join("ab.rs"), "hit\n").unwrap();
        fs::write(dir.path().join("abc.rs"), "hit\n").unwrap();

        // `?` 恰好一个字符: 只命中 ab.rs (a.rs 少一字符, abc.rs 多一字符)
        let mut q = opts("hit");
        q.include_globs = Some(vec!["a?.rs".into()]);
        let hits = search(dir.path(), &q).unwrap();
        assert_eq!(hits.len(), 1, "? 恰好一个字符, 只该命中 ab.rs");
        assert_eq!(fname(&hits[0].path), "ab.rs");

        // `*` 跨任意长度: 三个都命中
        let mut s = opts("hit");
        s.include_globs = Some(vec!["a*.rs".into()]);
        assert_eq!(search(dir.path(), &s).unwrap().len(), 3, "a*.rs 应命中三个");
    }

    #[test]
    fn regex_mode_uses_builtin_engine() {
        let dir = TempDir::new("re");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), "foo123\nbar\nfoo456\n").unwrap();
        let mut o = opts(r"foo\d+");
        o.regex = Some(true);
        assert_eq!(
            search(dir.path(), &o).unwrap().len(),
            2,
            r"foo\d+ 应命中两行"
        );
    }

    #[test]
    fn literal_mode_does_not_treat_pattern_as_regex() {
        let dir = TempDir::new("lit");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("a.txt"), "a.c\nabc\n").unwrap();
        let mut o = opts("a.c");
        o.regex = Some(false);
        assert_eq!(
            search(dir.path(), &o).unwrap().len(),
            1,
            "字面量模式只有第 1 行含 a.c"
        );
        let mut r = opts("a.c");
        r.regex = Some(true);
        assert_eq!(
            search(dir.path(), &r).unwrap().len(),
            2,
            "正则模式下 '.' 是通配"
        );
    }

    // --- MiniRegex 引擎 ---

    fn m(p: &str) -> MiniRegex {
        MiniRegex::new(p, true).unwrap()
    }

    #[test]
    fn regex_anchors() {
        assert!(m(r"^abc").is_match("abcdef"));
        assert!(!m(r"^abc").is_match("zabcdef"));
        assert!(m(r"abc$").is_match("xxabc"));
        assert!(m(r"^abc$").is_match("abc"));
        assert!(!m(r"^abc$").is_match("abcd"));
    }

    #[test]
    fn regex_quantifiers() {
        assert!(m(r"ab*c").is_match("ac"));
        assert!(m(r"ab+c").is_match("abbc"));
        assert!(!m(r"ab+c").is_match("ac"));
        assert!(m(r"a{2,3}").is_match("aa"));
        assert!(m(r"a{2,3}").is_match("aaa"));
        assert!(!m(r"a{2,3}").is_match("a"));
    }

    #[test]
    fn regex_classes_and_escapes() {
        assert!(m(r"\d+").is_match("abc123"));
        assert!(m(r"[a-c]+").is_match("bbb"));
        assert!(!m(r"[^a-c]").is_match("bbb"));
        assert!(m(r"[^a-c]").is_match("zzz"));
        assert!(m(r"\bword\b").is_match("a word here"));
        assert!(!m(r"\bword\b").is_match("wordy"));
        assert!(m(r"a\.b").is_match("a.b"));
        assert!(!m(r"a\.b").is_match("axb"));
    }

    #[test]
    fn regex_alternation_and_groups() {
        assert!(m(r"cat|dog").is_match("hotdog"));
        assert!(m(r"(ab)+c").is_match("ababc"));
        assert!(m(r"(?:xy)z").is_match("xyz"));
    }

    #[test]
    fn regex_case_insensitive() {
        assert!(MiniRegex::new("hello", false)
            .unwrap()
            .is_match("HELLO World"));
        assert!(!MiniRegex::new("hello", true).unwrap().is_match("HELLO"));
    }

    #[test]
    fn regex_invalid_pattern_errors() {
        assert!(MiniRegex::new("(unclosed", true).is_err());
        assert!(MiniRegex::new("[unclosed", true).is_err());
        assert!(MiniRegex::new("*bad", true).is_err());
        assert!(MiniRegex::new("a)", true).is_err());
    }

    #[test]
    fn regex_zero_width_repeat_terminates() {
        // (a*)* 在纯 a 上若无零宽保护会无限递归.
        let re = m(r"^(a*)*b");
        assert!(!re.is_match(&"a".repeat(12)), "不该匹配无 b 的串");
        assert!(re.is_match("aab"), "应以 b 收尾");
        assert!(re.is_match("b"), "空重复也能到 b");
    }

    #[test]
    fn dot_does_not_cross_newline() {
        let re = m(r"a.b");
        assert!(re.is_match("axb"));
        assert!(!re.is_match("a\nb"), ". 默认不该匹配换行");
    }

    // --- フォルダー 栏 (G-21 / G-23) 与沙箱校验 ---

    /// 造一个多目录树:
    ///   <dir>/src/a.rs   alpha
    ///   <dir>/src/deep/b.rs alpha
    ///   <dir>/docs/c.md   alpha
    fn setup_multi(dir: &Path) {
        fs::create_dir_all(dir.join("src/deep")).unwrap();
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::write(dir.join("src/a.rs"), "alpha\n").unwrap();
        fs::write(dir.join("src/deep/b.rs"), "alpha\n").unwrap();
        fs::write(dir.join("docs/c.md"), "alpha\n").unwrap();
    }

    #[test]
    fn multiple_search_folders_are_all_searched() {
        let dir = TempDir::new("multi");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.search_folders = Some(vec!["src".into(), "docs".into()]);
        let hits = search(dir.path(), &o).unwrap();
        let paths: Vec<String> = hits.iter().map(|h| h.path.replace('\\', "/")).collect();
        assert!(
            paths.iter().any(|p| p.ends_with("src/a.rs")),
            "应搜到 src/a.rs"
        );
        assert!(
            paths.iter().any(|p| p.ends_with("src/deep/b.rs")),
            "子目录也应递归搜到"
        );
        assert!(
            paths.iter().any(|p| p.ends_with("docs/c.md")),
            "应搜到 docs/c.md"
        );
    }

    #[test]
    fn search_folder_excludes_other_trees() {
        let dir = TempDir::new("single");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.search_folders = Some(vec!["src".into()]);
        let hits = search(dir.path(), &o).unwrap();
        assert!(
            hits.iter().all(|h| !h.path.contains("docs")),
            "只指定 src 时不该命中 docs, 实际: {:?}",
            hits.iter().map(|h| h.path.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn empty_search_folders_means_whole_project() {
        let dir = TempDir::new("all");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.search_folders = Some(vec![]);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 3, "留空 = 整个项目");
    }

    /// 沙箱: `..` 逃出项目根必须被拒绝, 且**不能影响其它合法目录**的搜索.
    #[test]
    fn all_folders_escaping_is_an_error() {
        let dir = TempDir::new("allbad");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.search_folders = Some(vec!["../..".into(), "../../..".into()]);
        let err = search(dir.path(), &o).unwrap_err();
        assert!(err.contains("越出项目范围"), "应给出越界原因, 实际: {err}");
    }

    /// 同名前缀的兄弟目录不能被误判成子目录 (`proj` vs `proj-evil`).
    #[test]
    fn sibling_dir_with_same_prefix_is_rejected() {
        let dir = TempDir::new("prefix");
        setup_multi(dir.path());
        let sibling = dir.path().with_extension("evil");
        fs::create_dir_all(&sibling).unwrap();
        fs::write(sibling.join("leak.rs"), "alpha\n").unwrap();

        let mut o = opts("alpha");
        o.search_folders = Some(vec![sibling.to_string_lossy().to_string()]);
        // 同名前缀的兄弟目录必须被当越界处理 —— 按字符串前缀比较会漏掉, 按路径分量不会
        let err = search(dir.path(), &o).unwrap_err();
        assert!(
            err.contains("越出项目范围"),
            "兄弟目录应被拒绝, 实际: {err}"
        );
    }

    /// 显式指定的文件夹即使是编译产物目录 (target) 也要能搜进去.
    #[test]
    fn explicit_folder_overrides_skip_dirs() {
        let dir = TempDir::new("skipexplicit");
        fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        fs::write(dir.path().join("target/debug/gen.rs"), "alpha\n").unwrap();
        let mut o = opts("alpha");
        // 不指定时 target 被跳过
        assert!(
            search(dir.path(), &o).unwrap().is_empty(),
            "默认应跳过 target"
        );
        // 显式指定后可以搜到
        o.search_folders = Some(vec!["target".into()]);
        assert_eq!(
            search(dir.path(), &o).unwrap().len(),
            1,
            "用户点名要搜 target 就该能搜到"
        );
    }

    /// subfolders=false 只搜顶层 (G-40).
    ///
    /// 语义对照上游 `CGrepAgent.cpp` 的 `EnumerateGrepTargetFiles` 调用点 (~1010 行):
    /// 先枚举**指定目录里自己**的文件, 只有 `bGrepSubFolder` 为真才继续下钻子目录。
    /// 所以关闭递归时该目录下的文件仍会命中, 只是不进子目录。
    #[test]
    fn subfolders_false_searches_only_top_level() {
        let dir = TempDir::new("norecurse");
        fs::create_dir_all(dir.path().join("sub/deep")).unwrap();
        fs::write(dir.path().join("top.txt"), "alpha\n").unwrap();
        fs::write(dir.path().join("sub/mid.txt"), "alpha\n").unwrap();
        fs::write(dir.path().join("sub/deep/low.txt"), "alpha\n").unwrap();

        let mut on = opts("alpha");
        on.subfolders = Some(true);
        assert_eq!(
            search(dir.path(), &on).unwrap().len(),
            3,
            "递归时应搜到 3 个文件"
        );

        let mut off = opts("alpha");
        off.subfolders = Some(false);
        let hits = search(dir.path(), &off).unwrap();
        let names: Vec<String> = hits.iter().map(|h| h.path.replace('\\', "/")).collect();
        assert_eq!(
            names.len(),
            1,
            "关闭递归只应命中指定目录自己的文件, 实际: {names:?}"
        );
        assert!(names[0].ends_with("top.txt"));
    }

    #[test]
    fn empty_pattern_degrades_to_filename_search() {
        let dir = TempDir::new("nameonly");
        setup_multi(dir.path());
        let mut o = opts("");
        o.include_files = Some(vec!["*.md".into()]);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 1, "只应命中 .md");
        assert_eq!(hits[0].line, 0, "文件名搜索的行号应为 0");
        assert!(hits[0].path.ends_with("c.md"));
    }

    /// 除外フォルダー 独立字段 (G-31).
    #[test]
    fn exclude_folders_field_prunes_subtree() {
        let dir = TempDir::new("excdir");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.exclude_folders = Some(vec!["src".into()]);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("c.md"), "src 子树应被排除");
    }

    /// 除外ファイル 独立字段 (G-30).
    #[test]
    fn exclude_files_field_removes_files() {
        let dir = TempDir::new("excfile");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.exclude_files = Some(vec!["*.md".into()]);
        let hits = search(dir.path(), &o).unwrap();
        assert!(
            hits.iter().all(|h| !h.path.ends_with("c.md")),
            "*.md 应被排除"
        );
    }

    /// G-04 単語単位: 只命中独立成词的地方.
    #[test]
    fn whole_word_matches_only_standalone() {
        let dir = TempDir::new("word");
        fs::write(dir.path().join("a.txt"), "foo\nfoobar\nbar foo baz\n").unwrap();
        let mut o = opts("foo");
        o.whole_word = Some(true);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 2, "只应命中两处独立的 foo, 实际: {hits:?}");
    }

    /// 正则模式下 whole_word 被忽略 (用户的正则自带边界语义).
    #[test]
    fn whole_word_ignored_in_regex_mode() {
        let dir = TempDir::new("wordre");
        fs::write(dir.path().join("a.txt"), "foobar\n").unwrap();
        let mut o = opts("foo");
        o.whole_word = Some(true);
        o.regex = Some(true);
        assert_eq!(
            search(dir.path(), &o).unwrap().len(),
            1,
            "正则模式不该被 whole_word 收窄"
        );
    }

    /// 结果稳定排序: 多次调用顺序一致 (多目录搜索下尤其重要).
    #[test]
    fn results_are_stably_sorted() {
        let dir = TempDir::new("sorted");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.search_folders = Some(vec!["docs".into(), "src".into()]);
        let a: Vec<String> = search(dir.path(), &o)
            .unwrap()
            .iter()
            .map(|h| h.path.clone())
            .collect();
        let b: Vec<String> = search(dir.path(), &o)
            .unwrap()
            .iter()
            .map(|h| h.path.clone())
            .collect();
        assert_eq!(a, b, "两次搜索顺序必须一致");
        let mut sorted = a.clone();
        sorted.sort();
        assert_eq!(a, sorted, "结果应按路径升序");
    }

    // --- 快速路径 vs 解码路径 必须给出完全相同的结果 ---

    /// 建一棵带各种边界情况的树, 专门用来给两条路径对答案。
    fn setup_edges(dir: &Path) {
        fs::create_dir_all(dir.join("sub")).unwrap();
        // 首行 / 末行无换行
        fs::write(dir.join("a.txt"), "needle at start\nmiddle\nneedle at end").unwrap();
        // 空行 / 连续命中 / CRLF / 非 ASCII / 一行多次
        fs::write(dir.join("b.txt"), "\n\nneedle needle\nx\r\nneedle\r\n日本語 needle テスト\nneedle").unwrap();
        // needle 恰好在缓冲区末尾 (测越界)
        fs::write(dir.join("sub/c.txt"), "no hit here\nne").unwrap();
        // 空文件
        fs::write(dir.join("d.txt"), "").unwrap();
        // 只有换行
        fs::write(dir.join("e.txt"), "\n\n\n").unwrap();
    }

    /// 强制走解码路径 (绕开快速路径), 用于对答案。
    fn search_slow(root: &Path, o: &SearchOptions) -> Result<Vec<SearchResult>, String> {
        let case_sensitive = o.case_sensitive.unwrap_or(false);
        let max = o.max_results.unwrap_or(usize::MAX);
        let mut files = Vec::new();
        walk(root, &[], &mut files, 0, true, false);
        let m = Matcher::new(&o.pattern, o.regex.unwrap_or(false), case_sensitive, false)?;
        let mut out = Vec::new();
        'w: for f in files {
            let name = f.file_name().unwrap().to_string_lossy().to_string();
            if !file_passes(&name, &[], &[], case_sensitive) { continue; }
            let Ok(file) = fs::File::open(&f) else { continue };
            for (i, line) in std::io::BufReader::new(file).lines().enumerate() {
                let Ok(t) = line else { continue };
                if m.matches(&t) {
                    out.push(SearchResult { path: f.to_string_lossy().into_owned(), line: i as u64 + 1, text: t });
                    if out.len() >= max { break 'w; }
                }
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        Ok(out)
    }

    #[test]
    fn fast_path_matches_slow_path_exactly() {
        let dir = TempDir::new("parity");
        setup_edges(dir.path());
        for (pattern, cs) in [
            ("needle", true),   // 区分大小写
            ("NEEDLE", true),
            ("needle", false),  // 忽略大小写
            ("NEEDLE", false),
            ("nee", true),      // 短 needle (前缀截断风险)
            ("needle", true),
            ("不存在的串", true), // 非 ASCII needle
            ("日本", true),
        ] {
            let o = SearchOptions {
                pattern: pattern.into(),
                case_sensitive: Some(cs),
                ..Default::default()
            };
            let fast = search(dir.path(), &o).unwrap();
            let slow = search_slow(dir.path(), &o).unwrap();
            assert_eq!(
                fast, slow,
                "pattern={pattern:?} case_sensitive={cs}: 快速路径与解码路径结果不一致\nfast={fast:#?}\nslow={slow:#?}"
            );
        }
    }

    #[test]
    fn fast_path_respects_max_results() {
        let dir = TempDir::new("fastmax");
        fs::write(dir.path().join("a.txt"), "x\n").unwrap();
        let mut s = String::new();
        for i in 0..100 { s.push_str(&format!("needle {i}\n")); }
        fs::write(dir.path().join("b.txt"), &s).unwrap();

        let mut o = opts("needle");
        o.max_results = Some(7);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 7, "max_results 应在快速路径上生效");
        assert!(hits.iter().all(|h| h.line > 0));
        // 行号必须单调递增且与文本对齐
        for h in &hits {
            assert!(h.text.contains("needle"), "行文本应含关键字: {}", h.text);
        }
    }

    #[test]
    fn fast_path_line_numbers_are_correct() {
        let dir = TempDir::new("fastline");
        fs::write(dir.path().join("a.txt"), "l1\nl2\nneedle\nl4\nneedle\n").unwrap();
        let o = opts("needle");
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 3, "第一个命中在第 3 行");
        assert_eq!(hits[0].text, "needle");
        assert_eq!(hits[1].line, 5, "第二个命中在第 5 行");
    }

    #[test]
    fn fast_path_same_line_multiple_occurrences_reported_once() {
        let dir = TempDir::new("fastmulti");
        fs::write(dir.path().join("a.txt"), "needle needle needle\n").unwrap();
        let o = opts("needle");
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 1, "同一行多次出现只报一次 (与逐行实现一致)");
        assert_eq!(hits[0].line, 1);
    }

    #[test]
    fn fast_path_preserves_original_case_in_output() {
        let dir = TempDir::new("fastcase");
        fs::write(dir.path().join("a.txt"), "Hello World\n").unwrap();
        let mut o = opts("hello");
        o.case_sensitive = Some(false);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "Hello World", "忽略大小写命中时返回的应是原文");
    }

    #[test]
    fn fast_path_handles_crlf_and_missing_trailing_newline() {
        let dir = TempDir::new("fastcrlf");
        fs::write(dir.path().join("a.txt"), "one\r\nneedle\r\nthree").unwrap();
        let o = opts("needle");
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        assert_eq!(hits[0].text, "needle", "CRLF 不该混进结果文本");
    }

    #[test]
    fn fast_path_still_honours_whole_word_by_falling_back() {
        // whole_word 不能走字节路径, 必须退回解码路径, 结果仍要正确
        let dir = TempDir::new("fastword");
        fs::write(dir.path().join("a.txt"), "foo\nfoobar\nbar foo\n").unwrap();
        let mut o = opts("foo");
        o.whole_word = Some(true);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 2, "只该命中两处独立成词的 foo");
    }

    #[test]
    fn fast_path_regex_falls_back_and_still_works() {
        let dir = TempDir::new("fastregex");
        fs::write(dir.path().join("a.txt"), "foo123\nbar\nfoo456\n").unwrap();
        let mut o = opts(r"foo\d+");
        o.regex = Some(true);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[1].line, 3);
    }

    // --- 全盘 / 跨盘搜索 (allow_outside_root) ---

    #[test]
    fn allow_outside_root_permits_paths_beyond_project() {
        let dir = TempDir::new("outside");
        setup_multi(dir.path());
        // 项目根之外另造一个目录
        let other = dir.path().with_extension("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("far.rs"), "alpha\n").unwrap();

        let mut o = opts("alpha");
        o.search_folders = Some(vec![other.to_string_lossy().to_string()]);
        // 默认拒绝
        assert!(search(dir.path(), &o).is_err(), "默认应拒绝项目外路径");
        // 打开开关后放行
        o.allow_outside_root = Some(true);
        let hits = search(dir.path(), &o).unwrap();
        assert_eq!(hits.len(), 1, "开关打开后应能搜到项目外的文件");
        assert!(hits[0].path.replace('\\', "/").ends_with("far.rs"));
    }

    #[test]
    fn allow_outside_root_still_skips_nonexistent_dirs_with_clear_error() {
        let dir = TempDir::new("outside2");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.search_folders = Some(vec!["Z:/绝对不存在的目录xyz".into()]);
        o.allow_outside_root = Some(true);
        let err = search(dir.path(), &o).unwrap_err();
        assert!(err.contains("不存在"), "应说明目录不存在, 实际: {err}");
    }

    #[test]
    fn allow_outside_root_does_not_disable_existing_filters() {
        // 打开跨盘开关不应让 除外フォルダー 之类的过滤失效
        let dir = TempDir::new("outside3");
        setup_multi(dir.path());
        let mut o = opts("alpha");
        o.search_folders = Some(vec!["src".into()]);
        o.allow_outside_root = Some(true);
        o.exclude_folders = Some(vec!["src".into()]);
        assert!(search(dir.path(), &o).unwrap().is_empty(), "排除项仍应生效");
    }
}
