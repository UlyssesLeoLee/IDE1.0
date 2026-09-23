//! IDE1.0 × aci-emitter 集成测试 (per ULYS-191 §4.2.2 brief v0.1).
//!
//! 3 个 placeholder IT, 每个 IT 都通过 `AciEmitter::new(Layer::It).build(...)`
//! 构造 assertion + 通过 `AciEmitter::from_file()` 反序列化回验证.
//!
//! | IT | 名称 | 校验 |
//! |----|------|------|
//! | IT-1 | `test_aci_schema_v0_1_roundtrip` | `.aci.json` schema v0.1 与 aci-emitter 完全一致 |
//! | IT-2 | `test_ide_cli_emits_valid_aci_assertion` | CLI 输出 JSON 含全部 10 必填字段 + 字段名 1:1 对齐 |
//! | IT-3 | `test_aci_emitter_v0_1_compatibility` | IDE1.0 用 aci-emitter v0.1 跨语言 parity (Rust 与 Star `_lib_aci_emit.py` 字段名一致) |

use std::fs;
use std::path::PathBuf;

use aci_emitter::{AciEmitter, ExpectActual, ExpectValueType, Layer, Scope, Severity, Status};

/// 临时目录用于 file roundtrip 测试.
fn tmp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ide1.0-test-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 10 个 schema 必填字段 (per `.aci.json` schema_required_fields).
/// 按 sort_keys 实际顺序排列: aci_version < actual < assertion_id < captured_at
///                            < expect < layer < reasoning < scope < severity < status
const REQUIRED_FIELDS: [&str; 10] = [
    "aci_version",
    "actual",
    "assertion_id",
    "captured_at",
    "expect",
    "layer",
    "reasoning",
    "scope",
    "severity",
    "status",
];

// ============================================================
// IT-1: `.aci.json` schema v0.1 与 aci-emitter 完全一致 (sort_keys + 字段名)
// ============================================================

#[test]
fn test_aci_schema_v0_1_roundtrip() {
    let dir = tmp_dir();
    let path = dir.join("assertion-it1.json");

    // 构造一条 assertion
    let em = AciEmitter::new(Layer::It);
    let original = em
        .build(
            "ide1.0:it:schema-v0-1-roundtrip",
            Scope::new(
                "ide1.0".to_string(),
                Some("it".to_string()),
                Some("aci-schema".to_string()),
                None,
                None,
                None,
            ),
            ExpectActual::new(
                ExpectValueType::WorkflowCompletes,
                serde_json::json!(true),
                "ide-cli --aci-emit should complete the emit workflow",
            ),
            ExpectActual::new(
                ExpectValueType::WorkflowCompletes,
                serde_json::json!(true),
                "workflow did complete (placeholder)",
            ),
            Status::Pass,
            Severity::Info,
            "schema v0.1 roundtrip placeholder",
        )
        .expect("build must succeed");

    // 写文件 + 读回
    em.write(&original, &path).expect("write must succeed");
    let loaded = em.from_file(&path).expect("from_file must succeed");

    // 字段 1:1 对齐 (核心 10 字段)
    assert_eq!(original.assertion_id, loaded.assertion_id);
    assert_eq!(original.aci_version, loaded.aci_version);
    assert_eq!(original.layer, loaded.layer);
    assert_eq!(original.status, loaded.status);
    assert_eq!(original.severity, loaded.severity);
    assert_eq!(original.scope.project, loaded.scope.project);
    assert_eq!(original.expect.value_type, loaded.expect.value_type);
    assert_eq!(original.actual.value, loaded.actual.value);

    // 文件内容校验: 10 必填字段全在
    let content = fs::read_to_string(&path).unwrap();
    for field in &REQUIRED_FIELDS {
        assert!(
            content.contains(field),
            "field {field} missing in roundtrip output"
        );
    }

    // schema 版本校验
    assert!(
        content.contains("\"aci_version\": \"0.1.0-draft\""),
        "aci_version mismatch: {content}"
    );

    // sort_keys 校验: 10 字段按字典序输出
    // 实际顺序 (per sort_keys): aci_version < actual < assertion_id < captured_at
    //                          < expect < layer < reasoning < scope < severity < status
    let idx_ver = content.find("\"aci_version\"").unwrap();
    let idx_act = content.find("\"actual\"").unwrap();
    let idx_aid = content.find("\"assertion_id\"").unwrap();
    let idx_cap = content.find("\"captured_at\"").unwrap();
    let idx_exp = content.find("\"expect\"").unwrap();
    let idx_lay = content.find("\"layer\"").unwrap();
    let idx_sta = content.find("\"status\"").unwrap();
    assert!(
        idx_ver < idx_act
            && idx_act < idx_aid
            && idx_aid < idx_cap
            && idx_cap < idx_exp
            && idx_exp < idx_lay
            && idx_lay < idx_sta,
        "10 fields must be in sort_keys order: {content}"
    );

    // 清理
    let _ = fs::remove_dir_all(&dir);
}

// ============================================================
// IT-2: CLI 输出 JSON 含全部 10 必填字段 + 字段名 1:1 对齐
// ============================================================

