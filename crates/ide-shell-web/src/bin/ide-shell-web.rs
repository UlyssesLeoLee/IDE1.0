//! ide-shell-web binary entry.

use std::env;

fn main() -> std::io::Result<()> {
    let addr = env::var("IDE_SHELL_WEB_ADDR").unwrap_or_else(|_| "127.0.0.1:8123".to_string());
    ide_shell_web::serve(&addr)
}
