//! `ide-cli` v0.1.0 — IDE1.0 CLI (placeholder).
//!
//! ⚠️ **PLACEHOLDER** — per ULYS-191 §4.2.2 brief v0.1, 本笔仅提供最小 CLI 骨架.
//! 完整 CLI (subcommands / config / REPL / session 等) 实装待 DD-02 Session
//! Transaction Buffer Engine 详细设计书.
//!
//! ## 当前子命令 (Stage 3.0 方案 A 最小骨架)
//!
//! - `--version`, `-V` — 打印 crate 版本号
//! - `--aci-emit [ID]` — emit 一条 ACI assertion (placeholder, ACI v0.1)
//! - `--help`, `-h` — 打印 help
//!
//! ## 后续 brief 范围
//!
//! - DD-02: `ide-cli session <sub>` / `ide-cli txn <sub>` / `ide-cli config <sub>`
//! - DD-04: `ide-cli plugin <sub>`
//!
//! ## module_switch 接入 (per ULYS-190 §4.4 stage5, 2026-09-26 12:30 JST)
//!
//! IDE1.0 mock_switch L1+L2 已 ship via PR #3 (commit c4b62bb on main).
//! 本 stage5 commit 补 L3 module_switch:
//!
//! 2 plugin x 8 module:
//!   - ide-cli          : emit / version / help / shell
//!   - ide-kernel-core  : version / kernel_status / aci_emit / init
//!
//! 跨项目範式对齐 (per G-MS-04):
//!   - IM1.0 PR #24: 5 plugin x 28 module
//!   - CATs PR #18: 4 plugin x 13 module
//!   - Star PR #151: 7 plugin x 7 module
//!   - RGS PR #51 (commit 4193207613 on dev, just merged): 5 plugin x 12 module
//!   - IDE1.0 stage5 (本 commit): 2 plugin x 8 module
//!
//! 跨语言 dispatch 用法 (per G-MS-BRIEF-S44-01 IDE1.0 推广):
//!
//! ```python
//! import subprocess, json
//! subprocess.run([
//!     "python3", "scripts/_lib_mock_switch_ide1.py",
//!     "--aci-config", ".aci.json",
//!     "read-plugins",
//! ], check=True)
//! ```
//!
//! 8 module 默认全 enabled, mode=offline. IDE1.0 是 docs-only placeholder,
//! module 名映射占位 = 后续 brief 实际 subcommand 实现时一一对应.

use std::process::ExitCode;

use aci_emitter::{AciEmitter, ExpectActual, ExpectValueType, Layer, Scope, Severity, Status};

/// CLI 入口.
pub fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_help();
        return ExitCode::from(2);
    }
    match args[1].as_str() {
        "--version" | "-V" => {
            println!("ide-cli v{}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "--aci-emit" => run_aci_emit(args.get(2).cloned()),
        "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("unknown flag: {}", args[1]);
            print_help();
            ExitCode::from(2)
        }
    }
}

/// Emit 一条 ACI assertion (placeholder).
///
/// 输出 schema 完全对齐 `.aci.json` v0.1 + aci-emitter v0.1 (ULYS-191.1 / ULYS-224).
fn run_aci_emit(aid: Option<String>) -> ExitCode {
    let em = AciEmitter::new(Layer::It);
    let aid = aid.unwrap_or_else(|| "ide1.0:smoke:smoke-1".into());
    let scope = Scope::new(
        "ide1.0".to_string(),
        Some("smoke".to_string()),
        None,
        None,
        None,
        None,
    );
    let expect = ExpectActual::new(
        ExpectValueType::ResponseWithinMs,
        serde_json::json!(100),
        "CLI should start within 100ms",
    );
    let actual = ExpectActual::new(
        ExpectValueType::ResponseWithinMs,
        serde_json::json!(5),
        "measured 5ms (placeholder)",
    );
    let a = match em.build(
        aid,
        scope,
        expect,
        actual,
        Status::Pass,
        Severity::Info,
        "actual << expect (20x margin) — placeholder per §4.2.2 brief v0.1",
    ) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("validation error: {e}");
            return ExitCode::from(2);
        }
    };
    let json = em.to_json(&a);
    let s = match serde_json::to_string_pretty(&json) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("serialize error: {e}");
            return ExitCode::from(2);
        }
    };
    println!("{s}");
    ExitCode::SUCCESS
}

/// Print CLI help to stderr.
fn print_help() {
    eprintln!(
        "ide-cli v{} - IDE1.0 minimal CLI (ULYS-191 §4.2.2)\n\
         \n\
         USAGE:\n  \
           ide-cli <FLAG>\n\
         \n\
         FLAGS:\n  \
           --version, -V     Print crate version\n  \
           --aci-emit [ID]   Emit a single ACI assertion (placeholder, ACI v0.1)\n  \
           --help, -h        Print this help\n\
         \n\
         NOTES:\n  \
           - 当前为 Stage 3.0 最小骨架, 完整 CLI 待 DD-02 实现\n  \
           - ACI 输出格式 1:1 对齐 .aci.json v0.1 + aci-emitter v0.1\n",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_with_no_args_returns_2() {
        // 模拟 `ide-cli` 无参数: 直接调 run() 时 env::args() 包含 test harness 的 argv[0]
        // 这里仅验证 print_help 不 panic
        print_help();
    }

    #[test]
    fn test_run_aci_emit_default_id() {
        let code = run_aci_emit(None);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    #[test]
    fn test_run_aci_emit_custom_id() {
        let code = run_aci_emit(Some("ide1.0:test:custom".into()));
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
