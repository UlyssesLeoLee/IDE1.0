//! ide-shell-desktop — Tauri 2 desktop app 主体.
//!
//! 架构:
//! - Tauri `manage()` 注入两个单例 state:
//!   * `ShellState(Mutex<App>)`        — 底部 Shell 面板 (复用 ide-shell core, UAT 兼容)
//!   * `ProjectRoot(Arc<Mutex<..>>)`   — 当前打开的项目根目录 (文件树/读写沙箱边界)
//! - 11 个 `#[tauri::command]` 暴露 webview invoke 端点:
//!   * `frame() -> WebFrame`             拉当前 shell frame
//!   * `reset() -> WebFrame`             重置 shell 状态
//!   * `key(code, modifiers) -> ...`     派发键盘事件到 shell
//!   * `mouse(button) -> ...`            派发鼠标点击到 shell
//!   * `kernel_banner() -> String`       复用 ide-kernel-core
//!   * `get_mode() -> String`            取 shell mode (避开 Rust 关键字 `mode`)
//!   * `pick_folder() -> Option<String>` 原生文件夹选择器 (rfd) → 设为项目根
//!   * `open_project(path) -> String`    直接以路径打开项目 (最近项目复用)
//!   * `list_dir(path) -> Vec<DirEntry>` 列目录 (限项目根内)
//!   * `read_file(path) -> FileContent`  读文件 (限项目根内, UTF-8, ≤4MB)
//!   * `write_file(path, content) -> usize` 写文件 (限项目根内)
//!   * `help_wiki() -> String`           完整内置 wiki (与 CLI --help 同源)
//!
//! 注: Tauri 2.12 已知限制 — `#[tauri::command]` 不能 `pub fn`, 而且 `__cmd__xxx`
//! macro 是 private 不能跨 module. 所以 commands 必须放 lib.rs root 且不用 `pub`.
//! invoke_handler 在同 module 引用.
//!
//! 测试:
//! - UT 在 #[cfg(test)] mod (state 操作 + path 沙箱 + wiki)
//! - IT 在 tests/e2e_commands.rs (纯函数 pub(crate 路径解析 + 沙箱 + 文件读写)
//! - IT 在 tests/integration.rs (跨 crate, 真 Tauri managed state 模拟)

pub mod outline;
pub mod scm;
pub mod search;
pub mod terminal;
pub mod wiki;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ide_shell::render::web::{render as render_web, WebFrame};
use ide_shell::{App, AppConfig, Mode};

/// 单文件读取上限 (4 MB) — 保持轻量, 大文件/二进制请走 Shell 面板外部工具.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Tauri managed state wrapper — 单用户 demo, App 由 Tauri runtime 持有.
pub struct ShellState(pub Mutex<App>);

impl Default for ShellState {
    fn default() -> Self {
        Self(Mutex::new(App::new(AppConfig::default())))
    }
}

/// 当前项目根目录 (文件树 + 文件读写的沙箱边界). `None` = 尚未打开项目.
pub struct ProjectRoot(pub Arc<Mutex<Option<PathBuf>>>);

impl Default for ProjectRoot {
    /// 默认读 IDE_SHELL_DESKTOP_TEST_ROOT 环境变量 (演示/调试场景), 失败则 None.
    /// 不在 Default 里设默认 cwd, 因为用户期望 "首次启动是空 welcome, 显式 import 项目".
    fn default() -> Self {
        let root = std::env::var("IDE_SHELL_DESKTOP_TEST_ROOT")
            .ok()
            .map(PathBuf::from)
            .and_then(|p| p.canonicalize().ok())
            .filter(|p| p.is_dir());
        eprintln!("[ProjectRoot::default] test_root = {:?}", root);
        Self(Arc::new(Mutex::new(root)))
    }
}

impl ProjectRoot {
    pub fn get(&self) -> Option<PathBuf> {
        self.0.lock().ok().and_then(|g| g.clone())
    }

    pub fn set(&self, root: PathBuf) -> Result<PathBuf, String> {
        let canon = root
            .canonicalize()
            .map_err(|e| format!("无法打开目录 {}: {e}", root.display()))?;
        if !canon.is_dir() {
            return Err(format!("不是目录: {}", canon.display()));
        }
        let mut guard = self.0.lock().map_err(|e| e.to_string())?;
        *guard = Some(canon.clone());
        Ok(canon)
    }
}

