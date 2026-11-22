//! 基于 `nema_codec` 官方解析器的 NEMA v1.5 结构化域感知合并 (P1-06)。
//!
//! 不变量:
//! - 复用 `nema_codec::parse_source` 作为唯一正式语法规范（支持 Table / 5W1H / Graph / Dictionary）
//! - 保留域结构、显式表头、记录顺序、空白与转义语义
//! - 合并结果必须通过 `nema_codec::parse_source` 语法封闭性验证；任何失败均作为冲突保留提案而非污染 HEAD

use super::algo::{three_way_merge, ConflictHunk, MergeOutcome};
use nema_codec::{parse_source, DomainMode, NemaDocument, NemaDomain, NemaRecord};
use std::collections::{HashMap, HashSet};

fn line_at_offset(src: &str, byte_offset: usize) -> &str {
    let tail = &src[byte_offset..];
    match tail.find('\n') {
        Some(pos) => &tail[..pos],
        None => tail,
    }
}

fn domain_header_prefix<'a>(src: &'a str, dom: &NemaDomain, next_dom_start: usize) -> Vec<&'a str> {
    let start = dom.header_span.start_byte;
    let end = dom
        .records
        .first()
        .map(|r| r.span.start_byte)
        .unwrap_or(next_dom_start)
        .min(src.len());
    let slice = src[start..end].trim_end_matches('\n');
    if slice.is_empty() {
        return vec![line_at_offset(src, dom.header_span.start_byte)];
    }
    slice.split('\n').filter(|l| !l.is_empty()).collect()
}

fn record_slice<'a>(src: &'a str, rec: &NemaRecord) -> &'a str {
    let s = rec.span.start_byte.min(src.len());
    let e = rec.span.end_byte.min(src.len());
    &src[s..e]
}

fn dict_block_lines<'a>(src: &'a str, doc: &NemaDocument) -> Vec<&'a str> {
    if doc.dictionary.is_empty() {
        return Vec::new();
    }
    let first_dom = doc
        .domains
        .first()
        .map(|d| d.header_span.start_byte)
        .unwrap_or(src.len());
    src[..first_dom]
        .split('\n')
        .filter(|l| !l.is_empty())
        .collect()
}

#[derive(Clone)]
struct TaggedRecord<'a> {
    key: String,
    slice: &'a str,
}

fn table_row_key(rec: &NemaRecord) -> Option<String> {
    let first = rec.cells.first()?.first()?.raw.as_str();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

fn extract_tagged_records<'a>(src: &'a str, dom: &NemaDomain) -> Vec<TaggedRecord<'a>> {
    if dom.mode == DomainMode::FiveWOneH && dom.is_v12 && dom.records.len() == 1 {
        let block = record_slice(src, &dom.records[0]);
        if block.contains('\n') {
            return block
                .split('\n')
                .filter(|l| !l.is_empty())
                .map(|line| TaggedRecord {
                    key: format!("exact:{line}"),
                    slice: line,
                })
                .collect();
        }
    }

    let use_table_keys = dom.mode == DomainMode::Table && {
        let mut keys = Vec::new();
        let mut set = HashSet::new();
        let mut all_valid = !dom.records.is_empty();
        for rec in &dom.records {
            if let Some(k) = table_row_key(rec) {
                if !set.insert(k.clone()) {
                    all_valid = false;
                    break;
                }
                keys.push(k);
            } else {
                all_valid = false;
                break;
            }
        }
        all_valid && !keys.is_empty()
    };

    dom.records
        .iter()
        .map(|rec| {
            let slice = record_slice(src, rec);
            let key = match dom.mode {
                DomainMode::Graph => {
                    if let Some(nid) = &rec.graph_node_id {
                        format!("node:{nid}")
                    } else if let Some(edge) = &rec.graph_edge {
                        format!("edge:{}", edge.id)
                    } else {
                        format!("exact:{slice}")
                    }
                }
                DomainMode::Table if use_table_keys => {
                    format!("row:{}", table_row_key(rec).unwrap_or_default())
                }
                _ => format!("exact:{slice}"),
            };
            TaggedRecord { key, slice }
        })
        .collect()
}

