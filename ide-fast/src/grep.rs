//! Grep: 条件 -> 引擎选项, 分片后台任务 (流式、可取消、5000 上限), 结果文本 (Sakura 格式) 与结果行解析, 侧栏里的那一页控件。
//! 搜索语义全在 `editcore::grep_engine` (和 Tauri 版同一套); 这里只管"怎么跑、怎么给人看"。

use eframe::egui;
use editcore::grep_engine::{self, ReplaceOptions, SearchOptions, SearchResult, SKIP_DIRS};
use editcore::search_ops::{self, SearchReq};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// 命中数上限 (到了就停, 结果里注明)
pub const CAP: usize = 5000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fmt {
    /// 路径(行,列): 文字
    Normal,
    /// 只列文件名
    Files,
    /// 每个文件的命中数
    Count,
}

/// 对话框里的全部输入 (一一对应 Sakura 的 Grep 对话框)
#[derive(Clone, Debug)]
pub struct GrepOpts {
    pub pattern: String,
    pub repl: String,
    /// 文件名 (`*.rs;*.toml`; `!x` = 排除文件, `#x` = 排除文件夹)
    pub files: String,
    /// 文件夹 (`;` 分隔, 空 = 整个项目)
    pub folders: String,
    pub exc_files: String,
    pub exc_folders: String,
    /// 字符集标签, 空 = 逐文件自动判别
    pub charset: String,
    pub case: bool,
    pub regex: bool,
    pub word: bool,
    pub sub: bool,
    pub outside: bool,
    /// 只搜当前文件 (用编辑器里的当前正文, 含未保存的修改)
    pub this_file: bool,
    pub fmt: Fmt,
}

impl Default for GrepOpts {
    fn default() -> Self {
        GrepOpts { pattern: String::new(), repl: String::new(), files: String::new(), folders: String::new(), exc_files: String::new(), exc_folders: String::new(), charset: String::new(), case: false, regex: false, word: false, sub: true, outside: false, this_file: false, fmt: Fmt::Normal }
    }
}

fn split(s: &str, seps: &[char]) -> Vec<String> {
    s.split(seps).map(str::trim).filter(|x| !x.is_empty()).map(str::to_string).collect()
}

fn some(v: Vec<String>) -> Option<Vec<String>> {
    if v.is_empty() { None } else { Some(v) }
}

/// 输入框 -> 引擎条件。搜索和置换共用这一份, 两边不会对不上
pub fn build_opts(o: &GrepOpts) -> SearchOptions {
    let (mut inc, mut exf, mut exd) = (vec![], split(&o.exc_files, &[',', ';']), split(&o.exc_folders, &[';']));
    for t in split(&o.files, &[',', ';', ' ']) {
        if let Some(x) = t.strip_prefix('!') {
            exf.push(x.to_string());
        } else if let Some(x) = t.strip_prefix('#') {
            exd.push(x.to_string());
        } else {
            inc.push(t);
        }
    }
    SearchOptions {
        pattern: o.pattern.clone(),
        case_sensitive: Some(o.case),
        regex: Some(o.regex),
        whole_word: Some(o.word),
        include_files: some(inc),
        exclude_files: some(exf),
        exclude_folders: some(exd),
        search_folders: some(split(&o.folders, &[';'])),
        allow_outside_root: Some(o.outside),
        subfolders: Some(o.sub),
        max_results: Some(CAP),
        charset: if o.charset.is_empty() { None } else { Some(o.charset.clone()) },
        ..Default::default()
    }
}

// ---------------------------------------------------------------- 分片

