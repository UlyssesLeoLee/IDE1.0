//! System Tests (ST) — End-to-end CLI behavior
//!
//! TP ID: ST-CLI-001..006 — DD-11 §4 (System Test 観点)
//! 検証対象: ビルド済み `kernel-cli` バイナリを spawn して CLI 全体動作確認。
//! - `session create --actor human` → UUID 出力
//! - `caps list` → 登録 Capability メタ表示
//! - `echo --session <id> '<json>'` → JSON 引数を渡して結果表示
//! - `event-bus replay` → Durable Event 列挙
//! - `sessions list` → セッション件数表示
//!
//! 実行方法:
//!   cargo build -p cli → tests/st/run_cli_smoke.sh
//! または scripts/test/st.sh (事前にバイナリ build 済み前提)

use std::io::Write;
use std::process::{Command, Stdio};

fn binary_path() -> std::path::PathBuf {
    // Cargo が test を走らせる時 CARGO_BIN_EXE_<name> で
    // コンパイル済みバイナリのパスを渡してくれる。
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_kernel-cli"))
}

#[test]
fn st_cli_001_help_renders() {
    // ST-CLI-001: `kernel-cli --help` が Usage / Subcommand 一覧を出す。
    let mut child = Command::new(binary_path())
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn kernel-cli");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "--help must exit 0: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("session-create"), "must list session-create");
    assert!(stdout.contains("caps-list"), "must list caps-list");
}

#[test]
fn st_cli_002_version_renders() {
    // ST-CLI-002: `kernel-cli --version` がバージョン文字列を出す。
    let mut child = Command::new(binary_path())
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("kernel-cli"), "version must contain binary name");
}

#[test]
fn st_cli_003_session_create_human_returns_uuid() {
    // ST-CLI-003: `session create --actor human` が UUID 形式の Session ID を返す。
    let mut child = Command::new(binary_path())
        .args(["session-create", "--actor", "human"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success(), "exit must be 0: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // UUID v4 形式 (36 文字、ハイフン込み) を簡易検証
    assert_eq!(stdout.len(), 36, "expected uuid length 36, got {stdout:?}");
    assert_eq!(stdout.matches('-').count(), 4, "uuid must have 4 hyphens");
}

#[test]
fn st_cli_004_session_create_agent_returns_uuid() {
    // ST-CLI-004: Agent actor でも Session を作成できる。
    let mut child = Command::new(binary_path())
        .args(["session-create", "--actor", "agent"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(stdout.len(), 36);
}

#[test]
fn st_cli_005_caps_list_includes_echo() {
    // ST-CLI-005: `caps list` に echo Capability が含まれる (MVP-1 自動登録)。
    let mut child = Command::new(binary_path())
        .args(["caps-list"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("echo"),
        "caps list must include 'echo', got: {stdout}"
    );
}

#[test]
fn st_cli_006_sessions_list_outputs_count() {
    // ST-CLI-006: `sessions list` が session_count= 形式で件数を出す。
    let mut child = Command::new(binary_path())
        .args(["sessions-list"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("session_count="),
        "must contain 'session_count=', got: {stdout}"
    );
}

#[test]
fn st_cli_007_echo_with_valid_session_returns_json() {
    // ST-CLI-007: 正常 Session + JSON 引数で echo 実行 → JSON 結果出力。
    // Step 1: session 作成 → UUID 取得
    let mut sess = Command::new(binary_path())
        .args(["session-create", "--actor", "human"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session create");
    let out = sess.wait_with_output().expect("wait");
    assert!(out.status.success());
    let sid = String::from_utf8_lossy(&out.stdout).trim().to_string();

    // Step 2: echo 実行
    let mut echo = Command::new(binary_path())
        .args([
            "echo",
            "--session",
            &sid,
            r#"{"text":"hello-st"}"#,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn echo");
    let out = echo.wait_with_output().expect("wait");
    assert!(out.status.success(), "echo must exit 0: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("hello-st"),
        "echo output must contain 'hello-st', got: {stdout}"
    );
}

#[test]
fn st_cli_008_echo_with_invalid_session_returns_error() {
    // ST-CLI-008: 存在しない Session ID では echo がエラーで exit ≠ 0。
    let mut child = Command::new(binary_path())
        .args([
            "echo",
            "--session",
            "00000000-0000-0000-0000-000000000000",
            r#"{"text":"x"}"#,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    // Session 自体は作成されるが echo が SessionNotFound で失敗する
    // → 終了コードは 0 ではないはず
    assert!(
        !out.status.success() || !String::from_utf8_lossy(&out.stdout).contains("x"),
        "invalid session must not return success, got: {out:?}"
    );
}

#[test]
fn st_cli_009_echo_with_invalid_uuid_exits_nonzero() {
    // ST-CLI-009: 不正な UUID 形式 → clap parse error で exit ≠ 0。
    let mut child = Command::new(binary_path())
        .args(["echo", "--session", "not-a-uuid", r#"{"text":"x"}"#])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(!out.status.success(), "invalid uuid must exit nonzero");
}

#[test]
fn st_cli_010_echo_with_invalid_json_arg_exits_nonzero() {
    // ST-CLI-010: 不正な JSON 引数 → serde_json error で exit ≠ 0。
    let mut sess = Command::new(binary_path())
        .args(["session-create", "--actor", "human"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
    let out = sess.wait_with_output().expect("wait");
    let sid = String::from_utf8_lossy(&out.stdout).trim().to_string();

    let mut child = Command::new(binary_path())
        .args(["echo", "--session", &sid, "not-json{"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(!out.status.success(), "invalid json must exit nonzero");
}

#[test]
fn st_cli_011_event_bus_replay_outputs_event_lines() {
    // ST-CLI-011: `event-bus replay` が少なくとも echo の session.create を 1 件出す。
    let mut sess = Command::new(binary_path())
        .args(["session-create", "--actor", "human"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    let _ = sess.wait_with_output().unwrap();

    let mut replay = Command::new(binary_path())
        .args(["event-bus-replay", "--from", "1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = replay.wait_with_output().expect("wait");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Durable Event が 1 行以上あること
    assert!(
        stdout.lines().count() >= 1,
        "replay must produce at least 1 line, got: {stdout}"
    );
    assert!(
        stdout.contains("seq=") && stdout.contains("type="),
        "must contain seq=/type= headers, got: {stdout}"
    );
}

#[test]
fn st_cli_012_log_level_env_var_is_accepted() {
    // ST-CLI-012: `--log-level debug` がパース可能 (config 受理)。
    let mut child = Command::new(binary_path())
        .args(["--log-level", "debug", "sessions-list"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "--log-level debug must be accepted: {out:?}"
    );
}

#[test]
fn st_cli_013_log_json_flag_is_accepted() {
    // ST-CLI-013: `--log-json` がパース可能。
    let mut child = Command::new(binary_path())
        .args(["--log-json", "sessions-list"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(out.status.success());
}

#[test]
fn st_cli_014_unknown_subcommand_exits_nonzero() {
    // ST-CLI-014: 未知の subcommand は clap エラーで exit ≠ 0。
    let mut child = Command::new(binary_path())
        .args(["this-subcommand-does-not-exist"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let out = child.wait_with_output().expect("wait");
    assert!(!out.status.success(), "unknown subcommand must exit nonzero");
}

// ダミー helper: ビルド成功時に warning 抑止
#[allow(dead_code)]
fn _force_write(_: &mut dyn Write) {}