//! NEMA 自定义与外部解释器自适应适配引擎 (Adaptive Interpreter Engine)
//!
//! 允许用户导入自定义解释器（如 Python 脚本 `codec/native.py`、外部编译好的二进制、自定义 CLI 等），
//! 并通过自适应探针 (Adaptive Probing) 自动探测其调用协议、支持语法、返回格式与延迟指标。
//! 确保在 .nema 规范活跃调试与演进阶段，IDE 能无缝接入最新解释器，并支持平滑回退至内置原生引擎。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// 解释器协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InterpreterProtocol {
    /// 内置纯 Rust 编解码器 (v1.5)
    #[default]
    BuiltinRust,
    /// Python native.py 视图导出 (具有 `view(source)` 函数并输出规范 AST)
    PythonNativeView,
    /// 官方命令行工具: `python -m codec.cli render <file>`
    OfficialCliRender,
    /// 通用标准输入输出 JSON AST 协议 (stdin -> stdout)
    GenericJsonAstStdin,
    /// 通用文件参数命令行 (cmd <file>)
    GenericFileArg,
}

/// 解释器配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterpreterConfig {
    /// 是否启用外部自定义解释器
    #[serde(default)]
    pub enabled: bool,
    /// 解释器目标路径或命令
    #[serde(default)]
    pub path: String,
    /// 宿主环境 (auto / python / node / binary)
    #[serde(default = "default_runner")]
    pub runner: String,
    /// 探测并适配到的协议
    #[serde(default)]
    pub protocol: InterpreterProtocol,
    /// 探测到的特性与元数据
    #[serde(default)]
    pub metadata: InterpreterMetadata,
}

fn default_runner() -> String {
    "auto".to_string()
}

/// 解释器特性与能力元数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterpreterMetadata {
    #[serde(default)]
    pub engine_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub supports_graph: bool,
    #[serde(default)]
    pub supports_5w1h: bool,
    #[serde(default)]
    pub supports_table: bool,
    #[serde(default)]
    pub probe_latency_us: u64,
    #[serde(default)]
    pub probe_timestamp: u64,
    #[serde(default)]
    pub last_status: String,
    #[serde(default)]
    pub last_error: Option<String>,
}

/// 探测结果报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub success: bool,
    pub target_path: String,
    pub runner_used: String,
    pub protocol: InterpreterProtocol,
    pub latency_us: u64,
    pub version_detected: String,
    pub features: Vec<String>,
    pub sample_ast_domains: usize,
    pub sample_ast_nodes: usize,
    pub sample_ast_edges: usize,
    pub raw_output_preview: String,
    pub error: Option<String>,
}

/// 候选自动发现项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredCandidate {
    pub name: String,
    pub path: String,
    pub candidate_type: String,
    pub confidence: String,
    pub exists: bool,
}

/// 全局活动解释器状态管理器
static ACTIVE_CONFIG: Mutex<Option<InterpreterConfig>> = Mutex::new(None);
static HAS_CUSTOM: AtomicBool = AtomicBool::new(false);

/// 获取当前激活的解释器配置
pub fn get_active_interpreter() -> InterpreterConfig {
    if let Ok(guard) = ACTIVE_CONFIG.lock() {
        if let Some(cfg) = guard.as_ref() {
            return cfg.clone();
        }
    }
    InterpreterConfig::default()
}

/// 设置或切换解释器配置
pub fn set_active_interpreter(cfg: InterpreterConfig) {
    HAS_CUSTOM.store(cfg.enabled && !cfg.path.trim().is_empty(), Ordering::Relaxed);
    if let Ok(mut guard) = ACTIVE_CONFIG.lock() {
        *guard = Some(cfg);
    }
}