fn rel_of(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// 带 `/` 的目录排除项改成相对分片根的写法 (分片根 = 引擎看到的"项目根"); 不在这个分片里的丢掉
fn rebase(pat: &str, shard_rel: &str) -> Option<String> {
    let p = pat.trim().trim_end_matches('/');
    if !p.contains('/') {
        return Some(p.to_string());
    }
    p.strip_prefix(&format!("{shard_rel}/")).map(str::to_string)
}

/// 把一次 Grep 切成若干片, 顺序执行: 每片是 (引擎的"项目根", 条件)。
/// 没点名文件夹时, 分片根就是该子目录本身, 引擎照旧跳过 target / node_modules 等; 点名了文件夹 (`r != root`) 则保持"点名要搜就搜"
/// 的语义 (不跳过 SKIP_DIRS), 用 `search_folders` 指定分片。每个目录先搜它自己的文件 (不含子目录), 再按子目录分, 共两层, 更深的整棵一片。
pub fn plan(root: &Path, o: &GrepOpts) -> Result<Vec<(PathBuf, SearchOptions)>, String> {
    let base = build_opts(o);
    let folders = base.search_folders.clone().unwrap_or_default();
    let roots = grep_engine::resolve_search_folders(root, &folders, o.outside)?;
    let sub = o.sub;
    let exd: Vec<String> = base.exclude_folders.clone().unwrap_or_default();
    let mut out = Vec::new();
    for r in roots {
        let explicit = r != root;
        if explicit {
            let name = r.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            if exd.iter().any(|p| grep_engine::is_dir_exclude(p, &name, &rel_of(root, &r))) {
                continue;
            }
        }
        push_shards(&mut out, root, &r, explicit, &base, &exd, 0, sub);
    }
    Ok(out)
}

fn shard(root: &Path, dir: &Path, explicit: bool, base: &SearchOptions, exd: &[String], subfolders: bool) -> (PathBuf, SearchOptions) {
    let mut so = base.clone();
    so.subfolders = Some(subfolders);
    if explicit {
        so.search_folders = Some(vec![dir.to_string_lossy().into_owned()]);
        so.allow_outside_root = Some(true);
        (root.to_path_buf(), so)
    } else if dir == root {
        so.search_folders = None;
        (root.to_path_buf(), so)
    } else {
        let srel = rel_of(root, dir);
        let ex: Vec<String> = exd.iter().filter_map(|p| rebase(p, &srel)).collect();
        so.search_folders = None;
        so.exclude_folders = some(ex);
        (dir.to_path_buf(), so)
    }
}

#[allow(clippy::too_many_arguments)]
fn push_shards(out: &mut Vec<(PathBuf, SearchOptions)>, root: &Path, dir: &Path, explicit: bool, base: &SearchOptions, exd: &[String], depth: usize, sub: bool) {
    out.push(shard(root, dir, explicit, base, exd, false)); // 这一层自己的文件
    if !sub {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut kids: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false)) // 符号链接不算目录: 不跟随
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            !n.starts_with('.') && (explicit || !SKIP_DIRS.contains(&n.as_str())) && !exd.iter().any(|p| grep_engine::is_dir_exclude(p, &n, &rel_of(root, &e.path())))
        })
        .map(|e| e.path())
        .collect();
    kids.sort();
    for k in kids {
        if depth + 1 < 2 {
            push_shards(out, root, &k, explicit, base, exd, depth + 1, sub);
        } else {
            out.push(shard(root, &k, explicit, base, exd, true));
        }
    }
}

// ---------------------------------------------------------------- 后台任务

pub enum Msg {
    Batch(Vec<SearchResult>),
    Done(End),
}

#[derive(Debug, Default, Clone)]
pub struct End {
    pub capped: bool,
    pub cancelled: bool,
    pub error: Option<String>,
}

/// 按分片顺序搜, 每片完成就 emit 一批; 取消标志在片与片之间查; 总数到 CAP 即停
pub fn run_job(root: &Path, o: &GrepOpts, cancel: &AtomicBool, emit: &mut dyn FnMut(Msg)) {
    let shards = match plan(root, o) {
        Ok(p) => p,
        Err(e) => return emit(Msg::Done(End { error: Some(e), ..Default::default() })),
    };
    let mut total = 0usize;
    for (r, mut so) in shards {
        if cancel.load(Ordering::Relaxed) {
            return emit(Msg::Done(End { cancelled: true, ..Default::default() }));
        }
        so.max_results = Some(CAP - total);
        match grep_engine::search(&r, &so) {
            Ok(v) => {
                total += v.len();
                if !v.is_empty() {
                    emit(Msg::Batch(v));
                }
                if total >= CAP {
                    return emit(Msg::Done(End { capped: true, ..Default::default() }));
                }
            }
            Err(e) => return emit(Msg::Done(End { error: Some(e), ..Default::default() })),
        }
    }
    emit(Msg::Done(End::default()));
}

pub struct Job {
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
}

