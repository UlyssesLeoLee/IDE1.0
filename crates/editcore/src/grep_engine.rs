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
#[cfg(test)]
#[cfg(test)]
use std::io::BufRead;
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
    /// 文件字符集: `auto` / 缺省 = 逐文件自动判别 (BOM -> UTF-8 -> SJIS/EUC-JP/JIS/UTF-16 启发式);
    /// 也可显式指定 `utf-8` `sjis` `euc-jp` `jis` `utf-16le` `utf-16be` `latin1` `gbk` `big5` `euc-kr` 等
    /// (名字同 `document_io::Charset::from_label`). 不识别的名字让搜索报错.
    #[serde(default)]
    pub charset: Option<String>,
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
fn scan_buf_fast(raw: &[u8], needle: &LiteralNeedle, fold: bool, max: usize) -> Vec<(u64, String)> {
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
        raw
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
        out.push((line, line_at(raw, at, false)));
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

/// 枚举候选文件 (含「フォルダー」栏解析 / 目录排除), 返回 (文件, 包含项, 排除项).
fn collect_candidates(
    root: &Path,
    opts: &SearchOptions,
) -> Result<(Vec<PathBuf>, Vec<String>, Vec<String>), String> {
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

    Ok((files, inc, exc))
}

/// 候选文件再过一遍文件名 include / exclude, 返回 (展示路径, 文件名). search 与 grep_replace 共用。
fn filtered_files(root: &Path, opts: &SearchOptions) -> Result<Vec<(PathBuf, String)>, String> {
    let case_sensitive = opts.case_sensitive.unwrap_or(false);
    let (files, inc, exc) = collect_candidates(root, opts)?;
    let mut out = Vec::new();
    for f in files {
        // `canonicalize()` 在 Windows 上会加 `\?\` 前缀, 展示前剥掉。
        let f = strip_verbatim_prefix(&f);
        let Some(name) = f.file_name().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        if file_passes(&name, &inc, &exc, case_sensitive) {
            out.push((f, name));
        }
    }
    Ok(out)
}

pub fn search(root: &Path, opts: &SearchOptions) -> Result<Vec<SearchResult>, String> {
    let case_sensitive = opts.case_sensitive.unwrap_or(false);
    let max = opts.max_results.unwrap_or(usize::MAX);
    let forced_charset = parse_charset_label(opts.charset.as_deref())?;
    let files = filtered_files(root, opts)?;

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
    'outer: for (f, name) in files {
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
        let Ok(raw) = fs::read(&f) else {
            continue;
        };
        // 按文件判别编码 (BOM -> UTF-8 校验 -> SJIS/EUC/JIS/UTF-16 启发式); 用户显式指定则以其为准。
        let text = decode_for_grep(&raw, forced_charset);
        let path_s = f.to_string_lossy().into_owned();
        if let Some(needle) = &fast {
            // 字节级路径: 在 UTF-8 文本上 SIMD 找偏移, 只在命中处切行 (行号按解码后的文本数, 与原文件一致).
            let fold = !case_sensitive;
            for (line, text) in scan_buf_fast(text.as_bytes(), needle, fold, budget) {
                results.push(SearchResult {
                    path: path_s.clone(),
                    line,
                    text,
                });
            }
            if results.len() >= max {
                break 'outer;
            }
            continue;
        }
        for (i, line) in text.lines().enumerate() {
            if matcher.matches(line) {
                results.push(SearchResult {
                    path: path_s.clone(),
                    line: i as u64 + 1,
                    text: line.to_string(),
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
        '*' | '+' | '?' => return Err(ctx.err("量词前面没有可重复的内容。要查找字符 * + ? 本身, 请写 \\* \\+ \\? 或 [*] [+] [?]")),
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
// ---------------------------------------------------------------------------
// 按文件编码解码 (Grep 检索与置换共用)
// ---------------------------------------------------------------------------

use crate::document_io::{self, Bom, Charset};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// 解析请求里的 `charset` 字段: None / "" / "auto" => 自动判别; 否则必须是已知字符集名.
pub fn parse_charset_label(label: Option<&str>) -> Result<Option<Charset>, String> {
    match label.map(str::trim) {
        None | Some("") => Ok(None),
        Some(l) if l.eq_ignore_ascii_case("auto") => Ok(None),
        Some(l) => Charset::from_label(l)
            .map(Some)
            .ok_or_else(|| format!("不支持的字符集: {l}")),
    }
}

/// 无 BOM 的 UTF-16 启发式: ASCII 为主的 UTF-16 文本里, 高字节(LE 的奇数位 / BE 的偶数位)几乎全是 NUL.
fn sniff_utf16_without_bom(b: &[u8]) -> Option<Charset> {
    if b.len() < 4 {
        return None;
    }
    let sample = &b[..b.len().min(4096) & !1];
    let (mut even_nul, mut odd_nul) = (0usize, 0usize);
    for (i, &x) in sample.iter().enumerate() {
        if x == 0 {
            if i % 2 == 0 { even_nul += 1 } else { odd_nul += 1 }
        }
    }
    let half = sample.len() / 2;
    if half == 0 {
        return None;
    }
    // 一侧 NUL 占 >= 30%, 另一侧 (近乎) 没有; 全 NUL 的二进制块不算
    if odd_nul * 10 >= half * 3 && even_nul * 20 <= half {
        Some(Charset::Utf16Le)
    } else if even_nul * 10 >= half * 3 && odd_nul * 20 <= half {
        Some(Charset::Utf16Be)
    } else {
        None
    }
}

/// 自动判别字符集: BOM -> (无 BOM 的 UTF-16) -> ISO-2022-JP 转义 -> UTF-8 合法性 -> SJIS/EUC-JP (CP932 优先) -> 其他.
pub fn detect_grep_charset(bytes: &[u8]) -> Charset {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Charset::Utf8;
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Charset::Utf16Le;
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Charset::Utf16Be;
    }
    if let Some(cs) = sniff_utf16_without_bom(bytes) {
        return cs;
    }
    document_io::detect_charset(bytes)
}

fn bom_len(bytes: &[u8], cs: Charset) -> usize {
    match cs {
        Charset::Utf8 if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) => 3,
        Charset::Utf16Le if bytes.starts_with(&[0xFF, 0xFE]) => 2,
        Charset::Utf16Be if bytes.starts_with(&[0xFE, 0xFF]) => 2,
        _ => 0,
    }
}

fn encoding_of(cs: Charset) -> Option<&'static encoding_rs::Encoding> {
    use encoding_rs as e;
    Some(match cs {
        Charset::Utf8 => e::UTF_8,
        Charset::ShiftJis => e::SHIFT_JIS,
        Charset::EucJp => e::EUC_JP,
        Charset::Iso2022Jp => e::ISO_2022_JP,
        Charset::Utf16Le => e::UTF_16LE,
        Charset::Utf16Be => e::UTF_16BE,
        Charset::Latin1 => e::WINDOWS_1252,
        Charset::Gbk => e::GBK,
        Charset::Big5 => e::BIG5,
        Charset::EucKr => e::EUC_KR,
        Charset::Cesu8 | Charset::Utf7 => return None,
    })
}

/// 原样解码 (保留文件里的各种换行), 返回 (文本, 是否遇到非法字节, BOM 字节数).
fn decode_raw(bytes: &[u8], cs: Charset) -> Result<(String, bool, usize), String> {
    let skip = bom_len(bytes, cs);
    let body = &bytes[skip..];
    match encoding_of(cs) {
        Some(enc) => {
            let (s, had_err) = enc.decode_without_bom_handling(body);
            Ok((s.into_owned(), had_err, skip))
        }
        None => {
            // UTF-7 / CESU-8: 只能借 document_io 解码, 换行会被规整为 LF
            let d = document_io::decode_document(bytes, Some(cs));
            Ok((d.content, d.lossy, 0))
        }
    }
}

/// 把一个文件的字节解成供检索的文本 (换行规整为 LF 仅在需要解码时; 纯 UTF-8 零拷贝).
///
/// `forced` = 用户在请求里指定的字符集; None = 自动. 自动时没有 BOM / 不含 NUL 或 ESC / 是合法 UTF-8
/// 的常见情形直接借用原字节, 不付解码代价.
pub fn decode_for_grep(bytes: &[u8], forced: Option<Charset>) -> Cow<'_, str> {
    if forced.is_none() && !bytes.contains(&0u8) && !bytes.contains(&0x1b) && !bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        if let Ok(s) = std::str::from_utf8(bytes) {
            return Cow::Borrowed(s);
        }
    }
    let cs = forced.unwrap_or_else(|| detect_grep_charset(bytes));
    Cow::Owned(document_io::decode_document(bytes, Some(cs)).content)
}

/// 读文件并按编码解码 (供 `grep.rs` 的简易 API 使用).
pub fn read_text_for_grep(path: &Path, forced: Option<Charset>) -> std::io::Result<String> {
    let raw = fs::read(path)?;
    Ok(decode_for_grep(&raw, forced).into_owned())
}

// ---------------------------------------------------------------------------
// Grep 置換: 真正改文件
// ---------------------------------------------------------------------------

/// 置换请求: 检索条件同 [`SearchOptions`] (含 `charset`), 另加置换串与写回选项.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplaceOptions {
    /// 与 Grep 检索完全相同的条件 (pattern 不可为空).
    #[serde(flatten)]
    pub search: SearchOptions,
    /// 置换串. 正则模式下可写 `$0`..`$9` / `$&` (整个匹配) / `$$` (字面 `$`); 字面量模式原样使用.
    #[serde(default)]
    pub replacement: String,
    /// 改写前把原文件复制为备份. 默认 false.
    #[serde(default)]
    pub backup: Option<bool>,
    /// 备份文件后缀, 默认 `.bak` (备份路径 = 原路径 + 后缀).
    #[serde(default)]
    pub backup_suffix: Option<String>,
    /// 「已打开且已修改」的文件: 不改写, 只报告为 skipped (路径比较不区分大小写 / 分隔符).
    #[serde(default)]
    pub skip_paths: Option<Vec<String>>,
}

