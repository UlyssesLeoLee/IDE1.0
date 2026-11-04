//! 关键字帮助 (同 Tauri 的 `help-search` / `ext-help` / `toggle-key-search`): 在内置手册里查一个词。
//! 手册 = `help.txt` (讲概念) + 「命令一览」(从命令表现场生成, 用的是此刻的标签和键位, 所以不会和实际脱节)。
//! 这里只准备文字和做检索, 不碰界面 / 文档。

use crate::keymap;
use crate::pick::Row;

/// 手册开成标签时的名字
pub const MANUAL_NAME: &str = "帮助手册.txt";

/// 自动显示用的词至少这么长 (一个字母几乎每行都命中, 提示就没有意义了)
pub const AUTO_MIN_CHARS: usize = 2;

/// 提示里最多显示这么多字符
const TIP_MAX_CHARS: usize = 120;

const BODY: &str = include_str!("help.txt");

/// 正文, 换行统一成 LF (Windows 上 git 的 autocrlf 会把 help.txt 检出成 CRLF, include_str! 原样带进来)
fn body() -> String {
    BODY.replace("\r\n", "\n")
}

/// 命令一览: 每个命令一行 `名称 <TAB> 键 <TAB> [id] <TAB> 分类`; 没有键的写 `-`
pub fn command_lines() -> Vec<String> {
    keymap::entries()
        .iter()
        .map(|e| {
            let k = keymap::display_entry(e);
            let k = if k.is_empty() { "-".to_string() } else { k };
            format!("{}\t{}\t[{}]\t{}", e.label.text(), k, e.id, e.cat.text())
        })
        .collect()
}

/// 整本手册
pub fn manual() -> String {
    let mut out = body().trim_end_matches('\n').to_string();
    out.push_str("\n\n[11] 命令一览\n-------------\n名称\t键\t[命令 id]\t分类\n");
    for l in command_lines() {
        out.push_str(&l);
        out.push('\n');
    }
    out
}

/// 手册里每个非空行一行: 显示文字去掉首尾空白, 右侧灰字写 `L行号`, 值 (`Row::value`) 是行号 (从 1 数)
pub fn rows(manual: &str) -> Vec<Row> {
    manual
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| Row::new(l.trim().replace('\t', "  "), format!("L{}", i + 1)).with_value((i + 1).to_string()))
        .collect()
}

/// 光标处的单词, 按 Sakura 的词模型 (`editcore::word::word_run`: 漢字 / ひらがな / カタカナ / 英数 各自成词);
/// 光标在单词末尾也算; 空白和记号不是词; 没有 = 空串
pub fn word_at(line: &str, col: usize) -> String {
    use editcore::word::{char_kind, word_run, CharKind};
    let cs: Vec<char> = line.chars().collect();
    let col = col.min(cs.len());
    let wordy = |i: usize| cs.get(i).is_some_and(|&c| !matches!(char_kind(c), CharKind::Space | CharKind::ZenSpace | CharKind::Kigo));
    let at = if wordy(col) { col } else if col > 0 && wordy(col - 1) { col - 1 } else { return String::new() };
    let (a, b) = word_run(&cs, at);
    cs[a..b].iter().collect()
}

/// 手册里第一处提到 `word` 的行 (忽略大小写): (行号 从 1 数, 去掉首尾空白的行)。空词没有结果
pub fn first_hit(manual: &str, word: &str) -> Option<(usize, String)> {
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return None;
    }
    manual.lines().enumerate().find(|(_, l)| l.to_lowercase().contains(&w)).map(|(i, l)| (i + 1, l.trim().replace('\t', "  ")))
}

/// 自动显示的提示 (状态栏): `帮助: <行>`; 词太短 / 没有命中 = None
pub fn auto_tip(manual: &str, word: &str) -> Option<String> {
    if word.chars().count() < AUTO_MIN_CHARS {
        return None;
    }
    let (_, line) = first_hit(manual, word)?;
    let mut t: String = line.chars().take(TIP_MAX_CHARS).collect();
    if line.chars().count() > TIP_MAX_CHARS {
        t.push('…');
    }
    Some(format!("帮助: {t}"))
}

