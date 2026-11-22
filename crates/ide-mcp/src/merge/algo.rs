//! 安全的 3-Way 行级合并算法与合并后语法完整性校验 (P1-02)。

use crate::diff::{diff_lines, regions, split_lines, Region};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictHunk {
    pub base_line_start: usize,
    pub base_line_end: usize,
    pub ours: String,
    pub theirs: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeOutcome {
    pub clean: bool,
    pub merged_text: String,
    pub conflicts: Vec<ConflictHunk>,
    pub ours_changes: usize,
    pub theirs_changes: usize,
}

/// 寻找从 base 到 target 的单一变更范围（公共前后缀修剪）。
pub fn find_single_hunk(base: &[&str], target: &[&str]) -> (usize, usize, usize, usize) {
    let mut pre = 0;
    while pre < base.len() && pre < target.len() && base[pre] == target[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < base.len() - pre
        && suf < target.len() - pre
        && base[base.len() - 1 - suf] == target[target.len() - 1 - suf]
    {
        suf += 1;
    }
    (pre, base.len() - suf, pre, target.len() - suf)
}

fn regions_overlap(a: Region, b: Region) -> bool {
    if a.a0 == a.a1 && b.a0 == b.a1 && a.a0 == b.a0 {
        return true;
    }
    !(a.a1 <= b.a0 || b.a1 <= a.a0)
}

fn reconstruct_side_slice(
    base_lines: &[&str],
    side_lines: &[&str],
    cluster_start: usize,
    cluster_end: usize,
    regs: &[Region],
) -> String {
    let mut out = String::new();
    let mut pos = cluster_start;
    for r in regs {
        while pos < r.a0 && pos < base_lines.len() {
            out.push_str(base_lines[pos]);
            pos += 1;
        }
        for line in &side_lines[r.b0..r.b1] {
            out.push_str(line);
        }
        pos = r.a1;
    }
    while pos < cluster_end && pos < base_lines.len() {
        out.push_str(base_lines[pos]);
        pos += 1;
    }
    out
}

fn try_disjoint_fast_merge(
    base: &str,
    ours: &str,
    theirs: &str,
    base_lines: &[&str],
    ours_lines: &[&str],
    theirs_lines: &[&str],
) -> Option<MergeOutcome> {
    let (a0, a1, o0, o1) = find_single_hunk(base_lines, ours_lines);
    let (b0, b1, t0, t1) = find_single_hunk(base_lines, theirs_lines);
    let cap = base.len().max(ours.len()).max(theirs.len()) + 64;

    if a1 <= b0 && !(a0 == a1 && b0 == b1 && a0 == b0) {
        let mut merged = String::with_capacity(cap);
        base_lines[..a0].iter().for_each(|l| merged.push_str(l));
        ours_lines[o0..o1].iter().for_each(|l| merged.push_str(l));
        base_lines[a1..b0].iter().for_each(|l| merged.push_str(l));
        theirs_lines[t0..t1].iter().for_each(|l| merged.push_str(l));
        base_lines[b1..].iter().for_each(|l| merged.push_str(l));
        return Some(MergeOutcome {
            clean: true,
            merged_text: merged,
            conflicts: Vec::new(),
            ours_changes: 1,
            theirs_changes: 1,
        });
    }
    if b1 <= a0 && !(a0 == a1 && b0 == b1 && a0 == b0) {
        let mut merged = String::with_capacity(cap);
        base_lines[..b0].iter().for_each(|l| merged.push_str(l));
        theirs_lines[t0..t1].iter().for_each(|l| merged.push_str(l));
        base_lines[b1..a0].iter().for_each(|l| merged.push_str(l));
        ours_lines[o0..o1].iter().for_each(|l| merged.push_str(l));
        base_lines[a1..].iter().for_each(|l| merged.push_str(l));
        return Some(MergeOutcome {
            clean: true,
            merged_text: merged,
            conflicts: Vec::new(),
            ours_changes: 1,
            theirs_changes: 1,
        });
    }
    None
}

fn collect_overlap_cluster(
    ours_regs: &[Region],
    theirs_regs: &[Region],
    oi: &mut usize,
    ti: &mut usize,
) -> (usize, usize, usize, usize, usize, usize) {
    let o0_idx = *oi;
    let t0_idx = *ti;
    let first_o = ours_regs[*oi];
    let first_t = theirs_regs[*ti];
    let c_start = first_o.a0.min(first_t.a0);
    let mut c_end = first_o.a1.max(first_t.a1);
    *oi += 1;
    *ti += 1;

    loop {
        let mut extended = false;
        let span_reg = Region {
            a0: c_start,
            a1: c_end,
            b0: 0,
            b1: 0,
        };
        if let Some(&no) = ours_regs.get(*oi) {
            if regions_overlap(span_reg, no) {
                c_end = c_end.max(no.a1);
                *oi += 1;
                extended = true;
            }
        }
        if let Some(&nt) = theirs_regs.get(*ti) {
            if regions_overlap(span_reg, nt) {
                c_end = c_end.max(nt.a1);
                *ti += 1;
                extended = true;
            }
        }
        if !extended {
            break;
        }
    }
    (c_start, c_end, o0_idx, *oi, t0_idx, *ti)
}

fn emit_conflict_preview(merged: &mut String, o_slice: &str, t_slice: &str) {
    merged.push_str("<<<<<<< HEAD (ours)\n");
    merged.push_str(o_slice);
    if !o_slice.is_empty() && !o_slice.ends_with('\n') {
        merged.push('\n');
    }
    merged.push_str("=======\n");
    merged.push_str(t_slice);
    if !t_slice.is_empty() && !t_slice.ends_with('\n') {
        merged.push('\n');
    }
    merged.push_str(">>>>>>> INCOMING (theirs)\n");
}

/// 基于区间聚类与 Myers Diff 的安全 3-Way 行级合并（正确处理多处插入/删除与行号漂移）。
pub fn three_way_merge(base: &str, ours: &str, theirs: &str) -> MergeOutcome {
    if ours == theirs {
        return MergeOutcome {
            clean: true,
            merged_text: ours.to_string(),
            conflicts: Vec::new(),
            ours_changes: 0,
            theirs_changes: 0,
        };
    }
    if ours == base {
        return MergeOutcome {
            clean: true,
            merged_text: theirs.to_string(),
            conflicts: Vec::new(),
            ours_changes: 0,
            theirs_changes: 1,
        };
    }
    if theirs == base {
        return MergeOutcome {
            clean: true,
            merged_text: ours.to_string(),
            conflicts: Vec::new(),
            ours_changes: 1,
            theirs_changes: 0,
        };
    }

    let base_lines = split_lines(base);
    let ours_lines = split_lines(ours);
    let theirs_lines = split_lines(theirs);

    if let Some(fast) =
        try_disjoint_fast_merge(base, ours, theirs, &base_lines, &ours_lines, &theirs_lines)
    {
        return fast;
    }

    let (ours_steps, _) = diff_lines(&base_lines, &ours_lines);
    let (theirs_steps, _) = diff_lines(&base_lines, &theirs_lines);
    let ours_regs = regions(&ours_steps);
    let theirs_regs = regions(&theirs_steps);

    let mut merged = String::with_capacity(base.len().max(ours.len()).max(theirs.len()));
    let mut conflicts = Vec::new();
    let (mut base_idx, mut oi, mut ti) = (0usize, 0usize, 0usize);

    while oi < ours_regs.len() || ti < theirs_regs.len() {
        match (ours_regs.get(oi).copied(), theirs_regs.get(ti).copied()) {
            (Some(o), Some(t)) if regions_overlap(o, t) => {
                let (c0, c1, o_s, o_e, t_s, t_e) =
                    collect_overlap_cluster(&ours_regs, &theirs_regs, &mut oi, &mut ti);
                while base_idx < c0 && base_idx < base_lines.len() {
                    merged.push_str(base_lines[base_idx]);
                    base_idx += 1;
                }
                let o_slice =
                    reconstruct_side_slice(&base_lines, &ours_lines, c0, c1, &ours_regs[o_s..o_e]);
                let t_slice = reconstruct_side_slice(
                    &base_lines,
                    &theirs_lines,
                    c0,
                    c1,
                    &theirs_regs[t_s..t_e],
                );
                if o_slice == t_slice {
                    merged.push_str(&o_slice);
                } else {
                    emit_conflict_preview(&mut merged, &o_slice, &t_slice);
                    conflicts.push(ConflictHunk {
                        base_line_start: c0,
                        base_line_end: c1,
                        ours: o_slice,
                        theirs: t_slice,
                    });
                }
                base_idx = c1;
            }
            (Some(o), Some(t)) if o.a1 <= t.a0 => {
                while base_idx < o.a0 && base_idx < base_lines.len() {
                    merged.push_str(base_lines[base_idx]);
                    base_idx += 1;
                }
                ours_lines[o.b0..o.b1]
                    .iter()
                    .for_each(|l| merged.push_str(l));
                base_idx = o.a1;
                oi += 1;
            }
            (Some(_), Some(t)) => {
                while base_idx < t.a0 && base_idx < base_lines.len() {
                    merged.push_str(base_lines[base_idx]);
                    base_idx += 1;
                }
                theirs_lines[t.b0..t.b1]
                    .iter()
                    .for_each(|l| merged.push_str(l));
                base_idx = t.a1;
                ti += 1;
            }
            (Some(o), None) => {
                while base_idx < o.a0 && base_idx < base_lines.len() {
                    merged.push_str(base_lines[base_idx]);
                    base_idx += 1;
                }
                ours_lines[o.b0..o.b1]
                    .iter()
                    .for_each(|l| merged.push_str(l));
                base_idx = o.a1;
                oi += 1;
            }
            (None, Some(t)) => {
                while base_idx < t.a0 && base_idx < base_lines.len() {
                    merged.push_str(base_lines[base_idx]);
                    base_idx += 1;
                }
                theirs_lines[t.b0..t.b1]
                    .iter()
                    .for_each(|l| merged.push_str(l));
                base_idx = t.a1;
                ti += 1;
            }
            (None, None) => break,
        }
    }

    while base_idx < base_lines.len() {
        merged.push_str(base_lines[base_idx]);
        base_idx += 1;
    }

    MergeOutcome {
        clean: conflicts.is_empty(),
        merged_text: merged,
        conflicts,
        ours_changes: ours_regs.len(),
        theirs_changes: theirs_regs.len(),
    }
}

fn check_bracket_balance(text: &str, is_rust: bool) -> Result<(), String> {
    let bytes = text.as_bytes();
    let mut stack: Vec<(u8, usize)> = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i = skip_block_comment(bytes, i + 2, is_rust, &mut line)?;
            continue;
        }
        if is_rust && b == b'r' {
            if let Some(next_i) = try_skip_rust_raw_str(bytes, i, &mut line) {
                i = next_i;
                continue;
            }
        }
        if b == b'"' || (!is_rust && (b == b'\'' || b == b'`')) {
            i = skip_quoted(bytes, i + 1, b, &mut line)?;
            continue;
        }
        if is_rust && b == b'\'' {
            i = skip_rust_char_or_lifetime(bytes, i);
            continue;
        }
        if matches!(b, b'(' | b'{' | b'[') {
            stack.push((b, line));
        } else if matches!(b, b')' | b'}' | b']') {
            let expected = match b {
                b')' => b'(',
                b'}' => b'{',
                _ => b'[',
            };
            match stack.pop() {
                Some((open, _)) if open == expected => {}
                Some((open, open_line)) => {
                    return Err(format!(
                        "Mismatched delimiter '{}' at line {line} (expected close for '{}' from line {open_line})",
                        b as char, open as char
                    ));
                }
                None => {
                    return Err(format!("Unmatched closing '{}' at line {line}", b as char));
                }
            }
        }
        i += 1;
    }

    if let Some((open, open_line)) = stack.pop() {
        return Err(format!(
            "Unclosed delimiter '{}' opened at line {open_line}",
            open as char
        ));
    }
    Ok(())
}