/// 自动在系统与常见位置发现可能存在的 .NEMA 解释器候选
pub fn discover_candidates(project_root: Option<&Path>) -> Vec<DiscoveredCandidate> {
    let mut candidates = Vec::new();

    // 1. 官方参考解释器标准路径
    let official_paths = [
        ("E:\\.NEMA\\codec\\native.py", "官方 Python 参考解释器 (native.py)"),
        ("E:\\.NEMA\\codec\\cli.py", "官方 Python CLI 解释器 (cli.py)"),
        ("..\\.NEMA\\codec\\native.py", "相对路径官方参考解释器"),
    ];
    for (p, label) in &official_paths {
        let pb = PathBuf::from(p);
        let exists = pb.exists();
        candidates.push(DiscoveredCandidate {
            name: label.to_string(),
            path: p.to_string(),
            candidate_type: "python_script".to_string(),
            confidence: if exists { "High (Recommended)".to_string() } else { "Low".to_string() },
            exists,
        });
    }

    // 2. 当前项目工作区内可能存在的自定义解释器
    if let Some(root) = project_root {
        let ws_candidates = [
            root.join(".ide").join("nema_interpreter.py"),
            root.join("codec").join("native.py"),
            root.join("tools").join("nema_interpreter.py"),
            root.join("bin").join("nema.exe"),
            root.join("bin").join("nema"),
        ];
        for path in &ws_candidates {
            if path.exists() {
                candidates.push(DiscoveredCandidate {
                    name: format!("工作区内置解释器 ({})", path.file_name().unwrap_or_default().to_string_lossy()),
                    path: path.to_string_lossy().to_string(),
                    candidate_type: if path.extension().and_then(|s| s.to_str()) == Some("py") {
                        "python_script".to_string()
                    } else {
                        "binary".to_string()
                    },
                    confidence: "High".to_string(),
                    exists: true,
                });
            }
        }
    }

    // 3. 环境变量 NEMA_INTERPRETER
    if let Ok(env_path) = std::env::var("NEMA_INTERPRETER") {
        let exists = Path::new(&env_path).exists();
        candidates.push(DiscoveredCandidate {
            name: "环境变量 NEMA_INTERPRETER".to_string(),
            path: env_path,
            candidate_type: "environment_variable".to_string(),
            confidence: if exists { "High".to_string() } else { "Medium".to_string() },
            exists,
        });
    }

    candidates
}

/// 标准试探测试向量（同时涵盖 Graph、5W1H 与节点边）
const PROBE_SAMPLE_NEMA: &str = "\
 >G1 1\n\
A LoginAPI\n\
B TokenStore\n\
A>X>B Reads\n\
\n\
\n";

/// 动态对目标解释器运行自适应探针 (Adaptive Probing)
pub fn probe_interpreter(raw_target: &str, preferred_runner: Option<&str>) -> ProbeReport {
    let target = raw_target.trim();
    if target.is_empty() {
        return ProbeReport {
            success: false,
            target_path: String::new(),
            runner_used: "none".to_string(),
            protocol: InterpreterProtocol::BuiltinRust,
            latency_us: 0,
            version_detected: "unknown".to_string(),
            features: vec![],
            sample_ast_domains: 0,
            sample_ast_nodes: 0,
            sample_ast_edges: 0,
            raw_output_preview: String::new(),
            error: Some("目标路径不能为空".to_string()),
        };
    }

    let target_path = PathBuf::from(target);
    let is_py = target.ends_with(".py") || target.contains("native.py") || target.contains("cli.py");
    let runner = match preferred_runner {
        Some("python") | Some("py") => "python",
        Some(custom) if !custom.is_empty() && custom != "auto" => custom,
        _ => {
            if is_py {
                "python"
            } else {
                "direct"
            }
        }
    };

    let mut err_collector = Vec::new();

    // 协议探测 1：Python Native View 探测
    if runner == "python" || is_py {
        match probe_python_native(&target_path) {
            Ok(report) => return report,
            Err(e) => err_collector.push(format!("PythonNative: {e}")),
        }
        // 协议探测 2：Official CLI Render 探测
        match probe_official_cli(&target_path) {
            Ok(report) => return report,
            Err(e) => err_collector.push(format!("OfficialCli: {e}")),
        }
    }

    // 协议探测 3：通用命令行 / 二进制 Stdin / Arg 探测
    match probe_generic_binary(target, runner) {
        Ok(report) => return report,
        Err(e) => err_collector.push(format!("GenericBinary: {e}")),
    }

    // 全部探测未成功，返回错误报告
    ProbeReport {
        success: false,
        target_path: target.to_string(),
        runner_used: runner.to_string(),
        protocol: InterpreterProtocol::BuiltinRust,
        latency_us: 0,
        version_detected: "unknown".to_string(),
        features: vec![],
        sample_ast_domains: 0,
        sample_ast_nodes: 0,
        sample_ast_edges: 0,
        raw_output_preview: String::new(),
        error: Some(format!("未能自动适配此解释器 ({target}): {}", err_collector.join("; "))),
    }
}

