//! 同语义对照格式生成器与 .NEMA v1.2 (R7) 实测 Benchmark 数据导出

use crate::{
    encode_with_profile, normalize_nema_newlines, render_source, DomainMode, NemaError,
    RenderedDocument, RenderedDomain, TokenProfile, AXES_5W1H,
};
use serde::{Deserialize, Serialize};

/// 将 `RenderedDocument` 转换为与 `native.py::view` 完全一致的 `serde_json::Value` 数组
pub fn render_to_json_value(doc: &RenderedDocument) -> serde_json::Value {
    let mut arr = Vec::with_capacity(doc.domains.len());
    for d in &doc.domains {
        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), serde_json::Value::String(d.id.clone()));
        obj.insert(
            "revision".to_string(),
            serde_json::Value::Number(serde_json::Number::from(d.revision)),
        );
        obj.insert(
            "mode".to_string(),
            serde_json::Value::String(d.mode.as_str().to_string()),
        );
        match d.mode {
            DomainMode::FiveWOneH => {
                let mut fields = serde_json::Map::new();
                let first_row = d.rows.first();
                for (i, &axis) in AXES_5W1H.iter().enumerate() {
                    let val = first_row
                        .and_then(|r| r.cells.get(i))
                        .cloned()
                        .unwrap_or_default();
                    fields.insert(axis.to_string(), serde_json::Value::String(val));
                }
                obj.insert("fields".to_string(), serde_json::Value::Object(fields));
            }
            DomainMode::Table => {
                obj.insert(
                    "columns".to_string(),
                    serde_json::Value::Array(
                        d.columns
                            .iter()
                            .cloned()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
                let mut records = Vec::with_capacity(d.rows.len());
                for r in &d.rows {
                    let mut rec_map = serde_json::Map::new();
                    for (i, col_name) in d.columns.iter().enumerate() {
                        let val = r.cells.get(i).cloned().unwrap_or_default();
                        rec_map.insert(col_name.clone(), serde_json::Value::String(val));
                    }
                    records.push(serde_json::Value::Object(rec_map));
                }
                obj.insert("records".to_string(), serde_json::Value::Array(records));
            }
            DomainMode::Graph => {
                let mut nodes = Vec::new();
                let mut edges = Vec::new();
                for r in &d.rows {
                    if let Some(nid) = &r.node_id {
                        let mut n_map = serde_json::Map::new();
                        n_map.insert("id".to_string(), serde_json::Value::String(nid.clone()));
                        n_map.insert(
                            "name".to_string(),
                            serde_json::Value::String(r.cells.get(1).cloned().unwrap_or_default()),
                        );
                        n_map.insert(
                            "description".to_string(),
                            serde_json::Value::String(r.cells.get(2).cloned().unwrap_or_default()),
                        );
                        if !r.extensions.is_empty() {
                            let mut ext_map = serde_json::Map::new();
                            for (k, v) in &r.extensions {
                                ext_map.insert(k.clone(), serde_json::Value::String(v.clone()));
                            }
                            n_map.insert(
                                "extensions".to_string(),
                                serde_json::Value::Object(ext_map),
                            );
                        }
                        nodes.push(serde_json::Value::Object(n_map));
                    } else if let Some(edge) = &r.edge {
                        let mut e_map = serde_json::Map::new();
                        e_map.insert("id".to_string(), serde_json::Value::String(edge.id.clone()));
                        e_map.insert(
                            "source".to_string(),
                            serde_json::Value::String(edge.from.clone()),
                        );
                        e_map.insert(
                            "target".to_string(),
                            serde_json::Value::String(edge.to.clone()),
                        );
                        e_map.insert(
                            "direction".to_string(),
                            serde_json::Value::String(edge.raw_operator.to_string()),
                        );
                        e_map.insert(
                            "name".to_string(),
                            serde_json::Value::String(r.cells.get(1).cloned().unwrap_or_default()),
                        );
                        e_map.insert(
                            "description".to_string(),
                            serde_json::Value::String(r.cells.get(2).cloned().unwrap_or_default()),
                        );
                        if !r.extensions.is_empty() {
                            let mut ext_map = serde_json::Map::new();
                            for (k, v) in &r.extensions {
                                ext_map.insert(k.clone(), serde_json::Value::String(v.clone()));
                            }
                            e_map.insert(
                                "extensions".to_string(),
                                serde_json::Value::Object(ext_map),
                            );
                        }
                        edges.push(serde_json::Value::Object(e_map));
                    }
                }
                obj.insert("nodes".to_string(), serde_json::Value::Array(nodes));
                obj.insert("edges".to_string(), serde_json::Value::Array(edges));
            }
        }
        arr.push(serde_json::Value::Object(obj));
    }
    serde_json::Value::Array(arr)
}

