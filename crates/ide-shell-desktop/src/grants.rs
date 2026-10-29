//! 原生「打开 / 另存为」对话框授予的文件访问 (WBS-02 1.2)。
//!
//! 桌面版平时把文件读写限制在项目根内 (沙箱)。用户在**系统对话框**里亲手选定的文件是明确的授权: 记进来, 这些**具体文件**
//! (不是整个目录) 之后可以按绝对路径读 / 写 / 取文件指纹, 其余项目外路径照旧被拒绝。授权只存在进程内, 重启后要重新选。
//! 网页版没有系统对话框, 沙箱不变。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
pub struct Grants(Mutex<HashSet<String>>);

/// 路径归一: 规范化 (`..` / 符号链接)、去掉 `\?\` 前缀、Windows 上不分大小写
fn key_of(p: &Path) -> Option<(String, PathBuf)> {
    let c = crate::safe_canonicalize(p).ok()?;
    let real = crate::strip_verbatim(&c);
    let k = real.to_string_lossy().replace('/', "\\");
    Some((if cfg!(windows) { k.to_lowercase() } else { k }, real))
}

impl Grants {
    /// 授权一个具体文件; 返回规范化后的路径 (给前端显示 / 之后使用)
    pub fn grant(&self, p: &Path) -> Option<PathBuf> {
        let (k, real) = key_of(p)?;
        self.0.lock().ok()?.insert(k);
        Some(real)
    }
    /// 这个路径 (任何写法) 是不是被授权过; 是 = 返回规范化后的真实路径
    pub fn granted(&self, path: &str) -> Option<PathBuf> {
        let (k, real) = key_of(Path::new(path))?;
        self.0.lock().ok()?.contains(&k).then_some(real)
    }
}

/// 系统「打开」对话框 (可多选)。返回用户选定的文件的绝对路径; 取消 = 空数组。选定的文件同时被授权。
#[tauri::command]
pub fn native_open_file(grants: tauri::State<'_, Grants>) -> Result<Vec<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<Vec<PathBuf>>>();
    std::thread::spawn(move || {
        let _ = tx.send(rfd::FileDialog::new().set_title("打开文件").pick_files());
    });
    let picked = rx.recv_timeout(std::time::Duration::from_secs(3600)).ok().flatten().unwrap_or_default();
    Ok(picked.iter().filter_map(|p| grants.grant(p)).map(|p| p.to_string_lossy().into_owned()).collect())
}

/// 系统「另存为」对话框。返回用户选定的目标路径 (文件可以还不存在); 取消 = None。选定的路径同时被授权。
#[tauri::command]
pub fn native_save_as(grants: tauri::State<'_, Grants>, default_name: Option<String>) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<PathBuf>>();
    std::thread::spawn(move || {
        let mut d = rfd::FileDialog::new().set_title("另存为");
        if let Some(n) = default_name.filter(|n| !n.is_empty()) {
            d = d.set_file_name(n);
        }
        let _ = tx.send(d.save_file());
    });
    let picked = rx.recv_timeout(std::time::Duration::from_secs(3600)).ok().flatten();
    Ok(picked.and_then(|p| grants.grant(&p)).map(|p| p.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ide-grants-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn only_the_exact_granted_file_is_allowed_under_any_spelling() {
        let d = tmpdir("exact");
        let (a, b) = (d.join("a.txt"), d.join("b.txt"));
        std::fs::write(&a, "x").unwrap();
        std::fs::write(&b, "y").unwrap();
        let g = Grants::default();
        assert!(g.granted(&a.to_string_lossy()).is_none(), "没授权前不放行");
        g.grant(&a).unwrap();
        assert!(g.granted(&a.to_string_lossy()).is_some());
        // 换一种写法 (带 .. / 正斜杠) 还是同一个文件
        let sneaky = format!("{}/sub/../a.txt", d.to_string_lossy().replace('\\', "/"));
        std::fs::create_dir_all(d.join("sub")).unwrap();
        assert!(g.granted(&sneaky).is_some(), "规范化后同一个文件");
        assert!(g.granted(&b.to_string_lossy()).is_none(), "同目录的别的文件不放行");
        assert!(g.granted(&d.to_string_lossy()).is_none(), "目录本身不放行");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_not_yet_existing_save_target_can_be_granted() {
        let d = tmpdir("new");
        let target = d.join("new.txt");
        let g = Grants::default();
        let real = g.grant(&target).expect("另存为的目标可以还不存在");
        assert!(real.ends_with("new.txt"));
        assert!(g.granted(&target.to_string_lossy()).is_some());
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// 命令行启动参数里的一个文件
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct LaunchFile {
    pub path: String,
    /// 行 / 列, 从 1 起; 0 = 没指定
    pub line: usize,
    pub col: usize,
}

/// 解析命令行 (不含程序名): 文件参数可写 `路径[:行[:列]]`; Sakura 风格开关 `-Y=行` `-X=列` 作为没写位置的文件的默认位置。
/// 不存在的文件忽略 (不替用户新建); 其余开关 (`-CODE=` 等) 目前不认, 直接跳过。
pub fn parse_launch_args(args: &[String]) -> Vec<LaunchFile> {
    let (mut dy, mut dx) = (0usize, 0usize);
    let mut files = Vec::new();
    for a in args {
        if let Some(rest) = a.strip_prefix("-Y=").or_else(|| a.strip_prefix("-y=")) {
            dy = rest.parse().unwrap_or(0);
        } else if let Some(rest) = a.strip_prefix("-X=").or_else(|| a.strip_prefix("-x=")) {
            dx = rest.parse().unwrap_or(0);
        } else if a.starts_with('-') {
            continue;
        } else {
            let (p, line, col) = editcore::text_util::split_path_line_col(a);
            // 整个参数本身就是存在的文件 (文件名里恰好带冒号数字) 时按原样
            let (path, line, col) = if std::path::Path::new(a).is_file() { (a.clone(), 0, 0) } else { (p, line, col) };
            if std::path::Path::new(&path).is_file() {
                let (line, col) = if line == 0 && dy > 0 { (dy, dx.max(1)) } else { (line, col) };
                files.push(LaunchFile { path, line, col });
            }
        }
    }
    files
}

/// 启动时命令行里带的文件: 用户亲手在命令行 / 文件关联里指定的, 视同授权 (项目外也能打开)
#[tauri::command]
pub fn launch_files(grants: tauri::State<'_, Grants>) -> Vec<LaunchFile> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    parse_launch_args(&args)
        .into_iter()
        .filter_map(|f| grants.grant(std::path::Path::new(&f.path)).map(|p| LaunchFile { path: p.to_string_lossy().into_owned(), ..f }))
        .collect()
}

#[cfg(test)]
mod launch_tests {
    use super::*;

    #[test]
    fn parses_files_positions_and_switches() {
        let d = std::env::temp_dir().join(format!("ide-launch-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("a.txt");
        std::fs::write(&f, "x").unwrap();
        let fs = f.to_string_lossy().into_owned();
        let args = vec!["-CODE=4".to_string(), format!("{fs}:3:2"), "-Y=9".to_string(), "-X=4".to_string(), fs.clone(), d.join("missing.txt").to_string_lossy().into_owned()];
        let r = parse_launch_args(&args);
        assert_eq!(r.len(), 2, "不存在的文件被忽略: {r:?}");
        assert_eq!((r[0].line, r[0].col), (3, 2), "路径里写的位置优先");
        assert_eq!((r[1].line, r[1].col), (9, 4), "-Y= / -X= 是没写位置的文件的默认位置");
        let _ = std::fs::remove_dir_all(&d);
    }
}
