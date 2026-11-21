//! NEMA 编解码器、语法矩阵与租约仲裁单元测试

use super::*;

fn doc(s: &str) -> String {
    format!("{s}\n\n\n")
}

#[test]
fn test_numeric_headers_and_default_revision() {
    let cases = [
        ("\t\\1", DomainMode::Table, 1u64),
        ("\t?2", DomainMode::FiveWOneH, 1u64),
        ("\t>3", DomainMode::Graph, 1u64),
        ("\t\\4 7", DomainMode::Table, 7u64),
    ];
    for (head, mode, rev) in cases {
        let body = if mode == DomainMode::Graph {
            "A Node"
        } else {
            "Hello"
        };
        let d = parse_source(&doc(&format!("{head}\n{body}")))
            .unwrap()
            .domains
            .remove(0);
        assert_eq!((d.mode, d.revision), (mode, rev));
    }
}

#[test]
fn test_header_vs_continuation() {
    let d = render_source(&doc("\t\\1\nA B\n  Continued\n\t\\2\nC D"))
        .unwrap()
        .domains;
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].rows[0].cells[1], "B Continued");
}

#[test]
fn test_reject_missing_graph_node_and_duplicate_edge() {
    let err1 = parse_source(&doc("\t>1\nA Node\nA>X>B Reads")).unwrap_err();
    assert!(matches!(err1, NemaError::UndefinedGraphNode(_, _)));

    let err2 = parse_source(&doc("\t>1\nA Node\nB Node\nA>X>B Reads\nB<X<A Writes")).unwrap_err();
    assert!(matches!(err2, NemaError::DuplicateEdgeId(_)));
}

#[test]
fn test_headerless_and_explicit_header_table() {
    let d1 = render_source(&doc("\t\\1\nClaude Ready HTTPItIs"))
        .unwrap()
        .domains
        .remove(0);
    assert!(!d1.explicit_header);
    assert_eq!(d1.columns, vec!["column1", "column2", "column3"]);
    assert_eq!(d1.rows[0].cells[2], "HTTP It Is");

    let d2 = render_source(&doc(
        "\t\\1\n\\ Agent Status Note\nClaude Ready CheckSession",
    ))
    .unwrap()
    .domains
    .remove(0);
    assert!(d2.explicit_header);
    assert_eq!(d2.columns, vec!["Agent", "Status", "Note"]);
    assert_eq!(d2.rows.len(), 1);
}

#[test]
fn test_expand_column_and_backslash_caret_not_header() {
    let d1 = render_source(&doc("\t\\1\nA B\nC D E\n    Fourth"))
        .unwrap()
        .domains
        .remove(0);
    assert_eq!(d1.columns, vec!["column1", "column2", "column3", "column4"]);
    assert_eq!(d1.rows.last().unwrap().cells[3], "Fourth");

    let d2 = render_source(&doc("\t\\1\n\\^0 A"))
        .unwrap()
        .domains
        .remove(0);
    assert!(!d2.explicit_header);
    assert_eq!(d2.rows[0].cells[0], "^0");
}

#[test]
fn test_5w1h_and_dictionary_atoms_and_cjk_join() {
    let d = render_source(&doc("\t?1\nBuildProject MaintainQuality Rust ."))
        .unwrap()
        .domains
        .remove(0);
    assert_eq!(d.columns, AXES_5W1H);
    assert!(d.rows[0].tail_empty);
    assert_eq!(&d.rows[0].cells[3..6], &["", "", ""]);

    let en_src = doc("\t$1\nAPIKey\n\t?1\nValidate^0BeforeAccess");
    assert_eq!(
        render_source(&en_src).unwrap().domains[0].rows[0].cells[0],
        "Validate APIKey Before Access"
    );

    let zh_src = doc("\t$1\nAPIKey\n\t?1\n请检查^0是否有效");
    assert_eq!(
        render_source(&zh_src).unwrap().domains[0].rows[0].cells[0],
        "请检查APIKey是否有效"
    );

    let ja_src = doc("\t?1\n私は\n 学生\n です。");
    assert_eq!(
        render_source(&ja_src).unwrap().domains[0].rows[0].cells[0],
        "私は学生です。"
    );
}