/// 单个文件的置换结果.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileReplaceResult {
    pub path: String,
    /// 置换的处数 (skipped 时 = 若改写将置换的处数).
    pub replacements: usize,
    /// 涉及的行数.
    pub lines: usize,
    /// 实际使用的字符集标签 (如 `SJIS`).
    pub charset: String,
    /// 因在 `skip_paths` 中而未改写.
    pub skipped: bool,
    /// 实际写了文件.
    pub written: bool,
    /// 备份文件路径 (启用备份且写了文件时).
    pub backup_path: Option<String>,
    /// 失败原因 (该文件保持原样).
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceReport {
    /// 只包含有命中的文件 (或出错的文件).
    pub files: Vec<FileReplaceResult>,
    pub total_replacements: usize,
    pub files_changed: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
}

/// 一行内的一处匹配: 字符区间 + 捕获组 (下标 0 = 整个匹配).
type Caps = Vec<Option<(usize, usize)>>;

enum ReplMatcher {
    Literal { needle: Vec<char>, fold: bool, whole_word: bool },
    Re { re: Box<MiniRegex>, group_ids: HashMap<*const Node, usize>, ngroups: usize },
}

fn fold_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// 先序遍历给捕获分组编号 (与源码里 `(` 出现的顺序一致); `capturing[i]` = 第 i 个 `(` 是否捕获.
fn number_groups(node: &Node, capturing: &[bool], next: &mut usize, ids: &mut HashMap<*const Node, usize>) {
    match node {
        Node::Group(inner) => {
            let i = *next;
            *next += 1;
            if capturing.get(i).copied().unwrap_or(true) {
                ids.insert(node as *const Node, 0); // 编号稍后补
            }
            number_groups(inner, capturing, next, ids);
        }
        Node::Repeat(inner, ..) => number_groups(inner, capturing, next, ids),
        Node::Alt(v) | Node::Seq(v) => v.iter().for_each(|n| number_groups(n, capturing, next, ids)),
        _ => {}
    }
}