#[test]
fn test_ide_cli_emits_valid_aci_assertion() {
    // 调 ide-cli 二进制, 验输出 JSON 含全部 10 必填字段
    let bin = env!("CARGO_BIN_EXE_ide-cli");
    let output = std::process::Command::new(bin)
        .arg("--aci-emit")
        .arg("ide1.0:it:cli-emits-valid")
        .output()
        .expect("ide-cli binary must exist (cargo build --workspace)");

    assert!(
        output.status.success(),
        "ide-cli exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 10 必填字段全在
    for field in &REQUIRED_FIELDS {
        assert!(
            stdout.contains(field),
            "field {field} missing in CLI output:\n{stdout}"
        );
    }

    // 字段值校验
    assert!(
        stdout.contains("\"assertion_id\": \"ide1.0:it:cli-emits-valid\""),
        "assertion_id mismatch:\n{stdout}"
    );
    assert!(
        stdout.contains("\"aci_version\": \"0.1.0-draft\""),
        "aci_version mismatch:\n{stdout}"
    );
    assert!(
        stdout.contains("\"layer\": \"it\""),
        "layer mismatch:\n{stdout}"
    );
    assert!(
        stdout.contains("\"status\": \"PASS\""),
        "status mismatch:\n{stdout}"
    );
    assert!(
        stdout.contains("\"severity\": \"info\""),
        "severity mismatch:\n{stdout}"
    );
    assert!(
        stdout.contains("\"project\": \"ide1.0\""),
        "scope.project mismatch:\n{stdout}"
    );
}

// ============================================================
// IT-3: 跨语言 parity (Rust 与 Star `_lib_aci_emit.py` 字段名 1:1)
// ============================================================

#[test]
fn test_aci_emitter_v0_1_compatibility() {
    // 核心字段名 (10 必填) 必须 1:1 对齐 Python `_lib_aci_emit.py` (Stage 1 已 ship).
    // Rust emitter 经 sort_keys 输出, Python emitter 经 json.dumps(sort_keys=True) 输出,
    // 字段名同 dict 序后除 `captured_at` 时戳外完全一致.
    let dir = tmp_dir();
    let path = dir.join("assertion-it3.json");

    let em = AciEmitter::new(Layer::It);
    let assertion = em
        .build(
            "ide1.0:it:aci-emitter-compat",
            Scope::new(
                "ide1.0".to_string(),
                Some("it".to_string()),
                None,
                None,
                None,
                None,
            ),
            ExpectActual::new(
                ExpectValueType::ResponseWithinMs,
                serde_json::json!(100),
                "CLI should start within 100ms",
            ),
            ExpectActual::new(
                ExpectValueType::ResponseWithinMs,
                serde_json::json!(5),
                "measured 5ms",
            ),
            Status::Pass,
            Severity::Info,
            "Rust ↔ Python emitter parity check (placeholder)",
        )
        .expect("build must succeed");

    em.write(&assertion, &path).expect("write must succeed");
    let content = fs::read_to_string(&path).unwrap();

    // sort_keys 顺序: REQUIRED_FIELDS 已按 alphabetic 序排列, 所以位置应严格递增.
    // 实际顺序 (per serde_json sort_keys): aci_version < actual < assertion_id < captured_at
    //                          < expect < layer < reasoning < scope < severity < status
    // 与 Python `json.dumps(sort_keys=True)` 输出 1:1 (除 captured_at 时戳值外).
    let positions: Vec<(usize, &str)> = REQUIRED_FIELDS
        .iter()
        .map(|f| {
            let pos = content.find(f).unwrap_or_else(|| {
                panic!("required field {f} missing in output");
            });
            // 校验每个字段仅出现 1 次
            let rest = &content[pos + 1..];
            if let Some(next_pos) = rest.find(f) {
                panic!(
                    "field {f} appears more than once at offset {}",
                    pos + 1 + next_pos
                );
            }
            (pos, *f)
        })
        .collect();
    let mut sorted = positions.clone();
    sorted.sort_by_key(|(pos, _)| *pos);
    assert_eq!(
        positions, sorted,
        "fields must be in alphabetic (sort_keys) order matching REQUIRED_FIELDS"
    );

    // 跨语言 parity: 解析 Rust emitter 实际 JSON 输出的 key 集合,
    // 与 Python `_lib_aci_emit.py` 字段名 (canonical, per Star `.aci.json::schema_required_fields`)
    // 做严格集合比对 (除 captured_at 时戳值外, key 完全一致).
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("Rust emitter JSON must parse");
    let obj = parsed.as_object().expect("must be object");
    let rust_keys: std::collections::BTreeSet<&str> = obj.keys().map(String::as_str).collect();

    // Python `_lib_aci_emit.py` 字段名 (Stage 1 PR #93 已 ship, ACI schema v0.1)
    // Per Star `tools/star-flash-mock/.aci.json::schema_required_fields`
    let python_required: std::collections::BTreeSet<&str> = [
        "aci_version",
        "assertion_id",
        "actual",
        "captured_at",
        "expect",
        "layer",
        "reasoning",
        "scope",
        "severity",
        "status",
    ]
    .into_iter()
    .collect();

    assert_eq!(
        rust_keys, python_required,
        "Rust ↔ Python emitter 10 必填字段不一致 (跨语言 parity broken)\n  rust_keys = {rust_keys:?}\n  python_required = {python_required:?}"
    );

    // expect / actual 必为 object { type, value, description } — 1:1 对齐 Python emitter
    let expect = obj.get("expect").expect("expect field present");
    assert!(expect.is_object(), "expect must be object");
    for sub in ["type", "value", "description"] {
        assert!(
            expect.get(sub).is_some(),
            "expect.{sub} missing — Python emitter 1:1 parity broken"
        );
    }
    let actual = obj.get("actual").expect("actual field present");
    assert!(actual.is_object(), "actual must be object");
    for sub in ["type", "value", "description"] {
        assert!(
            actual.get(sub).is_some(),
            "actual.{sub} missing — Python emitter 1:1 parity broken"
        );
    }

    // 清理
    let _ = fs::remove_dir_all(&dir);
}