impl Job {
    pub fn start(root: PathBuf, o: GrepOpts) -> Job {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let c2 = cancel.clone();
        std::thread::spawn(move || run_job(&root, &o, &c2, &mut |m| { let _ = tx.send(m); }));
        Job { rx, cancel }
    }
}

/// 只搜当前正文 (含未保存的修改): 一行最多一条, 和引擎的口径一致
pub fn search_text(label: &str, lines: &[String], o: &GrepOpts) -> Result<Vec<SearchResult>, String> {
    if o.pattern.is_empty() {
        return Ok(vec![]);
    }
    let req = SearchReq { op: "hits".into(), needle: o.pattern.clone(), match_case: o.case, use_regex: o.regex, whole_word: o.word, lines: lines.to_vec(), ..Default::default() };
    let res = search_ops::run(&req).unwrap_or_default();
    if let Some(e) = res.error {
        return Err(e);
    }
    let mut out: Vec<SearchResult> = Vec::new();
    for h in res.hits {
        if out.last().map(|r| r.line == h.row as u64 + 1).unwrap_or(false) {
            continue;
        }
        out.push(SearchResult { path: label.to_string(), line: h.row as u64 + 1, text: lines[h.row].clone() });
        if out.len() >= CAP {
            break;
        }
    }
    Ok(out)
}

/// 每条命中行里第一处命中的列 (1 起); 找不到 = 1
pub fn first_cols(texts: &[String], o: &GrepOpts) -> Vec<usize> {
    let mut cols = vec![1usize; texts.len()];
    if o.pattern.is_empty() || texts.is_empty() {
        return cols;
    }
    let req = SearchReq { op: "hits".into(), needle: o.pattern.clone(), match_case: o.case, use_regex: o.regex, whole_word: o.word, lines: texts.to_vec(), ..Default::default() };
    let mut seen = vec![false; texts.len()];
    for h in search_ops::run(&req).unwrap_or_default().hits {
        if h.row < cols.len() && !seen[h.row] {
            seen[h.row] = true;
            cols[h.row] = h.col + 1;
        }
    }
    cols
}

// ---------------------------------------------------------------- 结果文本

/// Sakura 风格的头
pub fn header(o: &GrepOpts, root: &Path) -> Vec<String> {
    let mut v = vec![
        if o.pattern.is_empty() { "□検索条件  (ファイル名のみ)".to_string() } else { format!("□検索条件  \"{}\"", o.pattern) },
        format!("検索対象   {}", if o.files.trim().is_empty() { "*.*" } else { o.files.trim() }),
        format!("フォルダー  {}", if o.this_file { "(現在のファイル)".to_string() } else if o.folders.trim().is_empty() { root.display().to_string() } else { o.folders.trim().to_string() }),
    ];
    let mut flags = String::new();
    if o.sub { flags += "(サブフォルダーも検索)"; }
    if o.case { flags += "(英大文字小文字を区別)"; }
    if o.regex { flags += "(正規表現)"; }
    if o.word { flags += "(単語単位)"; }
    v.push(format!("    {flags}"));
    v.push(String::new());
    v
}

/// 命中行 -> 结果文本行。Normal 逐条 `路径(行,列): 文字`; Files / Count 要看全部结果, 流式阶段不用
pub fn normal_lines(results: &[SearchResult], o: &GrepOpts) -> Vec<String> {
    let texts: Vec<String> = results.iter().map(|r| r.text.clone()).collect();
    let cols = first_cols(&texts, o);
    results.iter().zip(cols).map(|(r, c)| if r.line == 0 { r.path.clone() } else { format!("{}({},{}): {}", r.path, r.line, c, r.text) }).collect()
}

/// Files / Count 格式的整份正文 (Normal 返回空)
pub fn summary_lines(results: &[SearchResult], fmt: Fmt) -> Vec<String> {
    let mut order: Vec<(String, usize)> = Vec::new();
    for r in results {
        match order.iter_mut().find(|(p, _)| *p == r.path) {
            Some(e) => e.1 += 1,
            None => order.push((r.path.clone(), 1)),
        }
    }
    match fmt {
        Fmt::Normal => vec![],
        Fmt::Files => order.into_iter().map(|(p, _)| p).collect(),
        Fmt::Count => order.into_iter().map(|(p, n)| format!("{p}: {n}")).collect(),
    }
}

