//! 单一权威协同文档存储 (`DocumentStore`) — P0-01/04/05 与 P1-01/02/03 核心引擎。

use super::algo::{check_syntax_integrity, find_single_hunk, three_way_merge, ConflictHunk};
use super::hash::content_hash;
use super::nema::nema_merge;
use super::types::*;
use crate::diff::{diff_text, split_lines, unified};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

static PROPOSAL_SEQ: AtomicU64 = AtomicU64::new(1);
static EVENT_SEQ: AtomicU64 = AtomicU64::new(1);

pub(super) fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn infer_mode(file_id: &str, mode: Option<CollabMode>) -> CollabMode {
    mode.unwrap_or_else(|| {
        if file_id.ends_with(".nema") {
            CollabMode::Nema
        } else {
            CollabMode::Git
        }
    })
}

fn compute_full_payload_hash(req: &FullCoEditRequest, mode: CollabMode) -> String {
    let raw = format!(
        "coedit|{}|{}|{:?}|{}|{:?}|{}",
        req.file_id,
        req.author,
        req.base_revision,
        req.base_content_hash.as_deref().unwrap_or(""),
        mode,
        req.new_text
    );
    content_hash(&raw)
}

fn check_idempotency_in_doc(
    doc: &HeadlessDoc,
    op_id: Option<&str>,
    payload_hash: &str,
) -> Result<Option<Value>, String> {
    let Some(op) = op_id else {
        return Ok(None);
    };
    if let Some(entry) = doc.idempotency.get(op) {
        if entry.payload_hash != payload_hash {
            return Err(format!(
                "IDEMPOTENCY_PAYLOAD_MISMATCH: op_id '{op}' already committed on '{}' with different payload",
                doc.file_id
            ));
        }
        let mut c = entry.response.clone();
        c["idempotent_replay"] = json!(true);
        return Ok(Some(c));
    }
    Ok(None)
}

fn record_idempotency_in_doc(
    doc: &mut HeadlessDoc,
    op_id: Option<String>,
    payload_hash: String,
    res: &Value,
) {
    let Some(op) = op_id else { return };
    if doc.idempotency.len() >= MAX_IDEMPOTENCY_KEYS && !doc.idempotency.contains_key(&op) {
        if let Some(oldest) = doc.idempotency_order.pop_front() {
            doc.idempotency.remove(&oldest);
        }
    }
    doc.idempotency_order.push_back(op.clone());
    doc.idempotency.insert(
        op,
        IdempotencyEntry {
            payload_hash,
            response: res.clone(),
        },
    );
}

fn validate_cas_preconditions(
    doc: &HeadlessDoc,
    base_revision: Option<u64>,
    base_content_hash: Option<&str>,
    fencing_token: Option<u64>,
    now_ms: u64,
) -> Result<(u64, String), String> {
    let base_rev = base_revision.unwrap_or(doc.current_revision);
    let base_snapshot = doc
        .revisions
        .iter()
        .find(|r| r.revision == base_rev)
        .ok_or_else(|| {
            format!(
                "STALE_OR_MISSING_BASE_REVISION: Base revision {base_rev} is not available (retained: {}..={})",
                doc.revisions.front().map(|r| r.revision).unwrap_or(0),
                doc.current_revision
            )
        })?;

    if let Some(expected_hash) = base_content_hash {
        if base_snapshot.content_hash != expected_hash {
            return Err(format!(
                "BASE_HASH_MISMATCH: expected hash '{expected_hash}' at rev {base_rev}, got '{}'",
                base_snapshot.content_hash
            ));
        }
    }

    if let Some(tok) = fencing_token {
        let valid = match &doc.lease {
            Some(lease) => lease.fencing_token == tok && now_ms <= lease.expires_at_epoch_ms,
            None => false,
        };
        if !valid {
            return Err(format!(
                "FENCING_TOKEN_STALE: token {tok} is expired or preempted on '{}'",
                doc.file_id
            ));
        }
    } else if let Some(lease) = &doc.lease {
        if now_ms > lease.expires_at_epoch_ms && lease.fencing_token > 0 {
            // 租约过期不阻断未持旧令牌的常规协作
        }
    }

    Ok((base_rev, base_snapshot.content.clone()))
}

fn check_head_hash_consistency(
    doc: &HeadlessDoc,
    expected_head_rev: u64,
    expected_head_hash: &str,
) -> bool {
    doc.current_revision == expected_head_rev && doc.content_hash == expected_head_hash
}

struct CommitMeta<'a> {
    auto_merged: bool,
    cas_attempts: usize,
    strategy: &'a str,
    payload_hash: String,
}

