fn main() {
    #[cfg(windows)]
    {
        let mut res = tauri_winres::WindowsResource::new();
        res.set_icon("packaging/ide-fast.ico");
        res.set("FileDescription", "IDE1.0");
        res.set("ProductName", "IDE1.0");
        res.set("InternalName", "IDE1.0");
        res.set("OriginalFilename", "IDE1.0.exe");
        res.set("LegalCopyright", "Copyright (C) 2026 IDE1.0 Authors");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows PE resource: {e}");
        }
    }
}