pub fn footer(n: usize, end: &End) -> Vec<String> {
    let mut s = format!("{n} 個が見つかりました。");
    if end.capped { s += &format!(" (上限 {CAP} 件で打ち切り)"); }
    if end.cancelled { s += " (中断しました)"; }
    if let Some(e) = &end.error { s += &format!(" エラー: {e}"); }
    vec![String::new(), s]
}

/// 结果文档里的一行 -> (文件, 行, 列)。`路径(行,列): 文字`, 或只有一个存在的文件路径 (文件名格式)
pub fn parse_result_line(line: &str) -> Option<(PathBuf, usize, usize)> {
    for (i, _) in line.match_indices('(') {
        if i == 0 {
            continue;
        }
        let rest = &line[i + 1..];
        let Some((a, rest)) = rest.split_once(',') else { continue };
        let Some((b, tail)) = rest.split_once(')') else { continue };
        if let (Ok(l), Ok(c)) = (a.parse::<usize>(), b.parse::<usize>()) {
            if tail.starts_with(':') {
                return Some((PathBuf::from(&line[..i]), l, c));
            }
        }
    }
    let p = Path::new(line.trim());
    if !line.trim().is_empty() && p.is_file() { Some((p.to_path_buf(), 1, 1)) } else { None }
}

// ---------------------------------------------------------------- 置换

/// 全部置换 (改写磁盘文件, 备份 .bak, 已打开且有未保存修改的文件跳过)。返回 (说明, 实际写了的文件)
pub fn replace(root: &Path, o: &GrepOpts, dirty: Vec<String>) -> (String, Vec<String>) {
    if o.pattern.is_empty() {
        return ("请输入要搜索的内容".into(), vec![]);
    }
    let mut search = build_opts(o);
    search.max_results = None;
    let ro = ReplaceOptions { search, replacement: o.repl.clone(), backup: Some(true), skip_paths: Some(dirty), ..Default::default() };
    match grep_engine::grep_replace(root, &ro) {
        Ok(r) => {
            let written: Vec<String> = r.files.iter().filter(|f| f.written).map(|f| f.path.clone()).collect();
            let msg = format!("置换 {} 处, 改写 {} 个文件 (已备份为 .bak){}{}", r.total_replacements, r.files_changed, if r.files_skipped > 0 { format!(", 跳过 {} 个有未保存修改的已打开文件", r.files_skipped) } else { String::new() }, if r.files_failed > 0 { format!(", {} 个失败", r.files_failed) } else { String::new() });
            (msg, written)
        }
        Err(e) => (e, vec![]),
    }
}

/// 路径比较: 不分大小写 / 分隔符
pub fn same_path(a: &str, b: &str) -> bool {
    let n = |s: &str| s.replace('\\', "/").to_lowercase();
    n(a) == n(b)
}

// ---------------------------------------------------------------- 状态与控件

pub struct Grep {
    pub o: GrepOpts,
    pub results: Vec<SearchResult>,
    pub msg: String,
    /// 本次任务的条件快照 (列的计算 / 头 / 结果文档用) 与搜索根
    pub last: Option<(GrepOpts, PathBuf)>,
    /// 结果里已经交给结果文档的条数
    pub rendered: usize,
    /// 刚结束的任务的收尾信息 (结果文档取走后清掉)
    pub finished: Option<End>,
    /// 上一次任务的收尾信息 (一直保留; 结果文档晚开时用它补尾)
    pub end: Option<End>,
    job: Option<Job>,
    /// 下一帧把焦点放到搜索框 / 置换框
    pub focus_pattern: bool,
}

/// 侧栏控件向外报告的事情
pub enum Ev {
    Start,
    Stop,
    ReplaceAll,
    ResultDoc,
    UseCurDir,
    Open { path: String, line: usize, text: String },
}

impl Grep {
    pub fn new() -> Self {
        Grep { o: GrepOpts::default(), results: vec![], msg: String::new(), last: None, rendered: 0, finished: None, end: None, job: None, focus_pattern: false }
    }

    pub fn running(&self) -> bool {
        self.job.is_some()
    }

