fn main() {
    // 强制 dist 改动重 build: tauri-build 2.6.3 默认不 emit dist 的 rerun-if-changed.
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=dist/index.html");
    tauri_build::build()
}