/// 扫描源码, 标出每个分组的 `(` 是不是 `(?:` 非捕获.
fn scan_group_kinds(pattern: &str) -> Vec<bool> {
    let cs: Vec<char> = pattern.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut in_class) = (0usize, false);
    while i < cs.len() {
        match cs[i] {
            '\\' => i += 1,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => out.push(!(cs.get(i + 1) == Some(&'?') && cs.get(i + 2) == Some(&':'))),
            _ => {}
        }
        i += 1;
    }
    out
}

/// 带捕获的回溯匹配 (与 `match_node` 同语义, 只是额外记录分组区间).
fn cap_match(
    node: &Node,
    chars: &[char],
    pos: usize,
    ids: &HashMap<*const Node, usize>,
    caps: &RefCell<Caps>,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    match node {
        Node::Group(inner) => {
            let Some(&gid) = ids.get(&(node as *const Node)) else {
                return cap_match(inner, chars, pos, ids, caps, k);
            };
            cap_match(inner, chars, pos, ids, caps, &mut |p| {
                let prev = caps.borrow()[gid];
                caps.borrow_mut()[gid] = Some((pos, p));
                if k(p) {
                    true
                } else {
                    caps.borrow_mut()[gid] = prev;
                    false
                }
            })
        }
        Node::Alt(branches) => branches.iter().any(|b| cap_match(b, chars, pos, ids, caps, k)),
        Node::Seq(items) => cap_seq(items, chars, pos, ids, caps, k),
        Node::Repeat(inner, min, max, greedy) => {
            cap_repeat(inner, chars, pos, *min, *max, *greedy, 0, ids, caps, k)
        }
        // 不含子节点的叶子: 直接复用原匹配器
        leaf => match_node(leaf, chars, pos, k),
    }
}