    /// 开始搜索: 上一次没跑完的先取消
    pub fn start(&mut self, root: PathBuf) {
        self.cancel();
        self.results.clear();
        self.rendered = 0;
        self.finished = None;
        self.end = None;
        self.last = Some((self.o.clone(), root.clone()));
        self.msg = "搜索中…  (Esc 停止)".into();
        self.job = Some(Job::start(root, self.o.clone()));
    }

    /// 同步完成的搜索 (只搜当前文件)
    pub fn start_text(&mut self, root: PathBuf, label: &str, lines: &[String]) {
        self.cancel();
        self.results.clear();
        self.rendered = 0;
        self.last = Some((self.o.clone(), root));
        match search_text(label, lines, &self.o) {
            Ok(v) => {
                self.msg = format!("{} 处命中", v.len());
                self.results = v;
                self.end = Some(End::default());
                self.finished = self.end.clone();
            }
            Err(e) => {
                self.msg = e.clone();
                self.end = Some(End { error: Some(e), ..Default::default() });
                self.finished = self.end.clone();
            }
        }
    }

    pub fn cancel(&mut self) {
        if let Some(j) = self.job.take() {
            j.cancel.store(true, Ordering::Relaxed);
            let e = End { cancelled: true, ..Default::default() };
            self.end = Some(e.clone());
            self.finished = Some(e);
        }
    }