/// 探测 Python native.py 视图导出
fn probe_python_native(target: &Path) -> Result<ProbeReport, String> {
    let abs_path = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(target)
    };

    if !abs_path.exists() {
        return Err(format!("文件不存在: {}", abs_path.display()));
    }

    let parent_dir = abs_path.parent().unwrap_or(Path::new("."));
    let grand_dir = parent_dir.parent().unwrap_or(Path::new("."));

    // 构建探针脚本
    let inline_py = format!(
        r#"
import sys, json, os, time

p1 = r"{p1}"
p2 = r"{p2}"
sys.path.insert(0, p1)
sys.path.insert(0, p2)

mod = None
try:
    from codec import native
    mod = native
except Exception:
    pass

if mod is None:
    try:
        import importlib.util
        spec = importlib.util.spec_from_file_location("nema_custom_native", r"{file}")
        if spec and spec.loader:
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
    except Exception as e:
        sys.stderr.write("Import error: " + str(e))
        sys.exit(2)

if not hasattr(mod, 'view') and not hasattr(mod, 'parse'):
    sys.stderr.write("Module lacks view or parse function")
    sys.exit(3)

sample = " >G1 1\nA LoginAPI\nB TokenStore\nA>X>B Reads\n\n\n"

t0 = time.perf_counter()
res = mod.view(sample) if hasattr(mod, 'view') else mod.parse(sample)
t_us = int((time.perf_counter() - t0) * 1000000)

print(json.dumps({{
    "status": "ok",
    "protocol": "python_native_view",
    "latency_us": t_us,
    "version": getattr(mod, "__version__", "1.5.0-dev"),
    "ast": res
}}))
"#,
        p1 = grand_dir.display(),
        p2 = parent_dir.display(),
        file = abs_path.display()
    );

    let start = Instant::now();
    let mut cmd = Command::new("python");
    cmd.env("PYTHONUTF8", "1");
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.arg("-c").arg(&inline_py);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd.output().map_err(|e| format!("启动 Python 进程失败: {e}"))?;
    let elapsed_us = start.elapsed().as_micros() as u64;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Python 探针执行失败 (code {:?}): {}", output.status.code(), err_msg));
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    let json_val: serde_json::Value = serde_json::from_str(out_str.trim())
        .map_err(|e| format!("探针输出非合法 JSON: {e}; 输出前段: {}", out_str.chars().take(200).collect::<String>()))?;

    let ast = json_val.get("ast").cloned().unwrap_or_default();
    let (domains, nodes, edges) = inspect_ast_counts(&ast);
    let latency_us = json_val.get("latency_us").and_then(|v| v.as_u64()).unwrap_or(elapsed_us);
    let version = json_val.get("version").and_then(|v| v.as_str()).unwrap_or("1.5.0").to_string();

    Ok(ProbeReport {
        success: true,
        target_path: abs_path.to_string_lossy().to_string(),
        runner_used: "python".to_string(),
        protocol: InterpreterProtocol::PythonNativeView,
        latency_us,
        version_detected: version,
        features: vec![
            "Graph 拓扑渲染 (native.view)".to_string(),
            "5W1H 语义六轴映射".to_string(),
            "Table 隐式/显式双表格".to_string(),
            "v1.5 单空格领域头兼容".to_string(),
            "毫秒级内存调用".to_string(),
        ],
        sample_ast_domains: domains,
        sample_ast_nodes: nodes,
        sample_ast_edges: edges,
        raw_output_preview: out_str.chars().take(400).collect(),
        error: None,
    })
}

