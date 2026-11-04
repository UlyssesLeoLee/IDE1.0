//! 标签跳转 (tag jump): 找符号定义。
//! 先读 ctags 格式的 `tags` 文件 (从当前文件所在目录往上找); 没有就用 `editcore::tag_jump` 扫文件夹里的源码。
//! `tags` 文件当不可信数据: 只认相对路径, 拒绝 `..` / 绝对 / UNC; 所有扫描都有文件数 / 大小 / 深度上限, 不跟符号链接。

use editcore::tag_jump::{extract_tags, TagKind};
use std::path::{Component, Path, PathBuf};

pub const MAX_FILES: usize = 5000;
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_DEPTH: usize = 8;
pub const MAX_CANDS: usize = 200;
const MAX_PARENTS: usize = 8;
const SKIP_DIRS: &[&str] = &["target", "node_modules", "dist", "build", "out", "vendor"];
const EXTS: &[&str] = &["rs", "py", "js", "mjs", "ts", "jsx", "tsx", "c", "h", "cpp", "cc", "hpp", "cs", "java", "go", "rb", "php", "kt", "swift", "lua"];

/// 一个候选定义位置 (行号从 1 起)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cand {
    pub name: String,
    pub path: PathBuf,
    pub line: usize,
    pub kind: String,
}

fn kind_label(k: TagKind) -> &'static str {
    match k {
        TagKind::Function => "f",
        TagKind::Method => "m",
        TagKind::Class => "c",
        TagKind::Struct => "s",
        TagKind::Enum => "e",
        TagKind::Module => "n",
        TagKind::Variable => "v",
    }
}

/// 从 `from` 起往上最多几层找 `tags` 文件 (普通文件, 不是链接)
pub fn find_tags_file(from: &Path) -> Option<PathBuf> {
    let mut dir = Some(from);
    for _ in 0..MAX_PARENTS {
        let d = dir?;
        let p = d.join("tags");
        if std::fs::symlink_metadata(&p).map(|m| m.is_file()).unwrap_or(false) {
            return Some(p);
        }
        dir = d.parent();
    }
    None
}

/// tags 文件里的相对路径是否可信: 非空、不是绝对 / UNC / 带盘符、不含 `..`
pub fn safe_rel(rel: &str) -> bool {
    if rel.is_empty() || rel.starts_with("\\\\") || rel.starts_with("//") || rel.contains('\0') {
        return false;
    }
    let p = Path::new(rel);
    !p.is_absolute() && p.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// ctags 的地址 `/^text$/` -> 要匹配的整行文字; 不是这个形式 = None
fn pattern_of(addr: &str) -> Option<String> {
    let a = addr.strip_prefix('/').or_else(|| addr.strip_prefix('?'))?;
    let a = a.strip_suffix('/').or_else(|| a.strip_suffix('?'))?;
    let a = a.strip_prefix('^').unwrap_or(a);
    let a = a.strip_suffix('$').unwrap_or(a);
    let mut out = String::new();
    let mut it = a.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            if let Some(n) = it.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

/// 在 tags 文件内容里找名字为 `name` 的条目 (`base` = tags 文件所在目录); 地址是 /模式/ 的去源文件里找那一行
pub fn lookup_in_tags(text: &str, base: &Path, name: &str) -> Vec<Cand> {
    let mut out = vec![];
    for l in text.lines() {
        if l.starts_with('!') {
            continue;
        }
        let mut f = l.splitn(3, '\t');
        let (Some(n), Some(file), Some(rest)) = (f.next(), f.next(), f.next()) else { continue };
        if n != name || !safe_rel(file) {
            continue;
        }
        let (addr, tail) = match rest.find(";\"") {
            Some(i) => (&rest[..i], rest[i + 2..].trim_start_matches('\t')),
            None => (rest, ""),
        };
        let kind = tail.split('\t').next().unwrap_or("").trim_start_matches("kind:").to_string();
        let path = base.join(file);
        let line = match addr.trim().parse::<usize>() {
            Ok(n) if n > 0 => n,
            _ => match pattern_of(addr.trim()).and_then(|p| line_matching(&path, &p)) {
                Some(n) => n,
                None => continue,
            },
        };
        out.push(Cand { name: n.to_string(), path, line, kind });
        if out.len() >= MAX_CANDS {
            break;
        }
    }
    out
}

fn line_matching(path: &Path, pat: &str) -> Option<usize> {
    if std::fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().position(|l| l == pat).map(|i| i + 1)
}

/// 递归列出 `root` 下的源码文件 (有数量 / 深度上限; 不进隐藏目录和 SKIP_DIRS, 不跟符号链接)
pub fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut ents: Vec<_> = rd.flatten().collect();
        ents.sort_by_key(|e| e.file_name());
        for e in ents {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            let p = e.path();
            if ft.is_dir() {
                if depth < MAX_DEPTH && !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push((p, depth + 1));
                }
            } else if ft.is_file() && p.extension().and_then(|x| x.to_str()).map(|x| EXTS.contains(&x.to_ascii_lowercase().as_str())).unwrap_or(false) {
                out.push(p);
                if out.len() >= MAX_FILES {
                    return out;
                }
            }
        }
    }
    out
}