/// Windows: 剥掉 verbatim 前缀 (`\\?\C:\...`) 跟盘符 root 比较.
#[cfg(windows)]
fn strip_verbatim(p: &Path) -> std::path::PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}
#[cfg(not(windows))]
fn strip_verbatim(p: &Path) -> std::path::PathBuf {
    p.to_path_buf()
}

/// canonicalize 一个路径; 若路径不存在 (write_file 写入新文件场景),
/// 退到 canonicalize 父目录, 再 join 原 basename.
fn safe_canonicalize(p: &Path) -> std::io::Result<PathBuf> {
    match p.canonicalize() {
        Ok(c) => Ok(c),
        Err(_) => {
            // 逐级向上找存在的祖先
            let mut cur = p.to_path_buf();
            let mut tail = std::path::PathBuf::new();
            loop {
                match cur.canonicalize() {
                    Ok(c) => return Ok(c.join(tail)),
                    Err(_) => {
                        let name = match cur.file_name() {
                            Some(n) => n.to_os_string(),
                            None => return Err(std::io::ErrorKind::NotFound.into()),
                        };
                        if tail.as_os_str().is_empty() {
                            tail = std::path::PathBuf::from(name);
                        } else {
                            tail = std::path::PathBuf::from(name).join(tail);
                        }
                        match cur.parent() {
                            Some(par) => cur = par.to_path_buf(),
                            None => return Err(std::io::ErrorKind::NotFound.into()),
                        }
                    }
                }
            }
        }
    }
}

/// 沙箱检查: `target` 必须位于 `root` 内 (含 root 本身). 两侧都 canonicalize.
/// Windows 上额外剥 `\\?\` verbatim 前缀, 否则 E:\Temp\xxx vs C:\Users\...\Temp\xxx
/// 这类 8.3 短路径展开 / verbatim 形式会让 starts_with 假拒绝.
/// target 不存在时 (write_file 新建文件场景) 退到父目录 + basename 重拼, 不会假拒绝.
pub fn path_within(root: &Path, target: &Path) -> bool {
    let (r, t) = match (safe_canonicalize(root), safe_canonicalize(target)) {
        (Ok(r), Ok(t)) => (r, t),
        _ => return false,
    };
    let (r2, t2) = (strip_verbatim(&r), strip_verbatim(&t));
    t2.starts_with(r2)
}

/// 校验请求路径在项目根内, 返回 canonicalized target. **纯函数**, IT/UT 可直接调.
pub fn resolve_in_project_pub(root: &Path, path: &str) -> Result<PathBuf, String> {
    let root =
        safe_canonicalize(root).map_err(|e| format!("项目根无效: {} ({e})", root.display()))?;
    let target = PathBuf::from(path);
    if !path_within_pub(&root, &target) {
        return Err(format!(
            "拒绝访问项目外路径: {path} (安全沙箱限制在项目根内)"
        ));
    }
    safe_canonicalize(&target).map_err(|e| format!("路径无效 {path}: {e}"))
}

/// root 已 canonicalized 后的快查 — IT 用.
pub fn path_within_pub(canon_root: &Path, target: &Path) -> bool {
    let Ok(t) = safe_canonicalize(target) else {
        return false;
    };
    let (r2, t2) = (strip_verbatim(canon_root), strip_verbatim(&t));
    t2.starts_with(r2)
}

#[derive(Debug, serde::Serialize)]
pub struct KeyResponse {
    pub frame: WebFrame,
    pub keep_going: bool,
}

impl KeyResponse {
    fn from_app(state: &tauri::State<'_, ShellState>) -> Result<Self, String> {
        let app = state.0.lock().map_err(|e| e.to_string())?;
        Ok(Self {
            frame: render_web(&app),
            keep_going: !app.should_quit(),
        })
    }
}

/// `list_dir` 返回的目录项.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// `read_file` 返回的文件内容.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileContent {
    pub name: String,
    pub path: String,
    pub content: String,
}

// ===== Tauri commands =====
// 不写 `pub` (Tauri 2.12 macro private 限制), invoke_handler 在同 module 引用即可.

