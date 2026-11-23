use nema_codec::{
    adaptive_render_document, adaptive_render_view, decode_transport, decode_transport_v12,
    discover_candidates, encode_transport_v12, encode_with_profile, execute_interpreter,
    find_reused_positions, get_active_interpreter, join_intents, normalize_nema_newlines,
    parse_domain_header, parse_source, parse_strict, parse_strict_ast_json, probe_interpreter,
    render_source, render_strict, rendered_document_from_json, set_active_interpreter,
    toggle_table_header_in_source, unescape_opaque, CodecFormat, DomainMode, EncodeMethod,
    EncodedChoice, InterpreterConfig, InterpreterMetadata, InterpreterProtocol, InterpreterWatcher,
    NemaError, NemaLeaseArbitrator, OwnershipRole, SecretRef, TokenProfile, COMPLETE_V12_SAMPLE,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct ActiveConfigResetGuard;
impl Drop for ActiveConfigResetGuard {
    fn drop(&mut self) {
        set_active_interpreter(InterpreterConfig::default());
    }
}

fn make_temp_dir(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("nema_cov_{}_{}_{}", label, std::process::id(), ts));
    let _ = fs::create_dir_all(&dir);
    dir
}

#[test]
fn test_enum_helpers_and_lease_branches() {
    assert_eq!(DomainMode::Table.as_str(), "table");
    assert_eq!(DomainMode::FiveWOneH.as_str(), "5w1h");
    assert_eq!(DomainMode::Graph.as_str(), "graph");
    assert_eq!(DomainMode::Table.prefix_char(), '\\');
    assert_eq!(DomainMode::FiveWOneH.prefix_char(), '?');
    assert_eq!(DomainMode::Graph.prefix_char(), '>');

    assert_eq!(EncodeMethod::Native.as_str(), "native");
    assert_eq!(EncodeMethod::DictionaryAndRle.as_str(), "dictionary+rle");
    assert_eq!(EncodeMethod::DictionaryOnly.as_str(), "dictionary");
    assert_eq!(EncodeMethod::RleOnly.as_str(), "rle");
    assert_eq!(EncodeMethod::PrefixOnly.as_str(), "prefix");
    assert_eq!(EncodeMethod::Combined.as_str(), "combined");
    assert_eq!(CodecFormat::Nema.as_str(), "nema");
    assert_eq!(CodecFormat::Nemat.as_str(), "nemat");

    let sref = SecretRef::new("k1", "ENV_K1");
    assert_eq!(sref.key_id, "k1");

    let zero_choice = EncodedChoice {
        method: EncodeMethod::Native,
        text: String::new(),
        score: 0,
        baseline_score: 0,
        format: CodecFormat::Nema,
        dictionary: vec![],
        is_exact_token: false,
        metric_note: String::new(),
    };
    assert_eq!(zero_choice.saving_percent(), 0.0);

    let nonzero_choice = EncodedChoice {
        score: 40,
        baseline_score: 100,
        ..zero_choice.clone()
    };
    assert_eq!(nonzero_choice.saving_percent(), 60.0);

    // TokenProfile variants
    let (_, exact_bpe, _) = TokenProfile::BpeHeuristic {
        model_name: "gpt".into(),
    }
    .measure("LoginService2026_检查");
    assert!(!exact_bpe);
    let (_, exact_uni, _) = TokenProfile::UnigramHeuristic {
        model_name: "sp".into(),
    }
    .measure("abcdef 123 中文");
    assert!(!exact_uni);
    let (off_score, exact_off, _) = TokenProfile::OfficialModel {
        provider: "openai".into(),
        model: "gpt-4o".into(),
        secret_ref: Some(sref),
    }
    .measure("hello");
    assert_eq!(off_score, 5);
    assert!(!exact_off);

    // Lease arbitrator uncovered branches: current_epoch, stale epochs, invalid state transitions
    let mut arb = NemaLeaseArbitrator::new();
    assert_eq!(arb.current_epoch(), 0);
    assert_eq!(arb.role(), &OwnershipRole::Observer);
    let h = NemaLeaseArbitrator::hash_source(" >G1 1\nA n\n\n\n");
    let (ep1, cached) = arb.request_lease(h, 1);
    assert!(cached.is_none());
    assert_eq!(arb.current_epoch(), ep1);
    assert!(!arb.pause_ime_same_epoch(ep1 + 9));
    assert!(!arb.atomic_commit(ep1, zero_choice.clone())); // before paused
    assert!(arb.pause_ime_same_epoch(ep1));
    assert!(!arb.pause_ime_same_epoch(ep1)); // already paused
    assert!(!arb.atomic_commit(ep1 + 9, zero_choice.clone()));
    assert!(arb.atomic_commit(ep1, zero_choice));
    assert!(!arb.release_lease(ep1 + 9));
    let (ep_hit, cached2) = arb.request_lease(h, 1);
    assert_eq!(ep_hit, ep1);
    assert!(cached2.is_some());
    assert!(arb.release_lease(ep1));
    assert_eq!(arb.role(), &OwnershipRole::Observer);
}

