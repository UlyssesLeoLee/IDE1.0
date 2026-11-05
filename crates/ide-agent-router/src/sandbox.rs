//! Worker 隔离沙盒环境驱动 (DD-ROUTER-SANDBOX)
//!
//! 为每个 Worker 和 Task 提供轻量级、安全的工作目录隔离机制：
//! - 路径穿透检查（Prevent Directory Traversal）
//! - 只读 / 可写文件挂载
//! - 生命周期清理（Drop 自动删除与显式 cleanup）
//! - 沙盒执行环境（SandboxEnvironment）与模拟运行支持

use crate::identity::SupportedLanguage;
use crate::patch::{PatchEngine, PatchError, PatchProposal};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use thiserror::Error;

static SANDBOX_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("Path traversal detected: '{0}'")]
    PathTraversalDetected(String),

    #[error("Read-only violation on path: '{0}'")]
    ReadOnlyViolation(String),

    #[error("Sandbox inactive or destroyed: '{0}'")]
    Inactive(String),

    #[error("Execution failed: '{0}'")]
    ExecutionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Patch error: {0}")]
    Patch(#[from] PatchError),
}

/// 沙盒执行模拟结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub duration_ms: u64,
}

/// Worker 隔离沙盒
pub struct WorkerSandbox {
    pub id: String,
    pub worker_id: String,
    pub task_id: String,
    pub sandbox_root: PathBuf,
    pub auto_cleanup: bool,
    readonly_mounts: HashSet<PathBuf>,
    readwrite_mounts: HashSet<PathBuf>,
}

impl WorkerSandbox {
    /// 在系统的临时目录下为指定 Worker 和 Task 创建唯一隔离沙盒目录
    pub fn new(worker_id: &str, task_id: &str) -> Result<Self, SandboxError> {
        let seq = SANDBOX_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let safe_worker = worker_id.replace(['/', '\\', ':', '.', ' '], "_");
        let safe_task = task_id.replace(['/', '\\', ':', '.', ' '], "_");
        let dir_name = format!("ide_sbx_{}_{}_{}_{}", safe_worker, safe_task, timestamp, seq);

        let base_tmp = std::env::temp_dir().join("ide_worker_sandboxes");
        let sandbox_root = base_tmp.join(dir_name);

        std::fs::create_dir_all(&sandbox_root)?;

        let id = format!("{}:{}", worker_id, task_id);
        Ok(Self {
            id,
            worker_id: worker_id.to_string(),
            task_id: task_id.to_string(),
            sandbox_root,
            auto_cleanup: true,
            readonly_mounts: HashSet::new(),
            readwrite_mounts: HashSet::new(),
        })
    }

    /// 自定义根路径创建沙盒（便于单元测试自定义目录）
    pub fn with_root(
        worker_id: &str,
        task_id: &str,
        custom_root: impl AsRef<Path>,
    ) -> Result<Self, SandboxError> {
        let sandbox_root = custom_root.as_ref().to_path_buf();
        std::fs::create_dir_all(&sandbox_root)?;

        let id = format!("{}:{}", worker_id, task_id);
        Ok(Self {
            id,
            worker_id: worker_id.to_string(),
            task_id: task_id.to_string(),
            sandbox_root,
            auto_cleanup: true,
            readonly_mounts: HashSet::new(),
            readwrite_mounts: HashSet::new(),
        })
    }

    /// 设置是否在 Drop 时自动清理沙盒目录
    pub fn set_auto_cleanup(&mut self, enabled: bool) {
        self.auto_cleanup = enabled;
    }

