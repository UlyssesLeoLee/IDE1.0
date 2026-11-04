//! 自更新装完后新进程带的 `--relaunch` 开关 (`updapply` 启动新 exe 时加上): 旧进程可能还没完全退出 (单实例端口文件还指着它),
//! 新进程带这个开关就不去转交文件, 直接当第一个实例。
//! (以前这里还有「立即重启」: 半透明主题要重启才能透明; 现在窗口一律透明, 玻璃主题运行中切换, 见 `app/sysopts.rs::glass_sync`)

/// 命令行里的这个开关 = 这是「安装并重启」拉起来的新进程
pub const FLAG: &str = "--relaunch";

/// 去掉命令行里的 `--relaunch`, 告诉调用方有没有
pub fn take_flag(args: Vec<String>) -> (bool, Vec<String>) {
    let on = args.iter().any(|a| a == FLAG);
    (on, args.into_iter().filter(|a| a != FLAG).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_is_taken_out_of_the_file_args() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(take_flag(v(&["--relaunch", "a"])), (true, v(&["a"])));
        assert_eq!(take_flag(v(&["a"])), (false, v(&["a"])));
    }
}
