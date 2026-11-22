//! 统一工作区封装：使 `CoEditWorkspace` 与 `ConcurrentCoEditWorkspace` 共享单一 `DocumentStore` (P1-01)。

use super::store::DocumentStore;
use super::types::{EditRequest, FullCoEditRequest, HeadlessDoc, OpenMode};
use serde_json::Value;
use std::sync::Arc;

/// 并发协同工作区即为单一权威 `DocumentStore`。
pub type ConcurrentCoEditWorkspace = DocumentStore;

/// `CoEditWorkspace` 统一委托至共享 `Arc<DocumentStore>`，彻底消除双份状态副本。
#[derive(Clone, Default)]
pub struct CoEditWorkspace {
    pub store: Arc<DocumentStore>,
}

impl CoEditWorkspace {
    pub fn from_store(store: Arc<DocumentStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<DocumentStore> {
        &self.store
    }

    pub fn get_doc(&self, file_id: &str) -> Option<HeadlessDoc> {
        self.store.get_doc_snapshot_struct(file_id)
    }

    pub fn open_doc(
        &mut self,
        file_id: &str,
        content: &str,
        language: &str,
        provider: &str,
        agent_id: &str,
    ) -> Value {
        self.store
            .open_doc(file_id, content, language, provider, agent_id)
    }

    pub fn open_doc_with_mode(
        &self,
        file_id: &str,
        content: &str,
        language: &str,
        provider: &str,
        agent_id: &str,
        mode: OpenMode,
    ) -> Result<Value, String> {
        self.store
            .open_doc_with_mode(file_id, content, language, provider, agent_id, mode)
    }

    pub fn read_doc(
        &self,
        file_id: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<Value, String> {
        self.store.read_doc(file_id, start_line, end_line)
    }

    pub fn edit_doc(&mut self, req: EditRequest) -> Result<Value, String> {
        self.store.edit_doc(req)
    }

    pub fn coedit_full(&mut self, req: FullCoEditRequest) -> Result<Value, String> {
        self.store.coedit_concurrent(req)
    }

    pub fn diff_revisions(
        &self,
        file_id: &str,
        from_rev: u64,
        to_rev: u64,
        context: usize,
    ) -> Result<Value, String> {
        self.store
            .diff_revisions(file_id, from_rev, to_rev, context)
    }

    pub fn list_docs(&self) -> Value {
        self.store.list_docs()
    }

    pub fn doc_history(&self, file_id: &str) -> Result<Value, String> {
        self.store.doc_history(file_id)
    }
}
