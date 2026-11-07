//! 命令表: 菜单项与快捷键都从这里生成 (一处定义, 菜单文字里的快捷键提示和实际绑定不会对不上)。
//! 语义不在这里: `Edit` 交给 editcore 的 `edit_cmd`, `Transform` / `Lines` 是纯文本函数 (见下), 其余是窗口级动作。

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// editcore `edit_cmd` 的命令名
    Edit(&'static str),
    /// 对选区文字做变换
    Transform(&'static str),
    /// 对选中的行 (没有选区 = 全文) 做整行变换
    Lines(&'static str),
    Indent,
    Outdent,
    ToggleComment,
    GotoLine,
    ToggleOverwrite,
    ToggleReadonly,
    OpenReviewDoc,
    TriggerCoeditDemo,
    RunCollabPipeline,
    /// 选区 (没有 = 整篇) 的编码内容转换, 后缀见 `textconv::CODECNV`
    CodeCnv(&'static str),
}

pub struct Cmd {
    pub group: &'static str,
    pub label: &'static str,
    /// 形如 `Ctrl+Shift+K`; 空 = 只有菜单项
    pub keys: &'static str,
    pub action: Action,
}

use Action::*;

/// 菜单里按 group 分成子菜单 (group 为空 = 直接放在「编辑」下)
pub const CMDS: &[Cmd] = &[
    Cmd {
        group: "行",
        label: "剪切行",
        keys: "Ctrl+L",
        action: Edit("cut-line"),
    },
    Cmd {
        group: "行",
        label: "复制行 (含换行)",
        keys: "",
        action: Edit("copy-lines"),
    },
    Cmd {
        group: "行",
        label: "复制当前行",
        keys: "Ctrl+D|Shift+F10",
        action: Edit("duplicate-line"),
    },
    Cmd {
        group: "行",
        label: "删除当前行",
        keys: "Ctrl+Shift+K",
        action: Edit("delete-line"),
    },
    Cmd {
        group: "行",
        label: "删除上方行",
        keys: "",
        action: Edit("delete-line-up"),
    },
    Cmd {
        group: "行",
        label: "删除下方行",
        keys: "",
        action: Edit("delete-line-down"),
    },
    Cmd {
        group: "行",
        label: "与上一行合并",
        keys: "",
        action: Edit("join-with-prev"),
    },
    Cmd {
        group: "行",
        label: "在上方插入空行",
        keys: "Ctrl+Shift+Enter",
        action: Edit("add-line-above"),
    },
    Cmd {
        group: "行",
        label: "在下方插入空行",
        keys: "Ctrl+Enter",
        action: Edit("add-line-below"),
    },
    Cmd {
        group: "行",
        label: "上移一行",
        keys: "Alt+Up",
        action: Edit("move-line-up"),
    },
    Cmd {
        group: "行",
        label: "下移一行",
        keys: "Alt+Down",
        action: Edit("move-line-down"),
    },
    Cmd {
        group: "行",
        label: "剪切到行尾",
        keys: "",
        action: Edit("line-cut-to-end"),
    },
    Cmd {
        group: "行",
        label: "剪切到行首",
        keys: "",
        action: Edit("cut-line-start"),
    },
    Cmd {
        group: "行",
        label: "删除到行尾",
        keys: "",
        action: Edit("delete-line-to-end"),
    },
    Cmd {
        group: "行",
        label: "删除到行首",
        keys: "",
        action: Edit("delete-line-to-start"),
    },
    Cmd {
        group: "单词",
        label: "选择当前单词",
        keys: "",
        action: Edit("select-word"),
    },
    Cmd {
        group: "单词",
        label: "剪切单词",
        keys: "",
        action: Edit("word-cut"),
    },
    Cmd {
        group: "单词",
        label: "删除单词",
        keys: "Ctrl+Shift+D",
        action: Edit("word-delete"),
    },
    Cmd {
        group: "复制",
        label: "复制 (行尾统一 CRLF)",
        keys: "Shift+F8",
        action: Edit("copy-crlf"),
    },
    Cmd {
        group: "复制",
        label: "复制 (带行号)",
        keys: "",
        action: Edit("copy-linenum"),
    },
    Cmd {
        group: "复制",
        label: "复制引用 (> 前缀)",
        keys: "",
        action: Edit("copy-passage"),
    },
    Cmd {
        group: "复制",
        label: "矩形粘贴",
        keys: "",
        action: Edit("paste-box"),
    },
    Cmd {
        group: "转换",
        label: "转为大写",
        keys: "Ctrl+Shift+U|Ctrl+F7",
        action: Transform("upper"),
    },
    Cmd {
        group: "转换",
        label: "转为小写",
        keys: "Ctrl+U|Ctrl+F6",
        action: Transform("lower"),
    },
    Cmd {
        group: "转换",
        label: "半角 → 全角",
        keys: "",
        action: Transform("full"),
    },
    Cmd {
        group: "转换",
        label: "全角 → 半角",
        keys: "",
        action: Transform("half"),
    },
    Cmd {
        group: "转换",
        label: "半角 → 全角 (含假名)",
        keys: "Ctrl+F9",
        action: Transform("to-zenkaku"),
    },
    Cmd {
        group: "转换",
        label: "全角 → 半角 (含假名)",
        keys: "Ctrl+F8",
        action: Transform("to-halfwidth"),
    },
    Cmd {
        group: "转换",
        label: "转为全角平假名",
        keys: "Ctrl+F10",
        action: Transform("to-zenhira"),
    },
    Cmd {
        group: "转换",
        label: "转为半角片假名",
        keys: "",
        action: Transform("to-hankata"),
    },
    Cmd {
        group: "转换",
        label: "半角假名 → 全角片假名",
        keys: "Ctrl+F11",
        action: Transform("hankata-to-zenkata"),
    },
    Cmd {
        group: "转换",
        label: "半角假名 → 全角平假名",
        keys: "Ctrl+F12",
        action: Transform("hankata-to-zenhira"),
    },
    Cmd {
        group: "转换",
        label: "半角英数 → 全角英数",
        keys: "",
        action: Transform("to-zenei"),
    },
    Cmd {
        group: "转换",
        label: "全角英数 → 半角英数",
        keys: "",
        action: Transform("to-hanei"),
    },
    Cmd {
        group: "转换",
        label: "Tab → 空格",
        keys: "Ctrl+Alt+F5",
        action: Transform("tab2space"),
    },
    Cmd {
        group: "转换",
        label: "行首空格 → Tab",
        keys: "Ctrl+Alt+Shift+F5",
        action: Transform("space2tab"),
    },
    Cmd {
        group: "整理行",
        label: "升序排序",
        keys: "Alt+A",
        action: Lines("sort-asc"),
    },
    Cmd {
        group: "整理行",
        label: "降序排序",
        keys: "Alt+D",
        action: Lines("sort-desc"),
    },
    Cmd {
        group: "整理行",
        label: "去重 (保留首次出现)",
        keys: "",
        action: Lines("unique"),
    },
    Cmd {
        group: "整理行",
        label: "反转行顺序",
        keys: "",
        action: Lines("reverse"),
    },
    Cmd {
        group: "整理行",
        label: "去除行尾空白",
        keys: "Alt+R",
        action: Lines("trim-end"),
    },
    Cmd {
        group: "整理行",
        label: "去除行首空白",
        keys: "Alt+L",
        action: Lines("trim-start"),
    },
    Cmd {
        group: "整理行",
        label: "合并连续重复行",
        keys: "",
        action: Lines("uniq-consecutive"),
    },
    Cmd {
        group: "整理行",
        label: "删除空行",
        keys: "",
        action: Lines("drop-empty"),
    },
    Cmd {
        group: "缩进",
        label: "增加缩进",
        keys: "",
        action: Indent,
    },
    Cmd {
        group: "缩进",
        label: "减少缩进",
        keys: "Shift+Tab",
        action: Outdent,
    },
    Cmd {
        group: "",
        label: "切换行注释",
        keys: "Ctrl+/",
        action: ToggleComment,
    },
    Cmd {
        group: "",
        label: "转到行…",
        keys: "Ctrl+J",
        action: GotoLine,
    },
    Cmd {
        group: "",
        label: "改写 / 插入模式",
        keys: "Insert",
        action: ToggleOverwrite,
    },
    Cmd {
        group: "",
        label: "只读",
        keys: "",
        action: ToggleReadonly,
    },
    Cmd {
        group: "编码转换",
        label: "自动判别 → SJIS",
        keys: "Alt+F12",
        action: CodeCnv("auto2sjis"),
    },
    Cmd {
        group: "编码转换",
        label: "邮件 (JIS) → SJIS",
        keys: "Alt+F11",
        action: CodeCnv("email"),
    },
    Cmd {
        group: "编码转换",
        label: "EUC → SJIS",
        keys: "Alt+F10",
        action: CodeCnv("euc2sjis"),
    },
    Cmd {
        group: "编码转换",
        label: "Unicode → SJIS",
        keys: "Alt+F9",
        action: CodeCnv("unicode2sjis"),
    },
    Cmd {
        group: "编码转换",
        label: "UnicodeBE → SJIS",
        keys: "",
        action: CodeCnv("unicodebe2sjis"),
    },
    Cmd {
        group: "编码转换",
        label: "UTF-8 → SJIS",
        keys: "Alt+F8",
        action: CodeCnv("utf82sjis"),
    },
    Cmd {
        group: "编码转换",
        label: "UTF-7 → SJIS",
        keys: "Alt+F7",
        action: CodeCnv("utf72sjis"),
    },
    Cmd {
        group: "编码转换",
        label: "SJIS → JIS",
        keys: "Alt+Shift+F11",
        action: CodeCnv("sjis2jis"),
    },
    Cmd {
        group: "编码转换",
        label: "SJIS → EUC",
        keys: "Alt+Shift+F10",
        action: CodeCnv("sjis2euc"),
    },
    Cmd {
        group: "编码转换",
        label: "SJIS → UTF-8",
        keys: "Alt+Shift+F8",
        action: CodeCnv("sjis2utf8"),
    },
    Cmd {
        group: "编码转换",
        label: "SJIS → UTF-7",
        keys: "Alt+Shift+F7",
        action: CodeCnv("sjis2utf7"),
    },
    Cmd {
        group: "Markdown",
        label: "粗体",
        keys: "Ctrl+Alt+B",
        action: Edit("md-bold"),
    },
    Cmd {
        group: "Markdown",
        label: "斜体",
        keys: "Ctrl+Alt+I",
        action: Edit("md-italic"),
    },
    Cmd {
        group: "Markdown",
        label: "删除线",
        keys: "",
        action: Edit("md-strike"),
    },
    Cmd {
        group: "Markdown",
        label: "行内代码",
        keys: "",
        action: Edit("md-code"),
    },
    Cmd {
        group: "Markdown",
        label: "高亮",
        keys: "",
        action: Edit("md-highlight"),
    },
    Cmd {
        group: "Markdown",
        label: "标题 1",
        keys: "",
        action: Edit("md-h1"),
    },
    Cmd {
        group: "Markdown",
        label: "标题 2",
        keys: "",
        action: Edit("md-h2"),
    },
    Cmd {
        group: "Markdown",
        label: "标题 3",
        keys: "",
        action: Edit("md-h3"),
    },
    Cmd {
        group: "Markdown",
        label: "标题 4",
        keys: "",
        action: Edit("md-h4"),
    },
    Cmd {
        group: "Markdown",
        label: "引用",
        keys: "",
        action: Edit("md-quote"),
    },
    Cmd {
        group: "Markdown",
        label: "无序列表",
        keys: "",
        action: Edit("md-ul"),
    },
    Cmd {
        group: "Markdown",
        label: "有序列表",
        keys: "",
        action: Edit("md-ol"),
    },
    Cmd {
        group: "Markdown",
        label: "任务列表",
        keys: "",
        action: Edit("md-task"),
    },
    Cmd {
        group: "Markdown",
        label: "链接",
        keys: "",
        action: Edit("md-link"),
    },
    Cmd {
        group: "Markdown",
        label: "图片",
        keys: "",
        action: Edit("md-image"),
    },
    Cmd {
        group: "Markdown",
        label: "代码块",
        keys: "",
        action: Edit("md-codeblock"),
    },
    Cmd {
        group: "Markdown",
        label: "Mermaid 图",
        keys: "",
        action: Edit("md-mermaid"),
    },
    Cmd {
        group: "Markdown",
        label: "表格",
        keys: "",
        action: Edit("md-table"),
    },
    Cmd {
        group: "Markdown",
        label: "分隔线",
        keys: "",
        action: Edit("md-hr"),
    },
    Cmd {
        group: "Markdown",
        label: "提示块",
        keys: "",
        action: Edit("md-callout"),
    },
    Cmd {
        group: "AI 协同",
        label: "协同审查看板",
        keys: "Ctrl+Shift+R",
        action: OpenReviewDoc,
    },
    Cmd {
        group: "AI 协同",
        label: "模拟多 AI 协同编辑",
        keys: "",
        action: TriggerCoeditDemo,
    },
    Cmd {
        group: "AI 协同",
        label: "运行四阶段协同流水线",
        keys: "",
        action: RunCollabPipeline,
    },
];

// ---------------------------------------------------------------- 纯文本变换 (可单测)

pub fn transform(kind: &str, text: &str, tab: usize) -> String {
    match kind {
        "full" => editcore::zenkaku_hankaku::to_fullwidth(text),
        "half" => editcore::zenkaku_hankaku::to_halfwidth(text),
        // 大小写 / 假名 / 英数 / Tab 与空格: 语义都在 editcore::edit_ops (Sakura 实测), 这里不再抄第二份; 这边的老名字对到 editcore 的名字
        k => {
            let core = match k {
                "upper" => "to-upper",
                "lower" => "to-lower",
                "tab2space" => "tab-to-space",
                "space2tab" => "space-to-tab",
                other => other,
            };
            editcore::edit_ops::transform_text(core, text, tab).unwrap_or_else(|| text.to_string())
        }
    }
}

pub fn lines_op(kind: &str, lines: Vec<String>) -> Vec<String> {
    // 排序 / 去行首行尾空白 / 合并连续重复行 / SPACE 缩进: 语义在 editcore::edit_ops (Sakura 实测; 去空白只去空格和 Tab, 排序按 UTF-16 顺序)
    let core = match kind {
        "trim-start" => "ltrim-lines",
        "trim-end" => "rtrim-lines",
        other => other,
    };
    if let Some(v) = editcore::edit_ops::transform_lines(core, &lines) {
        return v;
    }
    match kind {
        "unique" => {
            let mut seen = std::collections::HashSet::new();
            lines
                .into_iter()
                .filter(|l| seen.insert(l.clone()))
                .collect()
        }
        "reverse" => lines.into_iter().rev().collect(),
        "drop-empty" => lines.into_iter().filter(|l| !l.trim().is_empty()).collect(),
        _ => lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_line_above_and_below_remove_the_neighbour_not_the_current_line() {
        let mut d = crate::doc::Doc::untitled(1, 1);
        d.lines = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        d.goto(2, 1);
        d.edit("delete-line-up", None);
        assert_eq!(d.lines, ["a", "c", "d"], "删掉光标行上面那一行 (b)");
        d.goto(0, 0);
        d.edit("delete-line-down", None);
        assert_eq!(d.lines, ["a", "d"], "删掉光标行下面那一行 (c)");
        for label in ["删除上方行", "删除下方行"] {
            assert!(
                CMDS.iter().any(|c| c.label == label),
                "命令表里要有「{label}」"
            );
        }
    }

    #[test]
    fn every_edit_command_exists_in_editcore() {
        for c in CMDS {
            if let Edit(name) = c.action {
                let req = editcore::edit_cmd::EditReq {
                    cmd: name.to_string(),
                    total_rows: 1,
                    rows: vec![(0, "x".into())],
                    ..Default::default()
                };
                assert!(
                    editcore::edit_cmd::run(&req).is_some(),
                    "editcore 不认识命令 {name}"
                );
            }
        }
    }

    #[test]
    fn transforms_and_line_ops() {
        assert_eq!(transform("upper", "aBc", 4), "ABC");
        assert_eq!(
            transform("tab2space", "a\tb", 2),
            "a b",
            "Tab 展开到下一个制表位 (Sakura 实测), 不是固定换成 N 个空格"
        );
        assert_eq!(transform("tab2space", "a\tb", 4), "a   b");
        assert_eq!(transform("lower", "ÀB", 4), "àb");
        assert_eq!(transform("space2tab", "    x\n  y", 4), "\tx\n  y");
        assert_eq!(transform("full", "ab1", 4), "ａｂ１");
        // 假名互转走 editcore::edit_ops
        assert_eq!(
            transform("to-zenhira", "ｶﾀｶﾅ カタカナ", 4),
            "かたかな　かたかな"
        );
        assert_eq!(transform("to-hankata", "カタカナガパ", 4), "ｶﾀｶﾅｶﾞﾊﾟ");
        assert_eq!(transform("hankata-to-zenkata", "ｶﾞﾊﾟ", 4), "ガパ");
        assert_eq!(transform("hankata-to-zenhira", "ｶﾞﾊﾟ", 4), "がぱ");
        assert_eq!(transform("to-zenei", "ab1", 4), "ａｂ１");
        assert_eq!(transform("to-hanei", "ａｂ１", 4), "ab1");
        assert_eq!(transform("no-such-op", "x", 4), "x");
        for c in CMDS {
            if let Transform(k) = c.action {
                assert!(
                    ["upper", "lower", "full", "half", "tab2space", "space2tab"].contains(&k)
                        || editcore::edit_ops::transform_text(k, "a", 4).is_some(),
                    "editcore 不认识变换 {k}"
                );
            }
        }
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            lines_op("sort-asc", v(&["b", "a", "c"])),
            v(&["a", "b", "c"])
        );
        assert_eq!(lines_op("unique", v(&["a", "b", "a"])), v(&["a", "b"]));
        assert_eq!(lines_op("trim-end", v(&["a  ", " b "])), v(&["a", " b"]));
        assert_eq!(lines_op("drop-empty", v(&["a", " ", "b"])), v(&["a", "b"]));
    }
}
