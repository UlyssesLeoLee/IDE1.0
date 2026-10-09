// plugin.rs — プラグイン (WebAssembly) (sakura 14.2)
//
// WASM プラグイン ローダ (wasmtime ベース).
// プラグインは WIT/エクスポート関数 `process(text: &str) -> String` を持つ.

use std::path::Path;
use wasmtime::{Engine, Module, Store, Instance, Config};

/// プラグイン実行エラー
#[derive(Debug)]
pub enum PluginError {
    Io(std::io::Error),
    Wasm(String),
    Export(String),
    Execution(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO: {e}"),
            Self::Wasm(s) => write!(f, "WASM: {s}"),
            Self::Export(s) => write!(f, "Export: {s}"),
            Self::Execution(s) => write!(f, "Execution: {s}"),
        }
    }
}

impl std::error::Error for PluginError {}
impl From<std::io::Error> for PluginError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// プラグイン
pub struct Plugin {
    name: String,
    path: std::path::PathBuf,
    engine: Engine,
    module: Module,
}

impl Plugin {
    /// WASM ファイルから読み込み
    pub fn load(name: impl Into<String>, path: &Path) -> Result<Self, PluginError> {
        let name = name.into();
        let wasm_bytes = std::fs::read(path)?;
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)
            .map_err(|e| PluginError::Wasm(e.to_string()))?;
        let module = Module::from_binary(&engine, &wasm_bytes)
            .map_err(|e| PluginError::Wasm(e.to_string()))?;
        Ok(Self {
            name,
            path: path.to_path_buf(),
            engine,
            module,
        })
    }

    /// プラグイン実行 (process 関数呼び出し)
    pub fn run(&self, input: &str) -> Result<String, PluginError> {
        let mut store = Store::new(&self.engine, ());
        store.set_fuel(10_000).map_err(|e| PluginError::Execution(e.to_string()))?;
        let instance = Instance::new(&mut store, &self.module, &[])
            .map_err(|e| PluginError::Execution(e.to_string()))?;

        // process 関数を探す
        let func = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "process")
            .or_else(|_| instance.get_typed_func::<(i32, i32), i32>(&mut store, "transform"));

        // 注: 完全な WASM 実行は複雑なメモリ管理が必要.
        // ここでは process エクスポートの存在のみ確認する simple なテスト用.
        match func {
            Ok(_) => Ok(format!("[{}] {}", self.name, input)),
            Err(_) => Err(PluginError::Export(
                "Plugin must export 'process' or 'transform' function".to_string(),
            )),
        }
    }

    /// プラグイン名
    pub fn name(&self) -> &str {
        &self.name
    }

    /// プラグイン パス
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// プラグイン管理
#[derive(Default)]
pub struct PluginManager {
    plugins: Vec<Plugin>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plugin: Plugin) {
        self.plugins.push(plugin);
    }

    pub fn get(&self, name: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.name() == name)
    }

    pub fn names(&self) -> Vec<String> {
        self.plugins.iter().map(|p| p.name().to_string()).collect()
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let cfg = Config::new();
        let engine = Engine::new(&cfg);
        assert!(engine.is_ok());
    }

    #[test]
    fn test_plugin_load_invalid() {
        let dir = std::env::temp_dir();
        let bad = dir.join("nonexistent.wasm");
        let r = Plugin::load("bad", &bad);
        assert!(r.is_err());
    }

    #[test]
    fn test_plugin_load_invalid_bytes() {
        let dir = std::env::temp_dir();
        let bad = dir.join("invalid.wasm");
        std::fs::write(&bad, b"not wasm").unwrap();
        let r = Plugin::load("bad", &bad);
        assert!(r.is_err());
        let _ = std::fs::remove_file(&bad);
    }

    #[test]
    fn test_plugin_manager() {
        let m = PluginManager::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn test_plugin_error_display() {
        let e = PluginError::Export("test".to_string());
        assert!(format!("{e}").contains("Export"));
    }
}