fn merge_domain_records<'a>(
    base_recs: Vec<TaggedRecord<'a>>,
    ours_recs: Vec<TaggedRecord<'a>>,
    theirs_recs: Vec<TaggedRecord<'a>>,
    conflicts: &mut Vec<ConflictHunk>,
) -> Vec<&'a str> {
    let ours_map: HashMap<String, &str> =
        ours_recs.iter().map(|r| (r.key.clone(), r.slice)).collect();
    let theirs_map: HashMap<String, &str> = theirs_recs
        .iter()
        .map(|r| (r.key.clone(), r.slice))
        .collect();

    let mut out = Vec::new();
    let mut processed = HashSet::new();

    for br in &base_recs {
        processed.insert(br.key.clone());
        let o = ours_map.get(&br.key).copied();
        let t = theirs_map.get(&br.key).copied();
        match (o, t) {
            (Some(ov), Some(tv)) => {
                if ov == tv {
                    out.push(ov);
                } else if ov == br.slice {
                    out.push(tv);
                } else if tv == br.slice {
                    out.push(ov);
                } else {
                    conflicts.push(ConflictHunk {
                        base_line_start: 1,
                        base_line_end: 1,
                        ours: ov.to_string(),
                        theirs: tv.to_string(),
                    });
                    out.push(ov);
                }
            }
            (Some(ov), None) if ov != br.slice => {
                conflicts.push(ConflictHunk {
                    base_line_start: 1,
                    base_line_end: 1,
                    ours: ov.to_string(),
                    theirs: "<deleted>".to_string(),
                });
                out.push(ov);
            }
            (None, Some(tv)) if tv != br.slice => {
                conflicts.push(ConflictHunk {
                    base_line_start: 1,
                    base_line_end: 1,
                    ours: "<deleted>".to_string(),
                    theirs: tv.to_string(),
                });
                out.push(tv);
            }
            _ => {}
        }
    }

    for or in &ours_recs {
        if !processed.insert(or.key.clone()) {
            continue;
        }
        if let Some(tv) = theirs_map.get(&or.key).copied() {
            if tv != or.slice {
                conflicts.push(ConflictHunk {
                    base_line_start: 1,
                    base_line_end: 1,
                    ours: or.slice.to_string(),
                    theirs: tv.to_string(),
                });
            }
        }
        out.push(or.slice);
    }

    for tr in &theirs_recs {
        if processed.insert(tr.key.clone()) {
            out.push(tr.slice);
        }
    }

    out
}

fn next_dom_byte(doc: &NemaDocument, idx: usize, total_len: usize) -> usize {
    doc.domains
        .get(idx + 1)
        .map(|d| d.header_span.start_byte)
        .unwrap_or(total_len)
}

fn resolve_header_triple<'a>(
    b_hdr: Option<&'a [&'a str]>,
    o_hdr: Option<&'a [&'a str]>,
    t_hdr: Option<&'a [&'a str]>,
    conflicts: &mut Vec<ConflictHunk>,
) -> Option<Vec<&'a str>> {
    match (b_hdr, o_hdr, t_hdr) {
        (Some(bh), Some(oh), Some(th)) => {
            if oh == th || th == bh {
                Some(oh.to_vec())
            } else if oh == bh {
                Some(th.to_vec())
            } else {
                conflicts.push(ConflictHunk {
                    base_line_start: 1,
                    base_line_end: 1,
                    ours: oh.join("\n"),
                    theirs: th.join("\n"),
                });
                Some(oh.to_vec())
            }
        }
        (_, Some(oh), Some(th)) if oh == th => Some(oh.to_vec()),
        (_, Some(oh), Some(th)) => {
            conflicts.push(ConflictHunk {
                base_line_start: 1,
                base_line_end: 1,
                ours: oh.join("\n"),
                theirs: th.join("\n"),
            });
            Some(oh.to_vec())
        }
        (_, Some(oh), None) => Some(oh.to_vec()),
        (_, None, Some(th)) => Some(th.to_vec()),
        (Some(_), None, None) | (None, None, None) => None,
    }
}

fn mode_tag(mode: DomainMode) -> u8 {
    match mode {
        DomainMode::FiveWOneH => 0,
        DomainMode::Table => 1,
        DomainMode::Graph => 2,
    }
}

fn collect_ordered_domain_keys(
    b_doc: &NemaDocument,
    o_doc: &NemaDocument,
    t_doc: &NemaDocument,
) -> Vec<(String, u8)> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    for d in b_doc
        .domains
        .iter()
        .chain(o_doc.domains.iter())
        .chain(t_doc.domains.iter())
    {
        let k = (d.id.clone(), mode_tag(d.mode));
        if seen.insert(k.clone()) {
            ordered.push(k);
        }
    }
    ordered
}

fn index_domains(doc: &NemaDocument) -> HashMap<(String, u8), usize> {
    doc.domains
        .iter()
        .enumerate()
        .map(|(i, d)| ((d.id.clone(), mode_tag(d.mode)), i))
        .collect()
}

fn is_declared_table_domain(doc: &NemaDocument, idx: Option<usize>) -> bool {
    idx.map(|i| doc.domains[i].mode == DomainMode::Table && !doc.domains[i].columns.is_empty())
        .unwrap_or(false)
}