#[test]
fn test_graph_directions_and_vertical_edge() {
    let fwd = render_source(&doc("\t>1\nA Node\nB Node\nA>X>B Reads"))
        .unwrap()
        .domains
        .remove(0);
    let e1 = fwd.rows[2].edge.as_ref().unwrap();
    assert_eq!((e1.from.as_str(), e1.to.as_str()), ("A", "B"));

    let rev = render_source(&doc("\t>1\nA Node\nB Node\nA<X<B Reads"))
        .unwrap()
        .domains
        .remove(0);
    let e2 = rev.rows[2].edge.as_ref().unwrap();
    assert_eq!((e2.from.as_str(), e2.to.as_str()), ("B", "A"));

    let und = render_source(&doc("\t>1\nA Node\nB Node\nA=X=B Shares"))
        .unwrap()
        .domains
        .remove(0);
    let e3 = und.rows[2].edge.as_ref().unwrap();
    assert_eq!(e3.direction, EdgeDirection::Undirected);

    let vert = render_source(&doc(
        "\t>1\nA Node\nB Node\nA>X>B Reads CheckSession\n   RetryOnce",
    ))
    .unwrap()
    .domains
    .remove(0);
    assert_eq!(
        vert.rows.last().unwrap().cells[2],
        "Check Session Retry Once"
    );
}

#[test]
fn test_transport_roundtrips_and_rle() {
    let codec = StandardNemaCodec;
    let profile = TokenProfile::ByteHeuristic;

    let s1 = doc("\t$1\nAPIKey\n\t?1\nValidate^0BeforeAccess\n\t?2\nValidate^0BeforeAccess");
    let ch1 = codec.encode(&s1, &profile).unwrap();
    let dec1 = if ch1.format == CodecFormat::Nemat {
        codec.decode(&ch1.text).unwrap()
    } else {
        ch1.text.clone()
    };
    assert_eq!(dec1, s1);

    let s2 = doc(&format!("\t\\1\n{} AAAAAAAA", "哈".repeat(200)));
    let ch2 = codec.encode(&s2, &profile).unwrap();
    assert_eq!(ch2.format, CodecFormat::Nemat);
    assert_eq!(codec.decode(&ch2.text).unwrap(), s2);

    let s3 = doc("\t\\1\n\\^ \\^7 \\8A \\\\ \\.");
    let ch3 = codec.encode(&s3, &profile).unwrap();
    let dec3 = if ch3.format == CodecFormat::Nemat {
        codec.decode(&ch3.text).unwrap()
    } else {
        ch3.text.clone()
    };
    assert_eq!(dec3, s3);

    // Bounds check on RLE
    assert_eq!(
        codec.decode(&doc("\t$0\n\t\\1\n\\9999999A")).unwrap_err(),
        NemaError::RleCountExceedsBounds
    );
    assert_eq!(
        codec.decode(&doc("\t$0\n\t0\n\t\\1\nA")).unwrap_err(),
        NemaError::UnsupportedLegacyPrefixSelector
    );
}

#[test]
fn test_reference_examples_if_present() {
    let examples_dir = std::path::Path::new("E:/.NEMA/examples");
    if !examples_dir.exists() {
        return;
    }
    let codec = StandardNemaCodec;
    let profile = TokenProfile::ByteHeuristic;
    for entry in std::fs::read_dir(examples_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) == Some("nema") {
            let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if file_name.contains("_v13") || file_name.contains("_v15") {
                continue;
            }
            let raw = normalize_nema_newlines(&std::fs::read_to_string(&path).unwrap());
            let parsed = codec.parse(&raw).unwrap();
            assert!(!parsed.domains.is_empty());
            let ch = codec.encode(&raw, &profile).unwrap();
            if path.file_name().and_then(|s| s.to_str()) == Some("bench_high_entropy_120.nema") {
                assert_eq!(ch.format, CodecFormat::Nema);
            }
            let roundtrip = if ch.format == CodecFormat::Nemat {
                codec.decode(&ch.text).unwrap()
            } else {
                ch.text
            };
            assert_eq!(roundtrip, raw, "roundtrip mismatch on {:?}", path);
        }
    }
}

#[test]
fn test_lease_arbitrator_epoch_and_idempotency() {
    let mut arb = NemaLeaseArbitrator::new();
    assert_eq!(*arb.role(), OwnershipRole::Observer);

    let src = doc("\t\\1\nHello World");
    let h = NemaLeaseArbitrator::hash_source(&src);
    let (ep1, cached) = arb.request_lease(h, 1);
    assert!(cached.is_none());
    assert!(arb.pause_ime_same_epoch(ep1));

    // Stale epoch must be rejected
    assert!(!arb.pause_ime_same_epoch(ep1 + 99));

    let choice = StandardNemaCodec
        .encode(&src, &TokenProfile::ByteHeuristic)
        .unwrap();
    assert!(arb.atomic_commit(ep1, choice.clone()));
    assert!(arb.release_lease(ep1));
    assert_eq!(*arb.role(), OwnershipRole::Observer);

    // Duplicate (source_hash, revision) returns idempotent cached result without bumping epoch
    let (ep_dup, cached_dup) = arb.request_lease(h, 1);
    assert_eq!(ep_dup, ep1);
    assert_eq!(cached_dup.unwrap(), choice);
}

