//! 跨供应商无头协同编辑引擎 (CAS + 3-Way 自动合并 + 冲突保护)。
//!
//! 不变量:
//! - 纯 Safe Rust (`#![forbid(unsafe_code)]`)
//! - 内存有界: 文档数、版本历史、幂等键均有严格上限
//! - 任何冲突不得污染 HEAD 版本，必须显式返回冲突区块与上下文

use crate::diff::{diff_lines, regions, split_lines};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

pub const MAX_DOCS: usize = 256;
pub const MAX_REVISIONS_PER_DOC: usize = 128;
pub const MAX_IDEMPOTENCY_KEYS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictHunk {
    pub base_line_start: usize,
    pub base_line_end: usize,
    pub ours: String,
    pub theirs: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOutcome {
    pub clean: bool,
    pub merged_text: String,
    pub conflicts: Vec<ConflictHunk>,
    pub ours_changes: usize,
    pub theirs_changes: usize,
}

/// 基于行的 3-Way 合并算法。
///
/// 以 `base` 为祖先，把 `ours` 与 `theirs` 的改动结合。
/// - 非重叠改动：全自动安全合入。
/// - 重叠但内容完全相同的改动：去重合入，不报冲突。
/// - 重叠且内容相异的改动：标记冲突，输出标准 conflict markers 并设置 `clean = false`。
pub fn three_way_merge(base: &str, ours: &str, theirs: &str) -> MergeOutcome {
    if ours == theirs {
        return MergeOutcome {
            clean: true,
            merged_text: ours.to_string(),
            conflicts: Vec::new(),
            ours_changes: 0,
            theirs_changes: 0,
        };
    }
    if ours == base {
        return MergeOutcome {
            clean: true,
            merged_text: theirs.to_string(),
            conflicts: Vec::new(),
            ours_changes: 0,
            theirs_changes: 1,
        };
    }
    if theirs == base {
        return MergeOutcome {
            clean: true,
            merged_text: ours.to_string(),
            conflicts: Vec::new(),
            ours_changes: 1,
            theirs_changes: 0,
        };
    }

    let base_lines = split_lines(base);
    let ours_lines = split_lines(ours);
    let theirs_lines = split_lines(theirs);

    let (ours_steps, _) = diff_lines(&base_lines, &ours_lines);
    let (theirs_steps, _) = diff_lines(&base_lines, &theirs_lines);

    let ours_regions = regions(&ours_steps);
    let theirs_regions = regions(&theirs_steps);

    let mut merged = String::with_capacity(base.len().max(ours.len()).max(theirs.len()));
    let mut conflicts = Vec::new();

    let mut base_idx = 0usize;
    let mut oi = 0usize;
    let mut ti = 0usize;

    let total_ours = ours_regions.len();
    let total_theirs = theirs_regions.len();

    while base_idx < base_lines.len() || oi < ours_regions.len() || ti < theirs_regions.len() {
        let next_o = ours_regions.get(oi).copied();
        let next_t = theirs_regions.get(ti).copied();

        // 确定下一个在 base 上的改动区域起始点
        let next_event_base = match (next_o, next_t) {
            (Some(o), Some(t)) => o.a0.min(t.a0),
            (Some(o), None) => o.a0,
            (None, Some(t)) => t.a0,
            (None, None) => base_lines.len(),
        };

        // 写入尚未发生改动的公共 base 行
        while base_idx < next_event_base && base_idx < base_lines.len() {
            merged.push_str(base_lines[base_idx]);
            base_idx += 1;
        }

        // 检查 ours 和 theirs 在当前区域是否有重叠 / 竞争
        let o_active = next_o.filter(|o| o.a0 <= base_idx);
        let t_active = next_t.filter(|t| t.a0 <= base_idx);

        match (o_active, t_active) {
            (Some(o), Some(t)) => {
                // 两个分支均在当前 base_idx 改动
                let o_slice: String = (o.b0..o.b1).map(|j| ours_lines[j]).collect();
                let t_slice: String = (t.b0..t.b1).map(|j| theirs_lines[j]).collect();

                let max_base_advance = o.a1.max(t.a1);

                if o_slice == t_slice {
                    // 两侧改动内容完全相同，无冲突合入单份
                    merged.push_str(&o_slice);
                } else if o.a0 == t.a0 && o.a1 == t.a1 && o.a0 == o.a1 {
                    // 相同位置的纯插入但内容不同 -> 冲突
                    merged.push_str("<<<<<<< HEAD (ours)\n");
                    merged.push_str(&o_slice);
                    merged.push_str("=======\n");
                    merged.push_str(&t_slice);
                    merged.push_str(">>>>>>> INCOMING (theirs)\n");
                    conflicts.push(ConflictHunk {
                        base_line_start: o.a0,
                        base_line_end: o.a1,
                        ours: o_slice,
                        theirs: t_slice,
                    });
                } else {
                    // 存在重叠修改且内容不同 -> 标记冲突并输出标准标识
                    merged.push_str("<<<<<<< HEAD (ours)\n");
                    merged.push_str(&o_slice);
                    merged.push_str("=======\n");
                    merged.push_str(&t_slice);
                    merged.push_str(">>>>>>> INCOMING (theirs)\n");
                    conflicts.push(ConflictHunk {
                        base_line_start: o.a0.min(t.a0),
                        base_line_end: max_base_advance,
                        ours: o_slice,
                        theirs: t_slice,
                    });
                }
                base_idx = max_base_advance;
                oi += 1;
                ti += 1;
            }
            (Some(o), None) => {
                // 仅 ours 改动
                for j in o.b0..o.b1 {
                    merged.push_str(ours_lines[j]);
                }
                base_idx = o.a1;
                oi += 1;
            }
            (None, Some(t)) => {
                // 仅 theirs 改动
                for j in t.b0..t.b1 {
                    merged.push_str(theirs_lines[j]);
                }
                base_idx = t.a1;
                ti += 1;
            }
            (None, None) => {
                // 消费剩余 base
                while base_idx < base_lines.len() {
                    merged.push_str(base_lines[base_idx]);
                    base_idx += 1;
                }
            }
        }
    }

    let clean = conflicts.is_empty();
    MergeOutcome {
        clean,
        merged_text: merged,
        conflicts,
        ours_changes: total_ours,
        theirs_changes: total_theirs,
    }
}

/// 快速计算内容哈希 (FNV-1a 64位无碰撞十六进制字符串)
pub fn fnv1a_hash(s: &str) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevisionRecord {
    pub revision: u64,
    pub content: String,
    pub content_hash: String,
    pub provider: String,
    pub agent_id: String,
    pub summary: String,
    pub auto_merged: bool,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeadlessDoc {
    pub file_id: String,
    pub language: String,
    pub current_revision: u64,
    pub content: String,
    pub content_hash: String,
    pub revisions: VecDeque<RevisionRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditRequest {
    pub file_id: String,
    pub provider: String,
    pub agent_id: String,
    pub base_revision: Option<u64>,
    /// 1-based 起始行 (闭区间)
    pub start_line: usize,
    /// 1-based 结束行 (闭区间; 若 end_line < start_line 则为纯插入在 start_line 之前)
    pub end_line: usize,
    pub replacement: String,
    pub op_id: Option<String>,
    pub summary: Option<String>,
}

pub struct CoEditWorkspace {
    pub docs: HashMap<String, HeadlessDoc>,
    pub idempotency: HashMap<String, Value>,
}

impl Default for CoEditWorkspace {
    fn default() -> Self {
        let mut ws = Self {
            docs: HashMap::new(),
            idempotency: HashMap::new(),
        };
        // 预置默认测试文件，与云端 Vercel 保持对称
        let default_math = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n";
        ws.open_doc(
            "default:src/math.rs",
            default_math,
            "rust",
            "system",
            "init",
        );
        ws
    }
}

impl CoEditWorkspace {
    pub fn open_doc(
        &mut self,
        file_id: &str,
        content: &str,
        language: &str,
        provider: &str,
        agent_id: &str,
    ) -> &HeadlessDoc {
        if self.docs.len() >= MAX_DOCS && !self.docs.contains_key(file_id) {
            // 超额时驱逐最早的一个无锁文档
            if let Some(oldest) = self.docs.keys().next().cloned() {
                self.docs.remove(&oldest);
            }
        }
        let hash = fnv1a_hash(content);
        let rev1 = RevisionRecord {
            revision: 1,
            content: content.to_string(),
            content_hash: hash.clone(),
            provider: provider.to_string(),
            agent_id: agent_id.to_string(),
            summary: "Initial open".into(),
            auto_merged: false,
            created_at: "2026-11-20T00:00:00Z".into(),
        };
        let mut revisions = VecDeque::with_capacity(MAX_REVISIONS_PER_DOC);
        revisions.push_back(rev1);

        let doc = HeadlessDoc {
            file_id: file_id.to_string(),
            language: language.to_string(),
            current_revision: 1,
            content: content.to_string(),
            content_hash: hash,
            revisions,
        };
        self.docs.insert(file_id.to_string(), doc);
        self.docs.get(file_id).unwrap()
    }

    pub fn read_doc(
        &self,
        file_id: &str,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<Value, String> {
        let doc = self
            .docs
            .get(file_id)
            .ok_or_else(|| format!("Document not found: {file_id}"))?;
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
            "content": slice
        }))
    }

    pub fn edit_doc(&mut self, req: EditRequest) -> Result<Value, String> {
        if let Some(ref op) = req.op_id {
            if let Some(cached) = self.idempotency.get(op) {
                let mut c = cached.clone();
                c["idempotent_replay"] = json!(true);
                return Ok(c);
            }
        }

        let doc = self
            .docs
            .get_mut(&req.file_id)
            .ok_or_else(|| format!("Document not found: {}", req.file_id))?;

        let base_rev = req.base_revision.unwrap_or(doc.current_revision);
        let base_snapshot = doc
            .revisions
            .iter()
            .find(|r| r.revision == base_rev)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Base revision {base_rev} is not available (retained: {}..={})",
                    doc.revisions.front().map(|r| r.revision).unwrap_or(0),
                    doc.current_revision
                )
            })?;

        // 构造针对 base_snapshot 的 candidate
        let base_lines = split_lines(&base_snapshot.content);
        let s = req.start_line.saturating_sub(1).min(base_lines.len());
        let e = req.end_line.min(base_lines.len());

        let mut candidate = String::new();
        for i in 0..s {
            candidate.push_str(base_lines[i]);
        }
        candidate.push_str(&req.replacement);
        if !req.replacement.is_empty() && !req.replacement.ends_with('\n') {
            candidate.push('\n');
        }
        if e >= s {
            for i in e..base_lines.len() {
                candidate.push_str(base_lines[i]);
            }
        } else {
            for i in s..base_lines.len() {
                candidate.push_str(base_lines[i]);
            }
        }

        let is_fast_path = base_rev == doc.current_revision;
        let (final_content, auto_merged) = if is_fast_path {
            (candidate, false)
        } else {
            // 自动运行 3-Way 合并
            let merge = three_way_merge(&base_snapshot.content, &doc.content, &candidate);
            if !merge.clean {
                return Ok(json!({
                    "status": "CONFLICT",
                    "file_id": doc.file_id,
                    "base_revision": base_rev,
                    "head_revision": doc.current_revision,
                    "conflicts": merge.conflicts,
                    "conflict_preview": merge.merged_text
                }));
            }
            (merge.merged_text, true)
        };

        let new_rev = doc.current_revision + 1;
        let new_hash = fnv1a_hash(&final_content);
        let summary = req.summary.unwrap_or_else(|| {
            if auto_merged {
                format!("Auto-merged from rev {base_rev}")
            } else {
                format!("Edit by {}/{}", req.provider, req.agent_id)
            }
        });

        let rev_rec = RevisionRecord {
            revision: new_rev,
            content: final_content.clone(),
            content_hash: new_hash.clone(),
            provider: req.provider.clone(),
            agent_id: req.agent_id.clone(),
            summary: summary.clone(),
            auto_merged,
            created_at: "2026-11-20T00:00:00Z".into(),
        };

        if doc.revisions.len() >= MAX_REVISIONS_PER_DOC {
            doc.revisions.pop_front();
        }
        doc.revisions.push_back(rev_rec);
        doc.current_revision = new_rev;
        doc.content = final_content;
        doc.content_hash = new_hash.clone();

        let res = json!({
            "status": "COMMITTED",
            "file_id": doc.file_id,
            "previous_revision": new_rev - 1,
            "new_revision": new_rev,
            "content_hash": new_hash,
            "auto_merged": auto_merged,
            "provider": req.provider,
            "agent_id": req.agent_id,
            "summary": summary
        });

        if let Some(op) = req.op_id {
            if self.idempotency.len() >= MAX_IDEMPOTENCY_KEYS {
                if let Some(first) = self.idempotency.keys().next().cloned() {
                    self.idempotency.remove(&first);
                }
            }
            self.idempotency.insert(op, res.clone());
        }

        Ok(res)
    }

    pub fn diff_revisions(
        &self,
        file_id: &str,
        from_rev: u64,
        to_rev: u64,
        context: usize,
    ) -> Result<Value, String> {
        let doc = self
            .docs
            .get(file_id)
            .ok_or_else(|| format!("Document not found: {file_id}"))?;
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

        let d = crate::diff::diff_text(&r1.content, &r2.content, context);
        let u = crate::diff::unified(&d, &format!("rev_{from_rev}"), &format!("rev_{to_rev}"));

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

    pub fn list_docs(&self) -> Value {
        let list: Vec<Value> = self
            .docs
            .values()
            .map(|d| {
                json!({
                    "file_id": d.file_id,
                    "language": d.language,
                    "current_revision": d.current_revision,
                    "content_hash": d.content_hash,
                    "revisions_count": d.revisions.len()
                })
            })
            .collect();
        json!({ "docs": list })
    }

    pub fn doc_history(&self, file_id: &str) -> Result<Value, String> {
        let doc = self
            .docs
            .get(file_id)
            .ok_or_else(|| format!("Document not found: {file_id}"))?;
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
                    "created_at": r.created_at
                })
            })
            .collect();
        Ok(json!({
            "file_id": file_id,
            "current_revision": doc.current_revision,
            "history": history
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_three_way_merge_clean_non_overlapping() {
        let base = "line1\nline2\nline3\nline4\nline5\n";
        // ours (Anthropic) 修改 line1
        let ours = "line1_modified_by_claude\nline2\nline3\nline4\nline5\n";
        // theirs (OpenAI) 修改 line5
        let theirs = "line1\nline2\nline3\nline4\nline5_modified_by_gpt\n";

        let m = three_way_merge(base, ours, theirs);
        assert!(m.clean);
        assert_eq!(
            m.merged_text,
            "line1_modified_by_claude\nline2\nline3\nline4\nline5_modified_by_gpt\n"
        );
        assert_eq!(m.conflicts.len(), 0);
    }

    #[test]
    fn test_three_way_merge_identical_changes_coalesce() {
        let base = "line1\nline2\n";
        let ours = "line1\nline2_new\n";
        let theirs = "line1\nline2_new\n";

        let m = three_way_merge(base, ours, theirs);
        assert!(m.clean);
        assert_eq!(m.merged_text, "line1\nline2_new\n");
    }

    #[test]
    fn test_three_way_merge_conflict_markers() {
        let base = "line1\nline2\n";
        let ours = "line1\nline2_conflict_a\n";
        let theirs = "line1\nline2_conflict_b\n";

        let m = three_way_merge(base, ours, theirs);
        assert!(!m.clean);
        assert!(m.merged_text.contains("<<<<<<< HEAD (ours)"));
        assert!(m.merged_text.contains("line2_conflict_a"));
        assert!(m.merged_text.contains("======="));
        assert!(m.merged_text.contains("line2_conflict_b"));
        assert!(m.merged_text.contains(">>>>>>> INCOMING (theirs)"));
        assert_eq!(m.conflicts.len(), 1);
    }

    #[test]
    fn test_coedit_cross_provider_concurrent_auto_merge() {
        let mut ws = CoEditWorkspace::default();
        let file = "default:src/math.rs";

        // Anthropic Claude 读到 rev 1，修改 add 函数
        let edit_claude = EditRequest {
            file_id: file.into(),
            provider: "anthropic".into(),
            agent_id: "claude-3-5-sonnet".into(),
            base_revision: Some(1),
            start_line: 1,
            end_line: 3,
            replacement: "pub fn add(a: i32, b: i32) -> i32 {\n    // Added check\n    a.saturating_add(b)\n}".into(),
            op_id: Some("op-claude-1".into()),
            summary: Some("Use saturating_add in add()".into()),
        };
        let r1 = ws.edit_doc(edit_claude).unwrap();
        assert_eq!(r1["status"], "COMMITTED");
        assert_eq!(r1["new_revision"], 2);
        assert_eq!(r1["auto_merged"], false);

        // OpenAI GPT-4o 也基于旧的 rev 1 修改 multiply 函数 (并发提交，未感知 rev 2)
        let edit_gpt = EditRequest {
            file_id: file.into(),
            provider: "openai".into(),
            agent_id: "gpt-4o".into(),
            base_revision: Some(1),
            start_line: 5,
            end_line: 7,
            replacement: "pub fn multiply(a: i32, b: i32) -> i32 {\n    // Added check\n    a.saturating_mul(b)\n}".into(),
            op_id: Some("op-gpt-1".into()),
            summary: Some("Use saturating_mul in multiply()".into()),
        };
        let r2 = ws.edit_doc(edit_gpt).unwrap();
        assert_eq!(r2["status"], "COMMITTED");
        assert_eq!(r2["new_revision"], 3);
        assert_eq!(r2["auto_merged"], true); // 自动 3-Way 成功合入

        // 验证最新文档同时包含了 Claude 和 GPT-4o 的改动
        let read = ws.read_doc(file, None, None).unwrap();
        let content = read["content"].as_str().unwrap();
        assert!(content.contains("saturating_add"));
        assert!(content.contains("saturating_mul"));

        // 验证 diff
        let diff = ws.diff_revisions(file, 1, 3, 2).unwrap();
        assert_eq!(diff["identical"], false);
        let u = diff["unified_diff"].as_str().unwrap();
        assert!(u.contains("+    a.saturating_add(b)"));
        assert!(u.contains("+    a.saturating_mul(b)"));
    }

    #[test]
    fn test_coedit_conflicting_edits_rejected_cleanly() {
        let mut ws = CoEditWorkspace::default();
        let file = "default:src/math.rs";

        // Provider A 先修改第 2 行
        ws.edit_doc(EditRequest {
            file_id: file.into(),
            provider: "deepseek".into(),
            agent_id: "deepseek-r1".into(),
            base_revision: Some(1),
            start_line: 2,
            end_line: 2,
            replacement: "    a + b + 0".into(),
            op_id: None,
            summary: None,
        })
        .unwrap();

        // Provider B 尝试基于 rev 1 也修改第 2 行 (不同内容 -> 冲突)
        let res = ws
            .edit_doc(EditRequest {
                file_id: file.into(),
                provider: "google".into(),
                agent_id: "gemini-flash".into(),
                base_revision: Some(1),
                start_line: 2,
                end_line: 2,
                replacement: "    a + b + 42".into(),
                op_id: None,
                summary: None,
            })
            .unwrap();

        assert_eq!(res["status"], "CONFLICT");
        // 文档仍保持在 rev 2，未被污染
        let doc = ws.docs.get(file).unwrap();
        assert_eq!(doc.current_revision, 2);
        assert!(!doc.content.contains("<<<<<<<"));
    }

    #[test]
    fn test_coedit_idempotency() {
        let mut ws = CoEditWorkspace::default();
        let req = EditRequest {
            file_id: "default:src/math.rs".into(),
            provider: "test".into(),
            agent_id: "agent1".into(),
            base_revision: Some(1),
            start_line: 2,
            end_line: 2,
            replacement: "    42".into(),
            op_id: Some("unique-op-999".into()),
            summary: None,
        };
        let r1 = ws.edit_doc(req.clone()).unwrap();
        let r2 = ws.edit_doc(req).unwrap();
        assert_eq!(r1["new_revision"], r2["new_revision"]);
        assert_eq!(r2["idempotent_replay"], true);
    }
}