fn finalize_nema_merge(
    base: &str,
    ours: &str,
    theirs: &str,
    lines_out: Vec<String>,
    conflicts: Vec<ConflictHunk>,
) -> MergeOutcome {
    if !conflicts.is_empty() {
        return MergeOutcome {
            clean: false,
            merged_text: three_way_merge(base, ours, theirs).merged_text,
            conflicts,
            ours_changes: 1,
            theirs_changes: 1,
        };
    }

    let mut assembled = lines_out.join("\n");
    assembled.push_str("\n\n\n");
    match parse_source(&assembled) {
        Ok(_) => MergeOutcome {
            clean: true,
            merged_text: assembled,
            conflicts: Vec::new(),
            ours_changes: 1,
            theirs_changes: 1,
        },
        Err(err) => MergeOutcome {
            clean: false,
            merged_text: three_way_merge(base, ours, theirs).merged_text,
            conflicts: vec![ConflictHunk {
                base_line_start: 1,
                base_line_end: 1,
                ours: format!("nema_ast_closure_error: {err}"),
                theirs: "manual_resolution_required".into(),
            }],
            ours_changes: 1,
            theirs_changes: 1,
        },
    }
}

fn try_nema_ast_merge(base: &str, ours: &str, theirs: &str) -> Option<MergeOutcome> {
    let (b_doc, o_doc, t_doc) = (
        parse_source(base).ok()?,
        parse_source(ours).ok()?,
        parse_source(theirs).ok()?,
    );
    let (b_dict, o_dict, t_dict) = (
        dict_block_lines(base, &b_doc),
        dict_block_lines(ours, &o_doc),
        dict_block_lines(theirs, &t_doc),
    );
    let mut conflicts = Vec::new();
    let final_dict = if o_dict == t_dict || t_dict == b_dict {
        o_dict
    } else if o_dict == b_dict {
        t_dict
    } else {
        conflicts.push(ConflictHunk {
            base_line_start: 1,
            base_line_end: 1,
            ours: o_dict.join("\n"),
            theirs: t_dict.join("\n"),
        });
        o_dict
    };

    let (b_map, o_map, t_map) = (
        index_domains(&b_doc),
        index_domains(&o_doc),
        index_domains(&t_doc),
    );
    let mut lines_out: Vec<String> = final_dict.into_iter().map(str::to_string).collect();

    for k in collect_ordered_domain_keys(&b_doc, &o_doc, &t_doc) {
        let (bi, oi, ti) = (
            b_map.get(&k).copied(),
            o_map.get(&k).copied(),
            t_map.get(&k).copied(),
        );
        let b_hdr = bi.map(|i| {
            domain_header_prefix(
                base,
                &b_doc.domains[i],
                next_dom_byte(&b_doc, i, base.len()),
            )
        });
        let o_hdr = oi.map(|i| {
            domain_header_prefix(
                ours,
                &o_doc.domains[i],
                next_dom_byte(&o_doc, i, ours.len()),
            )
        });
        let t_hdr = ti.map(|i| {
            domain_header_prefix(
                theirs,
                &t_doc.domains[i],
                next_dom_byte(&t_doc, i, theirs.len()),
            )
        });
        let Some(hdr) = resolve_header_triple(
            b_hdr.as_deref(),
            o_hdr.as_deref(),
            t_hdr.as_deref(),
            &mut conflicts,
        ) else {
            continue;
        };

        let merged_recs = merge_domain_records(
            bi.map(|i| extract_tagged_records(base, &b_doc.domains[i]))
                .unwrap_or_default(),
            oi.map(|i| extract_tagged_records(ours, &o_doc.domains[i]))
                .unwrap_or_default(),
            ti.map(|i| extract_tagged_records(theirs, &t_doc.domains[i]))
                .unwrap_or_default(),
            &mut conflicts,
        );
        if merged_recs.is_empty()
            && !is_declared_table_domain(&o_doc, oi)
            && !is_declared_table_domain(&t_doc, ti)
        {
            continue;
        }
        lines_out.extend(hdr.into_iter().map(str::to_string));
        lines_out.extend(merged_recs.into_iter().map(str::to_string));
    }

    Some(finalize_nema_merge(
        base, ours, theirs, lines_out, conflicts,
    ))
}

/// NEMA 结构化协同合并：官方 AST 解析 + 域结构保持 + 语法封闭性检验 (P1-06)。
pub fn nema_merge(base: &str, ours: &str, theirs: &str) -> Result<MergeOutcome, String> {
    parse_source(ours).map_err(|e| format!("Ours NEMA 语法校验未通过 (Red Guard): {e}"))?;
    parse_source(theirs).map_err(|e| format!("Theirs NEMA 语法校验未通过 (Red Guard): {e}"))?;

    if let Some(semantic_res) = try_nema_ast_merge(base, ours, theirs) {
        return Ok(semantic_res);
    }

    let outcome = three_way_merge(base, ours, theirs);
    if outcome.clean {
        match parse_source(&outcome.merged_text) {
            Ok(_) => Ok(outcome),
            Err(err) => Ok(MergeOutcome {
                clean: false,
                merged_text: outcome.merged_text,
                conflicts: vec![ConflictHunk {
                    base_line_start: 1,
                    base_line_end: 1,
                    ours: format!("valid_nema_ours ({err})"),
                    theirs: "valid_nema_theirs".into(),
                }],
                ours_changes: outcome.ours_changes,
                theirs_changes: outcome.theirs_changes,
            }),
        }
    } else {
        Ok(outcome)
    }
}