fn domain_to_positional_json_value(d: &RenderedDomain, tagged: bool) -> serde_json::Value {
    let mut row = Vec::with_capacity(if tagged { 5 } else { 4 });
    if tagged {
        let tag = match d.mode {
            DomainMode::FiveWOneH => "w",
            DomainMode::Table => "t",
            DomainMode::Graph => "g",
        };
        row.push(serde_json::Value::String(tag.to_string()));
    }
    row.push(serde_json::Value::String(d.id.clone()));
    row.push(serde_json::Value::Number(serde_json::Number::from(
        d.revision,
    )));
    match d.mode {
        DomainMode::FiveWOneH => {
            let first = d.rows.first();
            let fields: Vec<serde_json::Value> = (0..6)
                .map(|i| {
                    serde_json::Value::String(
                        first
                            .and_then(|r| r.cells.get(i))
                            .cloned()
                            .unwrap_or_default(),
                    )
                })
                .collect();
            row.push(serde_json::Value::Array(fields));
        }
        DomainMode::Table => {
            row.push(serde_json::Value::Array(
                d.columns
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ));
            let recs: Vec<serde_json::Value> = d
                .rows
                .iter()
                .map(|r| {
                    serde_json::Value::Array(
                        (0..d.columns.len())
                            .map(|i| {
                                serde_json::Value::String(
                                    r.cells.get(i).cloned().unwrap_or_default(),
                                )
                            })
                            .collect(),
                    )
                })
                .collect();
            row.push(serde_json::Value::Array(recs));
        }
        DomainMode::Graph => {
            let mut nodes = Vec::new();
            let mut edges = Vec::new();
            for r in &d.rows {
                if let Some(nid) = &r.node_id {
                    let mut n_arr = vec![
                        serde_json::Value::String(nid.clone()),
                        serde_json::Value::String(r.cells.get(1).cloned().unwrap_or_default()),
                        serde_json::Value::String(r.cells.get(2).cloned().unwrap_or_default()),
                    ];
                    for (_, ext_v) in &r.extensions {
                        n_arr.push(serde_json::Value::String(ext_v.clone()));
                    }
                    nodes.push(serde_json::Value::Array(n_arr));
                } else if let Some(edge) = &r.edge {
                    let mut e_arr = vec![
                        serde_json::Value::String(edge.id.clone()),
                        serde_json::Value::String(edge.from.clone()),
                        serde_json::Value::String(edge.to.clone()),
                        serde_json::Value::String(edge.raw_operator.to_string()),
                        serde_json::Value::String(r.cells.get(1).cloned().unwrap_or_default()),
                        serde_json::Value::String(r.cells.get(2).cloned().unwrap_or_default()),
                    ];
                    for (_, ext_v) in &r.extensions {
                        e_arr.push(serde_json::Value::String(ext_v.clone()));
                    }
                    edges.push(serde_json::Value::Array(e_arr));
                }
            }
            row.push(serde_json::Value::Array(nodes));
            row.push(serde_json::Value::Array(edges));
        }
    }
    serde_json::Value::Array(row)
}

/// 生成同语义 `JSON objects` 紧凑字符串
pub fn to_equivalent_json_objects(doc: &RenderedDocument) -> String {
    serde_json::to_string(&render_to_json_value(doc)).unwrap_or_default()
}

