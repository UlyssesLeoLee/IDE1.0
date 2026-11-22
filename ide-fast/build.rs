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
        res.set_manifest(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
</assembly>"#,
        );
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows PE resource: {e}");
        }
    }
}