fn cap_seq(
    items: &[Node],
    chars: &[char],
    pos: usize,
    ids: &HashMap<*const Node, usize>,
    caps: &RefCell<Caps>,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    match items.split_first() {
        None => k(pos),
        Some((head, rest)) => cap_match(head, chars, pos, ids, caps, &mut |p| {
            if rest.is_empty() { k(p) } else { cap_seq(rest, chars, p, ids, caps, k) }
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn cap_repeat(
    inner: &Node,
    chars: &[char],
    pos: usize,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    done: usize,
    ids: &HashMap<*const Node, usize>,
    caps: &RefCell<Caps>,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let can_more = max.map(|m| done < m).unwrap_or(true);
    let try_more = |k: &mut dyn FnMut(usize) -> bool| -> bool {
        can_more
            && cap_match(inner, chars, pos, ids, caps, &mut |np| {
                if np == pos && done >= min {
                    return false;
                }
                cap_repeat(inner, chars, np, min, max, greedy, done + 1, ids, caps, k)
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

impl ReplMatcher {
    fn new(opts: &SearchOptions) -> Result<Self, String> {
        let case_sensitive = opts.case_sensitive.unwrap_or(false);
        if opts.pattern.is_empty() {
            return Err("置换需要非空的检索条件".to_string());
        }
        if opts.regex.unwrap_or(false) {
            let re = MiniRegex::new(&opts.pattern, case_sensitive).map(Box::new)
                .map_err(|e| format!("正则表达式无效: {e}"))?;
            let kinds = scan_group_kinds(&opts.pattern);
            let mut ids = HashMap::new();
            let mut next = 0usize;
            number_groups(&re.root, &kinds, &mut next, &mut ids);
            // 捕获组编号 = 其 `(` 在捕获 `(` 中的序号 (1 起)
            let mut order: Vec<*const Node> = Vec::new();
            fn collect(node: &Node, ids: &HashMap<*const Node, usize>, out: &mut Vec<*const Node>) {
                match node {
                    Node::Group(inner) => {
                        if ids.contains_key(&(node as *const Node)) {
                            out.push(node as *const Node);
                        }
                        collect(inner, ids, out);
                    }
                    Node::Repeat(inner, ..) => collect(inner, ids, out),
                    Node::Alt(v) | Node::Seq(v) => v.iter().for_each(|n| collect(n, ids, out)),
                    _ => {}
                }
            }
            collect(&re.root, &ids, &mut order);
            for (n, p) in order.iter().enumerate() {
                ids.insert(*p, n + 1);
            }
            let ngroups = order.len() + 1;
            Ok(ReplMatcher::Re { re, group_ids: ids, ngroups })
        } else {
            let fold = !case_sensitive;
            let needle: Vec<char> = opts
                .pattern
                .chars()
                .map(|c| if fold { fold_char(c) } else { c })
                .collect();
            Ok(ReplMatcher::Literal { needle, fold, whole_word: opts.whole_word.unwrap_or(false) })
        }
    }

    /// 在 `line` 中从字符位置 `from` 起找下一处匹配.
    fn find_at(&self, chars: &[char], from: usize) -> Option<Caps> {
        match self {
            ReplMatcher::Literal { needle, fold, whole_word } => {
                let nb = needle.len();
                let mut at = from;
                while at + nb <= chars.len() {
                    let hit = (0..nb).all(|j| {
                        let c = chars[at + j];
                        (if *fold { fold_char(c) } else { c }) == needle[j]
                    });
                    if hit {
                        let ok = !*whole_word
                            || ((at == 0 || !is_word_char(chars[at - 1]))
                                && (at + nb >= chars.len() || !is_word_char(chars[at + nb])));
                        if ok {
                            return Some(vec![Some((at, at + nb))]);
                        }
                    }
                    at += 1;
                }
                None
            }
            ReplMatcher::Re { re, group_ids, ngroups } => {
                // 与 MiniRegex::is_match 同步: 不区分大小写时按字符逐个小写化 (长度对齐, 位置可直接用于原文)
                let hay: Cow<[char]> = if re.case_sensitive {
                    Cow::Borrowed(chars)
                } else {
                    Cow::Owned(chars.iter().map(|c| fold_char(*c)).collect())
                };
                for start in from..=hay.len() {
                    let caps = RefCell::new(vec![None; *ngroups]);
                    let mut end = None;
                    if cap_match(&re.root, &hay, start, group_ids, &caps, &mut |p| {
                        end = Some(p);
                        true
                    }) {
                        let mut c = caps.into_inner();
                        c[0] = Some((start, end.unwrap_or(start)));
                        return Some(c);
                    }
                }
                None
            }
        }
    }

    fn expand(&self, template: &str, chars: &[char], caps: &Caps, out: &mut String) {
        if matches!(self, ReplMatcher::Literal { .. }) {
            out.push_str(template);
            return;
        }
        let t: Vec<char> = template.chars().collect();
        let group = |n: usize, out: &mut String| {
            if let Some(Some((a, b))) = caps.get(n) {
                out.extend(&chars[*a..*b]);
            }
        };
        let mut i = 0;
        while i < t.len() {
            if t[i] == '$' && i + 1 < t.len() {
                match t[i + 1] {
                    '$' => { out.push('$'); i += 2; continue; }
                    '&' => { group(0, out); i += 2; continue; }
                    d if d.is_ascii_digit() => { group(d as usize - '0' as usize, out); i += 2; continue; }
                    _ => {}
                }
            }
            out.push(t[i]);
            i += 1;
        }
    }
}

/// 按行置换 (行内换行符原样保留). 返回 (新文本, 处数, 涉及行数).
fn replace_in_text(text: &str, m: &ReplMatcher, template: &str) -> (String, usize, usize) {
    let mut out = String::with_capacity(text.len());
    let (mut total, mut lines) = (0usize, 0usize);
    let b = text.as_bytes();
    let mut start = 0usize;
    while start < b.len() {
        // 行 = [start, line_end) + eol
        let mut line_end = start;
        while line_end < b.len() && b[line_end] != b'\n' && b[line_end] != b'\r' {
            line_end += 1;
        }
        let mut eol_end = line_end;
        if eol_end < b.len() {
            if b[eol_end] == b'\r' && eol_end + 1 < b.len() && b[eol_end + 1] == b'\n' {
                eol_end += 2;
            } else {
                eol_end += 1;
            }
        }
        let line = &text[start..line_end];
        let chars: Vec<char> = line.chars().collect();
        let mut pos = 0usize;
        let mut last_end: Option<usize> = None;
        let mut copied = 0usize;
        let mut n = 0usize;
        let mut buf = String::new();
        while pos <= chars.len() {
            let Some(caps) = m.find_at(&chars, pos) else { break };
            let (a, e) = caps[0].unwrap_or((pos, pos));
            if a == e && last_end == Some(a) {
                // 紧接上一处匹配之后的空匹配不算 (同 regex crate)
                pos = a + 1;
                continue;
            }
            buf.extend(&chars[copied..a]);
            m.expand(template, &chars, &caps, &mut buf);
            copied = e;
            n += 1;
            last_end = Some(e);
            pos = if e == a { e + 1 } else { e };
        }
        if n > 0 {
            buf.extend(&chars[copied.min(chars.len())..]);
            out.push_str(&buf);
            total += n;
            lines += 1;
        } else {
            out.push_str(line);
        }
        out.push_str(&text[line_end..eol_end]);
        start = eol_end;
    }
    (out, total, lines)
}

/// ISO-2022-JP 编码 (JIS X 0208 + ASCII): 借 EUC-JP 编码器取双字节码再减 0x80. 其他字符 (半角假名等) 返回 Err.
fn encode_iso2022jp(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut in_jis = false;
    for ch in text.chars() {
        if ch.is_ascii() {
            if in_jis {
                out.extend_from_slice(&[0x1b, b'(', b'B']);
                in_jis = false;
            }
            out.push(ch as u8);
        } else {
            let mut tmp = [0u8; 4];
            let (bytes, _, bad) = encoding_rs::EUC_JP.encode(ch.encode_utf8(&mut tmp));
            if bad || bytes.len() != 2 || bytes[0] < 0xA1 || bytes[1] < 0xA1 {
                return Err(format!("字符 {ch:?} 无法用 ISO-2022-JP 表示"));
            }
            if !in_jis {
                out.extend_from_slice(&[0x1b, b'$', b'B']);
                in_jis = true;
            }
            out.push(bytes[0] - 0x80);
            out.push(bytes[1] - 0x80);
        }
    }
    if in_jis {
        out.extend_from_slice(&[0x1b, b'(', b'B']);
    }
    Ok(out)
}

/// 编码回磁盘字节 (换行已在文本里, 原样输出), 并回读校验: 解码结果必须等于输入, 否则拒绝写.
fn encode_back(text: &str, cs: Charset, bom: Bom) -> Result<Vec<u8>, String> {
    let body = if cs == Charset::Iso2022Jp {
        encode_iso2022jp(text)?
    } else {
        document_io::encode_document(text, cs, Bom::None, document_io::Eol::Lf)
            .map_err(|bad| format!("以下字符无法用 {} 表示: {}", cs.label(), bad.iter().collect::<String>()))?
    };
    let mut out = match bom {
        Bom::None => Vec::new(),
        Bom::Utf8 => vec![0xEF, 0xBB, 0xBF],
        Bom::Utf16Le => vec![0xFF, 0xFE],
        Bom::Utf16Be => vec![0xFE, 0xFF],
    };
    out.extend_from_slice(&body);
    let (back, lossy, _) = decode_raw(&out, cs)?;
    if lossy || back != text {
        return Err(format!("按 {} 编码后回读不一致, 已放弃写入", cs.label()));
    }
    Ok(out)
}

/// 原子写: 同目录临时文件 + sync + rename 覆盖; 保留原文件的权限.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.grep-{}-{nanos}.tmp", std::process::id()));
    let res = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        if let Ok(meta) = fs::metadata(path) {
            let _ = fs::set_permissions(&tmp, meta.permissions());
        }
        fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

fn norm_key(p: &str) -> String {
    let s = p.replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s).trim_end_matches('/').to_string();
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// 对单个文件做置换 (含 skip / 备份 / 原子写). `Err` 只在无法解码等「该文件不动」的情形.
fn replace_one(
    f: &Path,
    m: &ReplMatcher,
    o: &ReplaceOptions,
    forced: Option<Charset>,
    skip: &HashSet<String>,
) -> Option<FileReplaceResult> {
    let path_s = f.to_string_lossy().into_owned();
    let mut res = FileReplaceResult {
        path: path_s.clone(),
        replacements: 0,
        lines: 0,
        charset: String::new(),
        skipped: false,
        written: false,
        backup_path: None,
        error: None,
    };
    let raw = match fs::read(f) {
        Ok(r) => r,
        Err(e) => {
            res.error = Some(format!("读取失败: {e}"));
            return Some(res);
        }
    };
    let cs = forced.unwrap_or_else(|| detect_grep_charset(&raw));
    res.charset = cs.label().to_string();
    let (text, lossy, skip_bytes) = match decode_raw(&raw, cs) {
        Ok(x) => x,
        Err(e) => {
            res.error = Some(e);
            return Some(res);
        }
    };
    let (new_text, n, lines) = replace_in_text(&text, m, &o.replacement);
    if n == 0 {
        return None; // 没命中: 不出现在报告里
    }
    res.replacements = n;
    res.lines = lines;
    if skip.contains(&norm_key(&path_s)) {
        res.skipped = true;
        return Some(res);
    }
    if lossy {
        res.error = Some(format!("文件含 {} 无法解码的字节, 为免损坏内容已跳过", cs.label()));
        return Some(res);
    }
    let bom = match (skip_bytes, cs) {
        (0, _) => Bom::None,
        (_, Charset::Utf8) => Bom::Utf8,
        (_, Charset::Utf16Le) => Bom::Utf16Le,
        (_, Charset::Utf16Be) => Bom::Utf16Be,
        _ => Bom::None,
    };
    // UTF-7 / CESU-8 解码时换行已被规整, 无法保证原样写回
    if encoding_of(cs).is_none() {
        res.error = Some(format!("{} 暂不支持置换写回", cs.label()));
        return Some(res);
    }
    let bytes = match encode_back(&new_text, cs, bom) {
        Ok(b) => b,
        Err(e) => {
            res.error = Some(e);
            return Some(res);
        }
    };
    if o.backup.unwrap_or(false) {
        let suffix = o.backup_suffix.as_deref().filter(|s| !s.is_empty()).unwrap_or(".bak");
        let mut bp = f.as_os_str().to_owned();
        bp.push(suffix);
        let bp = PathBuf::from(bp);
        if let Err(e) = fs::write(&bp, &raw) {
            res.error = Some(format!("备份失败: {e}"));
            return Some(res);
        }
        res.backup_path = Some(bp.to_string_lossy().into_owned());
    }
    match atomic_write(f, &bytes) {
        Ok(()) => res.written = true,
        Err(e) => res.error = Some(format!("写入失败: {e}")),
    }
    Some(res)
}

/// Grep 置换入口: 按 `opts.search` 枚举文件, 对命中的文件按其原编码 / BOM / 逐行换行写回.
///
/// - 不区分文件是否在编辑器里打开; 调用方把「已打开且已修改」的路径放进 `skip_paths`, 它们只被报告为 skipped.
/// - 单个文件失败 (读写 / 无法编码) 不影响其他文件, 记在该文件的 `error` 里.
/// - `max_results` 对置换无效 (总是替换全部命中).
pub fn grep_replace(root: &Path, opts: &ReplaceOptions) -> Result<ReplaceReport, String> {
    if opts.search.pattern.is_empty() {
        return Err("置换需要非空的检索条件".to_string());
    }
    let forced = parse_charset_label(opts.search.charset.as_deref())?;
    let matcher = ReplMatcher::new(&opts.search)?;
    let skip: HashSet<String> = opts
        .skip_paths
        .iter()
        .flatten()
        .map(|p| norm_key(p))
        .collect();
    let mut report = ReplaceReport::default();
    for (f, _name) in filtered_files(root, &opts.search)? {
        let Some(r) = replace_one(&f, &matcher, opts, forced, &skip) else { continue };
        if r.skipped {
            report.files_skipped += 1;
        } else if r.error.is_some() {
            report.files_failed += 1;
        } else if r.written {
            report.files_changed += 1;
            report.total_replacements += r.replacements;
        }
        report.files.push(r);
    }
    report.files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(report)
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

    // ---------------------------------------------------------------- 编码 / 置换 ---
    fn enc(label: &str, s: &str) -> Vec<u8> {
        match label {
            "sjis" => encoding_rs::SHIFT_JIS.encode(s).0.into_owned(),
            "euc" => encoding_rs::EUC_JP.encode(s).0.into_owned(),
            "u16le" => s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect(),
            "u16be" => s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect(),
            _ => s.as_bytes().to_vec(),
        }
    }

    fn with_bom(bom: &[u8], mut body: Vec<u8>) -> Vec<u8> {
        let mut v = bom.to_vec();
        v.append(&mut body);
        v
    }

    fn enc_tree(dir: &Path) {
        let jp = "first\r\n日本語の行 needle\r\nlast\r\n";
        fs::write(dir.join("sjis.txt"), enc("sjis", jp)).unwrap();
        fs::write(dir.join("euc.txt"), enc("euc", jp)).unwrap();
        fs::write(dir.join("u16le_bom.txt"), with_bom(&[0xFF, 0xFE], enc("u16le", jp))).unwrap();
        fs::write(dir.join("u16be_bom.txt"), with_bom(&[0xFE, 0xFF], enc("u16be", jp))).unwrap();
        fs::write(dir.join("u16le_nobom.txt"), enc("u16le", jp)).unwrap();
        fs::write(dir.join("utf8_bom.txt"), with_bom(&[0xEF, 0xBB, 0xBF], enc("utf8", jp))).unwrap();
        fs::write(dir.join("utf8.txt"), jp).unwrap();
        // ISO-2022-JP: 日本語 = 46 7C 4B 5C 38 6C
        let mut jis = b"first\nx ".to_vec();
        jis.extend_from_slice(&[0x1b, b'$', b'B', 0x46, 0x7c, 0x4b, 0x5c, 0x38, 0x6c, 0x1b, b'(', b'B']);
        jis.extend_from_slice(b" needle\n");
        fs::write(dir.join("jis.txt"), jis).unwrap();
    }

    #[test]
    fn search_decodes_every_encoding_with_correct_line_and_text() {
        let dir = TempDir::new("enc-search");
        enc_tree(dir.path());
        for pat in ["日本語", "needle"] {
            let mut o = opts(pat);
            o.case_sensitive = Some(true);
            let r = search(dir.path(), &o).unwrap();
            let names: Vec<String> = r.iter().map(|x| fname(&x.path)).collect();
            assert_eq!(r.len(), 8, "{pat}: {names:?}");
            for x in &r {
                let n = fname(&x.path);
                if n == "jis.txt" {
                    assert_eq!((x.line, x.text.as_str()), (2, "x 日本語 needle"), "{n}");
                } else {
                    assert_eq!((x.line, x.text.as_str()), (2, "日本語の行 needle"), "{n}");
                }
            }
        }
    }

    #[test]
    fn search_strips_bom_from_first_line() {
        let dir = TempDir::new("enc-bom");
        fs::write(dir.path().join("b.txt"), with_bom(&[0xEF, 0xBB, 0xBF], b"head\nx\n".to_vec())).unwrap();
        let mut o = opts("head");
        o.regex = Some(true);
        o.pattern = "^head$".into();
        assert_eq!(search(dir.path(), &o).unwrap().len(), 1, "BOM 不应混进首行");
    }

    #[test]
    fn search_regex_path_also_decodes() {
        let dir = TempDir::new("enc-re");
        enc_tree(dir.path());
        let mut o = opts("日本語.*needle");
        o.regex = Some(true);
        assert_eq!(search(dir.path(), &o).unwrap().len(), 8);
    }

    #[test]
    fn search_explicit_charset_overrides_detection() {
        let dir = TempDir::new("enc-force");
        enc_tree(dir.path());
        let mut o = opts("日本語");
        o.charset = Some("sjis".into());
        let r = search(dir.path(), &o).unwrap();
        let names: Vec<String> = r.iter().map(|x| fname(&x.path)).collect();
        assert_eq!(names, vec!["sjis.txt".to_string()], "只有 SJIS 文件能按 SJIS 读出");
        o.charset = Some("euc-jp".into());
        let r = search(dir.path(), &o).unwrap();
        assert_eq!(r.iter().map(|x| fname(&x.path)).collect::<Vec<_>>(), vec!["euc.txt".to_string()]);
        o.charset = Some("auto".into());
        assert_eq!(search(dir.path(), &o).unwrap().len(), 8);
        o.charset = Some("no-such".into());
        assert!(search(dir.path(), &o).is_err());
        // utf-16le 显式: 无 BOM 与有 BOM 的 LE 文件命中
        o.charset = Some("utf-16le".into());
        let r = search(dir.path(), &o).unwrap();
        let mut names: Vec<String> = r.iter().map(|x| fname(&x.path)).collect();
        names.sort();
        assert_eq!(names, vec!["u16le_bom.txt".to_string(), "u16le_nobom.txt".to_string()]);
    }

    #[test]
    fn search_options_charset_is_optional_in_json() {
        let o: SearchOptions = serde_json::from_str(r#"{"pattern":"a"}"#).unwrap();
        assert!(o.charset.is_none());
        let o: SearchOptions = serde_json::from_str(r#"{"pattern":"a","charset":"sjis"}"#).unwrap();
        assert_eq!(o.charset.as_deref(), Some("sjis"));
    }

    #[test]
    fn utf8_fast_path_unchanged_for_plain_files() {
        let dir = TempDir::new("enc-plain");
        fs::write(dir.path().join("p.txt"), "a\r\nHello\r\nb").unwrap();
        let mut o = opts("hello");
        o.case_sensitive = Some(false);
        let r = search(dir.path(), &o).unwrap();
        assert_eq!((r.len(), r[0].line, r[0].text.as_str()), (1, 2, "Hello"));
    }

    fn rep(search: SearchOptions, to: &str) -> ReplaceOptions {
        ReplaceOptions { search, replacement: to.into(), ..Default::default() }
    }

    #[test]
    fn replace_keeps_encoding_bom_and_mixed_eol() {
        let dir = TempDir::new("rep-enc");
        let p = dir.path();
        let body = "一行目 needle\r\n二行目\nneedle 三\r四行目 needle needle";
        let new_body = "一行目 針\r\n二行目\n針 三\r四行目 針 針";
        let cases: Vec<(&str, Vec<u8>, Vec<u8>)> = vec![
            ("sjis.txt", enc("sjis", body), enc("sjis", new_body)),
            ("euc.txt", enc("euc", body), enc("euc", new_body)),
            ("u8.txt", enc("utf8", body), enc("utf8", new_body)),
            ("u8bom.txt", with_bom(&[0xEF, 0xBB, 0xBF], enc("utf8", body)), with_bom(&[0xEF, 0xBB, 0xBF], enc("utf8", new_body))),
            ("u16le.txt", with_bom(&[0xFF, 0xFE], enc("u16le", body)), with_bom(&[0xFF, 0xFE], enc("u16le", new_body))),
            ("u16be.txt", with_bom(&[0xFE, 0xFF], enc("u16be", body)), with_bom(&[0xFE, 0xFF], enc("u16be", new_body))),
            ("u16nobom.txt", enc("u16le", body), enc("u16le", new_body)),
        ];
        for (n, before, _) in &cases {
            fs::write(p.join(n), before).unwrap();
        }
        let mut s = opts("needle");
        s.case_sensitive = Some(true);
        let rep_r = grep_replace(p, &rep(s, "針")).unwrap();
        assert_eq!(rep_r.files_changed, 7, "{rep_r:#?}");
        assert_eq!(rep_r.total_replacements, 7 * 4);
        assert_eq!(rep_r.files_failed + rep_r.files_skipped, 0);
        for (n, _, after) in &cases {
            assert_eq!(&fs::read(p.join(n)).unwrap(), after, "{n}");
        }
        // 临时文件不残留
        let leftovers: Vec<_> = fs::read_dir(p).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn replace_iso2022jp_roundtrip() {
        let dir = TempDir::new("rep-jis");
        enc_tree(dir.path());
        let mut s = opts("日本語");
        s.charset = Some("jis".into());
        let r = grep_replace(dir.path(), &rep(s, "漢字")).unwrap();
        assert_eq!(r.files_changed, 1, "{r:#?}");
        let b = fs::read(dir.path().join("jis.txt")).unwrap();
        // 漢字 = 34 41 3B 7A
        let mut want = b"first\nx ".to_vec();
        want.extend_from_slice(&[0x1b, b'$', b'B', 0x34, 0x41, 0x3b, 0x7a, 0x1b, b'(', b'B']);
        want.extend_from_slice(b" needle\n");
        assert_eq!(b, want);
    }

    #[test]
    fn replace_regex_captures_and_literal_dollar() {
        let dir = TempDir::new("rep-re");
        fs::write(dir.path().join("a.txt"), "key=1 key=22\nnone\nkey=x\n").unwrap();
        let mut s = opts(r"(\w+)=(\d+)");
        s.regex = Some(true);
        let r = grep_replace(dir.path(), &rep(s.clone(), "$2:$1 [$&] $$ $9")).unwrap();
        assert_eq!(r.total_replacements, 2);
        assert_eq!(r.files[0].lines, 1);
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "1:key [key=1] $  22:key [key=22] $ \nnone\nkey=x\n");
        // 非正则: 置换串中的 $1 是字面
        let dir2 = TempDir::new("rep-lit");
        fs::write(dir2.path().join("a.txt"), "x Foo X\n").unwrap();
        let mut s2 = opts("foo");
        s2.case_sensitive = Some(false);
        grep_replace(dir2.path(), &rep(s2, "$1")).unwrap();
        assert_eq!(fs::read_to_string(dir2.path().join("a.txt")).unwrap(), "x $1 X\n");
    }

    #[test]
    fn replace_noncapturing_group_does_not_shift_numbers() {
        let dir = TempDir::new("rep-nc");
        fs::write(dir.path().join("a.txt"), "ab-cd\n").unwrap();
        let mut s = opts(r"(?:a)(b)-(c)d");
        s.regex = Some(true);
        grep_replace(dir.path(), &rep(s, "$2$1")).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "cb\n");
    }

    #[test]
    fn replace_alternation_and_repeat_captures() {
        let dir = TempDir::new("rep-alt");
        fs::write(dir.path().join("a.txt"), "cat dog\n").unwrap();
        let mut s = opts(r"(cat|dog)");
        s.regex = Some(true);
        grep_replace(dir.path(), &rep(s, "<$1>")).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "<cat> <dog>\n");
    }

    #[test]
    fn replace_whole_word_and_case_insensitive() {
        let dir = TempDir::new("rep-ww");
        fs::write(dir.path().join("a.txt"), "Cat concat cat.\n").unwrap();
        let mut s = opts("cat");
        s.case_sensitive = Some(false);
        s.whole_word = Some(true);
        let r = grep_replace(dir.path(), &rep(s, "dog")).unwrap();
        assert_eq!(r.total_replacements, 2);
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "dog concat dog.\n");
    }

    #[test]
    fn replace_empty_matches_do_not_loop() {
        let dir = TempDir::new("rep-empty");
        fs::write(dir.path().join("a.txt"), "ab\n").unwrap();
        let mut s = opts("x*");
        s.regex = Some(true);
        let r = grep_replace(dir.path(), &rep(s, "-")).unwrap();
        assert_eq!(r.files_changed, 1);
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "-a-b-\n");
    }

    #[test]
    fn replace_backup_and_skip_list() {
        let dir = TempDir::new("rep-bak");
        let p = dir.path();
        fs::write(p.join("keep.txt"), "old\n").unwrap();
        fs::write(p.join("open.txt"), "old\n").unwrap();
        fs::write(p.join("none.txt"), "zzz\n").unwrap();
        let mut o = rep(opts("old"), "new");
        o.backup = Some(true);
        o.skip_paths = Some(vec![p.join("OPEN.txt").to_string_lossy().replace('\\', "/")]);
        let r = grep_replace(p, &o).unwrap();
        assert_eq!((r.files_changed, r.files_skipped, r.files_failed), (1, 1, 0), "{r:#?}");
        assert_eq!(r.files.len(), 2, "没命中的文件不进报告");
        assert_eq!(fs::read_to_string(p.join("keep.txt")).unwrap(), "new\n");
        assert_eq!(fs::read_to_string(p.join("keep.txt.bak")).unwrap(), "old\n");
        assert_eq!(fs::read_to_string(p.join("open.txt")).unwrap(), "old\n", "skipped 文件不动");
        assert!(!p.join("open.txt.bak").exists());
        let sk = r.files.iter().find(|f| f.skipped).unwrap();
        assert_eq!(sk.replacements, 1);
        assert!(!sk.written);
        assert!(r.files.iter().find(|f| f.written).unwrap().backup_path.as_deref().unwrap().ends_with("keep.txt.bak"));
        // 自定义后缀
        let mut o = rep(opts("new"), "newer");
        o.backup = Some(true);
        o.backup_suffix = Some(".orig".into());
        grep_replace(p, &o).unwrap();
        assert!(p.join("keep.txt.orig").exists());
    }

    #[test]
    fn replace_refuses_unencodable_and_lossy_files() {
        let dir = TempDir::new("rep-bad");
        let p = dir.path();
        // SJIS 文件换成 SJIS 表示不了的字符 => 报错且原文件不动
        let before = enc("sjis", "日本語 old\n");
        fs::write(p.join("a.txt"), &before).unwrap();
        let mut s = opts("old");
        s.charset = Some("sjis".into());
        let r = grep_replace(p, &rep(s, "😀")).unwrap();
        assert_eq!((r.files_failed, r.files_changed), (1, 0), "{r:#?}");
        assert_eq!(fs::read(p.join("a.txt")).unwrap(), before);
        // 强制 UTF-8 读一个含非法字节的文件 => 拒绝改写
        let dir2 = TempDir::new("rep-lossy");
        let bad = vec![b'o', b'l', b'd', 0xFF, 0xFE, 0xFD, b'\n'];
        fs::write(dir2.path().join("b.txt"), &bad).unwrap();
        let mut s = opts("old");
        s.charset = Some("utf-8".into());
        let r = grep_replace(dir2.path(), &rep(s, "x")).unwrap();
        assert_eq!(r.files_failed, 1);
        assert_eq!(fs::read(dir2.path().join("b.txt")).unwrap(), bad);
    }

    #[test]
    fn replace_requires_pattern_and_valid_charset() {
        let dir = TempDir::new("rep-args");
        assert!(grep_replace(dir.path(), &rep(opts(""), "x")).is_err());
        let mut s = opts("a");
        s.charset = Some("zzz".into());
        assert!(grep_replace(dir.path(), &rep(s, "x")).is_err());
        let mut s = opts("(");
        s.regex = Some(true);
        assert!(grep_replace(dir.path(), &rep(s, "x")).is_err());
    }

    #[test]
    fn replace_options_json_flattens_search_fields() {
        let o: ReplaceOptions = serde_json::from_str(
            r#"{"pattern":"a","regex":true,"charset":"sjis","replacement":"b","backup":true,"skip_paths":["x"]}"#,
        )
        .unwrap();
        assert_eq!(o.search.pattern, "a");
        assert_eq!(o.search.charset.as_deref(), Some("sjis"));
        assert_eq!((o.replacement.as_str(), o.backup, o.skip_paths.as_ref().map(|v| v.len())), ("b", Some(true), Some(1)));
    }

    #[test]
    fn replace_respects_file_filters() {
        let dir = TempDir::new("rep-filter");
        fs::write(dir.path().join("a.rs"), "old\n").unwrap();
        fs::write(dir.path().join("a.txt"), "old\n").unwrap();
        let mut s = opts("old");
        s.include_files = Some(vec!["*.rs".into()]);
        let r = grep_replace(dir.path(), &rep(s, "new")).unwrap();
        assert_eq!(r.files_changed, 1);
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "old\n");
    }

    #[test]
    fn atomic_write_replaces_without_leftovers() {
        let dir = TempDir::new("atomic");
        let f = dir.path().join("f.txt");
        fs::write(&f, "old").unwrap();
        atomic_write(&f, b"new").unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"new");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn detect_utf16_without_bom_only_when_pattern_is_clear() {
        assert_eq!(detect_grep_charset(&enc("u16le", "hello world")), Charset::Utf16Le);
        assert_eq!(detect_grep_charset(&enc("u16be", "hello world")), Charset::Utf16Be);
        assert_eq!(detect_grep_charset(b"plain ascii"), Charset::Utf8);
        assert_eq!(detect_grep_charset(&[0u8; 64]), Charset::Utf8, "全 NUL 不是 UTF-16 文本");
    }
}
