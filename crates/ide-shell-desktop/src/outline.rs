//! Outline service — read file + parse with sakura-rs TypeRegistry (desktop).

#![allow(dead_code)]

use sakura_rs::type_config::{OutlineEntry, TypeRegistry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineResult {
    pub path: String,
    pub language: String,
    pub entries: Vec<OutlineEntry>,
    pub error: Option<String>,
}

pub fn outline_file(path: &str, content: &str) -> OutlineResult {
    let registry = TypeRegistry::default();
    let lang = match registry.detect(path) {
        Some(t) => t,
        None => {
            return OutlineResult {
                path: path.to_string(),
                language: "Plain".to_string(),
                entries: Vec::new(),
                error: None,
            };
        }
    };
    let entries = sakura_rs::type_config::TypeConfig::outline_dispatch(&lang.name, content);
    OutlineResult {
        path: path.to_string(),
        language: lang.name.to_string(),
        entries,
        error: None,
    }
}

// ============================================================
// Tauri command wrapper
// ============================================================
use tauri::State;
use std::path::PathBuf;

#[tauri::command]
pub fn outline(
    state: State<'_, crate::ProjectRoot>,
    path: String,
) -> Result<OutlineResult, String> {
    let arc: &std::sync::Arc<std::sync::Mutex<Option<PathBuf>>> = &state.0;
    let guard = arc.lock().map_err(|e| e.to_string())?;
    let project = guard.clone().ok_or("project root not set")?;
    drop(guard);
    let full = if std::path::Path::new(&path).is_absolute() {
        PathBuf::from(&path)
    } else {
        project.join(&path)
    };
    match std::fs::read_to_string(&full) {
        Ok(content) => Ok(outline_file(&full.to_string_lossy(), &content)),
        Err(e) => Err(format!("read failed: {}", e)),
    }
}