#[test]
fn test_escapes_headers_pascal_and_strict_edges() {
    let un = unescape_opaque("\\n\\r\\t\\e\\^\\\\\\.\\ \\?\\>\\<\\=\\~\\#\\_\\%\\q");
    assert!(un.contains('\n') && un.contains('\r') && un.contains('\t') && un.contains("\\q"));
    assert_eq!(unescape_opaque("\\_"), "");
    assert_eq!(unescape_opaque("\\e"), "");

    // All parse_domain_header syntaxes (space-prefixed & TAB-prefixed)
    assert_eq!(
        parse_domain_header(" \\?W1 2"),
        Some((DomainMode::FiveWOneH, "W1".into(), 2))
    );
    assert_eq!(
        parse_domain_header(" \\>G1 3"),
        Some((DomainMode::Graph, "G1".into(), 3))
    );
    assert_eq!(
        parse_domain_header(" \\|T1 4"),
        Some((DomainMode::Table, "T1".into(), 4))
    );
    assert_eq!(
        parse_domain_header(" \\TT2 5"),
        Some((DomainMode::Table, "T2".into(), 5))
    );
    assert_eq!(
        parse_domain_header(" \\1"),
        Some((DomainMode::Table, "1".into(), 1))
    );
    assert_eq!(
        parse_domain_header("\t\\?W1"),
        Some((DomainMode::FiveWOneH, "W1".into(), 1))
    );
    assert_eq!(
        parse_domain_header("\t\\>G1"),
        Some((DomainMode::Graph, "G1".into(), 1))
    );
    assert_eq!(
        parse_domain_header("\t\\|T1"),
        Some((DomainMode::Table, "T1".into(), 1))
    );
    assert_eq!(
        parse_domain_header("\t\\TT1"),
        Some((DomainMode::Table, "T1".into(), 1))
    );
    assert_eq!(
        parse_domain_header("\t|T2 2"),
        Some((DomainMode::Table, "T2".into(), 2))
    );
    assert_eq!(
        parse_domain_header("\tGG2 2"),
        Some((DomainMode::Graph, "G2".into(), 2))
    );
    assert_eq!(
        parse_domain_header("\tD1 0"),
        None // revision 0 invalid
    );
    assert_eq!(parse_domain_header("\t!bad"), None);

    // PascalCase / lexical_words_v12 & join_intents
    assert_eq!(
        join_intents(&["HTTPResponse200OK", "login_v2", "验证完成"], "en", false),
        "HTTP response 200 OK login _ v 2验证完成"
    );
    assert_eq!(
        join_intents(&["APIKey", "HTTP401"], "en", true),
        "APIKey HTTP401"
    );

    // Strict grammar: continuations & error guards (single leading space in strict mode)
    let cont_ok = " \\1 1\n\\%4\n\\# K V1 V2 V3\nRowA val1\n 1 _more\n 2 \\~\n 3 \\e";
    let r = render_strict(cont_ok).expect("continuation into implicit missing should work");
    assert_eq!(r.domains[0].rows[0].cells[1], "val1_more");
    assert_eq!(r.domains[0].rows[0].cells[2], "~");
    assert_eq!(r.domains[0].rows[0].cells[3], "");

    for bad_strict in [
        " \\1 1\n\\%2\n\\# A B\nx ~\n 1 cont",
        " \\1 1\n\\%2\n\\# A B\nx \\_\n 1 cont",
        " \\1 1\n\\%2\n\\# A B\nx y\n badidx z",
        " \\1 1\n\\%2\n\\# A B\nx y\n 2 oob",
        " \\1 1\n\\%2\n\\# A B\nx y\n a b",
        " \\1 1\n\\%2\n\\# A B\nx y\n ~",
        " \\?W1 1\n1 2 3 4 5 6 7",
        " \\?W1 1\n1 2\n 6 oob",
        " \\>G1 1\nA nod\n 0 bad",
        " \\>G1 1\n1bad nod",
        " \\>G1 1\nA",
        " \\>G1 1\nA n\nB n\nA>e1>B",
        " \\>G1 1\nA n\nA x",
        " \\>G1 1\nA n\nB n\nA>e1>B r1\nA>e1>B r2",
        " \\>G1 1\nA n\nA>A>A selfOverlap",
        " \\>G1 1\nA n\nA>e1>Z dangling",
    ] {
        assert!(
            parse_strict(bad_strict).is_err(),
            "Should reject: {bad_strict}"
        );
    }

    // parse_source v1.2 pending_v12_mode table width & headers & errors
    let v12_width = "\tD1 1\n\\%3\nA B C\n\n\n";
    let d1 = parse_source(v12_width).unwrap();
    assert_eq!(d1.domains[0].columns, vec!["column1", "column2", "column3"]);

    let v12_hash_hdr = "\tD2 1\n\\# ColA ColB\n1 2\n\n\n";
    let d2 = parse_source(v12_hash_hdr).unwrap();
    assert!(d2.domains[0].explicit_header);
    assert_eq!(d2.domains[0].columns, vec!["ColA", "ColB"]);

    let v12_space_hdr = "\tD3 1\n ColA ColB\n1 2\n\n\n";
    let d3 = parse_source(v12_space_hdr).unwrap();
    assert!(d3.domains[0].explicit_header);

    assert!(matches!(
        parse_source("\tD4 1\n\t\tbad\n\n\n"),
        Err(NemaError::TabInBody)
    ));
}