#[tauri::command]
fn frame(state: tauri::State<'_, ShellState>) -> Result<WebFrame, String> {
    let app = state.0.lock().map_err(|e| e.to_string())?;
    Ok(render_web(&app))
}

#[tauri::command]
fn reset(state: tauri::State<'_, ShellState>) -> Result<WebFrame, String> {
    let mut app = state.0.lock().map_err(|e| e.to_string())?;
    *app = App::new(AppConfig::default());
    Ok(render_web(&app))
}

#[tauri::command]
fn key(
    state: tauri::State<'_, ShellState>,
    code: String,
    modifiers: Vec<String>,
) -> Result<KeyResponse, String> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let kc = match code.as_str() {
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Tab" => KeyCode::Tab,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        other => {
            if let Some(rest) = other.strip_prefix("Char:") {
                if let Some(c) = rest.chars().next() {
                    KeyCode::Char(c)
                } else {
                    return KeyResponse::from_app(&state);
                }
            } else {
                return KeyResponse::from_app(&state);
            }
        }
    };

    let mut mods = KeyModifiers::NONE;
    for m in &modifiers {
        match m.as_str() {
            "CONTROL" | "ctrl" => mods |= KeyModifiers::CONTROL,
            "ALT" | "alt" => mods |= KeyModifiers::ALT,
            "SHIFT" | "shift" => mods |= KeyModifiers::SHIFT,
            _ => {}
        }
    }

    let event = KeyEvent::new(kc, mods);
    let mut app = state.0.lock().map_err(|e| e.to_string())?;
    app.on_key(event);
    let frame = render_web(&app);
    let keep_going = !app.should_quit();
    Ok(KeyResponse { frame, keep_going })
}

#[tauri::command]
fn mouse(state: tauri::State<'_, ShellState>, button: String) -> Result<WebFrame, String> {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    let mb = match button.as_str() {
        "Right" => MouseButton::Right,
        _ => MouseButton::Left,
    };
    let event = MouseEvent {
        kind: MouseEventKind::Down(mb),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let mut app = state.0.lock().map_err(|e| e.to_string())?;
    app.on_mouse(event);
    Ok(render_web(&app))
}

#[tauri::command]
fn kernel_banner(state: tauri::State<'_, ShellState>) -> Result<String, String> {
    let app = state.0.lock().map_err(|e| e.to_string())?;
    Ok(app.kernel_banner().to_string())
}

#[tauri::command]
fn get_mode(state: tauri::State<'_, ShellState>) -> Result<String, String> {
    let app = state.0.lock().map_err(|e| e.to_string())?;
    Ok(match app.mode() {
        Mode::Normal => "Normal".into(),
        Mode::Insert => "Insert".into(),
        Mode::Command => "Command".into(),
    })
}

// ===== 项目导入 / 文件树 / 文件读写 (ULYS-191 §5 Cursor 风格 UI) =====

/// 诊断: 写一行到 IDE_LOG_FILE 文件 (默认 %TEMP%/ide-shell-desktop-diag.log)
/// 桌面 GUI 没 console, 但 eprintln 用户看不到, 所以 log 到文件.
pub fn diag_log(msg: &str) {
    eprintln!("[ide-diag] {}", msg);
    let path = std::env::var("IDE_LOG_FILE").unwrap_or_else(|_| {
        let mut p = std::env::temp_dir();
        p.push("ide-shell-desktop-diag.log");
        p.to_string_lossy().into_owned()
    });
    // eprintln 调试 fallback: 写完先确认文件被打开了
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| {
            use std::io::Write;
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            writeln!(f, "[{ts}] {}", msg)
        });
    if result.is_err() {
        eprintln!("[ide-diag] failed to write to {}: {:?}", path, result.err());
    } else {
        eprintln!("[ide-diag] wrote to {}", path);
    }
}

