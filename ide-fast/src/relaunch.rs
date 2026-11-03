//! 「立即重启」: 窗口是否透明在创建窗口时就定死了 (WGL 像素格式要带合成标志, winit 在 Windows 上运行中改 `set_transparent` 只改标志、
//! 不重做 DWM 的 blur-behind, 所以运行中切换半透明主题做不到), 所以换到半透明主题时给「立即重启」: 正常走关闭流程
//! (有未保存的修改照常先问), 关完后由 `main` 启动一个新进程, 带上现在打开的文件。
//! 新进程带 `--relaunch`: 旧进程可能还没完全退出 (单实例端口文件还指着它), 新进程不去转交文件, 直接当第一个实例。

use std::sync::Mutex;

/// Some(现在打开的文件) = 关窗口后要重启
static REQUEST: Mutex<Option<Vec<String>>> = Mutex::new(None);

/// 命令行里的这个开关 = 这是「立即重启」拉起来的新进程
pub const FLAG: &str = "--relaunch";

pub fn request(paths: Vec<String>) {
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(paths);
}
pub fn cancel() {
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = None;
}
pub fn requested() -> bool {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}
/// `main` 在窗口关完之后取走请求
pub fn take() -> Option<Vec<String>> {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// 窗口是透明的 (建窗口时定) 与主题要不要透明对不上, 而且主题要的是透明 = 需要重启才能真正生效。
/// 反过来 (窗口透明、主题不透明) 只是颜色不透, 不用重启。
pub fn needs_restart(window_glass: bool, theme_glass: bool) -> bool {
    theme_glass && !window_glass
}

/// 新进程的参数: 先是 `--relaunch`, 再是文件。开着「恢复上次会话」时文件由会话恢复负责, 不重复传 (否则同一文件开两次)
pub fn args(paths: &[String], restore_session: bool) -> Vec<String> {
    let mut v = vec![FLAG.to_string()];
    if !restore_session {
        v.extend(paths.iter().filter(|p| !p.is_empty()).take(20).cloned());
    }
    v
}

/// 去掉命令行里的 `--relaunch`, 告诉调用方有没有
pub fn take_flag(args: Vec<String>) -> (bool, Vec<String>) {
    let on = args.iter().any(|a| a == FLAG);
    (on, args.into_iter().filter(|a| a != FLAG).collect())
}

/// 启动新进程 (不等它)。失败就算了: 旧进程已经退出, 用户手动再开一次
pub fn spawn(paths: &[String], restore_session: bool) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe).args(args(paths, restore_session)).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_glass_theme_in_an_opaque_window_needs_a_restart() {
        assert!(needs_restart(false, true));
        assert!(!needs_restart(true, true));
        assert!(!needs_restart(true, false), "窗口已透明, 主题不透明: 只是不透, 不用重启");
        assert!(!needs_restart(false, false));
    }

    #[test]
    fn restart_args_carry_the_open_files_unless_the_session_restores_them() {
        let p = vec!["C:\\a.txt".to_string(), String::new(), "D:\\b.md".to_string()];
        assert_eq!(args(&p, false), vec!["--relaunch", "C:\\a.txt", "D:\\b.md"]);
        assert_eq!(args(&p, true), vec!["--relaunch"]);
    }

    #[test]
    fn the_flag_is_taken_out_of_the_file_args() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(take_flag(v(&["--relaunch", "a"])), (true, v(&["a"])));
        assert_eq!(take_flag(v(&["a"])), (false, v(&["a"])));
    }

    #[test]
    fn request_flag_roundtrips() {
        cancel();
        assert!(!requested());
        request(vec!["a".into()]);
        assert!(requested());
        assert_eq!(take(), Some(vec!["a".to_string()]));
        assert!(!requested(), "取走后清空");
    }
}