#[test]
fn test_v12_and_r7_extensions_and_equivalent_json() {
    let src = "\t>Gext1 1\n A LoginAPI Authentication APIKey\n B TokenStore SessionStorage\nA>X>B Reads VerifySession HTTP401\n   RetryOnTimeout\n\n\n";
    let rendered = render_source(src).unwrap();
    let g = &rendered.domains[0];
    assert_eq!(g.mode, DomainMode::Graph);
    assert_eq!(g.rows[0].node_id.as_deref(), Some("A"));
    assert_eq!(g.rows[0].cells[1], "LoginAPI");
    assert_eq!(g.rows[0].cells[2], "Authentication");
    assert_eq!(
        g.rows[0].extensions,
        vec![("X3".to_string(), "APIKey".to_string())]
    );
    assert!(g.rows[1].extensions.is_empty());
    assert_eq!(g.rows[2].cells[1], "Reads");
    assert_eq!(g.rows[2].cells[2], "Verify session");
    assert_eq!(
        g.rows[2].extensions,
        vec![("X3".to_string(), "HTTP401 RetryOnTimeout".to_string())]
    );

    // Dangling edge without declared node is rejected
    assert!(parse_source(&doc("\t>G1 1\nA>X>B Reads")).is_err());
    assert!(parse_source(&doc("\t>G1 1\nA LoginAPI\nB TokenStore\nA>X>B Reads")).is_ok());

    // Compare formats on COMPLETE_V12_SAMPLE
    let rows = compare_document_formats(COMPLETE_V12_SAMPLE).unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].format_name, ".nema");
    assert_eq!(rows[2].format_name, "JSON objects");
    assert_eq!(rows[3].format_name, "JSON positional");
    assert_eq!(rows[4].format_name, "JSON tagged (w/t/g)");
    assert_eq!(OFFICIAL_BENCHMARK_V12.len(), 8);

    // If E:/.NEMA/examples exists, verify 1:1 semantic JSON equality across all 8 benchmark scenarios
    let examples_dir = std::path::Path::new("E:/.NEMA/examples");
    if examples_dir.exists() {
        for name in [
            "english_60",
            "chinese_60",
            "japanese_60",
            "table_200",
            "graph_40n_160e",
            "high_entropy_60",
            "short_what",
            "graph_3n_4e",
        ] {
            let nema_path = examples_dir.join(format!("{name}.nema"));
            let json_path = examples_dir.join(format!("{name}.json"));
            if nema_path.exists() && json_path.exists() {
                let raw = std::fs::read_to_string(&nema_path)
                    .unwrap()
                    .replace("\r\n", "\n");
                let expected: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
                let actual = render_to_json_value(&render_source(&raw).unwrap());
                assert_eq!(actual, expected, "semantic JSON mismatch on {name}");
            }
        }
    }
}
#[test]
fn test_space_domain_header_v12() {
    // v1.2 字母域名头支持空格前缀与 \ 标记
    assert_eq!(
        parse_domain_header_v12(r" \?task1 1"),
        Some((false, "task1".into(), 1))
    );
    assert_eq!(
        parse_domain_header_v12(r" \>auth1 1"),
        Some((true, "auth1".into(), 1))
    );
    assert_eq!(
        parse_domain_header_v12(r" >auth1 1"),
        Some((true, "auth1".into(), 1))
    );
    assert_eq!(
        parse_domain_header_v12(r" ?task1 1"),
        Some((false, "task1".into(), 1))
    );
    assert_eq!(
        parse_domain_header_v12("\t\\>auth1 1"),
        Some((true, "auth1".into(), 1))
    );
    assert_eq!(
        parse_domain_header_v12("\t>Gauth1 1"),
        Some((true, "Gauth1".into(), 1))
    );
    assert!(
        parse_domain_header_v12(r"  \>auth1 1").is_none(),
        "double space must be rejected as continuation line"
    );

    // 全局域头解析支持单空格数字域头
    assert_eq!(
        parse_domain_header(r" \1 1"),
        Some((DomainMode::Table, "1".into(), 1))
    );
    assert_eq!(
        parse_domain_header(r" \?2 1"),
        Some((DomainMode::FiveWOneH, "2".into(), 1))
    );
    assert_eq!(
        parse_domain_header(r" \>3 1"),
        Some((DomainMode::Graph, "3".into(), 1))
    );

    // 空格前缀的 Table 领域头及表头识别测试
    let src_space_table = " Ttable1 1\n Agent Status Note\nClaude Ready 认证完成\n\n\n";
    let doc_space = render_source(src_space_table).expect("space table parse");
    assert_eq!(doc_space.domains[0].mode, DomainMode::Table);
    assert_eq!(
        doc_space.domains[0].columns,
        vec!["Agent", "Status", "Note"]
    );

    let src_slash_table = " \\table1 1\n \\ Agent Status Note\nClaude Ready 认证完成\n\n\n";
    let doc_slash = render_source(src_slash_table).expect("slash table parse");
    assert_eq!(doc_slash.domains[0].mode, DomainMode::Table);
    assert_eq!(
        doc_slash.domains[0].columns,
        vec!["Agent", "Status", "Note"]
    );
}