/// 打开项目 — 接受前端传来的路径.
///   * web mode: 前端走 `window.showDirectoryPicker` (WebView2 File System Access API)
///     或手输路径, 然后调 `open_project(path)`.
///   * 这个 command 保留作为 rfd 兜底: 当 webview2 拒绝 / 旧版没 FSA API 时,
///     仍能弹原生 Win32 dialog. 但已不在主流程中调用.
///   * 如果设了 IDE_SHELL_DESKTOP_TEST_ROOT env, 直接用它 (测试/演示场景).
#[tauri::command]
async fn pick_folder(state: tauri::State<'_, ProjectRoot>) -> Result<Option<String>, String> {
    diag_log(&format!(
        "[pick_folder] invoked, current root = {:?}",
        state.get()
    ));
    // 弹原生 Windows 文件夹选择对话框 (rfd 0.15). 这是用户最自然的体验 —
    // 点 toolbar「打开文件夹」按钮 → 弹 OS modal Explorer-style dialog 选文件夹.
    // rfd dialog 是 OS modal, 阻塞直到用户选完或取消. 不需要前端超时 (前端 30s race 是兜底).
    // 如果设了 IDE_SHELL_DESKTOP_TEST_ROOT env, 直接用它 (测试/演示场景, 不弹 dialog).
    if let Ok(env_root) = std::env::var("IDE_SHELL_DESKTOP_TEST_ROOT") {
        if let Ok(canon) = state.set(PathBuf::from(&env_root)) {
            diag_log(&format!(
                "[pick_folder] using IDE_SHELL_DESKTOP_TEST_ROOT = {:?}",
                canon
            ));
            return Ok(Some(canon.to_string_lossy().into_owned()));
        }
    }
    diag_log("[pick_folder] calling rfd native dialog");
    let root_arc = Arc::clone(&state.0);
    // rfd dialog 是 OS modal — 用户操作完 (选文件夹 / 取消) 才返回.
    // 用 spawn_blocking + 60 分钟上限 (实际由用户操作决定, 这只是保底防 hang).
    let (tx, rx) = std::sync::mpsc::channel::<Option<PathBuf>>();
    std::thread::spawn(move || {
        let r = rfd::FileDialog::new()
            .set_title("导入项目 — 选择文件夹")
            .pick_folder();
        let _ = tx.send(r);
    });
    let picked = match rx.recv_timeout(std::time::Duration::from_secs(3600)) {
        Ok(p) => p,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            diag_log("[pick_folder] rfd timed out after 1h — returning None");
            None
        }
        Err(e) => {
            diag_log(&format!("[pick_folder] rfd channel error: {e}"));
            None
        }
    };
    diag_log(&format!(
        "[pick_folder] rfd returned: {:?}",
        picked.as_ref().map(|p| p.display().to_string())
    ));
    let Some(picked) = picked else {
        return Ok(None);
    };
    let canon = {
        let pr = ProjectRoot(root_arc);
        pr.set(picked)?
    };
    Ok(Some(canon.to_string_lossy().into_owned()))
}

/// 直接以路径打开项目 (最近项目列表复用), 返回 canonicalized 根路径.
#[tauri::command]
fn open_project(state: tauri::State<'_, ProjectRoot>, path: String) -> Result<String, String> {
    let canon = state.set(PathBuf::from(&path))?;
    Ok(canon.to_string_lossy().into_owned())
}

