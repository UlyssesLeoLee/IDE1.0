//! 简易 Git 面板: 调系统 `git` 取状态 / 差异 / 暂存。只做日常最小集合 (不内置 git 实现)。
//! 解析是纯函数 (可单测); 真正跑 git 的函数在后台线程里调用, 界面不卡。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// 暂存区状态 (porcelain 第 1 列) / 工作区状态 (第 2 列); 空格 = 无
    pub x: char,
    pub y: char,
    /// 相对仓库根的路径 (斜杠分隔)
    pub path: String,
}

impl Change {
    pub fn staged(&self) -> bool {
        self.x != ' ' && self.x != '?'
    }

    /// 面板里显示的一个字母
    pub fn letter(&self) -> char {
        if self.x == '?' {
            'U'
        } else if self.y != ' ' {
            self.y
        } else {
            self.x
        }
    }
}

/// 解析 `git status --porcelain` (v1) 的输出; 重命名 `R  old -> new` 取新路径
pub fn parse_porcelain(text: &str) -> Vec<Change> {
    text.lines()
        .filter_map(|l| {
            let mut cs = l.chars();
            let (x, y) = (cs.next()?, cs.next()?);
            let rest = l.get(3..)?.trim();
            if rest.is_empty() {
                return None;
            }
            let path = rest
                .rsplit(" -> ")
                .next()
                .unwrap_or(rest)
                .trim_matches('"')
                .to_string();
            Some(Change { x, y, path })
        })
        .collect()
}

#[derive(Default)]
pub struct Status {
    pub branch: String,
    pub changes: Vec<Change>,
    /// 仓库根目录
    pub top: Option<PathBuf>,
    pub error: Option<String>,
}