    /// 路径穿透防御与标准化校验
    ///
    /// 确保相对路径不会逃逸出当前沙盒根目录（如 `../`、绝对路径、Windows 盘符冒号、Null 字节等）。
    pub fn resolve_safe_path(&self, rel_path: impl AsRef<Path>) -> Result<PathBuf, SandboxError> {
        let p = rel_path.as_ref();
        let p_str = p.to_string_lossy();

        // 拦截空字节注入
        if p_str.contains('\0') {
            return Err(SandboxError::PathTraversalDetected(
                "Null byte detected in path".to_string(),
            ));
        }

        // 拦截绝对路径或带有 Windows 盘符冒号的前缀
        if p.is_absolute() {
            return Err(SandboxError::PathTraversalDetected(format!(
                "Absolute path is forbidden: {}",
                p_str
            )));
        }

        let mut sanitized = PathBuf::new();
        for comp in p.components() {
            match comp {
                Component::Normal(part) => {
                    let part_str = part.to_string_lossy();
                    if part_str == ".." || part_str.contains(':') {
                        return Err(SandboxError::PathTraversalDetected(format!(
                            "Invalid component '{}' in path {}",
                            part_str, p_str
                        )));
                    }
                    sanitized.push(part);
                }
                Component::CurDir => continue,
                Component::ParentDir => {
                    return Err(SandboxError::PathTraversalDetected(format!(
                        "Parent directory traversal '..' detected in path: {}",
                        p_str
                    )));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(SandboxError::PathTraversalDetected(format!(
                        "Root or prefix component is forbidden in relative path: {}",
                        p_str
                    )));
                }
            }
        }

        if sanitized.as_os_str().is_empty() {
            return Err(SandboxError::PathTraversalDetected(
                "Empty target path".to_string(),
            ));
        }

        let full_path = self.sandbox_root.join(&sanitized);

        // 如果 full_path 已经存在，二次验证其规范路径必须以 sandbox_root 开头
        if full_path.exists() {
            let canon_root = self.sandbox_root.canonicalize().map_err(SandboxError::Io)?;
            let canon_full = full_path.canonicalize().map_err(SandboxError::Io)?;
            if !canon_full.starts_with(&canon_root) {
                return Err(SandboxError::PathTraversalDetected(format!(
                    "Path resolves outside of sandbox root: {}",
                    p_str
                )));
            }
        }

        Ok(sanitized)
    }

    /// 获取沙盒内绝对路径（已执行安全校验）
    pub fn to_sandbox_path(&self, rel_path: impl AsRef<Path>) -> Result<PathBuf, SandboxError> {
        let safe_rel = self.resolve_safe_path(rel_path)?;
        Ok(self.sandbox_root.join(safe_rel))
    }

    /// 挂载只读文件或目录
    pub fn mount_readonly(
        &mut self,
        source_path: &Path,
        rel_dest: &Path,
    ) -> Result<PathBuf, SandboxError> {
        let safe_dest = self.resolve_safe_path(rel_dest)?;
        let full_dest = self.sandbox_root.join(&safe_dest);

        if let Some(parent) = full_dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if source_path.is_dir() {
            copy_dir_recursive(source_path, &full_dest)?;
        } else {
            std::fs::copy(source_path, &full_dest)?;
        }

        self.readonly_mounts.insert(safe_dest.clone());
        Ok(full_dest)
    }

    /// 挂载可读写文件或目录
    pub fn mount_readwrite(
        &mut self,
        source_path: &Path,
        rel_dest: &Path,
    ) -> Result<PathBuf, SandboxError> {
        let safe_dest = self.resolve_safe_path(rel_dest)?;
        let full_dest = self.sandbox_root.join(&safe_dest);

        if let Some(parent) = full_dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if source_path.is_dir() {
            copy_dir_recursive(source_path, &full_dest)?;
        } else {
            std::fs::copy(source_path, &full_dest)?;
        }

        self.readwrite_mounts.insert(safe_dest.clone());
        Ok(full_dest)
    }

    /// 检查指定相对路径是否被只读挂载保护
    fn is_readonly_target(&self, rel_path: &Path) -> bool {
        for ro in &self.readonly_mounts {
            if rel_path == ro || rel_path.starts_with(ro) {
                return true;
            }
        }
        false
    }

    /// 在沙盒内安全写入文件
    pub fn write_file(
        &self,
        rel_path: impl AsRef<Path>,
        content: &[u8],
    ) -> Result<PathBuf, SandboxError> {
        let safe_rel = self.resolve_safe_path(rel_path)?;

        if self.is_readonly_target(&safe_rel) {
            return Err(SandboxError::ReadOnlyViolation(
                safe_rel.to_string_lossy().to_string(),
            ));
        }

        let full_path = self.sandbox_root.join(&safe_rel);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&full_path, content)?;
        Ok(full_path)
    }

    /// 在沙盒内安全读取文件
    pub fn read_file(&self, rel_path: impl AsRef<Path>) -> Result<Vec<u8>, SandboxError> {
        let safe_rel = self.resolve_safe_path(rel_path)?;
        let full_path = self.sandbox_root.join(&safe_rel);
        let data = std::fs::read(&full_path)?;
        Ok(data)
    }

    /// 在沙盒内安全读取文本文件
    pub fn read_to_string(&self, rel_path: impl AsRef<Path>) -> Result<String, SandboxError> {
        let bytes = self.read_file(rel_path)?;
        String::from_utf8(bytes).map_err(|e| {
            SandboxError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })
    }

    /// 删除沙盒内文件
    pub fn remove_file(&self, rel_path: impl AsRef<Path>) -> Result<(), SandboxError> {
        let safe_rel = self.resolve_safe_path(rel_path)?;
        if self.is_readonly_target(&safe_rel) {
            return Err(SandboxError::ReadOnlyViolation(
                safe_rel.to_string_lossy().to_string(),
            ));
        }
        let full_path = self.sandbox_root.join(&safe_rel);
        if full_path.exists() {
            std::fs::remove_file(full_path)?;
        }
        Ok(())
    }

    /// 创建沙盒运行环境
    pub fn create_environment(&self) -> SandboxEnvironment {
        SandboxEnvironment {
            sandbox_id: self.id.clone(),
            working_dir: self.sandbox_root.clone(),
            env_vars: std::collections::HashMap::new(),
            staged_files: std::collections::HashMap::new(),
        }
    }

    /// 显式清理沙盒物理目录
    pub fn cleanup(&self) -> Result<(), SandboxError> {
        if self.sandbox_root.exists() {
            std::fs::remove_dir_all(&self.sandbox_root)?;
        }
        Ok(())
    }
}