#[test]
fn test_transport_v12_profile_optimize_and_source_utilities() {
    // Source with multiple repeated prefix stems & literal `^` + repeating phrases
    let rich_v12 = concat!(
        "\tItem1 1\n",
        "\tKey Service Note\n",
        "AuthGateway AuthenticationService ProductionClusterHealthCheck\\^1\n",
        "AuthGateway AuthenticationService ProductionClusterHealthCheck\n",
        "\tItem2 1\n",
        "\tKey Service Note\n",
        "AuthGateway AuthenticationService ProductionClusterHealthCheck\n",
        "\tOther1 1\n",
        "\tKey Service Note\n",
        "AuthGateway AuthenticationService ProductionClusterHealthCheck\n",
        "\tItem3 1\n",
        "\tKey Service Note\n",
        "AuthGateway AuthenticationService ProductionClusterHealthCheck\n",
        "\n\n"
    );

    let encoded_v12 = encode_transport_v12(
        rich_v12,
        &[
            "ProductionClusterHealthCheck".into(),
            "AuthenticationService".into(),
        ],
        true,
    )
    .expect("v12 encode should succeed");
    assert!(encoded_v12.contains("\t3 1"));
    let decoded_v12 = decode_transport_v12(&encoded_v12).expect("v12 decode should roundtrip");
    assert_eq!(decoded_v12, rich_v12);
    assert_eq!(decode_transport(&encoded_v12).unwrap(), rich_v12);

    // Also test COMPLETE_V12_SAMPLE across profiles & locked dictionary
    for (idx, profile) in [
        TokenProfile::ByteHeuristic,
        TokenProfile::BpeHeuristic {
            model_name: "bpe".into(),
        },
        TokenProfile::UnigramHeuristic {
            model_name: "uni".into(),
        },
        TokenProfile::OfficialModel {
            provider: "p".into(),
            model: "m".into(),
            secret_ref: None,
        },
    ]
    .iter()
    .enumerate()
    {
        let doc_in = if idx == 0 {
            COMPLETE_V12_SAMPLE
        } else {
            "\tAuth1 1\nAuthenticationServiceToken AuthenticationServiceToken\n\tAuth2 1\nAuthenticationServiceToken AuthenticationServiceToken\n\n\n"
        };
        let choice = encode_with_profile(doc_in, profile, 1, &["Authentication".into()])
            .expect("v12 profile encode");
        assert!(choice.score <= choice.baseline_score);

        let v15_doc = " \\1 1\n\\%3\n\\# Service Status Region\nGateway Healthy US_East_Cluster_Production_Endpoint\nGateway Healthy US_East_Cluster_Production_Endpoint\nGateway Healthy US_East_Cluster_Production_Endpoint\nGateway Healthy US_East_Cluster_Production_Endpoint\nGateway Healthy US_East_Cluster_Production_Endpoint\nGateway Healthy US_East_Cluster_Production_Endpoint\n\n\n";
        let v15_choice = encode_with_profile(v15_doc, profile, 2, &["Gateway".into()]).unwrap();
        if v15_choice.format == CodecFormat::Nemat {
            assert_eq!(decode_transport(&v15_choice.text).unwrap(), v15_doc);
        } else {
            assert_eq!(v15_choice.text, v15_doc);
        }
    }

    // Error branches in encode/decode_transport_v12
    assert!(matches!(
        decode_transport_v12("bad\r\n\n\n"),
        Err(NemaError::InvalidTransportTermination)
    ));
    assert!(matches!(
        decode_transport_v12(" \\1 1\n\n"),
        Err(NemaError::InvalidTransportTermination)
    ));
    assert!(matches!(
        decode_transport_v12("\t$99\na\n\n\n"),
        Err(NemaError::BadTransportDictionarySize)
    ));
    assert!(matches!(
        decode_transport_v12("\t$1\nbad^ref\n\\1\na\n\n\n"),
        Err(NemaError::InvalidTransportDictRef)
    ));
    assert!(matches!(
        decode_transport_v12("\t$2\ndup\ndup\n\\1\n^0\n\n\n"),
        Err(NemaError::InvalidTransportDictionaryValue)
    ));
    assert!(matches!(
        decode_transport_v12("\t$1\n\n\\1\n^0\n\n\n"),
        Err(NemaError::InvalidTransportDictionaryValue)
    ));
    assert!(matches!(
        decode_transport_v12("\t9\n\t0 1\na\n\n\n"),
        Err(NemaError::UnsupportedLegacyPrefixSelector)
    ));
    assert!(matches!(
        decode_transport_v12("\t0 1\na\n\n\n"),
        Err(NemaError::UnsupportedLegacyPrefixSelector)
    ));
    assert!(matches!(
        decode_transport_v12("\tD1 1\n^5\n\n\n"),
        Err(NemaError::TransportDictIndexOutOfRange)
    ));
    assert!(encode_transport_v12(" \\1 1\na\n\n\n", &["".into()], false).is_err());
    assert!(encode_transport_v12(" \\1 1\na\n\n\n", &["a".into(), "a".into()], false).is_err());

    // find_reused_positions with ^N refs & repeated tokens
    let reused_src =
        "\t$1\nLongCluster\n\tD1 1\nAuthenticationClusterNode ^0\\^lit ^0\nAuthenticationClusterNode nodeA\nAuthenticationClusterNode nodeB\n\n\n";
    let pos = find_reused_positions(reused_src);
    assert!(pos.iter().any(|p| p.dict_index == Some(0)));
    assert!(pos.iter().any(|p| p.token == "AuthenticationClusterNode"));

    // normalize_nema_newlines
    assert_eq!(normalize_nema_newlines("\n\n\n"), COMPLETE_V12_SAMPLE);
    assert_eq!(
        normalize_nema_newlines(" \\1 1\r\na\r\n"),
        " \\1 1\na\n\n\n"
    );

    // toggle_table_header_in_source on both v1.2 and v1.5
    let v12_plain = "\tT1 1\n123bad healthy\n\n\n";
    let toggled_v12 = toggle_table_header_in_source(v12_plain, Some("T1"));
    assert!(toggled_v12.contains("\tColumn1 healthy"));
    let untoggled_v12 = toggle_table_header_in_source(&toggled_v12, Some("T1"));
    assert!(!untoggled_v12.contains("\n\tColumn1"));

    let v15_plain = " \\|T2 1\n9bad val\n\n\n";
    let toggled_v15 = toggle_table_header_in_source(v15_plain, None);
    assert!(toggled_v15.contains("\\# Column1 val"));
    let untoggled_v15 = toggle_table_header_in_source(&toggled_v15, None);
    assert!(!untoggled_v15.contains("\\# "));

    assert!(render_source(COMPLETE_V12_SAMPLE).is_ok());
}