fn tags_of(path: &Path) -> Vec<editcore::tag_jump::Tag> {
    match std::fs::metadata(path) {
        Ok(m) if m.len() <= MAX_FILE_BYTES => {}
        _ => return vec![],
    }
    match std::fs::read_to_string(path) {
        Ok(t) => extract_tags(&t, path),
        Err(_) => vec![],
    }
}

/// 没有 tags 文件时: 扫 `root` 下的源码找 `name` 的定义
pub fn scan(root: &Path, name: &str) -> Vec<Cand> {
    let mut out = vec![];
    for f in source_files(root) {
        for t in tags_of(&f) {
            if t.name == name {
                out.push(Cand { name: t.name, path: f.clone(), line: t.line, kind: kind_label(t.kind).into() });
            }
        }
        if out.len() >= MAX_CANDS {
            break;
        }
    }
    out
}

/// 找定义: 有 tags 文件且命中就用它; 否则扫 `scan_root`
pub fn lookup(name: &str, doc_dir: Option<&Path>, scan_root: Option<&Path>) -> Vec<Cand> {
    if name.is_empty() || name.len() > 256 {
        return vec![];
    }
    let from = doc_dir.or(scan_root);
    if let Some(tf) = from.and_then(find_tags_file) {
        if std::fs::metadata(&tf).map(|m| m.len() <= 64 * MAX_FILE_BYTES).unwrap_or(false) {
            if let (Some(base), Ok(text)) = (tf.parent(), std::fs::read_to_string(&tf)) {
                let hits = lookup_in_tags(&text, base, name);
                if !hits.is_empty() {
                    return hits;
                }
            }
        }
    }
    match scan_root.or(doc_dir) {
        Some(r) => scan(r, name),
        None => vec![],
    }
}

/// tags 文件的内容 (ctags 扩展格式的子集, 按名字排序): 每行 `名字<TAB>相对路径<TAB>行号;"<TAB>种类`
pub fn render_tags(root: &Path, entries: &[Cand]) -> String {
    let mut v: Vec<&Cand> = entries.iter().collect();
    v.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()).then(a.path.cmp(&b.path)).then(a.line.cmp(&b.line)));
    let mut s = String::from("!_TAG_FILE_FORMAT\t2\t/extended format/\n!_TAG_FILE_SORTED\t1\t/0=unsorted, 1=sorted/\n!_TAG_PROGRAM_NAME\tide-fast\n");
    for c in v {
        let rel = c.path.strip_prefix(root).unwrap_or(&c.path).to_string_lossy().replace('\\', "/");
        s.push_str(&format!("{}\t{}\t{};\"\t{}\n", c.name, rel, c.line, c.kind));
    }
    s
}

/// 生成 `root/tags`; 返回 (条目数, 文件路径)
pub fn generate(root: &Path) -> Result<(usize, PathBuf), String> {
    let mut all = vec![];
    for f in source_files(root) {
        for t in tags_of(&f) {
            all.push(Cand { name: t.name, path: f.clone(), line: t.line, kind: kind_label(t.kind).into() });
        }
    }
    let target = root.join("tags");
    if std::fs::symlink_metadata(&target).map(|m| !m.is_file()).unwrap_or(false) {
        return Err("tags 不是普通文件".into());
    }
    std::fs::write(&target, render_tags(root, &all)).map_err(|e| e.to_string())?;
    Ok((all.len(), target))
}

/// 标签跳转的界面状态: 候选列表 / 关键字输入框 / 返回栈
#[derive(Default)]
pub struct TagState {
    pub pick: Option<Vec<Cand>>,
    pub kw_open: bool,
    kw_text: String,
    /// 跳转前的位置 (文档 id, 光标), 后进先出
    pub stack: Vec<(u64, [usize; 2])>,
}

pub enum Ev {
    /// 按这个名字找
    Lookup(String),
    /// 去某个候选
    Go(Cand),
}

impl TagState {
    pub const STACK_MAX: usize = 100;