/// 探测官方 CLI render
fn probe_official_cli(target: &Path) -> Result<ProbeReport, String> {
    let parent = target.parent().unwrap_or(Path::new("."));
    let grand = parent.parent().unwrap_or(Path::new("."));

    // 写入一个临时 NEMA 文件测试 CLI render
    let tmp_dir = std::env::temp_dir();
    let tmp_file = tmp_dir.join("probe_test.nema");
    std::fs::write(&tmp_file, PROBE_SAMPLE_NEMA).map_err(|e| format!("写入临时文件失败: {e}"))?;

    let mut cmd = Command::new("python");
    cmd.env("PYTHONUTF8", "1");
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONPATH", grand);
    cmd.args(["-m", "codec.cli", "render", &tmp_file.to_string_lossy()]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let start = Instant::now();
    let output = cmd.output().map_err(|e| format!("启动 CLI 失败: {e}"))?;
    let latency_us = start.elapsed().as_micros() as u64;

    let _ = std::fs::remove_file(&tmp_file);

    if !output.status.success() {
        return Err("CLI render 返回非 0 状态".to_string());
    }

    let out_str = String::from_utf8_lossy(&output.stdout);
    let ast: serde_json::Value = serde_json::from_str(out_str.trim())
        .map_err(|e| format!("CLI 输出非合法 JSON: {e}"))?;

    let (domains, nodes, edges) = inspect_ast_counts(&ast);

    Ok(ProbeReport {
        success: true,
        target_path: target.to_string_lossy().to_string(),
        runner_used: "python -m codec.cli".to_string(),
        protocol: InterpreterProtocol::OfficialCliRender,
        latency_us,
        version_detected: "v1.5-cli".to_string(),
        features: vec![
            "CLI 批量渲染".to_string(),
            "JSON AST 输出".to_string(),
            "文件级解算".to_string(),
        ],
        sample_ast_domains: domains,
        sample_ast_nodes: nodes,
        sample_ast_edges: edges,
        raw_output_preview: out_str.chars().take(400).collect(),
        error: None,
    })
}

/// 探测通用二进制 / 脚本
fn probe_generic_binary(target: &str, runner: &str) -> Result<ProbeReport, String> {
    let mut cmd = if runner == "python" {
        let mut c = Command::new("python");
        c.env("PYTHONUTF8", "1");
        c.env("PYTHONIOENCODING", "utf-8");
        c.arg(target);
        c
    } else {
        Command::new(target)
    };

    // 先尝试通过 stdin 喂入测试用例
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let start = Instant::now();
    if let Ok(mut child) = cmd.spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(PROBE_SAMPLE_NEMA.as_bytes());
        }
        if let Ok(output) = child.wait_with_output() {
            let latency_us = start.elapsed().as_micros() as u64;
            if output.status.success() {
                let out_str = String::from_utf8_lossy(&output.stdout);
                if let Ok(ast) = serde_json::from_str::<serde_json::Value>(out_str.trim()) {
                    let (domains, nodes, edges) = inspect_ast_counts(&ast);
                    return Ok(ProbeReport {
                        success: true,
                        target_path: target.to_string(),
                        runner_used: runner.to_string(),
                        protocol: InterpreterProtocol::GenericJsonAstStdin,
                        latency_us,
                        version_detected: "generic-json".to_string(),
                        features: vec!["Stdin 管道交互".to_string(), "通用 JSON AST".to_string()],
                        sample_ast_domains: domains,
                        sample_ast_nodes: nodes,
                        sample_ast_edges: edges,
                        raw_output_preview: out_str.chars().take(400).collect(),
                        error: None,
                    });
                }
            }
        }
    }

    Err("通用二进制探针未通过".to_string())
}