    /// 每帧调用: 取后台送来的结果; 返回 true = 有新内容
    pub fn poll(&mut self) -> bool {
        let Some(job) = &self.job else { return false };
        let mut changed = false;
        loop {
            match job.rx.try_recv() {
                Ok(Msg::Batch(v)) => {
                    self.results.extend(v);
                    self.msg = format!("搜索中… {} 处  (Esc 停止)", self.results.len());
                    changed = true;
                }
                Ok(Msg::Done(end)) => {
                    self.msg = match (&end.error, end.cancelled, end.capped) {
                        (Some(e), _, _) => e.clone(),
                        (_, true, _) => format!("已停止, {} 处命中", self.results.len()),
                        (_, _, true) => format!("{} 处命中 (已达 {CAP} 上限)", self.results.len()),
                        _ => format!("{} 处命中", self.results.len()),
                    };
                    self.end = Some(end.clone());
                    self.finished = Some(end);
                    self.job = None;
                    return true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return changed,
                Err(_) => {
                    self.job = None;
                    return true;
                }
            }
        }
    }

    /// 侧栏的 Grep 页。`cur_dir` 是当前文件所在目录 (「当前文件夹」按钮用), `root` 是项目根 (显示相对路径用)
    pub fn panel(&mut self, ui: &mut egui::Ui, root: &Path, has_cur_file: bool) -> Vec<Ev> {
        use egui::Key;
        let mut ev = Vec::new();
        let w = f32::INFINITY;
        let r = ui.add(egui::TextEdit::singleline(&mut self.o.pattern).hint_text(crate::i18n::trz("搜索内容 (空 = 只按文件名找)")).desired_width(w));
        if self.focus_pattern {
            r.request_focus();
            self.focus_pattern = false;
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            ev.push(Ev::Start);
        }
        ui.add(egui::TextEdit::singleline(&mut self.o.files).hint_text(crate::i18n::trz("文件名: *.rs;*.toml  排除 !*.gen.rs  排除目录 #target")).desired_width(w));
        egui::CollapsingHeader::new(crate::i18n::trz("文件夹 / 排除 / 字符集")).default_open(true).show(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.o.folders).hint_text(crate::i18n::trz("文件夹 (留空 = 整个项目, 多个用 ; 分隔)")).desired_width(w));
            if ui.add_enabled(has_cur_file, egui::Button::new(crate::i18n::trz("当前文件所在文件夹")).small()).clicked() {
                ev.push(Ev::UseCurDir);
            }
            ui.add(egui::TextEdit::singleline(&mut self.o.exc_files).hint_text(crate::i18n::trz("排除文件, 如 *.min.js")).desired_width(w));
            ui.add(egui::TextEdit::singleline(&mut self.o.exc_folders).hint_text(crate::i18n::trz("排除文件夹, 如 target;dist")).desired_width(w));
            ui.horizontal(|ui| {
                ui.label(crate::i18n::trz("字符集"));
                let shown = if self.o.charset.is_empty() { crate::i18n::trz("自动") } else { self.o.charset.as_str() }.to_string();
                egui::ComboBox::from_id_salt("grep-charset").selected_text(shown).show_ui(ui, |ui| {
                    for (v, name) in [("", crate::i18n::trz("自动")), ("utf-8", "utf-8"), ("sjis", "sjis"), ("euc-jp", "euc-jp"), ("gbk", "gbk"), ("big5", "big5"), ("utf-16le", "utf-16le"), ("utf-16be", "utf-16be")] {
                        ui.selectable_value(&mut self.o.charset, v.to_string(), name);
                    }
                });
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.o.case, crate::i18n::trz("区分大小写"));
            ui.checkbox(&mut self.o.word, crate::i18n::trz("单词单位"));
            ui.checkbox(&mut self.o.regex, crate::i18n::trz("正则"));
            ui.checkbox(&mut self.o.sub, crate::i18n::trz("子文件夹"));
            ui.checkbox(&mut self.o.outside, crate::i18n::trz("项目外"));
            ui.checkbox(&mut self.o.this_file, crate::i18n::trz("仅当前文件"));
        });
        ui.horizontal(|ui| {
            ui.label(crate::i18n::trz("输出"));
            egui::ComboBox::from_id_salt("grep-fmt").selected_text(match self.o.fmt { Fmt::Normal => crate::i18n::trz("标准"), Fmt::Files => crate::i18n::trz("仅文件名"), Fmt::Count => crate::i18n::trz("各文件命中数") }).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.o.fmt, Fmt::Normal, crate::i18n::trz("标准"));
                ui.selectable_value(&mut self.o.fmt, Fmt::Files, crate::i18n::trz("仅文件名"));
                ui.selectable_value(&mut self.o.fmt, Fmt::Count, crate::i18n::trz("各文件命中数"));
            });
        });
        ui.horizontal(|ui| {
            if self.running() {
                if ui.button(crate::i18n::trz("停止  Esc")).clicked() { ev.push(Ev::Stop); }
            } else if ui.button(crate::i18n::trz("搜索")).clicked() {
                ev.push(Ev::Start);
            }
            if ui.button(crate::i18n::trz("结果开成标签")).on_hover_text(crate::i18n::trz("把结果放进一个只读标签: 可复制、可 Ctrl+F, Enter / 双击命中行跳转")).clicked() {
                ev.push(Ev::ResultDoc);
            }
        });
        ui.add(egui::TextEdit::singleline(&mut self.o.repl).hint_text(crate::i18n::trz("置换为 (正则下可用 $1)")).desired_width(w));
        if ui.button(crate::i18n::trz("全部置换 (改写文件, 保留 .bak)")).clicked() {
            ev.push(Ev::ReplaceAll);
        }
        ui.label(crate::msgtr::msg(&self.msg).as_ref());
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
        egui::ScrollArea::both().auto_shrink([false, false]).show_rows(ui, row_h, self.results.len(), |ui, range| {
            for i in range {
                let h = &self.results[i];
                let rel = Path::new(&h.path).strip_prefix(root).map(|p| p.display().to_string()).unwrap_or_else(|_| h.path.clone());
                let t: String = h.text.trim().chars().take(80).collect();
                let label = if h.line == 0 { rel } else { format!("{rel}:{}  {t}", h.line) };
                if ui.selectable_label(false, label).clicked() {
                    ev.push(Ev::Open { path: h.path.clone(), line: h.line.max(1) as usize, text: h.text.clone() });
                }
            }
        });
        ev
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ide-fast-grep2-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn collect(root: &Path, o: &GrepOpts) -> (Vec<SearchResult>, End, usize) {
        let mut all = vec![];
        let mut batches = 0;
        let mut end = End::default();
        run_job(root, o, &AtomicBool::new(false), &mut |m| match m {
            Msg::Batch(v) => { batches += 1; all.extend(v); }
            Msg::Done(e) => end = e,
        });
        (all, end, batches)
    }

    #[test]
    fn build_opts_splits_the_five_fields() {
        let o = GrepOpts { pattern: "x".into(), files: "*.rs;*.toml !gen.rs #target".into(), folders: "src;crates/a".into(), exc_files: "*.min.js".into(), exc_folders: "dist".into(), charset: "sjis".into(), word: true, sub: false, ..Default::default() };
        let s = build_opts(&o);
        assert_eq!(s.include_files, Some(vec!["*.rs".into(), "*.toml".into()]));
        assert_eq!(s.exclude_files, Some(vec!["*.min.js".into(), "gen.rs".into()]));
        assert_eq!(s.exclude_folders, Some(vec!["dist".into(), "target".into()]));
        assert_eq!(s.search_folders, Some(vec!["src".into(), "crates/a".into()]));
        assert_eq!((s.whole_word, s.subfolders, s.charset.as_deref()), (Some(true), Some(false), Some("sjis")));
        assert_eq!(build_opts(&GrepOpts::default()).include_files, None);
    }

    #[test]
    fn shards_stream_results_and_skip_nested_build_dirs() {
        let d = tmp("shards");
        for p in ["top.txt", "a/x.txt", "a/b/y.txt", "a/b/c/z.txt", "target/t.txt", "a/target/deep.txt", "b/w.txt", ".hid/h.txt"] {
            let f = d.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, "needle\n").unwrap();
        }
        let o = GrepOpts { pattern: "needle".into(), ..Default::default() };
        let (r, end, batches) = collect(&d, &o);
        let names: Vec<String> = r.iter().map(|x| rel_of(&d, Path::new(&x.path))).collect();
        assert!(batches >= 4, "按分片一批一批送: {batches}");
        for want in ["top.txt", "a/x.txt", "a/b/y.txt", "a/b/c/z.txt", "b/w.txt"] {
            assert!(names.iter().any(|n| n == want), "漏了 {want}: {names:?}");
        }
        assert!(!names.iter().any(|n| n.contains("target") || n.contains(".hid")), "嵌套的 target 不能被搜: {names:?}");
        assert_eq!(names.len(), 5, "{names:?}");
        assert!(!end.capped && !end.cancelled && end.error.is_none());
        // 和不分片的整体搜索结果一致
        let whole = grep_engine::search(&d, &build_opts(&o)).unwrap();
        assert_eq!(whole.len(), r.len());
        // 不含子文件夹: 只有顶层
        let (r2, _, _) = collect(&d, &GrepOpts { sub: false, ..o.clone() });
        assert_eq!(r2.len(), 1);
        // 点名文件夹 = 保持"点名要搜就搜": 搜 target 本身要搜得到
        let (r3, _, _) = collect(&d, &GrepOpts { folders: "target".into(), ..o.clone() });
        assert_eq!(r3.len(), 1);
        // 排除目录对分片同样有效 (含带 / 的写法)
        let (r4, _, _) = collect(&d, &GrepOpts { exc_folders: "a/b".into(), ..o.clone() });
        let n4: Vec<String> = r4.iter().map(|x| rel_of(&d, Path::new(&x.path))).collect();
        assert!(!n4.iter().any(|n| n.starts_with("a/b")) && n4.iter().any(|n| n == "a/x.txt"), "{n4:?}");
        let (r5, _, _) = collect(&d, &GrepOpts { exc_folders: "b".into(), ..o.clone() });
        assert!(!r5.iter().any(|x| x.path.ends_with("w.txt")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn empty_pattern_lists_files_and_cancel_and_cap_stop_the_job() {
        let d = tmp("cap");
        for i in 0..3 {
            std::fs::create_dir_all(d.join(format!("d{i}"))).unwrap();
            std::fs::write(d.join(format!("d{i}/f.rs")), "x\n").unwrap();
        }
        let (r, _, _) = collect(&d, &GrepOpts { files: "*.rs".into(), ..Default::default() });
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|x| x.line == 0), "空条件 = 文件名搜索");
        // 取消: 第一批之后置位, 后面的分片不再跑
        let cancel = AtomicBool::new(false);
        let mut got = 0;
        let mut end = End::default();
        run_job(&d, &GrepOpts { files: "*.rs".into(), ..Default::default() }, &cancel, &mut |m| match m {
            Msg::Batch(v) => { got += v.len(); cancel.store(true, Ordering::Relaxed); }
            Msg::Done(e) => end = e,
        });
        assert!(end.cancelled && got < 3, "got {got} {end:?}");
        // 上限: 一个文件里命中超过 CAP
        let big = "a\n".repeat(CAP + 50);
        std::fs::write(d.join("big.txt"), big).unwrap();
        let (r, end, _) = collect(&d, &GrepOpts { pattern: "a".into(), files: "big.txt".into(), ..Default::default() });
        assert_eq!(r.len(), CAP);
        assert!(end.capped);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn bad_folders_are_reported() {
        let d = tmp("bad");
        let (_, end, _) = collect(&d, &GrepOpts { pattern: "x".into(), folders: "../../nowhere".into(), ..Default::default() });
        assert!(end.error.is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn result_text_columns_and_jump_parsing() {
        let o = GrepOpts { pattern: "bar".into(), ..Default::default() };
        let r = vec![SearchResult { path: "C:\\p\\a.rs".into(), line: 3, text: "foo bar".into() }, SearchResult { path: "C:\\p\\b(1).rs".into(), line: 9, text: "x(1,2): bar".into() }];
        let lines = normal_lines(&r, &o);
        assert_eq!(lines[0], "C:\\p\\a.rs(3,5): foo bar");
        assert_eq!(parse_result_line(&lines[0]), Some((PathBuf::from("C:\\p\\a.rs"), 3, 5)));
        let (p, l, c) = parse_result_line(&lines[1]).unwrap();
        assert_eq!((p, l, c), (PathBuf::from("C:\\p\\b(1).rs"), 9, 9), "路径里的括号 / 文字里的 (1,2): 不干扰");
        assert_eq!(parse_result_line("□検索条件  \"bar\""), None);
        assert_eq!(parse_result_line(""), None);
        let h = header(&o, Path::new("C:\\p"));
        assert!(h[0].contains("\"bar\"") && h[2].contains("C:\\p") && h[3].contains("サブフォルダー"));
        let f = footer(2, &End { capped: true, ..Default::default() });
        assert!(f[1].starts_with("2 個が見つかりました") && f[1].contains("5000"));
        assert_eq!(summary_lines(&r, Fmt::Count), vec!["C:\\p\\a.rs: 1", "C:\\p\\b(1).rs: 1"]);
        assert_eq!(summary_lines(&r, Fmt::Files).len(), 2);
        // 文件名格式: 一个存在的文件路径也能跳
        let d = tmp("parse");
        let f = d.join("t.txt");
        std::fs::write(&f, "x").unwrap();
        assert_eq!(parse_result_line(&f.to_string_lossy()), Some((f.clone(), 1, 1)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn this_file_search_uses_the_given_text() {
        let lines: Vec<String> = ["alpha", "beta alpha alpha", "gamma"].iter().map(|s| s.to_string()).collect();
        let o = GrepOpts { pattern: "alpha".into(), ..Default::default() };
        let r = search_text("t.txt", &lines, &o).unwrap();
        assert_eq!(r.iter().map(|x| x.line).collect::<Vec<_>>(), vec![1, 2], "一行一条");
        assert!(search_text("t", &lines, &GrepOpts { pattern: "(".into(), regex: true, ..Default::default() }).is_err());
    }

    #[test]
    fn replace_reports_written_files_and_skips_dirty() {
        let d = tmp("repl");
        std::fs::write(d.join("a.txt"), "old\n").unwrap();
        std::fs::write(d.join("b.txt"), "old\n").unwrap();
        let o = GrepOpts { pattern: "old".into(), repl: "new".into(), ..Default::default() };
        let skip = d.join("b.txt").to_string_lossy().into_owned();
        let (msg, written) = replace(&d, &o, vec![skip]);
        assert!(msg.contains("跳过 1 个"), "{msg}");
        assert_eq!(written.len(), 1);
        assert!(same_path(&written[0], &d.join("A.TXT").to_string_lossy()));
        assert_eq!(std::fs::read_to_string(d.join("b.txt")).unwrap(), "old\n");
        assert!(replace(&d, &GrepOpts::default(), vec![]).0.contains("请输入"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn grep_poll_streams_then_finishes() {
        let d = tmp("poll");
        std::fs::write(d.join("a.rs"), "fn main() {}\nlet needle = 1;\n").unwrap();
        let mut g = Grep::new();
        g.o.pattern = "needle".into();
        g.start(d.clone());
        for _ in 0..200 {
            g.poll();
            if !g.running() { break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!((g.results.len(), g.results[0].line), (1, 2), "{}", g.msg);
        assert!(g.finished.is_some());
        let _ = std::fs::remove_dir_all(&d);
    }
}
