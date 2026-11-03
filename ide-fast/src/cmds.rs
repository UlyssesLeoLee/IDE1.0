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
    Cmd { group: "行", label: "剪切行", keys: "Ctrl+L", action: Edit("cut-line") },
    Cmd { group: "行", label: "复制行 (含换行)", keys: "", action: Edit("copy-lines") },
    Cmd { group: "行", label: "复制当前行", keys: "Ctrl+D", action: Edit("duplicate-line") },
    Cmd { group: "行", label: "删除当前行", keys: "Ctrl+Shift+K", action: Edit("delete-line") },
    Cmd { group: "行", label: "与上一行合并", keys: "", action: Edit("join-with-prev") },
    Cmd { group: "行", label: "在上方插入空行", keys: "Ctrl+Shift+Enter", action: Edit("add-line-above") },
    Cmd { group: "行", label: "在下方插入空行", keys: "Ctrl+Enter", action: Edit("add-line-below") },
    Cmd { group: "行", label: "上移一行", keys: "Alt+Up", action: Edit("move-line-up") },
    Cmd { group: "行", label: "下移一行", keys: "Alt+Down", action: Edit("move-line-down") },
    Cmd { group: "行", label: "剪切到行尾", keys: "", action: Edit("line-cut-to-end") },
    Cmd { group: "行", label: "剪切到行首", keys: "", action: Edit("cut-line-start") },
    Cmd { group: "行", label: "删除到行尾", keys: "", action: Edit("delete-line-to-end") },
    Cmd { group: "行", label: "删除到行首", keys: "", action: Edit("delete-line-to-start") },
    Cmd { group: "单词", label: "选择当前单词", keys: "", action: Edit("select-word") },
    Cmd { group: "单词", label: "剪切单词", keys: "", action: Edit("word-cut") },
    Cmd { group: "单词", label: "删除单词", keys: "", action: Edit("word-delete") },
    Cmd { group: "复制", label: "复制 (行尾统一 CRLF)", keys: "", action: Edit("copy-crlf") },
    Cmd { group: "复制", label: "复制 (带行号)", keys: "", action: Edit("copy-linenum") },
    Cmd { group: "复制", label: "复制引用 (> 前缀)", keys: "", action: Edit("copy-passage") },
    Cmd { group: "复制", label: "矩形粘贴", keys: "", action: Edit("paste-box") },
    Cmd { group: "转换", label: "转为大写", keys: "Ctrl+Shift+U", action: Transform("upper") },
    Cmd { group: "转换", label: "转为小写", keys: "Ctrl+U", action: Transform("lower") },
    Cmd { group: "转换", label: "半角 → 全角", keys: "", action: Transform("full") },
    Cmd { group: "转换", label: "全角 → 半角", keys: "", action: Transform("half") },
    Cmd { group: "转换", label: "半角 → 全角 (含假名)", keys: "Ctrl+F9", action: Transform("to-zenkaku") },
    Cmd { group: "转换", label: "全角 → 半角 (含假名)", keys: "Ctrl+F8", action: Transform("to-halfwidth") },
    Cmd { group: "转换", label: "转为全角平假名", keys: "Ctrl+F10", action: Transform("to-zenhira") },
    Cmd { group: "转换", label: "转为半角片假名", keys: "", action: Transform("to-hankata") },
    Cmd { group: "转换", label: "半角假名 → 全角片假名", keys: "Ctrl+F11", action: Transform("hankata-to-zenkata") },
    Cmd { group: "转换", label: "半角假名 → 全角平假名", keys: "Ctrl+F12", action: Transform("hankata-to-zenhira") },
    Cmd { group: "转换", label: "半角英数 → 全角英数", keys: "", action: Transform("to-zenei") },
    Cmd { group: "转换", label: "全角英数 → 半角英数", keys: "", action: Transform("to-hanei") },
    Cmd { group: "转换", label: "Tab → 空格", keys: "", action: Transform("tab2space") },
    Cmd { group: "转换", label: "行首空格 → Tab", keys: "", action: Transform("space2tab") },
    Cmd { group: "整理行", label: "升序排序", keys: "", action: Lines("sort-asc") },
    Cmd { group: "整理行", label: "降序排序", keys: "", action: Lines("sort-desc") },
    Cmd { group: "整理行", label: "去重 (保留首次出现)", keys: "", action: Lines("unique") },
    Cmd { group: "整理行", label: "反转行顺序", keys: "", action: Lines("reverse") },
    Cmd { group: "整理行", label: "去除行尾空白", keys: "", action: Lines("trim-end") },
    Cmd { group: "整理行", label: "去除行首空白", keys: "", action: Lines("trim-start") },
    Cmd { group: "整理行", label: "删除空行", keys: "", action: Lines("drop-empty") },
    Cmd { group: "缩进", label: "增加缩进", keys: "", action: Indent },
    Cmd { group: "缩进", label: "减少缩进", keys: "Shift+Tab", action: Outdent },
    Cmd { group: "", label: "切换行注释", keys: "Ctrl+/", action: ToggleComment },
    Cmd { group: "", label: "转到行…", keys: "Ctrl+J", action: GotoLine },
    Cmd { group: "", label: "改写 / 插入模式", keys: "Insert", action: ToggleOverwrite },
    Cmd { group: "", label: "只读", keys: "", action: ToggleReadonly },
    Cmd { group: "编码转换", label: "自动判别 → SJIS", keys: "Alt+F12", action: CodeCnv("auto2sjis") },
    Cmd { group: "编码转换", label: "邮件 (JIS) → SJIS", keys: "Alt+F11", action: CodeCnv("email") },
    Cmd { group: "编码转换", label: "EUC → SJIS", keys: "Alt+F10", action: CodeCnv("euc2sjis") },
    Cmd { group: "编码转换", label: "Unicode → SJIS", keys: "Alt+F9", action: CodeCnv("unicode2sjis") },
    Cmd { group: "编码转换", label: "UnicodeBE → SJIS", keys: "", action: CodeCnv("unicodebe2sjis") },
    Cmd { group: "编码转换", label: "UTF-8 → SJIS", keys: "Alt+F8", action: CodeCnv("utf82sjis") },
    Cmd { group: "编码转换", label: "UTF-7 → SJIS", keys: "Alt+F7", action: CodeCnv("utf72sjis") },
    Cmd { group: "编码转换", label: "SJIS → JIS", keys: "Alt+Shift+F11", action: CodeCnv("sjis2jis") },
    Cmd { group: "编码转换", label: "SJIS → EUC", keys: "Alt+Shift+F10", action: CodeCnv("sjis2euc") },
    Cmd { group: "编码转换", label: "SJIS → UTF-8", keys: "Alt+Shift+F8", action: CodeCnv("sjis2utf8") },
    Cmd { group: "编码转换", label: "SJIS → UTF-7", keys: "Alt+Shift+F7", action: CodeCnv("sjis2utf7") },
    Cmd { group: "Markdown", label: "粗体", keys: "Ctrl+Alt+B", action: Edit("md-bold") },
    Cmd { group: "Markdown", label: "斜体", keys: "Ctrl+Alt+I", action: Edit("md-italic") },
    Cmd { group: "Markdown", label: "删除线", keys: "", action: Edit("md-strike") },
    Cmd { group: "Markdown", label: "行内代码", keys: "", action: Edit("md-code") },
    Cmd { group: "Markdown", label: "高亮", keys: "", action: Edit("md-highlight") },
    Cmd { group: "Markdown", label: "标题 1", keys: "", action: Edit("md-h1") },
    Cmd { group: "Markdown", label: "标题 2", keys: "", action: Edit("md-h2") },
    Cmd { group: "Markdown", label: "标题 3", keys: "", action: Edit("md-h3") },
    Cmd { group: "Markdown", label: "标题 4", keys: "", action: Edit("md-h4") },
    Cmd { group: "Markdown", label: "引用", keys: "", action: Edit("md-quote") },
    Cmd { group: "Markdown", label: "无序列表", keys: "", action: Edit("md-ul") },
    Cmd { group: "Markdown", label: "有序列表", keys: "", action: Edit("md-ol") },
    Cmd { group: "Markdown", label: "任务列表", keys: "", action: Edit("md-task") },
    Cmd { group: "Markdown", label: "链接", keys: "", action: Edit("md-link") },
    Cmd { group: "Markdown", label: "图片", keys: "", action: Edit("md-image") },
    Cmd { group: "Markdown", label: "代码块", keys: "", action: Edit("md-codeblock") },
    Cmd { group: "Markdown", label: "Mermaid 图", keys: "", action: Edit("md-mermaid") },
    Cmd { group: "Markdown", label: "表格", keys: "", action: Edit("md-table") },
    Cmd { group: "Markdown", label: "分隔线", keys: "", action: Edit("md-hr") },
    Cmd { group: "Markdown", label: "提示块", keys: "", action: Edit("md-callout") },
];

