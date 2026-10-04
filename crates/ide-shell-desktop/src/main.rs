//! ide-shell-desktop — Tauri 2 桌面应用 entry.
//!
//! CLI 参数:
//! - `--help` / `-h`    打印完整内置 wiki (与 lib.rs APP_WIKI 同源) 后退出
//! - `--version` / `-V` 打印版本后退出
//! - 无参数              启动 GUI
//!
//! 注: Windows release 为 GUI 子系统 (windows_subsystem = "windows"), stdout 无
//! 控制台附着; `--help | more` / `--help > help.txt` 重定向时仍可见输出。
//! debug 构建带控制台, 直接可见。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", ide_shell_desktop::help_wiki_text());
        return;
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!(
            "ide-shell-desktop {} (IDE1.0 IDE Shell)",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    ide_shell_desktop::run();
}
