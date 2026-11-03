//! 联网只借系统自带的 curl: 路径解析和不弹控制台窗口的命令构造, AI 对话和 GitHub 插件共用。
//! 优先用 `%SystemRoot%\System32\curl.exe` (不从当前目录 / PATH 里随便找); 找不到才退回 `curl`。

use std::path::PathBuf;
use std::process::Command;

pub fn curl_path() -> PathBuf {
    std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32").join("curl.exe")).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("curl"))
}

/// curl 命令; Windows 下 GUI 进程里跑要加 CREATE_NO_WINDOW, 否则会弹控制台
pub fn curl_command() -> Command {
    let mut cmd = Command::new(curl_path());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}