impl Drop for WorkerSandbox {
    fn drop(&mut self) {
        if self.auto_cleanup && self.sandbox_root.exists() {
            let _ = std::fs::remove_dir_all(&self.sandbox_root);
        }
    }
}

/// 沙盒执行环境结构体：负责暂存 Patch 后的临时文件与模拟运行
#[derive(Debug, Clone)]
pub struct SandboxEnvironment {
    pub sandbox_id: String,
    pub working_dir: PathBuf,
    pub env_vars: std::collections::HashMap<String, String>,
    pub staged_files: std::collections::HashMap<String, PathBuf>,
}

impl SandboxEnvironment {
    /// 设置环境变量
    pub fn set_env(&mut self, key: &str, val: &str) {
        self.env_vars.insert(key.to_string(), val.to_string());
    }

    /// 在沙盒内暂存普通文件
    pub fn stage_file(
        &mut self,
        sandbox: &WorkerSandbox,
        rel_path: &str,
        content: &[u8],
    ) -> Result<PathBuf, SandboxError> {
        let full_path = sandbox.write_file(rel_path, content)?;
        self.staged_files
            .insert(rel_path.to_string(), full_path.clone());
        Ok(full_path)
    }

    /// 暂存 Patch 后的临时代码文件并在沙盒隔离区建立对应文件副本
    pub fn stage_patch(
        &mut self,
        sandbox: &WorkerSandbox,
        workspace_id: &str,
        file_id: &str,
        current_content: &str,
        current_revision: u64,
        language: SupportedLanguage,
        proposal: &PatchProposal,
    ) -> Result<PathBuf, SandboxError> {
        let patch_result = PatchEngine::relocate_and_apply(
            workspace_id,
            file_id,
            current_content,
            current_revision,
            language,
            proposal,
        )?;

        let full_path = sandbox.write_file(file_id, patch_result.new_content.as_bytes())?;
        self.staged_files
            .insert(file_id.to_string(), full_path.clone());
        Ok(full_path)
    }

    /// 在隔离区执行模拟运行（Dry-run / Verification Command）
    pub fn simulate_run(
        &self,
        command: &str,
        args: &[&str],
    ) -> Result<SimulationResult, SandboxError> {
        let start = Instant::now();

        // 内置模拟器支持：若 command 为 "echo" 或 "mock"，快速返回确定性结果
        if command == "mock" || command == "dry_run" {
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(SimulationResult {
                exit_code: 0,
                stdout: format!("Simulation passed for {} staged files", self.staged_files.len()),
                stderr: String::new(),
                success: true,
                duration_ms,
            });
        }

        // 调用宿主进程执行隔离区真实验证（命令在 working_dir 中受限执行）
        let mut cmd = std::process::Command::new(command);
        cmd.args(args)
            .current_dir(&self.working_dir)
            .envs(&self.env_vars);

        match cmd.output() {
            Ok(output) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let exit_code = output.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Ok(SimulationResult {
                    exit_code,
                    stdout,
                    stderr,
                    success: output.status.success(),
                    duration_ms,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                // 如果命令不存在，生成明确的拦截反馈
                Ok(SimulationResult {
                    exit_code: 127,
                    stdout: String::new(),
                    stderr: format!("Command '{}' failed to spawn: {}", command, e),
                    success: false,
                    duration_ms,
                })
            }
        }
    }
}

/// 辅助递归复制目录
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_child = entry.path();
        let dst_child = dst.join(entry.file_name());
        if src_child.is_dir() {
            copy_dir_recursive(&src_child, &dst_child)?;
        } else {
            std::fs::copy(&src_child, &dst_child)?;
        }
    }
    Ok(())
}