/// 生成同语义 `JSON positional` 紧凑字符串
pub fn to_equivalent_json_positional(doc: &RenderedDocument) -> String {
    let arr: Vec<serde_json::Value> = doc
        .domains
        .iter()
        .map(|d| domain_to_positional_json_value(d, false))
        .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

/// 生成同语义 `JSON positional (tagged w/t/g)` 紧凑字符串（自包含模式标签，无需外部模式注册表）
pub fn to_equivalent_json_tagged(doc: &RenderedDocument) -> String {
    let arr: Vec<serde_json::Value> = doc
        .domains
        .iter()
        .map(|d| domain_to_positional_json_value(d, true))
        .collect();
    serde_json::to_string(&arr).unwrap_or_default()
}

/// 生成同语义 `YAML positional` 紧凑字符串
pub fn to_equivalent_yaml_positional(doc: &RenderedDocument) -> String {
    doc.domains
        .iter()
        .map(|d| {
            let line = serde_json::to_string(&domain_to_positional_json_value(d, false))
                .unwrap_or_default();
            format!("- {line}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 单文档多格式同语义 Token / 字节实时对照行
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatComparisonRow {
    pub format_name: &'static str,
    pub bytes: usize,
    pub bpe_tokens: usize,
    pub unigram_tokens: usize,
    pub saving_vs_json_bpe_pct: f64,
    pub saving_vs_pos_json_bpe_pct: f64,
    pub sample_preview: String,
}

fn round_pct(base: usize, cur: usize) -> f64 {
    if base == 0 {
        0.0
    } else {
        let raw = ((base as f64 - cur as f64) * 100.0) / (base as f64);
        (raw * 100.0).round() / 100.0
    }
}

/// 计算当前 `.nema` 源码与同语义 `.nemat`、`JSON objects`、`JSON positional`、`JSON tagged (w/t/g)`、`YAML positional` 的实时对照
pub fn compare_document_formats(source: &str) -> Result<Vec<FormatComparisonRow>, NemaError> {
    let normalized = normalize_nema_newlines(source);
    let rendered = render_source(&normalized)?;
    let bpe_prof = TokenProfile::BpeHeuristic {
        model_name: "local-bpe".to_string(),
    };
    let uni_prof = TokenProfile::UnigramHeuristic {
        model_name: "local-unigram".to_string(),
    };
    let nemat_choice = encode_with_profile(&normalized, &bpe_prof, 12, &[])?;
    let json_obj = to_equivalent_json_objects(&rendered);
    let json_pos = to_equivalent_json_positional(&rendered);
    let json_tag = to_equivalent_json_tagged(&rendered);
    let yaml_pos = to_equivalent_yaml_positional(&rendered);

    let json_bpe = bpe_prof.measure(&json_obj).0;
    let pos_bpe = bpe_prof.measure(&json_pos).0;

    let formats: [(&'static str, String); 6] = [
        (".nema", normalized),
        (".nemat", nemat_choice.text),
        ("JSON objects", json_obj),
        ("JSON positional", json_pos),
        ("JSON tagged (w/t/g)", json_tag),
        ("YAML positional", yaml_pos),
    ];

    Ok(formats
        .into_iter()
        .map(|(format_name, text)| {
            let bytes = text.len();
            let bpe_tokens = bpe_prof.measure(&text).0;
            let unigram_tokens = uni_prof.measure(&text).0;
            let preview: String = text.chars().take(240).collect();
            FormatComparisonRow {
                format_name,
                bytes,
                bpe_tokens,
                unigram_tokens,
                saving_vs_json_bpe_pct: round_pct(json_bpe, bpe_tokens),
                saving_vs_pos_json_bpe_pct: round_pct(pos_bpe, bpe_tokens),
                sample_preview: preview,
            }
        })
        .collect())
}

/// 官方 `.NEMA v1.2 (2026-11-07)` 单个测试场景实测数据 (`latest_results.json` + `tagged_json_audit_r7.json`)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OfficialBenchmarkScenario {
    pub scenario: &'static str,
    pub label_zh: &'static str,
    pub domains: usize,
    pub raw_bytes_nema: usize,
    pub raw_bytes_json: usize,
    pub raw_bytes_json_positional: usize,
    pub raw_bytes_json_tagged: usize,
    pub raw_bytes_yaml_positional: usize,
    pub bpe_nema: usize,
    pub bpe_nemat: usize,
    pub bpe_json: usize,
    pub bpe_json_positional: usize,
    pub bpe_json_tagged: usize,
    pub bpe_yaml_positional: usize,
    pub nemat_mode: &'static str,
    pub bpe_nema_vs_json_pct: f64,
    pub bpe_nema_vs_pos_json_pct: f64,
    pub bpe_nema_vs_tagged_json_pct: f64,
    pub unigram_nema: usize,
    pub unigram_nemat: usize,
    pub unigram_json: usize,
    pub unigram_json_positional: usize,
    pub unigram_json_tagged: usize,
    pub unigram_yaml_positional: usize,
}

/// 官方 `.NEMA v1.2` 全部 8 组基准场景实测表（含高熵低收益场景，与 `https://nema-sigma.vercel.app/website/index.html#benchmark` 1:1 对齐）
pub const OFFICIAL_BENCHMARK_V12: &[OfficialBenchmarkScenario] = &[
    OfficialBenchmarkScenario {
        scenario: "english_60",
        label_zh: "英文 60 域 (5W1H)",
        domains: 60,
        raw_bytes_nema: 4842,
        raw_bytes_json: 10912,
        raw_bytes_json_positional: 6172,
        raw_bytes_json_tagged: 6412,
        raw_bytes_yaml_positional: 6290,
        bpe_nema: 3012,
        bpe_nemat: 1953,
        bpe_json: 6502,
        bpe_json_positional: 3851,
        bpe_json_tagged: 3971,
        bpe_yaml_positional: 3969,
        nemat_mode: "combined",
        bpe_nema_vs_json_pct: 53.68,
        bpe_nema_vs_pos_json_pct: 21.79,
        bpe_nema_vs_tagged_json_pct: 24.15,
        unigram_nema: 3898,
        unigram_nemat: 2455,
        unigram_json: 8198,
        unigram_json_positional: 4778,
        unigram_json_tagged: 4898,
        unigram_yaml_positional: 4896,
    },
    OfficialBenchmarkScenario {
        scenario: "chinese_60",
        label_zh: "中文 60 域 (5W1H)",
        domains: 60,
        raw_bytes_nema: 5732,
        raw_bytes_json: 11492,
        raw_bytes_json_positional: 6752,
        raw_bytes_json_tagged: 6992,
        raw_bytes_yaml_positional: 6870,
        bpe_nema: 5532,
        bpe_nemat: 3647,
        bpe_json: 8662,
        bpe_json_positional: 6011,
        bpe_json_tagged: 6131,
        bpe_yaml_positional: 6129,
        nemat_mode: "combined",
        bpe_nema_vs_json_pct: 36.13,
        bpe_nema_vs_pos_json_pct: 7.97,
        bpe_nema_vs_tagged_json_pct: 9.77,
        unigram_nema: 5588,
        unigram_nemat: 3703,
        unigram_json: 9488,
        unigram_json_positional: 6068,
        unigram_json_tagged: 6188,
        unigram_yaml_positional: 6186,
    },
    OfficialBenchmarkScenario {
        scenario: "japanese_60",
        label_zh: "日文 60 域 (5W1H)",
        domains: 60,
        raw_bytes_nema: 8432,
        raw_bytes_json: 14192,
        raw_bytes_json_positional: 9452,
        raw_bytes_json_tagged: 9692,
        raw_bytes_yaml_positional: 9570,
        bpe_nema: 7062,
        bpe_nemat: 4088,
        bpe_json: 10192,
        bpe_json_positional: 7541,
        bpe_json_tagged: 7661,
        bpe_yaml_positional: 7659,
        nemat_mode: "combined",
        bpe_nema_vs_json_pct: 30.71,
        bpe_nema_vs_pos_json_pct: 6.35,
        bpe_nema_vs_tagged_json_pct: 7.82,
        unigram_nema: 7248,
        unigram_nemat: 4220,
        unigram_json: 11148,
        unigram_json_positional: 7728,
        unigram_json_tagged: 7848,
        unigram_yaml_positional: 7846,
    },
    OfficialBenchmarkScenario {
        scenario: "table_200",
        label_zh: "表格 200 行 (Table)",
        domains: 1,
        raw_bytes_nema: 6671,
        raw_bytes_json: 13283,
        raw_bytes_json_positional: 8432,
        raw_bytes_json_tagged: 8436,
        raw_bytes_yaml_positional: 8432,
        bpe_nema: 4899,
        bpe_nemat: 2105,
        bpe_json: 7920,
        bpe_json_positional: 5890,
        bpe_json_tagged: 5892,
        bpe_yaml_positional: 5890,
        nemat_mode: "dictionary",
        bpe_nema_vs_json_pct: 38.14,
        bpe_nema_vs_pos_json_pct: 16.83,
        bpe_nema_vs_tagged_json_pct: 16.85,
        unigram_nema: 5208,
        unigram_nemat: 2153,
        unigram_json: 9067,
        unigram_json_positional: 6226,
        unigram_json_tagged: 6228,
        unigram_yaml_positional: 6226,
    },
    OfficialBenchmarkScenario {
        scenario: "graph_40n_160e",
        label_zh: "图谱 40 节点 / 160 边 (Graph)",
        domains: 1,
        raw_bytes_nema: 8488,
        raw_bytes_json: 21312,
        raw_bytes_json_positional: 11265,
        raw_bytes_json_tagged: 11269,
        raw_bytes_yaml_positional: 11265,
        bpe_nema: 6952,
        bpe_nemat: 3265,
        bpe_json: 13778,
        bpe_json_positional: 7551,
        bpe_json_tagged: 7553,
        bpe_yaml_positional: 7551,
        nemat_mode: "dictionary",
        bpe_nema_vs_json_pct: 49.54,
        bpe_nema_vs_pos_json_pct: 7.93,
        bpe_nema_vs_tagged_json_pct: 7.96,
        unigram_nema: 7152,
        unigram_nemat: 3306,
        unigram_json: 15484,
        unigram_json_positional: 8170,
        unigram_json_tagged: 8172,
        unigram_yaml_positional: 8170,
    },
    OfficialBenchmarkScenario {
        scenario: "high_entropy_60",
        label_zh: "高熵随机 60 域 (低复用对照)",
        domains: 60,
        raw_bytes_nema: 7502,
        raw_bytes_json: 15085,
        raw_bytes_json_positional: 10345,
        raw_bytes_json_tagged: 10585,
        raw_bytes_yaml_positional: 10463,
        bpe_nema: 6358,
        bpe_nemat: 6181,
        bpe_json: 9878,
        bpe_json_positional: 7228,
        bpe_json_tagged: 7348,
        bpe_yaml_positional: 7346,
        nemat_mode: "prefix",
        bpe_nema_vs_json_pct: 35.63,
        bpe_nema_vs_pos_json_pct: 12.04,
        bpe_nema_vs_tagged_json_pct: 13.47,
        unigram_nema: 6687,
        unigram_nemat: 6451,
        unigram_json: 11917,
        unigram_json_positional: 8497,
        unigram_json_tagged: 8617,
        unigram_yaml_positional: 8615,
    },
    OfficialBenchmarkScenario {
        scenario: "short_what",
        label_zh: "极短单域 (What HTTPItIs)",
        domains: 1,
        raw_bytes_nema: 22,
        raw_bytes_json: 125,
        raw_bytes_json_positional: 46,
        raw_bytes_json_tagged: 50,
        raw_bytes_yaml_positional: 46,
        bpe_nema: 16,
        bpe_nemat: 16,
        bpe_json: 73,
        bpe_json_positional: 29,
        bpe_json_tagged: 31,
        bpe_yaml_positional: 29,
        nemat_mode: "native",
        bpe_nema_vs_json_pct: 78.08,
        bpe_nema_vs_pos_json_pct: 44.83,
        bpe_nema_vs_tagged_json_pct: 48.39,
        unigram_nema: 20,
        unigram_nemat: 20,
        unigram_json: 91,
        unigram_json_positional: 34,
        unigram_json_tagged: 36,
        unigram_yaml_positional: 34,
    },
    OfficialBenchmarkScenario {
        scenario: "graph_3n_4e",
        label_zh: "小型认证图 (3 节点 / 4 边)",
        domains: 1,
        raw_bytes_nema: 240,
        raw_bytes_json: 689,
        raw_bytes_json_positional: 340,
        raw_bytes_json_tagged: 344,
        raw_bytes_yaml_positional: 340,
        bpe_nema: 144,
        bpe_nemat: 144,
        bpe_json: 417,
        bpe_json_positional: 205,
        bpe_json_tagged: 207,
        bpe_yaml_positional: 205,
        nemat_mode: "native",
        bpe_nema_vs_json_pct: 65.47,
        bpe_nema_vs_pos_json_pct: 29.76,
        bpe_nema_vs_tagged_json_pct: 30.43,
        unigram_nema: 186,
        unigram_nemat: 186,
        unigram_json: 491,
        unigram_json_positional: 239,
        unigram_json_tagged: 241,
        unigram_yaml_positional: 239,
    },
];

/// `.NEMA v1.2` Graph 单空格节点语法局部标记优化实测 (`old_graph_markup` vs `new_graph_markup`)
pub const OFFICIAL_SYNTAX_BEFORE_AFTER: (&str, &str, usize, usize, usize, usize, usize, usize) = (
    "\t>Gauth1 1\nA \\=LoginAPI Authentication\nB \\=TokenStore SessionStorage\nA>X>B Reads VerifySession",
    "\t>Gauth1 1\n A LoginAPI Authentication\n B TokenStore SessionStorage\nA>X>B Reads VerifySession",
    53, // old BPE
    45, // new BPE (-15.09%)
    74, // old Unigram
    72, // new Unigram (-2.70%)
    97, // old bytes
    95, // new bytes
);