fn make_revision_record(
    new_rev: u64,
    prev_content: &str,
    new_content: &str,
    author: &str,
    auto_merged: bool,
    mode: CollabMode,
    op_id: Option<String>,
) -> RevisionRecord {
    let cur_lines = split_lines(prev_content);
    let nxt_lines = split_lines(new_content);
    let (ta0, ta1, to0, to1) = find_single_hunk(&cur_lines, &nxt_lines);
    let t_delta = (to1 - to0) as isize - (ta1 - ta0) as isize;
    RevisionRecord {
        revision: new_rev,
        content: new_content.to_string(),
        content_hash: content_hash(new_content),
        provider: author.to_string(),
        agent_id: author.to_string(),
        summary: format!("coedit mode={mode:?} auto_merged={auto_merged}"),
        auto_merged,
        created_at: format!("ms:{}", now_epoch_ms()),
        touched_range: (ta0, ta1),
        line_delta: t_delta,
        op_id,
    }
}

fn make_rev_for_full(
    new_rev: u64,
    doc: &HeadlessDoc,
    new_content: &str,
    req: &FullCoEditRequest,
    meta: &CommitMeta<'_>,
) -> RevisionRecord {
    make_revision_record(
        new_rev,
        &doc.content,
        new_content,
        &req.author,
        meta.auto_merged,
        infer_mode(&doc.file_id, req.mode),
        req.op_id.clone(),
    )
}

fn run_three_way(
    mode: CollabMode,
    language: &str,
    base_text: &str,
    head_text: &str,
    incoming_text: &str,
) -> Result<(String, bool, Vec<ConflictHunk>), String> {
    let outcome = match mode {
        CollabMode::Git => three_way_merge(base_text, head_text, incoming_text),
        CollabMode::Nema => nema_merge(base_text, head_text, incoming_text)?,
    };
    if !outcome.clean {
        return Ok((outcome.merged_text, false, outcome.conflicts));
    }
    let syntax_lang = if mode == CollabMode::Nema {
        "nema"
    } else {
        language
    };
    if let Err(err) = check_syntax_integrity(syntax_lang, &outcome.merged_text) {
        return Ok((
            outcome.merged_text,
            false,
            vec![ConflictHunk {
                base_line_start: 1,
                base_line_end: 1,
                ours: "HEAD syntax".into(),
                theirs: format!("Post-merge syntax violation: {err}"),
            }],
        ));
    }
    Ok((outcome.merged_text, true, Vec::new()))
}

pub struct DocumentStore {
    docs: RwLock<HashMap<String, Arc<RwLock<HeadlessDoc>>>>,
    proposals: RwLock<HashMap<String, ConflictProposal>>,
    outbox: Mutex<VecDeque<OutboxEvent>>,
}

impl Default for DocumentStore {
    fn default() -> Self {
        let store = Self {
            docs: RwLock::new(HashMap::new()),
            proposals: RwLock::new(HashMap::new()),
            outbox: Mutex::new(VecDeque::with_capacity(MAX_OUTBOX_EVENTS)),
        };
        let default_math = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n";
        let _ = store.open_doc_with_mode(
            "default:src/math.rs",
            default_math,
            "rust",
            "system",
            "init",
            OpenMode::CreateIfAbsent,
        );
        store
    }
}

fn evaluate_candidate(
    mode: CollabMode,
    language: &str,
    base_rev: u64,
    head_rev: u64,
    base_content: &str,
    head_content: &str,
    new_text: &str,
) -> Result<(String, bool, Vec<ConflictHunk>), String> {
    if base_rev == head_rev {
        let syntax_lang = if mode == CollabMode::Nema {
            "nema"
        } else {
            language
        };
        match check_syntax_integrity(syntax_lang, new_text) {
            Ok(()) => Ok((new_text.to_string(), true, Vec::new())),
            Err(err) => Ok((
                new_text.to_string(),
                false,
                vec![ConflictHunk {
                    base_line_start: 1,
                    base_line_end: 1,
                    ours: head_content.to_string(),
                    theirs: format!("Syntax violation: {err}"),
                }],
            )),
        }
    } else {
        run_three_way(mode, language, base_content, head_content, new_text)
    }
}

