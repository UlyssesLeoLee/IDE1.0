//! 真实磁盘文件集成：原子写盘、外部修改检测、崩溃恢复保护与 Git Diff 协同 (P1-04)。

use super::hash::content_hash;
use super::store::DocumentStore;
use super::types::{read_lock, write_lock, FullCoEditRequest, OpenMode};
use editcore::auto_backup::{backup_before_save, SaveBackupConfig};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_SEQ: AtomicU64 = AtomicU64::new(1);

fn strip_win_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}

fn canonicalize_or_normalize(path: &Path) -> Result<PathBuf, String> {
    if let Ok(c) = fs::canonicalize(path) {
        return Ok(strip_win_verbatim(&c));
    }
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("Failed to resolve current_dir: {e}"))?
            .join(path)
    };
    if let Some(parent) = abs.parent() {
        if let Ok(cp) = fs::canonicalize(parent) {
            if let Some(name) = abs.file_name() {
                return Ok(strip_win_verbatim(&cp).join(name));
            }
        }
    }
    Ok(strip_win_verbatim(&abs))
}

/// 路径安全校验：禁止空路径、NUL 字节、`..` 遍历；若配置了 `IDE_MCP_WORKSPACE_ROOT` 则限制在其范围内。
pub fn validate_disk_path(path: &Path) -> Result<PathBuf, String> {
    let raw = path.to_string_lossy();
    if raw.trim().is_empty() || raw.contains('\0') {
        return Err("UNSAFE_PATH: empty path or NUL byte is not allowed".to_string());
    }
    for comp in path.components() {
        if matches!(comp, Component::ParentDir) {
            return Err(format!(
                "UNSAFE_PATH: parent traversal '..' is not allowed in '{}'",
                path.display()
            ));
        }
    }
    let resolved = canonicalize_or_normalize(path)?;
    if let Ok(root_raw) = std::env::var("IDE_MCP_WORKSPACE_ROOT") {
        if !root_raw.trim().is_empty() {
            let root = canonicalize_or_normalize(Path::new(&root_raw))?;
            if !resolved.starts_with(&root) {
                return Err(format!(
                    "UNSAFE_PATH: '{}' escapes configured IDE_MCP_WORKSPACE_ROOT '{}'",
                    resolved.display(),
                    root.display()
                ));
            }
        }
    }
    Ok(resolved)
}

#[cfg(windows)]
fn configure_headless_cmd(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_headless_cmd(_cmd: &mut Command) {}

/// 原子写盘（同目录临时文件 -> flush/sync_all -> rename），防止崩溃半写损坏真实源文件。
pub fn atomic_write_file(target: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dir: {e}"))?;
        }
    }

    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let tmp_path = target.with_file_name(format!(".{file_name}.ide10_tmp_{pid}_{seq}"));

    let write_res = (|| -> Result<(), String> {
        let mut f = fs::File::create(&tmp_path)
            .map_err(|e| format!("Failed to create temp file {}: {e}", tmp_path.display()))?;
        f.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write temp file: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("Failed to fsync temp file: {e}"))?;
        Ok(())
    })();

    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }

    if let Err(err) = fs::rename(&tmp_path, target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!("Atomic rename failed: {err}"));
    }

    Ok(())
}