#[test]
fn test_website_strict_grammar_rules() {
    // 1. Table \%N declared width + \# explicit header + \_ missing + \e empty + ~ null + \  escaped space
    let src_tbl =
        " \\1 1\n\\%3\n\\# Service State Note\nGateway \\_ Hello\\ World\nWorker ~ \\e\n\n\n";
    let tbl_doc = render_source(src_tbl).expect("table with width/header/escapes");
    let tbl = &tbl_doc.domains[0];
    assert_eq!(tbl.mode, DomainMode::Table);
    assert!(tbl.explicit_header);
    assert_eq!(tbl.columns, vec!["Service", "State", "Note"]);
    assert_eq!(tbl.rows[0].cells[0], "Gateway");
    assert_eq!(tbl.rows[0].cells[1], "");
    assert_eq!(tbl.rows[0].cells[2], "Hello World");
    assert_eq!(tbl.rows[1].cells[1], "~");
    assert_eq!(tbl.rows[1].groups[1], vec!["~"]);
    assert_eq!(tbl.rows[1].cells[2], "");

    // 2. Extended escapes in v1.2 (\? \> \< \= \~ \^ \# \_ \% \n \r \t \e)
    let unesc = unescape_v12(r"\?\>\<\=\~\^\#\_\%\n\r\t\e").expect("extended escapes");
    assert_eq!(unesc, "?><=~^#_%\n\r\t");
    assert_eq!(unescape_v12(r"\=LoginAPI").unwrap(), r"\=LoginAPI");

    // 3. Graph supporting both 1-space ` A Node` and 0-space `B Node` declarations + edges + continuations
    let src_graph = " \\>3 1\n A LoginAPI AuthGateway\nB TokenStore SessionStore\nA>X>B Reads VerifyToken\n  CheckExpiry\n\n\n";
    let g_doc = render_source(src_graph).expect("mixed 1-space and 0-space graph");
    let g = &g_doc.domains[0];
    assert_eq!(g.mode, DomainMode::Graph);
    assert_eq!(g.rows[0].node_id.as_deref(), Some("A"));
    assert_eq!(g.rows[1].node_id.as_deref(), Some("B"));
    assert_eq!(g.rows[2].edge.as_ref().unwrap().id, "X");

    // 4. Website candidate transport header \!NEMAT1 / $N / @0 Value / \!DATA
    let src_nemat1 =
        "\\!NEMAT1\n$1\n@0 SessionValidationPipeline\n\\!DATA\n \\?2 1\nVerify ^0 Service\n\n\n";
    let n_doc = render_source(src_nemat1).expect("nemat1 candidate transport");
    assert_eq!(n_doc.dictionary, vec!["SessionValidationPipeline"]);
    assert!(n_doc.domains[0].rows[0].cells[1].contains("SessionValidationPipeline"));
}