impl DocumentStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_doc_ref(&self, file_id: &str) -> Result<Arc<RwLock<HeadlessDoc>>, String> {
        let docs = self.docs.read().unwrap();
        docs.get(file_id)
            .cloned()
            .ok_or_else(|| format!("Document not found: {file_id}"))
    }

    pub fn get_doc_snapshot_struct(&self, file_id: &str) -> Option<HeadlessDoc> {
        let lock = self.get_doc_ref(file_id).ok()?;
        let doc = lock.read().unwrap().clone();
        Some(doc)
    }

    fn handle_existing_doc_open(
        existing_lock: &Arc<RwLock<HeadlessDoc>>,
        content: &str,
        provider: &str,
        agent_id: &str,
        mode: OpenMode,
    ) -> Value {
        match mode {
            OpenMode::CreateIfAbsent | OpenMode::OpenExisting => {
                let doc = existing_lock.read().unwrap();
                json!({
                    "ok": true,
                    "status": "ALREADY_EXISTS",
                    "created": false,
                    "file_id": doc.file_id,
                    "language": doc.language,
                    "current_revision": doc.current_revision,
                    "content_hash": doc.content_hash,
                    "content": doc.content
                })
            }
            OpenMode::ForceOverwrite => {
                let mut doc = existing_lock.write().unwrap();
                let new_rev = doc.current_revision + 1;
                let new_hash = content_hash(content);
                let rev_rec = RevisionRecord {
                    revision: new_rev,
                    content: content.to_string(),
                    content_hash: new_hash.clone(),
                    provider: provider.to_string(),
                    agent_id: agent_id.to_string(),
                    summary: "Explicit force_overwrite".into(),
                    auto_merged: false,
                    created_at: format!("ms:{}", now_epoch_ms()),
                    touched_range: (0, split_lines(&doc.content).len()),
                    line_delta: 0,
                    op_id: None,
                };
                if doc.revisions.len() >= MAX_REVISIONS_PER_DOC {
                    doc.revisions.pop_front();
                }
                doc.revisions.push_back(rev_rec);
                doc.current_revision = new_rev;
                doc.content = content.to_string();
                doc.content_hash = new_hash.clone();
                json!({
                    "ok": true,
                    "status": "OVERWRITTEN",
                    "created": false,
                    "file_id": doc.file_id,
                    "language": doc.language,
                    "current_revision": new_rev,
                    "content_hash": new_hash
                })
            }
        }
    }

    /// 打开或登记文件：已存在时绝不覆盖内容或重置版本号，除非显式 `OpenMode::ForceOverwrite` (P0-04)。
    pub fn open_doc_with_mode(
        &self,
        file_id: &str,
        content: &str,
        language: &str,
        provider: &str,
        agent_id: &str,
        mode: OpenMode,
    ) -> Result<Value, String> {
        let mut docs = self.docs.write().unwrap();
        if let Some(existing_lock) = docs.get(file_id).cloned() {
            drop(docs);
            return Ok(Self::handle_existing_doc_open(
                &existing_lock,
                content,
                provider,
                agent_id,
                mode,
            ));
        }

        if mode == OpenMode::OpenExisting {
            return Err(format!("Document not found: {file_id}"));
        }

        if docs.len() >= MAX_DOCS {
            if let Some(oldest) = docs.keys().next().cloned() {
                docs.remove(&oldest);
            }
        }

        let hash = content_hash(content);
        let rev1 = RevisionRecord {
            revision: 1,
            content: content.to_string(),
            content_hash: hash.clone(),
            provider: provider.to_string(),
            agent_id: agent_id.to_string(),
            summary: "Initial open".into(),
            auto_merged: false,
            created_at: format!("ms:{}", now_epoch_ms()),
            touched_range: (0, 0),
            line_delta: 0,
            op_id: Some("init".into()),
        };
        let mut revisions = VecDeque::with_capacity(MAX_REVISIONS_PER_DOC);
        revisions.push_back(rev1);

        let doc = HeadlessDoc {
            file_id: file_id.to_string(),
            language: language.to_string(),
            current_revision: 1,
            content: content.to_string(),
            content_hash: hash.clone(),
            revisions,
            idempotency: HashMap::new(),
            idempotency_order: VecDeque::new(),
            lease: None,
            next_fencing_token: 0,
            disk_path: None,
            last_disk_hash: None,
            last_synced_revision: None,
        };
        docs.insert(file_id.to_string(), Arc::new(RwLock::new(doc)));

        Ok(json!({
            "ok": true,
            "status": "CREATED",
            "created": true,
            "file_id": file_id,
            "language": language,
            "current_revision": 1,
            "content_hash": hash,
            "content": content
        }))
    }

    pub fn open_doc(
        &self,
        file_id: &str,
        content: &str,
        language: &str,
        provider: &str,
        agent_id: &str,
    ) -> Value {
        self.open_doc_with_mode(
            file_id,
            content,
            language,
            provider,
            agent_id,
            OpenMode::CreateIfAbsent,
        )
        .unwrap_or_else(|e| json!({ "ok": false, "error": e }))
    }

    pub fn read_doc(
        &self,
        file_id: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<Value, String> {
        let doc_lock = self.get_doc_ref(file_id)?;
        let doc = doc_lock.read().unwrap();
        let lines = split_lines(&doc.content);
        let s = start_line.unwrap_or(1).saturating_sub(1).min(lines.len());
        let e = end_line.unwrap_or(lines.len()).min(lines.len());
        let slice: String = if s < e {
            lines[s..e].iter().copied().collect()
        } else {
            String::new()
        };
        Ok(json!({
            "file_id": doc.file_id,
            "language": doc.language,
            "current_revision": doc.current_revision,
            "content_hash": doc.content_hash,
            "total_lines": lines.len(),
            "start_line": s + 1,
            "end_line": e,
            "content": slice,
            "disk_path": doc.disk_path,
            "last_disk_hash": doc.last_disk_hash,
            "dirty_in_memory": doc.last_synced_revision.is_some_and(|r| r != doc.current_revision)
        }))
    }

    pub fn snapshot_doc(&self, file_id: &str) -> Result<(u64, String), String> {
        let doc_lock = self.get_doc_ref(file_id)?;
        let doc = doc_lock.read().unwrap();
        Ok((doc.current_revision, doc.content.clone()))
    }

    pub fn snapshot_doc_with_hash(&self, file_id: &str) -> Result<(u64, String, String), String> {
        let doc_lock = self.get_doc_ref(file_id)?;
        let doc = doc_lock.read().unwrap();
        Ok((
            doc.current_revision,
            doc.content.clone(),
            doc.content_hash.clone(),
        ))
    }

    pub fn list_docs(&self) -> Value {
        let docs = self.docs.read().unwrap();
        let list: Vec<Value> = docs
            .values()
            .map(|dl| {
                let d = dl.read().unwrap();
                json!({
                    "file_id": d.file_id,
                    "language": d.language,
                    "current_revision": d.current_revision,
                    "content_hash": d.content_hash,
                    "revisions_count": d.revisions.len(),
                    "disk_path": d.disk_path
                })
            })
            .collect();
        json!({ "docs": list })
    }

    pub fn doc_history(&self, file_id: &str) -> Result<Value, String> {
        let doc_lock = self.get_doc_ref(file_id)?;
        let doc = doc_lock.read().unwrap();
        let history: Vec<Value> = doc
            .revisions
            .iter()
            .map(|r| {
                json!({
                    "revision": r.revision,
                    "content_hash": r.content_hash,
                    "provider": r.provider,
                    "agent_id": r.agent_id,
                    "summary": r.summary,
                    "auto_merged": r.auto_merged,
                    "created_at": r.created_at,
                    "op_id": r.op_id
                })
            })
            .collect();
        Ok(json!({
            "file_id": file_id,
            "current_revision": doc.current_revision,
            "content_hash": doc.content_hash,
            "history": history
        }))
    }

    pub fn diff_revisions(
        &self,
        file_id: &str,
        from_rev: u64,
        to_rev: u64,
        context: usize,
    ) -> Result<Value, String> {
        let doc_lock = self.get_doc_ref(file_id)?;
        let doc = doc_lock.read().unwrap();
        let r1 = doc
            .revisions
            .iter()
            .find(|r| r.revision == from_rev)
            .ok_or_else(|| format!("Revision {from_rev} not found"))?;
        let r2 = doc
            .revisions
            .iter()
            .find(|r| r.revision == to_rev)
            .ok_or_else(|| format!("Revision {to_rev} not found"))?;

        let d = diff_text(&r1.content, &r2.content, context);
        let u = unified(&d, &format!("rev_{from_rev}"), &format!("rev_{to_rev}"));
        Ok(json!({
            "file_id": file_id,
            "from_revision": from_rev,
            "to_revision": to_rev,
            "added": d.added,
            "removed": d.removed,
            "identical": d.identical,
            "coarse": d.coarse,
            "unified_diff": u
        }))
    }

    /// 申请或续期文档单调递增 Fencing Token 租约 (P0-05)。
    pub fn acquire_lease(
        &self,
        file_id: &str,
        agent_id: &str,
        ttl_ms: u64,
    ) -> Result<Value, String> {
        let doc_lock = self.get_doc_ref(file_id)?;
        let mut doc = doc_lock.write().unwrap();
        doc.next_fencing_token += 1;
        let token = doc.next_fencing_token;
        let expires = now_epoch_ms() + ttl_ms.max(100);
        doc.lease = Some(LeaseRecord {
            agent_id: agent_id.to_string(),
            fencing_token: token,
            expires_at_epoch_ms: expires,
        });
        Ok(json!({
            "ok": true,
            "file_id": file_id,
            "agent_id": agent_id,
            "fencing_token": token,
            "expires_at_epoch_ms": expires,
            "head_revision": doc.current_revision,
            "head_content_hash": doc.content_hash
        }))
    }

    pub fn has_pending_proposals(&self, file_id: &str) -> bool {
        let props = self.proposals.read().unwrap();
        props
            .values()
            .any(|p| p.file_id == file_id && p.status == "PENDING_REVIEW")
    }

    pub fn list_proposals(&self, file_id: Option<&str>) -> Value {
        let props = self.proposals.read().unwrap();
        let list: Vec<&ConflictProposal> = props
            .values()
            .filter(|p| file_id.map_or(true, |f| p.file_id == f))
            .collect();
        json!({ "proposals": list })
    }

    /// 人工或 Review Agent 解决冲突提案并按当前基线安全提交 (P0-01)。
    pub fn resolve_proposal(
        &self,
        proposal_id: &str,
        resolved_content: &str,
        base_revision: u64,
        agent_id: &str,
        op_id: &str,
    ) -> Result<Value, String> {
        let prop = {
            let props = self.proposals.read().unwrap();
            props
                .get(proposal_id)
                .cloned()
                .ok_or_else(|| format!("Proposal not found: {proposal_id}"))?
        };
        if prop.status != "PENDING_REVIEW" {
            return Err(format!(
                "Proposal '{proposal_id}' is already {}",
                prop.status
            ));
        }

        let res = self.coedit_concurrent(FullCoEditRequest {
            file_id: prop.file_id.clone(),
            author: agent_id.to_string(),
            new_text: resolved_content.to_string(),
            base_revision: Some(base_revision),
            base_content_hash: None,
            fencing_token: None,
            op_id: Some(op_id.to_string()),
            mode: None,
        })?;

        if res["ok"] == true {
            let mut props = self.proposals.write().unwrap();
            if let Some(p) = props.get_mut(proposal_id) {
                p.status = "RESOLVED".into();
            }
        }
        Ok(res)
    }

    pub fn outbox_events(&self) -> Vec<OutboxEvent> {
        self.outbox.lock().unwrap().iter().cloned().collect()
    }

    fn push_outbox(&self, event_type: &str, file_id: &str, revision: u64, payload: &Value) {
        let mut outbox = self.outbox.lock().unwrap();
        if outbox.len() >= MAX_OUTBOX_EVENTS {
            outbox.pop_front();
        }
        outbox.push_back(OutboxEvent {
            event_id: EVENT_SEQ.fetch_add(1, Ordering::Relaxed),
            event_type: event_type.into(),
            file_id: file_id.into(),
            revision,
            payload: payload.clone(),
            created_at: format!("ms:{}", now_epoch_ms()),
        });
    }

    fn create_conflict_proposal_and_reply(
        &self,
        doc: &mut HeadlessDoc,
        req: &FullCoEditRequest,
        base_rev: u64,
        conflict_preview: String,
        conflicts: Vec<ConflictHunk>,
        payload_hash: String,
    ) -> Value {
        let pid = format!(
            "prop_{}_{}",
            now_epoch_ms(),
            PROPOSAL_SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let d = diff_text(&doc.content, &req.new_text, 3);
        let cdiff = unified(
            &d,
            &format!("head_rev_{}", doc.current_revision),
            &format!("proposal_{pid}"),
        );
        let proposal = ConflictProposal {
            proposal_id: pid.clone(),
            file_id: doc.file_id.clone(),
            base_revision: base_rev,
            head_revision: doc.current_revision,
            head_content_hash: doc.content_hash.clone(),
            agent_id: req.author.clone(),
            op_id: req.op_id.clone(),
            proposed_content: req.new_text.clone(),
            proposed_hash: content_hash(&req.new_text),
            conflict_preview: conflict_preview.clone(),
            conflict_diff: cdiff.clone(),
            conflicts: conflicts.clone(),
            status: "PENDING_REVIEW".into(),
            created_at: format!("ms:{}", now_epoch_ms()),
        };

        {
            let mut props = self.proposals.write().unwrap();
            if props.len() >= MAX_PROPOSALS {
                if let Some(first_key) = props.keys().next().cloned() {
                    props.remove(&first_key);
                }
            }
            props.insert(pid.clone(), proposal);
        }

        let mode = infer_mode(&doc.file_id, req.mode);
        let res = json!({
            "ok": false,
            "status": "CONFLICT",
            "proposal_id": pid,
            "file_id": doc.file_id,
            "base_revision": base_rev,
            "head_revision": doc.current_revision,
            "revision": doc.current_revision,
            "head_content_hash": doc.content_hash,
            "content_hash": doc.content_hash,
            "merged": false,
            "auto_merged": false,
            "conflicts": conflicts.len(),
            "conflict_hunks": conflicts,
            "conflict_diff": cdiff,
            "conflict_preview": conflict_preview,
            "author": req.author,
            "mode": mode
        });
        record_idempotency_in_doc(doc, req.op_id.clone(), payload_hash, &res);
        self.push_outbox(
            "CONFLICT_PROPOSED",
            &doc.file_id,
            doc.current_revision,
            &res,
        );
        res
    }

    fn apply_clean_commit(
        &self,
        doc: &mut HeadlessDoc,
        req: &FullCoEditRequest,
        commit_content: String,
        meta: CommitMeta<'_>,
    ) -> Value {
        let mode = infer_mode(&doc.file_id, req.mode);
        let new_rev = doc.current_revision + 1;
        let rec = make_rev_for_full(new_rev, doc, &commit_content, req, &meta);
        let new_hash = rec.content_hash.clone();
        if doc.revisions.len() >= MAX_REVISIONS_PER_DOC {
            doc.revisions.pop_front();
        }
        doc.revisions.push_back(rec);
        doc.current_revision = new_rev;
        doc.content = commit_content.clone();
        doc.content_hash = new_hash.clone();

        let author_tag = if meta.auto_merged {
            format!("{} (auto-merged)", req.author)
        } else {
            req.author.clone()
        };
        let res = json!({
            "ok": true,
            "status": "COMMITTED",
            "file_id": doc.file_id,
            "previous_revision": new_rev - 1,
            "revision": new_rev,
            "new_revision": new_rev,
            "merged": meta.auto_merged,
            "auto_merged": meta.auto_merged,
            "conflicts": 0,
            "author": author_tag,
            "content": commit_content,
            "content_hash": new_hash,
            "mode": mode,
            "cas_attempts": meta.cas_attempts,
            "strategy": meta.strategy
        });
        record_idempotency_in_doc(doc, req.op_id.clone(), meta.payload_hash, &res);
        self.push_outbox("COEDIT_COMMITTED", &doc.file_id, new_rev, &res);
        res
    }

    pub fn edit_doc(&self, req: EditRequest) -> Result<Value, String> {
        let doc_lock = self.get_doc_ref(&req.file_id)?;
        let (base_rev, base_content, base_hash) = {
            let doc = doc_lock.read().unwrap();
            let b_rev = req.base_revision.unwrap_or(doc.current_revision);
            let snap = doc
                .revisions
                .iter()
                .find(|r| r.revision == b_rev)
                .ok_or_else(|| format!("Base revision {b_rev} not available"))?;
            if let Some(ref h) = req.base_content_hash {
                if &snap.content_hash != h {
                    return Err(format!("BASE_HASH_MISMATCH at rev {b_rev}"));
                }
            }
            (b_rev, snap.content.clone(), snap.content_hash.clone())
        };

        let base_lines = split_lines(&base_content);
        let s = req.start_line.saturating_sub(1).min(base_lines.len());
        let e = req.end_line.min(base_lines.len());
        let mut candidate = String::with_capacity(base_content.len() + req.replacement.len() + 64);
        base_lines[..s].iter().for_each(|l| candidate.push_str(l));
        candidate.push_str(&req.replacement);
        if !req.replacement.is_empty() && !req.replacement.ends_with('\n') {
            candidate.push('\n');
        }
        let tail_start = if e >= s { e } else { s };
        base_lines[tail_start..]
            .iter()
            .for_each(|l| candidate.push_str(l));

        let intent_hash = content_hash(&format!(
            "edit_lines|{}|{}|{base_rev}|{}|{}|{}",
            req.file_id, req.agent_id, req.start_line, req.end_line, req.replacement
        ));

        self.coedit_with_intent_hash(
            FullCoEditRequest {
                file_id: req.file_id,
                author: format!("{}/{}", req.provider, req.agent_id),
                new_text: candidate,
                base_revision: Some(base_rev),
                base_content_hash: Some(base_hash),
                fencing_token: req.fencing_token,
                op_id: req.op_id,
                mode: Some(CollabMode::Git),
            },
            intent_hash,
        )
    }

    pub fn replace_all_atomic(&self, req: ReplaceAllRequest) -> Result<Value, String> {
        if req.find.is_empty() {
            return Err("INVALID_ARGUMENT: 'find' string must not be empty".into());
        }
        let doc_lock = self.get_doc_ref(&req.file_id)?;
        let (base_rev, base_content, base_hash) = {
            let doc = doc_lock.read().unwrap();
            let b_rev = req.base_revision.unwrap_or(doc.current_revision);
            let snap = doc
                .revisions
                .iter()
                .find(|r| r.revision == b_rev)
                .ok_or_else(|| format!("Base revision {b_rev} not available"))?;
            if let Some(ref h) = req.base_content_hash {
                if &snap.content_hash != h {
                    return Err(format!("BASE_HASH_MISMATCH at rev {b_rev}"));
                }
            }
            (b_rev, snap.content.clone(), snap.content_hash.clone())
        };

        let intent_hash = content_hash(&format!(
            "replace_all|{}|{}|{base_rev}|{}|{}",
            req.file_id, req.agent_id, req.find, req.replace
        ));
        {
            let doc = doc_lock.read().unwrap();
            if let Some(replay) =
                check_idempotency_in_doc(&doc, req.op_id.as_deref(), &intent_hash)?
            {
                return Ok(replay);
            }
        }

        let matches_count = base_content.matches(&req.find).count();
        if matches_count == 0 {
            let mut doc = doc_lock.write().unwrap();
            validate_cas_preconditions(
                &doc,
                Some(base_rev),
                req.base_content_hash.as_deref(),
                req.fencing_token,
                now_epoch_ms(),
            )?;
            let res = json!({
                "ok": true,
                "status": "NO_OP",
                "file_id": doc.file_id,
                "revision": doc.current_revision,
                "new_revision": doc.current_revision,
                "content_hash": doc.content_hash,
                "replacements": 0
            });
            record_idempotency_in_doc(&mut doc, req.op_id, intent_hash, &res);
            return Ok(res);
        }

        let candidate = base_content.replace(&req.find, &req.replace);
        let mut res = self.coedit_with_intent_hash(
            FullCoEditRequest {
                file_id: req.file_id,
                author: format!("{}/{}", req.provider, req.agent_id),
                new_text: candidate,
                base_revision: Some(base_rev),
                base_content_hash: Some(base_hash),
                fencing_token: req.fencing_token,
                op_id: req.op_id,
                mode: Some(CollabMode::Git),
            },
            intent_hash,
        )?;
        res["replacements"] = json!(matches_count);
        Ok(res)
    }

    pub fn coedit_concurrent(&self, req: FullCoEditRequest) -> Result<Value, String> {
        let mode = infer_mode(&req.file_id, req.mode);
        let payload_hash = compute_full_payload_hash(&req, mode);
        self.coedit_with_intent_hash(req, payload_hash)
    }

    fn coedit_with_intent_hash(
        &self,
        req: FullCoEditRequest,
        payload_hash: String,
    ) -> Result<Value, String> {
        let mode = infer_mode(&req.file_id, req.mode);
        if mode == CollabMode::Nema {
            if let Err(e) = nema_codec::parse_source(&req.new_text) {
                return Ok(json!({
                    "ok": false,
                    "status": "REJECTED_NEMA_SYNTAX_ERROR",
                    "file_id": req.file_id,
                    "error": format!("NEMA 语法校验未通过 (Red Guard): {e}"),
                    "mode": mode
                }));
            }
        }

        let doc_ref = self.get_doc_ref(&req.file_id)?;

        // Phase 0: atomic idempotency fast-read & Direct Fast-Path under try_write
        if let Some(res) = self.try_direct_fast_path(&doc_ref, &req, mode, &payload_hash)? {
            return Ok(res);
        }

        // Phase 1..3: OCC 乐观并发协同与不重叠 Rebase
        const MAX_OCC_ATTEMPTS: usize = 4;
        for attempt in 0..MAX_OCC_ATTEMPTS {
            if let Some(res) =
                self.try_occ_attempt(&doc_ref, &req, mode, attempt + 1, &payload_hash)?
            {
                return Ok(res);
            }
            for _ in 0..(1 << (attempt + 2)) {
                std::hint::spin_loop();
            }
        }

        // Phase 4: Pessimistic fallback (0 starvation guarantee)
        self.commit_pessimistic(&doc_ref, &req, mode, MAX_OCC_ATTEMPTS + 1, &payload_hash)
    }

    fn try_direct_fast_path(
        &self,
        doc_ref: &Arc<RwLock<HeadlessDoc>>,
        req: &FullCoEditRequest,
        mode: CollabMode,
        payload_hash: &str,
    ) -> Result<Option<Value>, String> {
        let Ok(mut doc) = doc_ref.try_write() else {
            return Ok(None);
        };
        if let Some(replay) = check_idempotency_in_doc(&doc, req.op_id.as_deref(), payload_hash)? {
            return Ok(Some(replay));
        }
        let now_ms = now_epoch_ms();
        let (base_rev, _) = validate_cas_preconditions(
            &doc,
            req.base_revision,
            req.base_content_hash.as_deref(),
            req.fencing_token,
            now_ms,
        )?;
        if base_rev != doc.current_revision {
            return Ok(None);
        }
        let syntax_lang = if mode == CollabMode::Nema {
            "nema"
        } else {
            &doc.language
        };
        if let Err(err) = check_syntax_integrity(syntax_lang, &req.new_text) {
            let ours_snapshot = doc.content.clone();
            return Ok(Some(self.create_conflict_proposal_and_reply(
                &mut doc,
                req,
                base_rev,
                req.new_text.clone(),
                vec![ConflictHunk {
                    base_line_start: 1,
                    base_line_end: 1,
                    ours: ours_snapshot,
                    theirs: format!("Syntax check failed: {err}"),
                }],
                payload_hash.to_string(),
            )));
        }
        Ok(Some(self.apply_clean_commit(
            &mut doc,
            req,
            req.new_text.clone(),
            CommitMeta {
                auto_merged: false,
                cas_attempts: 1,
                strategy: "DirectFastPath",
                payload_hash: payload_hash.to_string(),
            },
        )))
    }

    fn try_occ_attempt(
        &self,
        doc_ref: &Arc<RwLock<HeadlessDoc>>,
        req: &FullCoEditRequest,
        mode: CollabMode,
        cas_attempt: usize,
        payload_hash: &str,
    ) -> Result<Option<Value>, String> {
        let (head_rev, head_hash, head_content, base_rev, base_content, lang) = {
            let doc = doc_ref.read().unwrap();
            if let Some(replay) =
                check_idempotency_in_doc(&doc, req.op_id.as_deref(), payload_hash)?
            {
                return Ok(Some(replay));
            }
            let (b_rev, b_content) = validate_cas_preconditions(
                &doc,
                req.base_revision,
                req.base_content_hash.as_deref(),
                req.fencing_token,
                now_epoch_ms(),
            )?;
            (
                doc.current_revision,
                doc.content_hash.clone(),
                doc.content.clone(),
                b_rev,
                b_content,
                doc.language.clone(),
            )
        };

        let (merged_text, clean, conflicts) = evaluate_candidate(
            mode,
            &lang,
            base_rev,
            head_rev,
            &base_content,
            &head_content,
            &req.new_text,
        )?;

        let mut doc = doc_ref.write().unwrap();
        if let Some(replay) = check_idempotency_in_doc(&doc, req.op_id.as_deref(), payload_hash)? {
            return Ok(Some(replay));
        }
        validate_cas_preconditions(
            &doc,
            Some(base_rev),
            req.base_content_hash.as_deref(),
            req.fencing_token,
            now_epoch_ms(),
        )?;

        if !check_head_hash_consistency(&doc, head_rev, &head_hash) {
            return Ok(None);
        }

        if !clean {
            return Ok(Some(self.create_conflict_proposal_and_reply(
                &mut doc,
                req,
                base_rev,
                merged_text,
                conflicts,
                payload_hash.to_string(),
            )));
        }

        Ok(Some(self.apply_clean_commit(
            &mut doc,
            req,
            merged_text,
            CommitMeta {
                auto_merged: base_rev != head_rev,
                cas_attempts: cas_attempt,
                strategy: "OCC",
                payload_hash: payload_hash.to_string(),
            },
        )))
    }

    fn commit_pessimistic(
        &self,
        doc_ref: &Arc<RwLock<HeadlessDoc>>,
        req: &FullCoEditRequest,
        mode: CollabMode,
        cas_attempt: usize,
        payload_hash: &str,
    ) -> Result<Value, String> {
        let mut doc = doc_ref.write().unwrap();
        if let Some(replay) = check_idempotency_in_doc(&doc, req.op_id.as_deref(), payload_hash)? {
            return Ok(replay);
        }
        let (base_rev, base_content) = validate_cas_preconditions(
            &doc,
            req.base_revision,
            req.base_content_hash.as_deref(),
            req.fencing_token,
            now_epoch_ms(),
        )?;
        let auto_merged = base_rev != doc.current_revision;
        let (merged_text, clean, conflicts) = evaluate_candidate(
            mode,
            &doc.language,
            base_rev,
            doc.current_revision,
            &base_content,
            &doc.content,
            &req.new_text,
        )?;

        if !clean {
            return Ok(self.create_conflict_proposal_and_reply(
                &mut doc,
                req,
                base_rev,
                merged_text,
                conflicts,
                payload_hash.to_string(),
            ));
        }

        Ok(self.apply_clean_commit(
            &mut doc,
            req,
            merged_text,
            CommitMeta {
                auto_merged,
                cas_attempts: cas_attempt,
                strategy: "PessimisticFallback",
                payload_hash: payload_hash.to_string(),
            },
        ))
    }
}