impl DocumentStore {
    /// 打开真实磁盘文件并绑定到文档存储（存在则返回已打开状态并附带磁盘漂移检测）。
    pub fn open_disk_file(
        &self,
        file_id: &str,
        disk_path: &Path,
        language: &str,
        provider: &str,
        agent_id: &str,
    ) -> Result<Value, String> {
        let safe_path = validate_disk_path(disk_path)?;
        let disk_content = fs::read_to_string(&safe_path)
            .map_err(|e| format!("Failed to read disk file '{}': {e}", safe_path.display()))?;
        let disk_hash = content_hash(&disk_content);
        let res = self.open_doc_with_mode(
            file_id,
            &disk_content,
            language,
            provider,
            agent_id,
            OpenMode::CreateIfAbsent,
        )?;
        let doc_ref = self.get_doc_ref(file_id)?;
        {
            let mut doc = write_lock(&doc_ref);
            doc.disk_path = Some(safe_path.to_string_lossy().into_owned());
            if doc.last_disk_hash.is_none() {
                doc.last_disk_hash = Some(disk_hash.clone());
                doc.last_synced_revision = Some(doc.current_revision);
            }
        }
        let mut out = res;
        out["disk_path"] = json!(safe_path.to_string_lossy());
        out["disk_hash"] = json!(disk_hash);
        Ok(out)
    }

    /// 安全将内存权威 HEAD 原子写回磁盘，写入前校验是否存在外部磁盘修改或未解决冲突。
    pub fn sync_to_disk(
        &self,
        file_id: &str,
        override_path: Option<&Path>,
        force: bool,
    ) -> Result<Value, String> {
        if self.has_pending_proposals(file_id) {
            return Err(format!(
                "PENDING_CONFLICT_PROPOSALS: resolve pending conflict proposals for '{file_id}' before syncing to disk"
            ));
        }

        let doc_ref = self.get_doc_ref(file_id)?;
        let (target_path, content, head_rev, head_hash, prev_disk_hash) = {
            let doc = read_lock(&doc_ref);
            let path_str = override_path
                .map(|p| p.to_string_lossy().into_owned())
                .or_else(|| doc.disk_path.clone())
                .ok_or_else(|| format!("No disk path bound for document '{file_id}'"))?;
            (
                PathBuf::from(path_str),
                doc.content.clone(),
                doc.current_revision,
                doc.content_hash.clone(),
                doc.last_disk_hash.clone(),
            )
        };
        let target_path = validate_disk_path(&target_path)?;

        if target_path.exists() && !force {
            let on_disk = fs::read_to_string(&target_path)
                .map_err(|e| format!("Failed to read existing disk file: {e}"))?;
            let current_disk_hash = content_hash(&on_disk);
            if let Some(expected_hash) = prev_disk_hash {
                if current_disk_hash != expected_hash && current_disk_hash != head_hash {
                    return Err(format!(
                        "EXTERNAL_DISK_MODIFICATION: '{}' was modified on disk externally (expected hash {expected_hash}, found {current_disk_hash})",
                        target_path.display()
                    ));
                }
            }
        }

        if target_path.exists() {
            backup_before_save(&target_path, &SaveBackupConfig::default())
                .map_err(|e| format!("Pre-save backup failed: {e}"))?;
        }

        atomic_write_file(&target_path, &content)?;

        {
            let mut doc = write_lock(&doc_ref);
            doc.disk_path = Some(target_path.to_string_lossy().into_owned());
            doc.last_disk_hash = Some(head_hash.clone());
            doc.last_synced_revision = Some(head_rev);
        }

        Ok(json!({
            "ok": true,
            "status": "SYNCED_TO_DISK",
            "file_id": file_id,
            "disk_path": target_path.to_string_lossy(),
            "synced_revision": head_rev,
            "content_hash": head_hash,
            "bytes_written": content.len()
        }))
    }

