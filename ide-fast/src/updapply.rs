//! 自更新的「安装并重启」: 和「立即重启」(`relaunch.rs`) 同一个套路 —— 界面只登记请求并发关闭命令 (有未保存修改照常先问),
//! 窗口关完、设置落盘之后由 `main` 取走请求并执行:
//! - msi 安装的: 用系统目录里的 `msiexec.exe /i <包> /passive` (绝对路径, 不从 PATH 找), 然后本进程退出;
//! - 便携版: 新 exe 已经校验并写成 `<exe>.new`, 直接运行它并带 `--apply-update`; 它把自己覆盖到旧 exe 上 (旧进程退出前覆盖会失败, 重试约 10 秒) 再启动新版。
//!
//! 不用 .bat / .ps1 / cmd.exe 帮手 (会弹控制台, 违背 GUI 子系统的约定), 帮手就是新版 exe 自己。
//! 安全: `--apply-update` 不接受任何路径参数, 目标路径只由「自己的文件名去掉 `.new`」推出, 所以这个开关不能被用来覆盖任意文件。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// 便携版换文件的帮手开关
pub const APPLY_FLAG: &str = "--apply-update";
/// 新版 exe 暂存的后缀
pub const NEW_SUFFIX: &str = ".new";
/// 覆盖旧 exe 的重试: 次数 x 间隔 = 约 10 秒
const RETRIES: u32 = 20;
const RETRY_GAP: Duration = Duration::from_millis(500);

#[derive(Clone, PartialEq, Debug)]
pub enum Apply {
    /// 装 msi
    Msi(PathBuf),
    /// 运行已校验的 `<exe>.new` 完成替换
    Swap(PathBuf),
}

static REQUEST: Mutex<Option<Apply>> = Mutex::new(None);

pub fn request(a: Apply) {
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(a);
}
pub fn cancel() {
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
pub fn requested() -> bool {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}
/// `main` 在窗口关完之后取走请求
pub fn take() -> Option<Apply> {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// 当前 exe 旁边的暂存路径: `ide-fast.exe` -> `ide-fast.exe.new`
pub fn new_path_of(exe: &Path) -> PathBuf {
    let mut s = exe.as_os_str().to_os_string();
    s.push(NEW_SUFFIX);
    PathBuf::from(s)
}

/// 帮手模式下: 从「自己是 `xxx.exe.new`」推出要覆盖的目标 `xxx.exe`; 文件名不是 `.new` 结尾 = None (拒绝)
pub fn target_of(new_exe: &Path) -> Option<PathBuf> {
    let name = new_exe.file_name()?.to_str()?;
    let base = name.strip_suffix(NEW_SUFFIX).filter(|b| !b.is_empty())?;
    Some(new_exe.with_file_name(base))
}

/// `msiexec` 的参数
pub fn msi_args(file: &Path) -> Vec<String> {
    vec!["/i".into(), file.display().to_string(), "/passive".into()]
}

pub fn msiexec_path() -> PathBuf {
    std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32").join("msiexec.exe")).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("msiexec"))
}

fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// 窗口关完后执行 (不等子进程)。失败由调用方忽略: 旧版文件原样没动, 用户重开即可
pub fn launch(a: &Apply) -> std::io::Result<()> {
    match a {
        Apply::Msi(file) => no_window(&mut std::process::Command::new(msiexec_path()).args(msi_args(file))).spawn().map(|_| ()),
        Apply::Swap(new_exe) => no_window(&mut std::process::Command::new(new_exe).arg(APPLY_FLAG)).spawn().map(|_| ()),
    }
}

/// 把 `src` 覆盖到 `target`: 先拷成同目录的 `.upd`, 再换名 (换名在目标还被旧进程占用时会失败, 由调用方重试)
pub fn swap_once(src: &Path, target: &Path) -> std::io::Result<()> {
    let mut s = target.as_os_str().to_os_string();
    s.push(".upd");
    let tmp = PathBuf::from(s);
    std::fs::copy(src, &tmp)?;
    std::fs::rename(&tmp, target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// 帮手入口 (`main` 在最前面调用): 覆盖旧 exe 并启动新版。无论成败都会尽量把 (新的或原来的) 程序再拉起来, 不让用户落到「应用没了」
pub fn run_apply() {
    let Some(me) = std::env::current_exe().ok() else { return };
    let Some(target) = target_of(&me) else { return };
    for _ in 0..RETRIES {
        if swap_once(&me, &target).is_ok() {
            break;
        }
        std::thread::sleep(RETRY_GAP);
    }
    // 单实例端口文件可能还指着刚退出的旧进程: 带 --relaunch 直接当第一个实例
    let _ = no_window(&mut std::process::Command::new(&target).arg(crate::relaunch::FLAG)).spawn();
}

/// 上次更新留下的 `.new` / `.upd` (正在以 `.new` 身份运行的帮手不会走到这里: 它只跑 `run_apply`)。启动后惰性清一次, 失败无所谓
pub fn cleanup_stale() {
    let Ok(exe) = std::env::current_exe() else { return };
    let new = new_path_of(&exe);
    for p in [new.clone(), PathBuf::from(format!("{}.upd", exe.display()))] {
        if p.is_file() && p != exe {
            let _ = std::fs::remove_file(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_target_paths_are_inverse_and_strict() {
        let exe = Path::new(r"D:\tools\ide-fast.exe");
        let new = new_path_of(exe);
        assert_eq!(new, Path::new(r"D:\tools\ide-fast.exe.new"));
        assert_eq!(target_of(&new).as_deref(), Some(exe));
        for bad in [r"D:\tools\ide-fast.exe", r"D:\tools\.new", "x.new.exe", ""] {
            assert_eq!(target_of(Path::new(bad)), None, "{bad:?}");
        }
    }

    #[test]
    fn msi_command_is_a_passive_install_by_absolute_path() {
        assert_eq!(msi_args(Path::new(r"C:\t\a b.msi")), ["/i", r"C:\t\a b.msi", "/passive"]);
        let p = msiexec_path();
        assert!(p.to_string_lossy().to_ascii_lowercase().ends_with("msiexec.exe") || p == Path::new("msiexec"));
    }

    #[test]
    fn request_roundtrips_and_cancels() {
        cancel();
        assert!(!requested());
        request(Apply::Msi("a.msi".into()));
        assert!(requested());
        cancel();
        assert_eq!(take(), None, "关闭被取消 (用户在未保存提示里点了取消) = 更新请求也作废");
        request(Apply::Swap("x.new".into()));
        assert_eq!(take(), Some(Apply::Swap("x.new".into())));
        assert!(!requested());
    }

    #[test]
    fn swap_replaces_the_target_and_leaves_no_temp() {
        let d = std::env::temp_dir().join(format!("ide-fast-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let (src, target) = (d.join("ide-fast.exe.new"), d.join("ide-fast.exe"));
        std::fs::write(&src, b"new").unwrap();
        std::fs::write(&target, b"old").unwrap();
        swap_once(&src, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        let mut names: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["ide-fast.exe", "ide-fast.exe.new"]);
        assert!(swap_once(&d.join("missing"), &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"new", "失败不动目标");
        let _ = std::fs::remove_dir_all(&d);
    }
}
