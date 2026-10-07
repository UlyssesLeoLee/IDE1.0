// Force rebuild when editor.html changes (cargo:rerun-if-changed)
fn main() {
    println!("cargo:rerun-if-changed=src/editor.html");
    println!("cargo:rerun-if-changed=src/lib.rs");
}