    /// 识别磁盘外部变更并通过协同合并安全导入到版本历史（不粗暴覆盖未同步内存改动）。
    pub fn reload_from_disk(
        &self,
        file_id: &str,
        override_path: Option<&Path>,
        agent_id: &str,
        op_id: &str,
    ) -> Result<Value, String> {
        let doc_ref = self.get_doc_ref(file_id)?;
        let (target_path, synced_rev, head_rev, head_hash) = {
            let doc = read_lock(&doc_ref);
            let path_str = override_path
                .map(|p| p.to_string_lossy().into_owned())
                .or_else(|| doc.disk_path.clone())
                .ok_or_else(|| format!("No disk path bound for document '{file_id}'"))?;
            (
                PathBuf::from(path_str),
                doc.last_synced_revision.unwrap_or(doc.current_revision),
                doc.current_revision,
                doc.content_hash.clone(),
            )
        };
        let target_path = validate_disk_path(&target_path)?;

        let disk_content = fs::read_to_string(&target_path)
            .map_err(|e| format!("Failed to read disk file '{}': {e}", target_path.display()))?;
        let disk_hash = content_hash(&disk_content);

        if disk_hash == head_hash {
            return Ok(json!({
                "ok": true,
                "status": "DISK_UP_TO_DATE",
                "file_id": file_id,
                "revision": head_rev,
                "content_hash": head_hash
            }));
        }

        let res = self.coedit_concurrent(FullCoEditRequest {
            file_id: file_id.to_string(),
            author: agent_id.to_string(),
            new_text: disk_content,
            base_revision: Some(synced_rev),
            base_content_hash: None,
            fencing_token: None,
            op_id: Some(op_id.to_string()),
            mode: None,
        })?;

        if res["ok"] == true {
            let mut doc = write_lock(&doc_ref);
            doc.last_disk_hash = Some(disk_hash);
            doc.last_synced_revision = Some(doc.current_revision);
        }
        Ok(res)
    }

    /// 回滚至历史版本并追加新提交（不破坏版本单调性），可选同时原子落盘。
    pub fn rollback_revision(
        &self,
        file_id: &str,
        target_revision: u64,
        agent_id: &str,
        op_id: &str,
        sync_disk: bool,
    ) -> Result<Value, String> {
        let doc_ref = self.get_doc_ref(file_id)?;
        let (target_content, head_rev, head_hash) = {
            let doc = read_lock(&doc_ref);
            let target = doc
                .revisions
                .iter()
                .find(|r| r.revision == target_revision)
                .ok_or_else(|| format!("Target revision {target_revision} not found"))?;
            (
                target.content.clone(),
                doc.current_revision,
                doc.content_hash.clone(),
            )
        };

        let mut res = self.coedit_concurrent(FullCoEditRequest {
            file_id: file_id.to_string(),
            author: agent_id.to_string(),
            new_text: target_content,
            base_revision: Some(head_rev),
            base_content_hash: Some(head_hash),
            fencing_token: None,
            op_id: Some(op_id.to_string()),
            mode: None,
        })?;
        res["rolled_back_to"] = json!(target_revision);

        if sync_disk && res["ok"] == true {
            let disk_res = self.sync_to_disk(file_id, None, true)?;
            res["disk_sync"] = disk_res;
        }
        Ok(res)
    }
}

/// 运行真实 `git diff` 查验工作区磁盘文件差异（无 shell 注入，Windows 无窗启动）。
pub fn git_diff_workspace(workspace_root: &Path, file_path: Option<&str>) -> Result<Value, String> {
    let safe_root = validate_disk_path(workspace_root)?;
    if let Some(p) = file_path {
        validate_disk_path(Path::new(p))?;
    }
    let mut cmd = Command::new("git");
    configure_headless_cmd(&mut cmd);
    cmd.current_dir(&safe_root).arg("diff");
    if let Some(p) = file_path {
        cmd.arg("--").arg(p);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to invoke git diff: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git diff failed: {stderr}"));
    }
    let diff_text = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut status_cmd = Command::new("git");
    configure_headless_cmd(&mut status_cmd);
    status_cmd
        .current_dir(&safe_root)
        .args(["status", "--short"]);
    if let Some(p) = file_path {
        status_cmd.arg("--").arg(p);
    }
    let status_out = status_cmd
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    Ok(json!({
        "ok": true,
        "workspace_root": safe_root.to_string_lossy(),
        "file_path": file_path,
        "has_changes": !diff_text.trim().is_empty() || !status_out.trim().is_empty(),
        "git_status": status_out,
        "unified_diff": diff_text
    }))
}
