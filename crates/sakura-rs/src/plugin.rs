// plugin.rs — プラグイン (WebAssembly) (sakura 14.2)
//
// WASM プラグイン ローダ (wasmtime ベース).
// プラグインは WIT/エクスポート関数 `process(text: &str) -> String` を持つ.
//
// This module requires the "wasm" feature to be enabled.

use std::path::Path;
#[cfg(feature = "wasm")]
use wasmtime::{Engine, Module, Store, Instance, Config};

/// プラグイン実行エラー
#[derive(Debug)]
pub enum PluginError {
    Io(std::io::Error),
    #[cfg(feature = "wasm")]
    Wasm(String),
    #[cfg(not(feature = "wasm"))]
    WasmUnavailable(String),
    Export(String),
    Execution(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::Io(e) => write!(f, "I/O error: {}", e),
            #[cfg(feature = "wasm")]
            PluginError::Wasm(e) => write!(f, "WASM error: {}", e),
            #[cfg(not(feature = "wasm"))]
            PluginError::WasmUnavailable(e) => write!(f, "WASM unavailable: {}", e),
            PluginError::Export(e) => write!(f, "Export error: {}", e),
            PluginError::Execution(e) => write!(f, "Execution error: {}", e),
        }
    }
}

impl std::error::Error for PluginError {}

#[cfg(feature = "wasm")]
pub fn load_and_run(wasm_path: &Path, input: &str) -> Result<String, PluginError> {
    let engine = Engine::default();
    let module = Module::from_file(&engine, wasm_path)
        .map_err(|e| PluginError::Wasm(format!("load module: {}", e)))?;
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[])
        .map_err(|e| PluginError::Wasm(format!("instantiate: {}", e)))?;
    
    let process_func = instance
        .get_typed_func::<(wasmtime::Val,), (wasmtime::Val,)>(&mut store, "process")
        .map_err(|e| PluginError::Export(format!("get process func: {}", e)))?;
    
    let input_val = wasmtime::Val::from(input.to_string());
    let result = process_func.call(&mut store, (input_val,))
        .map_err(|e| PluginError::Execution(format!("call: {}", e)))?;
    
    Ok(result.to_string())
}

#[cfg(not(feature = "wasm"))]
pub fn load_and_run(wasm_path: &Path, input: &str) -> Result<String, PluginError> {
    Err(PluginError::WasmUnavailable(
        "WASM support not compiled in (enable 'wasm' feature)".into(),
    ))
}


/// 简单的插件管理器 (sakura 14.2 对应)
#[derive(Default)]
pub struct PluginManager {
    plugins: std::collections::HashMap<String, String>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn load(&mut self, name: &str, code: &str) {
        self.plugins.insert(name.to_string(), code.to_string());
    }
}