// ---------------------------------------------------------------- 纯文本变换 (可单测)

pub fn transform(kind: &str, text: &str, tab: usize) -> String {
    match kind {
        "upper" => text.to_uppercase(),
        "lower" => text.to_lowercase(),
        "full" => editcore::zenkaku_hankaku::to_fullwidth(text),
        "half" => editcore::zenkaku_hankaku::to_halfwidth(text),
        // 假名 / 英数的全半角互转: 语义在 editcore::edit_ops (Sakura 实测), 这里不再抄第二份
        "to-zenkaku" | "to-halfwidth" | "to-zenhira" | "to-hankata" | "hankata-to-zenkata" | "hankata-to-zenhira" | "to-zenei" | "to-hanei" => {
            editcore::edit_ops::transform_text(kind, text, tab).unwrap_or_else(|| text.to_string())
        }
        "tab2space" => text.replace('\t', &" ".repeat(tab.max(1))),
        "space2tab" => text
            .split('\n')
            .map(|l| {
                let lead = l.chars().take_while(|c| *c == ' ').count();
                let tabs = lead / tab.max(1);
                format!("{}{}", "\t".repeat(tabs), &l[tabs * tab.max(1)..])
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => text.to_string(),
    }
}

pub fn lines_op(kind: &str, lines: Vec<String>) -> Vec<String> {
    match kind {
        "sort-asc" => {
            let mut v = lines;
            v.sort();
            v
        }
        "sort-desc" => {
            let mut v = lines;
            v.sort();
            v.reverse();
            v
        }
        "unique" => {
            let mut seen = std::collections::HashSet::new();
            lines.into_iter().filter(|l| seen.insert(l.clone())).collect()
        }
        "reverse" => lines.into_iter().rev().collect(),
        "trim-end" => lines.into_iter().map(|l| l.trim_end().to_string()).collect(),
        "trim-start" => lines.into_iter().map(|l| l.trim_start().to_string()).collect(),
        "drop-empty" => lines.into_iter().filter(|l| !l.trim().is_empty()).collect(),
        _ => lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_edit_command_exists_in_editcore() {
        for c in CMDS {
            if let Edit(name) = c.action {
                let req = editcore::edit_cmd::EditReq { cmd: name.to_string(), total_rows: 1, rows: vec![(0, "x".into())], ..Default::default() };
                assert!(editcore::edit_cmd::run(&req).is_some(), "editcore 不认识命令 {name}");
            }
        }
    }

    #[test]
    fn transforms_and_line_ops() {
        assert_eq!(transform("upper", "aBc", 4), "ABC");
        assert_eq!(transform("tab2space", "a\tb", 2), "a  b");
        assert_eq!(transform("space2tab", "    x\n  y", 4), "\tx\n  y");
        assert_eq!(transform("full", "ab1", 4), "ａｂ１");
        // 假名互转走 editcore::edit_ops
        assert_eq!(transform("to-zenhira", "ｶﾀｶﾅ カタカナ", 4), "かたかな　かたかな");
        assert_eq!(transform("to-hankata", "カタカナガパ", 4), "ｶﾀｶﾅｶﾞﾊﾟ");
        assert_eq!(transform("hankata-to-zenkata", "ｶﾞﾊﾟ", 4), "ガパ");
        assert_eq!(transform("hankata-to-zenhira", "ｶﾞﾊﾟ", 4), "がぱ");
        assert_eq!(transform("to-zenei", "ab1", 4), "ａｂ１");
        assert_eq!(transform("to-hanei", "ａｂ１", 4), "ab1");
        assert_eq!(transform("no-such-op", "x", 4), "x");
        for c in CMDS {
            if let Transform(k) = c.action {
                assert!(["upper", "lower", "full", "half", "tab2space", "space2tab"].contains(&k) || editcore::edit_ops::transform_text(k, "a", 4).is_some(), "editcore 不认识变换 {k}");
            }
        }
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(lines_op("sort-asc", v(&["b", "a", "c"])), v(&["a", "b", "c"]));
        assert_eq!(lines_op("unique", v(&["a", "b", "a"])), v(&["a", "b"]));
        assert_eq!(lines_op("trim-end", v(&["a  ", " b "])), v(&["a", " b"]));
        assert_eq!(lines_op("drop-empty", v(&["a", " ", "b"])), v(&["a", "b"]));
    }
}