/// 列目录 — 目录在前, 文件在后, 各自按不区分大小写排序. 限项目根内.
/// 列目录 — 目录在前, 文件在后, 各自按不区分大小写排序. 限项目根内.
/// **纯函数** — 不依赖 tauri::State, IT/UT 可直接调.
pub fn list_dir_pub(root: &Path, path: &str) -> Result<Vec<DirEntry>, String> {
    let target = resolve_in_project_pub(root, path)?;
    if !target.is_dir() {
        return Err(format!("不是目录: {path}"));
    }
    let mut entries: Vec<DirEntry> = std::fs::read_dir(&target)
        .map_err(|e| format!("读取目录失败 {path}: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| {
            let p = e.path();
            DirEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                path: p.to_string_lossy().into_owned(),
                is_dir: p.is_dir(),
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[tauri::command]
fn list_dir(state: tauri::State<'_, ProjectRoot>, path: String) -> Result<Vec<DirEntry>, String> {
    let root = state
        .get()
        .ok_or_else(|| "尚未打开项目 — 先点「打开文件夹」导入项目".to_string())?;
    list_dir_pub(&root, &path)
}

/// 读文件 — 限项目根内, UTF-8 文本, ≤ 4MB.
/// **纯函数** — 不依赖 tauri::State, IT/UT 可直接调.
pub fn read_file_pub(root: &Path, path: &str) -> Result<FileContent, String> {
    let target = resolve_in_project_pub(root, path)?;
    if !target.is_file() {
        return Err(format!("不是文件: {path}"));
    }
    let size = std::fs::metadata(&target)
        .map_err(|e| format!("读取元数据失败: {e}"))?
        .len();
    if size > MAX_FILE_BYTES {
        return Err(format!(
            "文件过大 ({size} bytes > {MAX_FILE_BYTES} bytes) — 保持轻量, 请用外部工具处理"
        ));
    }
    let content = std::fs::read_to_string(&target)
        .map_err(|e| format!("读取失败 (仅支持 UTF-8 文本): {e}"))?;
    Ok(FileContent {
        name: target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string()),
        path: target.to_string_lossy().into_owned(),
        content,
    })
}

#[tauri::command]
fn read_file(state: tauri::State<'_, ProjectRoot>, path: String) -> Result<FileContent, String> {
    let root = state
        .get()
        .ok_or_else(|| "尚未打开项目 — 先点「打开文件夹」导入项目".to_string())?;
    read_file_pub(&root, &path)
}

/// 写文件 — 限项目根内. 返回写入字节数.
/// **纯函数** — 不依赖 tauri::State, IT/UT 可直接调.
pub fn write_file_pub(root: &Path, path: &str, content: &str) -> Result<usize, String> {
    let target = resolve_in_project_pub(root, path)?;
    std::fs::write(&target, content.as_bytes()).map_err(|e| format!("写入失败 {path}: {e}"))?;
    Ok(content.len())
}

#[tauri::command]
fn write_file(
    state: tauri::State<'_, ProjectRoot>,
    path: String,
    content: String,
) -> Result<usize, String> {
    let root = state
        .get()
        .ok_or_else(|| "尚未打开项目 — 先点「打开文件夹」导入项目".to_string())?;
    write_file_pub(&root, &path, &content)
}

/// 完整内置 wiki — 与 CLI `--help` 同源 (main.rs 打印同一常量).
#[tauri::command]
fn help_wiki() -> String {
    APP_WIKI.to_string()
}

pub const APP_WIKI: &str = wiki::APP_WIKI;

/// CLI/应用内共用的 wiki 文本访问器.
pub fn help_wiki_text() -> &'static str {
    APP_WIKI
}

/// Tauri app entry — 注册 state + commands + 启动.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 强制 tauri::generate_context! 重展开 — cargo proc-macro 按 lib.rs 改时重生成 token stream.
    // 注意: 这只重 build 这一条函数. assets embed 走 build script (tauri-build) — 那条链通过
    // conf + dist mtime 触发. build script 走 frontendDist + cargo:rerun-if-changed.
    let _ = std::env::current_dir();
    tauri::Builder::default()
        .manage(ShellState::default())
        .manage(ProjectRoot::default())
        .manage(terminal::TerminalState::new())
        .invoke_handler(tauri::generate_handler![
            frame,
            reset,
            key,
            mouse,
            kernel_banner,
            get_mode,
            pick_folder,
            open_project,
            list_dir,
            read_file,
            write_file,
            help_wiki,
            terminal::terminal_create,
            terminal::terminal_input,
            terminal::terminal_close,
            terminal::terminal_list,
            scm::scm_status,
            scm::scm_stage,
            scm::scm_unstage,
            scm::scm_diff,
            scm::scm_commit,
            scm::scm_log,
            search::search_cmd,
            outline::outline,
        ])
        .setup(|_app| {
            eprintln!(
                "ide-shell-desktop starting (tauri {}, target {})",
                env!("CARGO_PKG_VERSION"),
                std::env::var("TAURI_TARGET_TRIPLE").unwrap_or_else(|_| "unknown".into()),
            );
            diag_log(&format!(
                "ide-shell-desktop setup: tauri={}, target={}",
                env!("CARGO_PKG_VERSION"),
                std::env::var("TAURI_TARGET_TRIPLE").unwrap_or_else(|_| "unknown".into()),
            ));
            diag_log(&format!(
                "ide-shell-desktop env: IDE_SHELL_DESKTOP_TEST_ROOT={:?}, IDE_LOG_FILE={:?}",
                std::env::var("IDE_SHELL_DESKTOP_TEST_ROOT").ok(),
                std::env::var("IDE_LOG_FILE").ok(),
            ));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ide-shell-desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_state() -> ShellState {
        ShellState::default()
    }

    #[test]
    fn test_initial_state_is_normal() {
        let s = new_state();
        let app = s.0.lock().unwrap();
        assert_eq!(app.mode(), Mode::Normal);
        assert_eq!(app.buffer().as_str(), "");
    }

    #[test]
    fn test_reset_restores_initial() {
        let mut s = new_state();
        {
            let mut app = s.0.lock().unwrap();
            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
            app.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
            for c in "junk".chars() {
                app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            }
        }
        s.0 = Mutex::new(App::new(AppConfig::default()));
        let app = s.0.lock().unwrap();
        assert_eq!(app.mode(), Mode::Normal);
        assert_eq!(app.buffer().as_str(), "");
    }

    #[test]
    fn test_kernel_banner_contains_kernel_core() {
        let s = new_state();
        let app = s.0.lock().unwrap();
        let banner = app.kernel_banner();
        assert!(banner.contains("ide-kernel-core"), "got: {banner}");
    }

    #[test]
    fn test_mode_serializes_as_expected() {
        let s = new_state();
        let app = s.0.lock().unwrap();
        let mode_str = match app.mode() {
            Mode::Normal => "Normal",
            Mode::Insert => "Insert",
            Mode::Command => "Command",
        };
        assert_eq!(mode_str, "Normal");
    }

    #[test]
    fn test_shell_state_send_sync() {
        fn assert_send<T: Send + Sync>() {}
        assert_send::<ShellState>();
        assert_send::<ProjectRoot>();
    }

    #[test]
    fn test_path_within_root() {
        let root = std::env::temp_dir();
        let inside = root.join("ide-shell-desktop-test-inside.txt");
        std::fs::write(&inside, "x").unwrap();
        assert!(path_within(&root, &inside));
        // 根自身也算 within (start_with 自反)
        assert!(path_within(&root, &root));
        std::fs::remove_file(&inside).ok();
    }

    #[test]
    fn test_path_within_rejects_outside() {
        let root = std::env::temp_dir().join("ide-shell-desktop-root-x");
        std::fs::create_dir_all(&root).unwrap();
        let outside = std::env::temp_dir().join("ide-shell-desktop-outside-y.txt");
        std::fs::write(&outside, "x").unwrap();
        assert!(!path_within(&root, &outside));
        // 不存在的路径但父目录在 root 内 → canonicalize 退到父目录后仍属于 root
        let in_existing_parent = root.join("no/such/file");
        assert!(path_within(&root, &in_existing_parent));
        std::fs::remove_file(&outside).ok();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_project_root_default_empty() {
        let pr = ProjectRoot::default();
        assert!(pr.get().is_none());
    }

    #[test]
    fn test_project_root_set_and_get() {
        let dir = std::env::temp_dir().join("ide-shell-desktop-prj-set");
        std::fs::create_dir_all(&dir).unwrap();
        let pr = ProjectRoot::default();
        let canon = pr.set(dir.clone()).unwrap();
        assert!(canon.is_dir());
        assert_eq!(pr.get().unwrap(), canon);
        // 非目录 → Err
        let file = dir.join("f.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(pr.set(file).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_wiki_covers_required_sections() {
        let w = help_wiki_text();
        assert!(w.len() > 2000, "wiki 应足够全, got {} bytes", w.len());
        for key in [
            "简介",
            "安装与启动",
            "界面布局",
            "项目导入",
            "键位表",
            "Shell 面板",
            "鼠标悬停说明",
            "架构与源码导览",
            "FAQ",
            "版本",
        ] {
            assert!(w.contains(key), "wiki 缺 section: {key}");
        }
        // 关键命令都要在 wiki 里可查
        for cmd in [
            ":w", ":q", ":wq", ":e", ":help", "--help", "Ctrl+S", "Ctrl+`",
        ] {
            assert!(w.contains(cmd), "wiki 缺命令说明: {cmd}");
        }
    }
}