fn skip_block_comment(
    bytes: &[u8],
    mut i: usize,
    nested: bool,
    line: &mut usize,
) -> Result<usize, String> {
    let mut depth = 1usize;
    let start_line = *line;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            *line += 1;
        } else if nested && bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            depth += 1;
            i += 1;
        } else if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return Ok(i);
            }
            continue;
        }
        i += 1;
    }
    Err(format!(
        "Unclosed block comment starting at line {start_line}"
    ))
}

fn try_skip_rust_raw_str(bytes: &[u8], i: usize, line: &mut usize) -> Option<usize> {
    let mut hashes = 0usize;
    let mut j = i + 1;
    while j < bytes.len() && bytes[j] == b'#' {
        hashes += 1;
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'"' {
        return None;
    }
    j += 1;
    while j < bytes.len() {
        if bytes[j] == b'\n' {
            *line += 1;
        } else if bytes[j] == b'"' {
            let mut match_h = 0usize;
            while match_h < hashes
                && j + 1 + match_h < bytes.len()
                && bytes[j + 1 + match_h] == b'#'
            {
                match_h += 1;
            }
            if match_h == hashes {
                return Some(j + 1 + hashes);
            }
        }
        j += 1;
    }
    Some(bytes.len())
}

fn skip_quoted(bytes: &[u8], mut i: usize, quote: u8, line: &mut usize) -> Result<usize, String> {
    let start_line = *line;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            *line += 1;
        } else if bytes[i] == b'\\' {
            i += 2;
            continue;
        } else if bytes[i] == quote {
            return Ok(i + 1);
        }
        i += 1;
    }
    Err(format!(
        "Unclosed string literal starting at line {start_line}"
    ))
}