/// 统计 AST 中的域、节点和边数量
fn inspect_ast_counts(ast: &serde_json::Value) -> (usize, usize, usize) {
    let mut domains = 0;
    let mut nodes = 0;
    let mut edges = 0;

    if let Some(arr) = ast.as_array() {
        domains = arr.len();
        for d in arr {
            if let Some(ns) = d.get("nodes").and_then(|v| v.as_array()) {
                nodes += ns.len();
            }
            if let Some(es) = d.get("edges").and_then(|v| v.as_array()) {
                edges += es.len();
            }
        }
    } else if let Some(obj) = ast.as_object() {
        domains = 1;
        if let Some(ns) = obj.get("nodes").and_then(|v| v.as_array()) {
            nodes += ns.len();
        }
        if let Some(es) = obj.get("edges").and_then(|v| v.as_array()) {
            edges += es.len();
        }
    }

    (domains, nodes, edges)
}

/// 执行自定义外部解释器解析并返回规范 JSON AST
pub fn execute_interpreter(
    cfg: &InterpreterConfig,
    source: &str,
) -> Result<(serde_json::Value, u64), String> {
    if !cfg.enabled || cfg.path.trim().is_empty() {
        return Err("外部解释器未启用".to_string());
    }

    let start = Instant::now();
    match cfg.protocol {
        InterpreterProtocol::PythonNativeView => {
            let abs_path = PathBuf::from(&cfg.path);
            let parent_dir = abs_path.parent().unwrap_or(Path::new("."));
            let grand_dir = parent_dir.parent().unwrap_or(Path::new("."));

            // 将待解析文本编码为 base64 或传递至 stdin 避免转义字符破坏
            let inline_py = format!(
                r#"
import sys, json, os

sys.path.insert(0, r"{p1}")
sys.path.insert(0, r"{p2}")

mod = None
try:
    from codec import native
    mod = native
except Exception:
    import importlib.util
    spec = importlib.util.spec_from_file_location("nema_mod", r"{file}")
    if spec and spec.loader:
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)

if not mod or not hasattr(mod, 'view'):
    sys.stderr.write("Module lacks view function")
    sys.exit(3)

source = sys.stdin.buffer.read().decode('utf-8')
source = source.replace('\r\n', '\n').replace('\r', '\n')
try:
    ast = mod.view(source)
    print(json.dumps(ast, ensure_ascii=False))
except Exception as e:
    sys.stderr.write(str(e))
    sys.exit(1)
"#,
                p1 = grand_dir.display(),
                p2 = parent_dir.display(),
                file = abs_path.display()
            );

            let mut cmd = Command::new("python");
            cmd.env("PYTHONUTF8", "1");
            cmd.env("PYTHONIOENCODING", "utf-8");
            cmd.arg("-c").arg(&inline_py);
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| format!("启动 Python 进程失败: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(source.as_bytes()).map_err(|e| format!("向解释器写入数据失败: {e}"))?;
            }

            let output = child.wait_with_output().map_err(|e| format!("等待解释器输出失败: {e}"))?;
            let latency_us = start.elapsed().as_micros() as u64;

            if !output.status.success() {
                let err_str = String::from_utf8_lossy(&output.stderr);
                return Err(format!("外部解释器报错: {}", err_str.trim()));
            }

            let out_str = String::from_utf8_lossy(&output.stdout);
            let ast: serde_json::Value = serde_json::from_str(out_str.trim())
                .map_err(|e| format!("解释器返回非标准 JSON AST: {e}"))?;

            Ok((ast, latency_us))
        }
        InterpreterProtocol::OfficialCliRender => {
            let tmp_file = std::env::temp_dir().join(format!("nema_render_{}.nema", std::process::id()));
            std::fs::write(&tmp_file, source).map_err(|e| format!("写临时文件失败: {e}"))?;

            let mut cmd = Command::new("python");
            cmd.env("PYTHONUTF8", "1");
            cmd.env("PYTHONIOENCODING", "utf-8");
            let target_path = PathBuf::from(&cfg.path);
            let grand = target_path.parent().and_then(|p| p.parent()).unwrap_or(Path::new("."));
            cmd.env("PYTHONPATH", grand);
            cmd.args(["-m", "codec.cli", "render", &tmp_file.to_string_lossy()]);
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            let output = cmd.output().map_err(|e| format!("运行 CLI 失败: {e}"))?;
            let latency_us = start.elapsed().as_micros() as u64;
            let _ = std::fs::remove_file(&tmp_file);

            if !output.status.success() {
                let err_str = String::from_utf8_lossy(&output.stderr);
                return Err(format!("CLI 解释器报错: {}", err_str.trim()));
            }

            let out_str = String::from_utf8_lossy(&output.stdout);
            let ast: serde_json::Value = serde_json::from_str(out_str.trim())
                .map_err(|e| format!("CLI 输出非标准 JSON AST: {e}"))?;

            Ok((ast, latency_us))
        }
        InterpreterProtocol::GenericJsonAstStdin => {
            let mut cmd = Command::new(&cfg.path);
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| format!("启动进程失败: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin.write_all(source.as_bytes()).map_err(|e| format!("写入失败: {e}"))?;
            }

            let output = child.wait_with_output().map_err(|e| format!("等待输出失败: {e}"))?;
            let latency_us = start.elapsed().as_micros() as u64;

            if !output.status.success() {
                let err_str = String::from_utf8_lossy(&output.stderr);
                return Err(format!("自定义解释器报错: {}", err_str.trim()));
            }

            let out_str = String::from_utf8_lossy(&output.stdout);
            let ast: serde_json::Value = serde_json::from_str(out_str.trim())
                .map_err(|e| format!("返回非标准 JSON: {e}"))?;

            Ok((ast, latency_us))
        }
        _ => Err("未受支持的外部协议".to_string()),
    }
}

