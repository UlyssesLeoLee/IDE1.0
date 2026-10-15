// build.rs — IDE1.0 Tauri 桌面应用构建脚本
//
// 前端单一来源: crates/ide-shell-web/src/editor.html
// 桌面版与网页版共用同一份前端, 避免功能分叉.
// 历史上 desktop/dist/index.html 长期落后于 web 版, 缺 15 项能力:
// 语法高亮 / 多重光标 / 列块选择 / 自动补全 / 行操作 / 剪贴板历史 /
// 编码转换 / 时间戳 / 查找替换 / 书签 / 宏 / 分屏 / 缩略图 / 折叠 / Extensions.
//
// 构建时把 editor.html 复制到 dist/index.html, 使 tauri 的 frontendDist
// 指向的目录始终与 web 版一致.
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let web_editor = manifest
        .parent()
        .expect("crate 根目录")
        .join("ide-shell-web")
        .join("src")
        .join("editor.html");
    let dist = manifest.join("dist");
    let dist_index = dist.join("index.html");

    // 源文件缺失时保留已有 dist (允许先构建 Rust 再放前端), 但要显式警告
    if !web_editor.exists() {
        println!(
            "cargo:warning=找不到共享前端 {} — 保留现有 dist/index.html",
            web_editor.display()
        );
    } else {
        let src_text = std::fs::read_to_string(&web_editor)
            .unwrap_or_else(|e| panic!("读取 {} 失败: {e}", web_editor.display()));
        let need_write = match std::fs::read_to_string(&dist_index) {
            Ok(existing) => existing != src_text,
            Err(_) => true,
        };
        if need_write {
            std::fs::create_dir_all(&dist)
                .unwrap_or_else(|e| panic!("创建 {} 失败: {e}", dist.display()));
            std::fs::write(&dist_index, &src_text)
                .unwrap_or_else(|e| panic!("写入 {} 失败: {e}", dist_index.display()));
            println!(
                "cargo:warning=已同步共享前端 -> dist/index.html ({} bytes)",
                src_text.len()
            );
        }
    }

    // tauri-build 2.6.3 不 emit 前端资源的 rerun-if-changed, 手动补上,
    // 否则改了 editor.html 不会触发重新打包 (IDE1.0 曾因此打包过期的 UI).
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=dist/index.html");
    println!("cargo:rerun-if-changed={}", rel(&web_editor, &manifest));

    tauri_build::build()
}

/// 把绝对路径转成相对 crate 根的形式, 让 rerun-if-changed 更可读
fn rel(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
