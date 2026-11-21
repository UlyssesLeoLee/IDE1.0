//! 同语义 JSON / YAML 格式对照生成器与文档 Token 实测比较

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
