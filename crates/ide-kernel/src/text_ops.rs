//! 查找 / 替换 —— 内核级实现 (不是 shell 层的字符串扫描).
//!
//! 与前端 `findInTab()` 的区别: 这里按 **char 位置** 工作, 结果可以直接喂给
//! `Document::selection()`, 因此外壳不需要自己解析行列。
//!
//! 刻意**不支持跨行匹配**: Sakura 默认也是行内查找, 跨行正则会让
//! 选区表示复杂化。跨行场景留给行内 regex 的显式开关。

use crate::document::{Document, Pos};

#[derive(Debug, Clone)]
pub struct FindOptions {
    pub pattern: String,
    pub case_sensitive: bool,
    pub regex: bool,
    /// 找不到时绕回文档开头 (Sakura 的 F3 行为).
    pub wrap: bool,
    /// 只在当前选区内搜索.
    pub within_selection: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            case_sensitive: false,
            regex: false,
            wrap: true,
            within_selection: false,
        }
    }
}

/// 一次匹配的位置.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindResult {
    /// 匹配起点 (char 位置).
    pub start: Pos,
    /// 匹配终点 (char 位置, 不含).
    pub end: Pos,
}

impl FindResult {
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// 在单个行内做匹配, 返回 char 区间的候选 (相对该行).
fn match_in_line(line: &str, opts: &FindOptions) -> Vec<(usize, usize)> {
    if opts.pattern.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = line.chars().collect();
    let pat: Vec<char> = opts.pattern.chars().collect();
    let mut out = Vec::new();

    if opts.regex {
        // 无 regex 依赖: 只支持字面量, 但保留接口以便将来接 regex crate.
        // 这里显式退回字面量匹配, 而不是编译失败 —— 前端的 :setre 不该让内核崩。
        let find_all = |hay: &[char], needle: &[char], cs: bool| -> Vec<usize> {
            let mut res = Vec::new();
            if needle.is_empty() || hay.len() < needle.len() {
                return res;
            }
            for i in 0..=(hay.len() - needle.len()) {
                let hit = if cs {
                    hay[i..i + needle.len()] == *needle
                } else {
                    hay[i..i + needle.len()]
                        .iter()
                        .zip(needle.iter())
                        .all(|(a, b)| {
                            a.eq_ignore_ascii_case(b) || a.to_lowercase().eq(b.to_lowercase())
                        })
                };
                if hit {
                    res.push(i);
                }
            }
            res
        };
        for i in find_all(&chars, &pat, opts.case_sensitive) {
            out.push((i, i + pat.len()));
        }
        return out;
    }

    let cs = opts.case_sensitive;
    for i in 0..=chars.len().saturating_sub(pat.len()) {
        if pat.is_empty() {
            break;
        }
        let hit = if cs {
            chars[i..i + pat.len()] == pat[..]
        } else {
            chars[i..i + pat.len()]
                .iter()
                .zip(pat.iter())
                .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
        };
        if hit {
            out.push((i, i + pat.len()));
        }
    }

    out
}

/// 在文档中查找第一处匹配 (从 `from` 之后开始).
///
/// `from` 语义: 只考虑**起始位置 >= from** 的匹配。绕回由调用方控制,
/// 这样调用方能实现 "找不到就重头再来" 的 UI 逻辑。
pub fn find_from(doc: &Document, opts: &FindOptions, from: Pos) -> Option<FindResult> {
    if opts.pattern.is_empty() {
        return None;
    }
    let (r0, r1) = if opts.within_selection {
        let sel = doc.selection()?;
        let n = sel.normalized();
        (n.start.row, n.end.row)
    } else {
        (0, doc.line_count() - 1)
    };

    for row in r0..=r1 {
        let line = doc.line_text(row);
        let hits = match_in_line(&line, opts);
        for (cs, ce) in hits {
            let start = Pos {
                row,
                col: cs as i32,
            };
            if pos_gte(start, from) {
                return Some(FindResult {
                    start,
                    end: Pos {
                        row,
                        col: ce as i32,
                    },
                });
            }
        }
    }
    None
}

/// 从文档开头重新查找 (F3 绕回).
pub fn find_from_start(doc: &Document, opts: &FindOptions) -> Option<FindResult> {
    let mut o = opts.clone();
    o.within_selection = false;
    find_from(doc, &o, Pos::ZERO)
}

/// 查找下一处: 先向后, 找不到且允许 wrap 则从头.
pub fn find_next(doc: &Document, opts: &FindOptions) -> Option<FindResult> {
    if let Some(sel) = doc.selection() {
        let n = sel.normalized();
        let after = Pos {
            row: n.end.row,
            col: n.end.col,
        };
        if let Some(r) = find_from(doc, opts, after) {
            return Some(r);
        }
    }
    if opts.wrap {
        return find_from_start(doc, opts);
    }
    None
}

/// 查找上一处 (Shift+F3 / 逆方向). 从当前选区**之前**开始找.
pub fn find_prev(doc: &Document, opts: &FindOptions) -> Option<FindResult> {
    let anchor = doc
        .selection()
        .map(|s| s.normalized().start)
        .unwrap_or(doc.caret());
    let mut best: Option<FindResult> = None;
    let rows = doc.line_count();
    for row in (0..rows).rev() {
        let line = doc.line_text(row);
        for (cs, ce) in match_in_line(&line, opts) {
            let start = Pos {
                row,
                col: cs as i32,
            };
            if pos_lt(start, anchor) {
                best = Some(FindResult {
                    start,
                    end: Pos {
                        row,
                        col: ce as i32,
                    },
                });
                break; // 本行更靠左的那个已找到
            }
        }
        if best.is_some() {
            break;
        }
    }
    best
}

/// 全部匹配 (用于「全部替换」与高亮).
pub fn find_all(doc: &Document, opts: &FindOptions, limit: usize) -> Vec<FindResult> {
    let mut out = Vec::new();
    if opts.pattern.is_empty() {
        return out;
    }
    for row in 0..doc.line_count() {
        for (cs, ce) in match_in_line(&doc.line_text(row), opts) {
            if out.len() >= limit {
                return out;
            }
            out.push(FindResult {
                start: Pos {
                    row,
                    col: cs as i32,
                },
                end: Pos {
                    row,
                    col: ce as i32,
                },
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplaceOutcome {
    pub replaced: usize,
    /// 是否触达 `max` 上限.
    pub truncated: bool,
}

/// 用 `replacement` 覆盖 `[start, end)` 范围.
///
/// 顺序必须是 **先删后插**: 先插会让后面的范围偏移失效.
/// 两步各自进 undo 栈, 因此一次替换 = 两条 undo 记录;
/// 这是刻意的 —— 与上游原本的 delete+insert 复合操作语义一致,
/// 用户按两次 Ctrl+Z 能分别撤销删除和插入.
fn replace_range(doc: &mut Document, start: Pos, end: Pos, replacement: &str) -> ReplaceOutcome {
    let range = editcore::cursor::LogicRange { start, end };
    if start != end {
        doc.delete_range(range);
    }
    let after = doc.insert(start, replacement);
    doc.set_caret(after);
    ReplaceOutcome {
        replaced: 1,
        truncated: false,
    }
}

/// 当前选区是否正好等于一处匹配.
fn selection_is_match(doc: &Document, opts: &FindOptions) -> Option<(Pos, Pos)> {
    let sel = doc.selection()?;
    let r = sel.normalized();
    let hits = match_in_line(&doc.line_text(r.start.row), opts);
    hits.iter()
        .find(|(cs, ce)| {
            Pos {
                row: r.start.row,
                col: *cs as i32,
            } == r.start
                && Pos {
                    row: r.start.row,
                    col: *ce as i32,
                } == r.end
        })
        .map(|(_, ce)| {
            (
                r.start,
                Pos {
                    row: r.start.row,
                    col: *ce as i32,
                },
            )
        })
}

/// 替换: 优先替换当前选区 (须正好是一处匹配), 否则替换下一处匹配.
pub fn replace_current(
    doc: &mut Document,
    opts: &FindOptions,
    replacement: &str,
) -> ReplaceOutcome {
    if let Some((s, e)) = selection_is_match(doc, opts) {
        return replace_range(doc, s, e, replacement);
    }
    match find_next(doc, opts) {
        Some(hit) => replace_range(doc, hit.start, hit.end, replacement),
        None => ReplaceOutcome {
            replaced: 0,
            truncated: false,
        },
    }
}

/// 全部替换. **倒序**执行, 避免前面的替换改变后面的位置.
pub fn replace_all(
    doc: &mut Document,
    opts: &FindOptions,
    replacement: &str,
    limit: usize,
) -> ReplaceOutcome {
    let hits = find_all(doc, opts, limit);
    let total = hits.len();
    for hit in hits.iter().rev() {
        replace_range(doc, hit.start, hit.end, replacement);
    }
    ReplaceOutcome {
        replaced: total,
        truncated: total >= limit && limit > 0,
    }
}

fn pos_gte(a: Pos, b: Pos) -> bool {
    a.row > b.row || (a.row == b.row && a.col >= b.col)
}

fn pos_lt(a: Pos, b: Pos) -> bool {
    a.row < b.row || (a.row == b.row && a.col < b.col)
}