/// 状态栏自动提示用: 先查正文, 正文里没有再查命令一览 (命令一览要现场生成, 比较贵, 能不生成就不生成)
pub fn tip_for(word: &str) -> Option<String> {
    auto_tip(&body(), word).or_else(|| auto_tip(&command_lines().join("\n"), word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_has_the_prose_and_every_command_once() {
        let m = manual();
        assert!(m.starts_with("IDE1.0 高速版"), "正文在前");
        let ids: Vec<String> = keymap::entries().iter().map(|e| format!("[{}]", e.id)).collect();
        for id in &ids {
            assert_eq!(m.matches(id.as_str()).count(), 1, "命令一览里 {id} 必须恰好出现一次");
        }
        assert!(m.contains("[c.palette]") && m.contains("Shift+F1"), "键位是现场从命令表读的");
    }

    #[test]
    fn rows_skip_blank_lines_and_carry_the_line_number() {
        let rs = rows("甲\n\n  乙\t丙  \n");
        assert_eq!(rs.len(), 2);
        assert_eq!((rs[0].label.as_str(), rs[0].value.as_str()), ("甲", "1"));
        assert_eq!((rs[1].label.as_str(), rs[1].value.as_str(), rs[1].detail.as_str()), ("乙  丙", "3", "L3"), "空行占行号, TAB 变两个空格");
    }

    #[test]
    fn rows_of_the_real_manual_point_at_their_own_text() {
        let m = manual();
        let lines: Vec<&str> = m.lines().collect();
        for r in rows(&m) {
            let n: usize = r.value.parse().expect("每一行的值都是行号");
            assert_eq!(lines[n - 1].trim().replace('\t', "  "), r.label);
        }
    }

    #[test]
    fn word_at_handles_edges_and_cjk() {
        assert_eq!(word_at("let foo_bar = 1;", 5), "foo_bar");
        assert_eq!(word_at("let foo_bar = 1;", 3), "let", "光标在单词末尾也算");
        assert_eq!(word_at("let foo_bar = 1;", 4), "foo_bar", "光标在单词开头");
        assert_eq!(word_at("a  b", 2), "", "两个空格之间没有词");
        assert_eq!(word_at("再変換 mode", 1), "再変換");
        assert_eq!(word_at("abc", 99), "abc", "列号越界不 panic");
        assert_eq!(word_at("", 0), "");
        assert_eq!(word_at("これは漢字です", 4), "漢字", "Sakura 的词: 漢字和ひらがな分开");
        assert_eq!(word_at("x = (y)", 4), "", "记号上不算词");
    }

    #[test]
    fn crlf_in_the_prose_is_normalized() {
        assert!(!manual().contains('\r'));
        assert!(!body().contains('\r'));
    }

    #[test]
    fn first_hit_is_case_insensitive_and_takes_the_earliest_line() {
        let m = "alpha\n  Beta Gamma\ngamma again\n";
        assert_eq!(first_hit(m, "GAMMA"), Some((2, "Beta Gamma".into())));
        assert_eq!(first_hit(m, "zeta"), None);
        assert_eq!(first_hit(m, "  "), None, "空词没有结果");
    }

    #[test]
    fn auto_tip_ignores_one_letter_words_and_truncates_long_lines() {
        let long = format!("宏 {}", "长".repeat(300));
        assert_eq!(auto_tip(&long, "宏"), None, "一个字符的词不提示");
        let t2 = auto_tip(&format!("宏函数 {}", "长".repeat(300)), "宏函数").unwrap();
        assert!(t2.starts_with("帮助: 宏函数") && t2.ends_with('…') && t2.chars().count() <= 4 + TIP_MAX_CHARS + 1);
        assert_eq!(auto_tip("宏函数\n", "宏函数").as_deref(), Some("帮助: 宏函数"));
    }

    #[test]
    fn tip_for_prefers_the_prose_and_falls_back_to_the_command_list() {
        assert!(tip_for("窗口一览").is_some_and(|t| t.starts_with("帮助: ")));
        assert!(tip_for("app.pick-reopen").is_some_and(|t| t.contains("[app.pick-reopen]")), "正文里没有的词查命令一览");
        assert_eq!(tip_for("zzqqxx-no-such-word"), None);
        assert_eq!(tip_for("a"), None, "太短不提示");
    }

    #[test]
    fn the_prose_mentions_the_things_the_menus_actually_have() {
        let m = manual();
        for w in ["窗口一览", "关键字帮助", "标签跳转", "宏登记", "命令面板"] {
            assert!(first_hit(&m, w).is_some(), "手册里应该讲到 {w}");
        }
    }
}