#[test]
fn test_interpreter_probe_execute_watch_and_adaptive_fallback() {
    let _reset = ActiveConfigResetGuard;
    let dir = make_temp_dir("interp");
    let ide_dir = dir.join(".ide");
    let codec_dir = dir.join("codec");
    let bin_dir = dir.join("bin");
    fs::create_dir_all(&ide_dir).unwrap();
    fs::create_dir_all(&codec_dir).unwrap();
    fs::create_dir_all(&bin_dir).unwrap();

    fs::write(ide_dir.join("nema_interpreter.py"), "# placeholder").unwrap();
    fs::write(codec_dir.join("__init__.py"), "").unwrap();

    let native_py = codec_dir.join("native.py");
    fs::write(
        &native_py,
        r#"__version__ = "1.5.9-test"
def view(src):
    if "FAIL_INTERP" in src:
        raise RuntimeError("boom")
    return [{
        "mode": "graph",
        "id": "G1",
        "revision": 1,
        "nodes": ["A", "B"],
        "edges": ["X"],
        "records": [
            {"kind": "node", "id": "A", "values": ["A", "LoginAPI", ""], "line": 2},
            {"kind": "node", "id": "B", "values": ["B", "TokenStore", 42], "line": 3},
            {"kind": "edge", "id": "X", "from": "A", "to": "B", "operator": ">", "values": ["A>X>B", "Reads"]}
        ]
    }]
"#,
    )
    .unwrap();

    // CLI-only codec package without native.py so python-native falls through to OfficialCliRender
    let cli_root = make_temp_dir("cli_only");
    let cli_pkg = cli_root.join("codec");
    fs::create_dir_all(&cli_pkg).unwrap();
    fs::write(cli_pkg.join("__init__.py"), "").unwrap();
    let cli_py = cli_pkg.join("cli.py");
    fs::write(
        &cli_py,
        r#"import sys, json
if len(sys.argv) >= 3 and sys.argv[1] == "render":
    with open(sys.argv[2], "r", encoding="utf-8") as f:
        s = f.read()
    print(json.dumps([{"mode": "graph", "id": "Gcli", "revision": 1, "records": [{"kind": "node", "id": "A", "values": ["A", "N"]}]}]))
else:
    sys.exit(1)
"#,
    )
    .unwrap();

    // Script without `view`/`parse` and no sibling native.py -> falls through to OfficialCliRender (`codec.cli render`)
    let no_view_py = cli_pkg.join("no_view.py");
    fs::write(&no_view_py, "X = 1\n").unwrap();

    // Create bin/nema dummy for discover_candidates binary detection + .bat wrapper for execution
    fs::write(bin_dir.join("nema"), "# bin placeholder").unwrap();
    let bat_path = bin_dir.join("nema.bat");
    fs::write(
        &bat_path,
        "@echo off\r\npython -c \"import sys, json; _=sys.stdin.read(); print(json.dumps({'nodes':['A'],'edges':['E']}))\"\r\n",
    )
    .unwrap();

    // 1. discover_candidates with project root + NEMA_INTERPRETER env var
    std::env::set_var("NEMA_INTERPRETER", &native_py);
    let cands = discover_candidates(Some(&dir));
    assert!(cands
        .iter()
        .any(|c| c.candidate_type == "environment_variable" && c.exists));
    assert!(cands
        .iter()
        .any(|c| c.candidate_type == "binary" && c.exists));
    std::env::remove_var("NEMA_INTERPRETER");

    // 2. probe_interpreter across all 3 protocols + failure case
    let rep_nat = probe_interpreter(&native_py.to_string_lossy(), Some("python"));
    assert!(rep_nat.success, "native probe err: {:?}", rep_nat.error);
    assert_eq!(rep_nat.protocol, InterpreterProtocol::PythonNativeView);
    assert_eq!(rep_nat.sample_ast_nodes, 2);

    let rep_cli = probe_interpreter(&no_view_py.to_string_lossy(), Some("py"));
    assert!(rep_cli.success, "cli probe err: {:?}", rep_cli.error);
    assert_eq!(rep_cli.protocol, InterpreterProtocol::OfficialCliRender);

    let rep_gen = probe_interpreter(&bat_path.to_string_lossy(), Some("custom_bat"));
    assert!(rep_gen.success, "bat probe err: {:?}", rep_gen.error);
    assert_eq!(rep_gen.protocol, InterpreterProtocol::GenericJsonAstStdin);
    assert_eq!(rep_gen.sample_ast_domains, 1);

    let rep_fail = probe_interpreter("Z:\\no_such_bin_12345.exe", None);
    assert!(!rep_fail.success);

    // 3. execute_interpreter across protocols & errors
    let disabled = InterpreterConfig::default();
    assert!(execute_interpreter(&disabled, "").is_err());

    let cfg_nat = InterpreterConfig {
        enabled: true,
        path: native_py.to_string_lossy().to_string(),
        runner: "python".into(),
        protocol: InterpreterProtocol::PythonNativeView,
        metadata: InterpreterMetadata::default(),
    };
    let (ast1, _) = execute_interpreter(&cfg_nat, " >G1 1\nA x\n\n\n").unwrap();
    assert!(ast1.is_array());
    assert!(execute_interpreter(&cfg_nat, "FAIL_INTERP").is_err());

    let cfg_cli = InterpreterConfig {
        enabled: true,
        path: cli_py.to_string_lossy().to_string(),
        runner: "python".into(),
        protocol: InterpreterProtocol::OfficialCliRender,
        metadata: InterpreterMetadata::default(),
    };
    let (ast2, _) = execute_interpreter(&cfg_cli, " >G1 1\nA x\n\n\n").unwrap();
    assert!(ast2.is_array());

    let cfg_gen = InterpreterConfig {
        enabled: true,
        path: bat_path.to_string_lossy().to_string(),
        runner: "direct".into(),
        protocol: InterpreterProtocol::GenericJsonAstStdin,
        metadata: InterpreterMetadata::default(),
    };
    let (ast3, _) = execute_interpreter(&cfg_gen, "hello").unwrap();
    assert!(ast3.is_object());

    let cfg_unsup = InterpreterConfig {
        enabled: true,
        path: "x".into(),
        runner: "auto".into(),
        protocol: InterpreterProtocol::GenericFileArg,
        metadata: InterpreterMetadata::default(),
    };
    assert!(execute_interpreter(&cfg_unsup, "x").is_err());

    // 4. adaptive_render_view & adaptive_render_document with empty & named engine_name + fallbacks
    set_active_interpreter(cfg_nat.clone());
    assert!(get_active_interpreter().enabled);
    let (_, _, lbl_view, used_ext) = adaptive_render_view(" >G1 1\nA x\n\n\n");
    assert!(used_ext && lbl_view.contains("native.py"));
    let (doc_ext, _, lbl_doc, used_ext2) = adaptive_render_document(" >G1 1\nA x\n\n\n");
    assert!(used_ext2 && doc_ext.is_ok() && lbl_doc.contains("native.py"));

    let mut cfg_named = cfg_nat;
    cfg_named.metadata.engine_name = "CustomNema".into();
    set_active_interpreter(cfg_named);
    let (_, _, lbl_named1, _) = adaptive_render_view("ok");
    let (_, _, lbl_named2, _) = adaptive_render_document(" >G1 1\nA x\n\n\n");
    assert_eq!(lbl_named1, "CustomNema");
    assert_eq!(lbl_named2, "CustomNema");

    // Fallback when external fails (`FAIL_INTERP`)
    let (v_err, _, lbl_fb, used_fb) = adaptive_render_view("FAIL_INTERP_INVALID");
    assert!(!used_fb && lbl_fb.contains("Rust") && v_err.get("error").is_some());
    let (doc_v12_err, _, _, _) = adaptive_render_document("\t!bad_v12\nFAIL_INTERP");
    assert!(doc_v12_err.is_err());
    let (doc_strict_fb, _, _, _) =
        adaptive_render_document(" \\1 1\n\\%1\n\\# A\nFAIL_INTERP\n\n\n");
    assert!(doc_strict_fb.is_ok());

    set_active_interpreter(InterpreterConfig::default());

    // 5. InterpreterWatcher
    let watch_file = dir.join("watched.txt");
    fs::write(&watch_file, "v1").unwrap();
    let mut watcher = InterpreterWatcher::new(watch_file.to_string_lossy().to_string());
    assert!(!watcher.poll_changed());
    fs::write(&watch_file, "version_two_longer").unwrap();
    assert!(watcher.poll_changed());
    watcher.set_path("");
    assert!(!watcher.poll_changed());
    watcher.set_path(dir.join("does_not_exist.py").to_string_lossy().to_string());
    assert!(!watcher.poll_changed());

    // 6. rendered_document_from_json & parse_strict_ast_json edge cases
    assert!(parse_strict_ast_json("not json").is_err());
    assert!(rendered_document_from_json(&json!({"error": "bad ast"})).is_err());
    assert!(rendered_document_from_json(&json!(12345)).is_err());
    let sample_doc = render_strict(" \\1 1\n\\%1\n\\# K\nV").unwrap();
    assert_eq!(
        rendered_document_from_json(&json!({ "doc": sample_doc }))
            .unwrap()
            .domains
            .len(),
        1
    );
    assert_eq!(
        rendered_document_from_json(&serde_json::to_value(&sample_doc).unwrap())
            .unwrap()
            .domains
            .len(),
        1
    );
    let ast_wrapped = rendered_document_from_json(&json!({
        "ast": [{
            "mode": "table",
            "id": 9,
            "records": [{ "values": ["", 99, true] }]
        }]
    }))
    .unwrap();
    assert_eq!(ast_wrapped.domains[0].rows[0].cells, vec!["", "99", "true"]);

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&cli_root);
}