    pub fn push(&mut self, doc: u64, pos: [usize; 2]) {
        if self.stack.len() >= Self::STACK_MAX {
            self.stack.remove(0);
        }
        self.stack.push((doc, pos));
    }

    pub fn open_keyword(&mut self) {
        self.kw_open = true;
    }

    pub fn windows(&mut self, ctx: &eframe::egui::Context) -> Vec<Ev> {
        use crate::i18n::trz;
        use eframe::egui::{self, Key};
        let mut evs = vec![];
        if self.kw_open {
            let mut open = true;
            egui::Window::new(trz("指定关键字的标签跳转")).open(&mut open).collapsible(false).resizable(false).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.kw_text).desired_width(240.0));
                r.request_focus();
                if ui.input(|i| i.key_pressed(Key::Enter)) && !self.kw_text.trim().is_empty() {
                    evs.push(Ev::Lookup(self.kw_text.trim().to_string()));
                    self.kw_open = false;
                }
            });
            if !open || ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.kw_open = false;
            }
        }
        if let Some(list) = &self.pick {
            let mut open = true;
            let mut go: Option<Cand> = None;
            egui::Window::new(trz("选择跳转位置")).open(&mut open).collapsible(false).default_width(480.0).default_pos(egui::pos2(24.0, 56.0)).show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for c in list {
                        let label = format!("{}  {}:{}  [{}]", c.name, c.path.display(), c.line, c.kind);
                        if ui.selectable_label(false, label).clicked() {
                            go = Some(c.clone());
                        }
                    }
                });
            });
            if let Some(c) = go {
                evs.push(Ev::Go(c));
                self.pick = None;
            } else if !open || ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.pick = None;
            }
        }
        evs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("idefast-tag-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rel_paths_are_checked() {
        assert!(safe_rel("src/a.rs"));
        assert!(safe_rel("./a.rs"));
        assert!(!safe_rel("../a.rs"));
        assert!(!safe_rel("a/../../b"));
        assert!(!safe_rel("\\\\host\\share\\a"));
        assert!(!safe_rel("//host/share/a"));
        assert!(!safe_rel("C:\\x"));
        assert!(!safe_rel(""));
    }

    #[test]
    fn pattern_addresses_are_unescaped() {
        assert_eq!(pattern_of("/^fn a() {$/").as_deref(), Some("fn a() {"));
        assert_eq!(pattern_of("/^a\\/b\\\\c$/").as_deref(), Some("a/b\\c"));
        assert_eq!(pattern_of("12"), None);
    }

    #[test]
    fn tags_file_lookup_handles_number_and_pattern() {
        let d = tmp("lookup");
        std::fs::write(d.join("a.rs"), "// x\nfn alpha() {}\nfn beta() {}\n").unwrap();
        let tags = "!_TAG_FILE_FORMAT\t2\nalpha\ta.rs\t2;\"\tf\nbeta\ta.rs\t/^fn beta() {}$/;\"\tf\nevil\t../x.rs\t1\nalpha\t\\\\h\\s\\a.rs\t1\n";
        let a = lookup_in_tags(tags, &d, "alpha");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].line, 2);
        assert_eq!(a[0].kind, "f");
        let b = lookup_in_tags(tags, &d, "beta");
        assert_eq!(b[0].line, 3);
        assert!(lookup_in_tags(tags, &d, "evil").is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn scan_and_generate_roundtrip() {
        let d = tmp("scan");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::create_dir_all(d.join("target")).unwrap();
        std::fs::write(d.join("sub").join("m.rs"), "struct Foo;\nfn go() {}\n").unwrap();
        std::fs::write(d.join("target").join("skip.rs"), "fn go() {}\n").unwrap();
        let hits = scan(&d, "go");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
        let (n, tf) = generate(&d).unwrap();
        assert_eq!(n, 2);
        let text = std::fs::read_to_string(&tf).unwrap();
        let foo = lookup_in_tags(&text, &d, "Foo");
        assert_eq!(foo.len(), 1);
        assert_eq!(foo[0].path, d.join("sub/m.rs"));
        assert_eq!(foo[0].line, 1);
        // 有 tags 文件时 lookup 走它
        assert_eq!(lookup("go", Some(&d.join("sub")), Some(&d)).len(), 1);
        assert!(lookup("", None, None).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn back_stack_is_bounded() {
        let mut s = TagState::default();
        for i in 0..150 {
            s.push(1, [i, 0]);
        }
        assert_eq!(s.stack.len(), TagState::STACK_MAX);
        assert_eq!(s.stack.last().unwrap().1, [149, 0]);
    }
}