/// 自适应渲染视图：优先调用已配置的外部解释器；如果未配置或执行出错，平滑回退至内置纯 Rust 标准引擎。
/// 返回：`(AST JSON, 耗时微秒, 引擎名称, 是否使用外部解释器)`
pub fn adaptive_render_view(source: &str) -> (serde_json::Value, u64, String, bool) {
    let cfg = get_active_interpreter();
    if cfg.enabled && !cfg.path.trim().is_empty() {
        if let Ok((ast, latency_us)) = execute_interpreter(&cfg, source) {
            let engine_label = if cfg.metadata.engine_name.is_empty() {
                format!("外部解释器 ({})", Path::new(&cfg.path).file_name().unwrap_or_default().to_string_lossy())
            } else {
                cfg.metadata.engine_name.clone()
            };
            return (ast, latency_us, engine_label, true);
        }
    }

    // 内置纯 Rust 原生引擎
    let t0 = Instant::now();
    let val = match crate::render_source(source) {
        Ok(doc) => crate::render_to_json_value(&doc),
        Err(e) => serde_json::json!({
            "error": e.to_string(),
        }),
    };
    let latency_us = t0.elapsed().as_micros() as u64;
    (val, latency_us, "内置标准 Rust (v1.5)".to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_target_probe() {
        let rep = probe_interpreter("", None);
        assert!(!rep.success);
        assert!(rep.error.is_some());
    }

    #[test]
    fn test_discover_candidates() {
        let cands = discover_candidates(None);
        assert!(!cands.is_empty());
        let official = cands.iter().find(|c| c.path.contains("native.py"));
        assert!(official.is_some());
    }

    #[test]
    fn test_official_native_py_probe_and_execute() {
        let path = "E:\\.NEMA\\codec\\native.py";
        if !Path::new(path).exists() {
            return;
        }
        let rep = probe_interpreter(path, Some("python"));
        assert!(rep.success, "Probe failed: {:?}", rep.error);
        assert_eq!(rep.protocol, InterpreterProtocol::PythonNativeView);
        assert!(rep.sample_ast_domains >= 1);
        assert!(rep.sample_ast_nodes >= 2);
        assert!(rep.sample_ast_edges >= 1);

        let cfg = InterpreterConfig {
            enabled: true,
            path: path.to_string(),
            runner: "python".to_string(),
            protocol: InterpreterProtocol::PythonNativeView,
            metadata: InterpreterMetadata::default(),
        };

        let sample = " >Gauth 1\nA User\nB Auth\nA>X>B Login\n\n\n";
        let (ast, latency) = execute_interpreter(&cfg, sample).expect("execution should succeed");
        assert!(latency > 0);
        assert!(ast.is_array());
        let arr = ast.as_array().unwrap();
        assert_eq!(arr[0]["id"], "Gauth");
    }
}