/// 仓库本地配置里的 filter / diff 驱动会在 add / diff 时执行外部命令: 有就拒绝, 让用户到终端里自己处理
fn local_drivers(cwd: &Path) -> bool {
    let mut c = Command::new("git");
    c.args([
        "config",
        "--includes",
        "--show-scope",
        "--name-only",
        "--get-regexp",
        r"^(filter\.|diff\.external$|diff\..*\.(command|textconv)$)",
    ])
    .current_dir(cwd)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    // 退出码 1 = 没有匹配 (安全); 其余失败一律当作不安全
    match c.output() {
        // 用户自己的 global / system 配置 (如 git-lfs 的 filter) 是可信的, 只看仓库一侧的 scope
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| !(l.starts_with("global") || l.starts_with("system"))),
        Ok(o) => o.status.code() != Some(1),
        Err(_) => true,
    }
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    if local_drivers(cwd) {
        return Err("这个仓库的本地配置定义了 filter / diff 外部命令, 高速版不自动运行 git (请在终端里操作)".into());
    }
    let mut c = Command::new("git");
    c.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1");
    // 状态 / 差异 / 暂存都是纯本地操作: 不许联网、不许弹凭据、不许惰性拉取
    c.env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .env("GIT_ASKPASS", "");
    // 仓库自带的配置不能让只读查询去执行命令
    c.args([
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.hooksPath=NUL",
        "-c",
        "core.sshCommand=false",
        "-c",
        "core.attributesFile=",
        "-c",
        "protocol.allow=never",
        "-c",
        "credential.helper=",
    ])
    .arg("--no-optional-locks")
    .args(args)
    .current_dir(cwd)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let o = c.output().map_err(|e| format!("无法运行 git: {e}"))?;
    if o.status.success() {
        Ok(String::from_utf8_lossy(&o.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

/// 取分支与改动 (同步, 调用方放进线程)
pub fn load_status(cwd: &Path) -> Status {
    let top = match git(cwd, &["rev-parse", "--show-toplevel"]) {
        Ok(t) => PathBuf::from(t.trim()),
        Err(e) => {
            return Status {
                error: Some(if e.is_empty() {
                    "不是 Git 仓库".into()
                } else {
                    e
                }),
                ..Default::default()
            }
        }
    };
    let branch = git(&top, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|b| b.trim().to_string())
        .unwrap_or_default();
    match git(
        &top,
        &[
            "-c",
            "core.quotepath=false",
            "status",
            "--porcelain",
            "--ignore-submodules=all",
        ],
    ) {
        Ok(t) => Status {
            branch,
            changes: parse_porcelain(&t),
            top: Some(top),
            error: None,
        },
        Err(e) => Status {
            branch,
            top: Some(top),
            error: Some(e),
            ..Default::default()
        },
    }
}

/// 某个文件的统一差异 (`staged` = 暂存区对 HEAD; 否则工作区对暂存区); 未跟踪文件没有差异可看
pub fn file_diff(top: &Path, path: &str, staged: bool) -> Result<String, String> {
    let mut args = vec![
        "-c",
        "core.quotepath=false",
        "diff",
        "--ignore-submodules=all",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
    ];
    if staged {
        args.push("--cached");
    }
    args.extend(["--", path]);
    git(top, &args)
}

pub fn stage(top: &Path, path: &str) -> Result<(), String> {
    git(top, &["add", "--", path]).map(|_| ())
}

pub fn unstage(top: &Path, path: &str) -> Result<(), String> {
    git(top, &["reset", "-q", "HEAD", "--", path]).map(|_| ())
}

/// 面板状态: 后台加载 + 当前结果
pub struct Scm {
    pub status: Status,
    rx: Option<Receiver<Status>>,
    pub loaded: bool,
}

impl Scm {
    pub fn new() -> Self {
        Scm {
            status: Status::default(),
            rx: None,
            loaded: false,
        }
    }

    pub fn loading(&self) -> bool {
        self.rx.is_some()
    }

    /// 在后台刷新 (已经在刷新就忽略)
    pub fn refresh(&mut self, cwd: PathBuf) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(load_status(&cwd));
        });
        self.rx = Some(rx);
    }

    /// 每帧调用: 取到结果返回 true
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok(s) => {
                self.status = s;
                self.loaded = true;
                self.rx = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(_) => {
                self.rx = None;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_lines_parse_into_changes() {
        let c = parse_porcelain(
            " M src/a.rs\nA  new.txt\n?? junk.log\nR  old.md -> docs/new.md\nMM both.rs\n",
        );
        assert_eq!(c.len(), 5);
        assert_eq!((c[0].x, c[0].y, c[0].path.as_str()), (' ', 'M', "src/a.rs"));
        assert!(c[1].staged() && !c[0].staged() && !c[2].staged());
        assert_eq!(c[2].letter(), 'U');
        assert_eq!(c[3].path, "docs/new.md", "重命名取新路径");
        assert_eq!(c[4].letter(), 'M');
        assert!(parse_porcelain("").is_empty());
    }

    #[test]
    fn real_repo_status_diff_and_stage_roundtrip() {
        if Command::new("git").arg("--version").output().is_err() {
            return; // 没装 git 就跳过
        }
        let d = std::env::temp_dir().join(format!("ide-fast-scm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let r = git(&d, &["init", "-q"]);
        assert!(r.is_ok(), "{r:?}");
        let cfg = ["-c", "user.name=t", "-c", "user.email=t@t"];
        std::fs::write(d.join("a.txt"), "one\n").unwrap();
        git(&d, &["add", "a.txt"]).unwrap();
        let mut args = cfg.to_vec();
        args.extend(["commit", "-q", "-m", "init"]);
        git(&d, &args).unwrap();
        std::fs::write(d.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(d.join("b.txt"), "new\n").unwrap();
        let s = load_status(&d);
        assert!(s.error.is_none(), "{:?}", s.error);
        assert!(s.changes.iter().any(|c| c.path == "a.txt" && c.y == 'M'));
        assert!(s
            .changes
            .iter()
            .any(|c| c.path == "b.txt" && c.letter() == 'U'));
        let top = s.top.unwrap();
        assert!(file_diff(&top, "a.txt", false).unwrap().contains("+two"));
        stage(&top, "a.txt").unwrap();
        assert!(file_diff(&top, "a.txt", true).unwrap().contains("+two"));
        unstage(&top, "a.txt").unwrap();
        assert!(file_diff(&top, "a.txt", true).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&d);
        // 非仓库目录给出错误而不是 panic
        let n = std::env::temp_dir().join(format!("ide-fast-norepo-{}", std::process::id()));
        std::fs::create_dir_all(&n).unwrap();
        let s = load_status(&n);
        assert!(s.error.is_some() || s.top.is_some());
        let _ = std::fs::remove_dir_all(&n);
    }
}