#[test]
fn test_website_strict_candidate_rules_and_dual_compat() {
    // Canonical strict parser rules from website strict-parser.js & grammar.html
    let tbl_ok = render_strict(" \\1 1\n\\%3\n\\# Service State Note\\ Text\nGateway \\_ Hello\\ World\n 2 _Cont\nWorker ~ \\e\nMain \\~ \\^0\nA").unwrap();
    let d = &tbl_ok.domains[0];
    assert_eq!(d.mode, DomainMode::Table);
    assert!(d.explicit_header);
    assert_eq!(d.columns, vec!["Service", "State", "Note Text"]);
    assert_eq!(d.rows[0].cells, vec!["Gateway", "", "Hello World_Cont"]);
    assert_eq!(d.rows[1].cells, vec!["Worker", "~", ""]);
    assert_eq!(d.rows[1].groups[1], vec!["~"]);
    assert_eq!(d.rows[2].cells, vec!["Main", "~", "^0"]);
    assert_eq!(d.rows[2].groups[1], vec!["\\~"]);
    assert_eq!(d.rows[3].cells, vec!["A", "", ""]);

    // 0-record table with \%3 or \#
    let empty_tbl = render_strict(" \\1 1\n\\%3").unwrap();
    assert_eq!(
        empty_tbl.domains[0].columns,
        vec!["column1", "column2", "column3"]
    );
    assert!(empty_tbl.domains[0].rows.is_empty());

    // Strict rejections
    for bad in [
        " \\0 1\nA",
        " \\01 1\nA",
        "  \\1 1\nA",
        " \\? 1\nA",
        " \\> 1\nA",
        " \\1 1\n \\?1 1\nA",
        " \\1 1\n\\# ~ B",
        " \\1 1\n\\# A \\e",
        " \\1 1\n\\# A \\_",
        " \\1 1\n\\# A A",
        " \\1 1\n\\# A B\n\\%3\n1 2",
        " \\1 1\n\\%2\nA B C",
        " \\1 1\nA B\n  Cont",
        " \\1 1\nA ~ B\n 1 App",
        " \\1 1\nA \\_ B\n 1 App",
        " \\1 1\nA a\\e",
        " \\1 1\nA a\\_",
        " \\1 1\nA \\q",
        " \\1 1\nA\\",
        " \\1 1\n\tA",
        " \\1 1\nA\u{3000}B",
        " \\>1 1\n A Node",
        " \\>1 1\nA",
        " \\>1 1\nA Node\nB Node\nA>X>B",
        " \\>1 1\nA Node\nB Node\nA\\>1>X>B Edge",
        " \\>1 1\nA Node\nA Node",
        " \\>1 1\nA Node\nA>A>A Self",
        " \\>1 1\nA Node\nA>X>B Edge",
        " \\>1 1\nA Node\n 0 Extra",
    ] {
        assert!(
            parse_strict(bad).is_err(),
            "strict parser should reject invalid input: {bad:?}"
        );
    }

    // Valid Graph with forward node reference, reversed <, undirected =, \e node label, ~ edge label, X3 extension, continuation
    let g_doc = render_strict(
            " \\>3 2\nA>X>B ~\nC<Y<A Uses\nA=Z=C Peer Desc v3\n 3 -strict\nA \\e Desc\n 2 +More\nB Target\nC Client",
        )
        .unwrap();
    let gd = &g_doc.domains[0];
    assert_eq!(gd.rows.len(), 6);
    let e1 = gd.rows[0].edge.as_ref().unwrap();
    assert_eq!(
        (e1.from.as_str(), e1.to.as_str(), e1.id.as_str()),
        ("A", "B", "X")
    );
    let e2 = gd.rows[1].edge.as_ref().unwrap();
    assert_eq!(
        (e2.from.as_str(), e2.to.as_str(), e2.id.as_str()),
        ("A", "C", "Y")
    );
    let e3 = gd.rows[2].edge.as_ref().unwrap();
    assert_eq!(e3.direction, EdgeDirection::Undirected);
    assert_eq!(
        gd.rows[2].extensions,
        vec![("X3".into(), "v3-strict".into())]
    );
    assert_eq!(gd.rows[3].cells[1], "");
    assert_eq!(gd.rows[3].cells[2], "Desc+More");

    // Backward-compatible parse_source / render_source supports explicit N-indexed continuations, 0-record \%3, and escaped headers
    let compat_doc = render_source(
            " \\1 1\n\\%3\n\\# Service\\ Name State Note\nGateway \\_ First\n 2 Second\n \\2 1\n\\%2\n\n\n",
        )
        .unwrap();
    assert_eq!(
        compat_doc.domains[0].columns,
        vec!["Service Name", "State", "Note"]
    );
    assert_eq!(compat_doc.domains[0].rows[0].cells[2], "First Second");
    assert!(compat_doc.domains[1].rows.is_empty());
    assert_eq!(compat_doc.domains[1].columns, vec!["column1", "column2"]);
}
