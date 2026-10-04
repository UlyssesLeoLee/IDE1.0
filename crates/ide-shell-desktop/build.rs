fn main() {
    println!("cargo:warning=TEST_BUILDRS_BEGIN");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=dist/index.html");
    println!("cargo:warning=TEST_BUILDRS_BEFORE_TAURI_BUILD");
    tauri_build::build();
    println!("cargo:warning=TEST_BUILDRS_AFTER_TAURI_BUILD");
}