fn skip_rust_char_or_lifetime(bytes: &[u8], i: usize) -> usize {
    if i + 2 < bytes.len() && bytes[i + 1] == b'\\' {
        if let Some(end) = (i + 2..bytes.len().min(i + 16)).find(|&k| bytes[k] == b'\'') {
            return end + 1;
        }
    } else if i + 2 < bytes.len() && bytes[i + 2] == b'\'' && bytes[i + 1] != b'\'' {
        return i + 3;
    }
    i + 1
}

/// 合并后执行目标语言语法完整性检验 (P1-02)。
pub fn check_syntax_integrity(language: &str, text: &str) -> Result<(), String> {
    for line in text.lines() {
        if line.starts_with("<<<<<<< ") || line.starts_with(">>>>>>> ") {
            return Err("Conflict marker detected in candidate content".into());
        }
    }
    match language.to_ascii_lowercase().as_str() {
        "nema" => nema_codec::parse_source(text)
            .map(|_| ())
            .map_err(|e| format!("NEMA syntax error: {e}")),
        "json" => serde_json::from_str::<serde_json::Value>(text)
            .map(|_| ())
            .map_err(|e| format!("JSON syntax error: {e}")),
        "rust" | "rs" => check_bracket_balance(text, true),
        "js" | "javascript" | "ts" | "typescript" | "c" | "cpp" | "java" | "go" => {
            check_bracket_balance(text, false)
        }
        _ => Ok(()),
    }
}
