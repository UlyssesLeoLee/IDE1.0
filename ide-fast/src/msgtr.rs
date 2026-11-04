//! 状态栏提示 / 面板消息的显示期翻译。
//! 这些消息在代码里是现拼的中文 (`format!("已保存 {n} 个文件")`), 分散在几十处; 与其改每个调用点, 不如在**显示时**按模板翻译:
//! 模板 = 中文原文里把可变部分写成 `{}`, 匹配到就取出各段可变内容 (它们自己也会再翻译一次, 嵌套的提示因此也会被译), 填进日 / 英模板。
//! 没匹配到 = 原样返回 (宁可显示中文也不显示空白)。中文界面直接返回原文, 不做匹配。

use crate::i18n::{cur, Lang};
use std::borrow::Cow;
use std::sync::OnceLock;

/// (中文模板, 日文模板, 英文模板); `{}` 依次对应
type Row = (&'static str, &'static str, &'static str);

const MSG_T: &[Row] = &[
    // ---- 文档 / 编辑
    ("已从磁盘重新载入", "ディスクから再読み込みしました", "Reloaded from disk"),
    ("已按 {} 重新打开", "{} で開き直しました", "Reopened as {}"),
    ("保存时的换行符: {}", "保存時の改行コード: {}", "Line ending on save: {}"),
    ("保存时的编码: {}", "保存時の文字コード: {}", "Encoding on save: {}"),
    ("只有 UTF-8 可以选择带不带 BOM", "BOM の有無を選べるのは UTF-8 だけです", "Only UTF-8 can choose whether to write a BOM"),
    ("保存时带 BOM", "保存時に BOM を付けます", "A BOM will be written on save"),
    ("保存时不带 BOM", "保存時に BOM を付けません", "No BOM will be written on save"),
    ("没有对应的括弧", "対応する括弧がありません", "No matching bracket"),
    ("只读: 不能修改 (编辑 → 只读 可以关掉)", "読み取り専用: 変更できません (編集 → 読み取り専用 で解除)", "Read-only: cannot modify (turn off via Edit > Read-only)"),
    ("未知命令: {}", "不明なコマンド: {}", "Unknown command: {}"),
    ("已处理 {} 行", "{} 行を処理しました", "Processed {} line(s)"),
    ("先选中要转换的文字", "変換する文字を先に選択してください", "Select the text to convert first"),
    ("范围选择模式: 关", "範囲選択モード: オフ", "Range selection mode: off"),
    ("范围选择模式: 用方向键扩选 (F6 / Esc 结束)", "範囲選択モード: 矢印キーで拡張 (F6 / Esc で終了)", "Range selection mode: extend with arrow keys (F6 / Esc to end)"),
    ("已插入戳记: {}", "スタンプを挿入しました: {}", "Inserted stamp: {}"),
    ("没有可撤销的操作", "元に戻す操作はありません", "Nothing to undo"),
    ("没有可重做的操作", "やり直す操作はありません", "Nothing to redo"),
    ("已设置书签", "ブックマークを設定しました", "Bookmark set"),
    ("已取消书签", "ブックマークを解除しました", "Bookmark cleared"),
    ("书签 {}/{}", "ブックマーク {}/{}", "Bookmark {}/{}"),
    ("没有书签", "ブックマークはありません", "No bookmarks"),
    ("宏录制结束 ({} 步) — Ctrl+Shift+L 回放", "マクロの記録を終了 ({} ステップ) — Ctrl+Shift+L で再生", "Macro recorded ({} steps) - Ctrl+Shift+L to replay"),
    ("● 录制宏… 再按 Ctrl+Shift+M 结束", "● マクロ記録中… もう一度 Ctrl+Shift+M で終了", "● Recording macro... press Ctrl+Shift+M again to stop"),
    ("还没有录下的宏", "記録済みのマクロがありません", "No recorded macro yet"),
    ("宏已回放", "マクロを再生しました", "Macro replayed"),
    ("{} (已折返)", "{} (折り返し)", "{} (wrapped)"),
    ("没有找到: {}", "見つかりません: {}", "Not found: {}"),
    ("已到文件末尾 (未折返)", "ファイルの末尾です (折り返しなし)", "Reached end of file (no wrap)"),
    ("已到文件开头 (未折返)", "ファイルの先頭です (折り返しなし)", "Reached start of file (no wrap)"),
    ("只读文档不能置换", "読み取り専用の文書は置換できません", "Cannot replace in a read-only document"),
    ("置换了 1 处; 后面没有了", "1 件置換しました。以降はありません", "Replaced 1; no more after this"),
    ("置换了 1 处; {}", "1 件置換しました; {}", "Replaced 1; {}"),
    ("置换了 {} 处", "{} 件置換しました", "Replaced {}"),
    ("{} · {} · 已读取", "{} · {} · 読み込み済み", "{} · {} · loaded"),
    ("{} · {} · 有无法解码的字节", "{} · {} · デコードできないバイトあり", "{} · {} · undecodable bytes present"),
    ("已设为只读", "読み取り専用にしました", "Set to read-only"),
    ("已取消只读", "読み取り専用を解除しました", "Read-only cleared"),
    ("改写模式", "上書きモード", "Overwrite mode"),
    ("插入模式", "挿入モード", "Insert mode"),
    ("这个类型没有行注释符", "この種類には行コメント記号がありません", "This type has no line-comment marker"),
    ("这一行不是命中行", "この行はヒット行ではありません", "This line is not a hit line"),
    // ---- 文件 / 标签
    ("磁盘上的文件已被删除", "ディスク上のファイルは削除されました", "The file on disk was deleted"),
    ("已保存 {} 个文件", "{} 個のファイルを保存しました", "Saved {} file(s)"),
    ("还没有保存过, 不能按别的编码重新打开", "未保存のため、別の文字コードで開き直せません", "Not saved yet; cannot reopen with another encoding"),
    ("还没有保存过, 没有可重新载入的文件", "未保存のため、再読み込みするファイルがありません", "Not saved yet; nothing to reload"),
    ("还没有保存过, 没有可重新打开的文件", "未保存のため、開き直すファイルがありません", "Not saved yet; nothing to reopen"),
    ("还没有保存过", "まだ保存されていません", "Not saved yet"),
    ("打不开: {}", "開けません: {}", "Cannot open: {}"),
    ("读不了 {}: {}", "{} を読み込めません: {}", "Cannot read {}: {}"),
    ("开不了新窗口: {}", "新しいウィンドウを開けません: {}", "Cannot open a new window: {}"),
    ("没有更早的移动履历", "これより前の移動履歴はありません", "No earlier jump history"),
    ("没有更新的移动履历", "これより後の移動履歴はありません", "No later jump history"),
    ("文件超过 8 MB", "ファイルが 8 MB を超えています", "File is larger than 8 MB"),
    ("文件太大 ({} MB, 上限 {} MB): 高速版把整篇载入内存, 请用专门的大文件查看器", "ファイルが大きすぎます ({} MB, 上限 {} MB): 高速版は全体をメモリに読み込みます。大容量ファイル用ビューアを使ってください", "File too large ({} MB, limit {} MB): the fast edition loads the whole file into memory; use a dedicated large-file viewer"),
    ("看起来是二进制文件 (含 NUL 字节), 不按文本打开", "バイナリファイルのようです (NUL バイトを含む)。テキストとしては開きません", "Looks like a binary file (contains NUL bytes); not opened as text"),
    // ---- 比较 / 宏 / 预览
    ("{}: 没有可显示的差异 (未跟踪文件?)", "{}: 表示できる差分がありません (未追跡ファイル?)", "{}: no differences to show (untracked file?)"),
    ("比较 {} ↔ {}: {}", "比較 {} ↔ {}: {}", "Compare {} <-> {}: {}"),
    ("没有进行中的比较", "進行中の比較はありません", "No comparison in progress"),
    ("当前标签不在比较里", "現在のタブは比較の対象ではありません", "The current tab is not part of the comparison"),
    ("两份文档完全相同", "2 つの文書は完全に同じです", "The two documents are identical"),
    ("{} 处差异", "{} 件の差分", "{} difference(s)"),
    ("读取宏失败: {}", "マクロの読み込みに失敗: {}", "Failed to read macro: {}"),
    ("宏已执行 ({} 步)", "マクロを実行しました ({} ステップ)", "Macro ran ({} steps)"),
    ("宏未执行: {}", "マクロを実行できませんでした: {}", "Macro not run: {}"),
    ("维基链接 [[{}]]: 预览里不跳转", "Wiki リンク [[{}]]: プレビューでは移動しません", "Wiki link [[{}]]: not followed in the preview"),
    ("没有找到标题 #{}", "見出し #{} が見つかりません", "Heading #{} not found"),
    ("不打开此链接: {}", "このリンクは開きません: {}", "Not opening this link: {}"),
    ("只能打开 http / https / mailto 链接", "http / https / mailto のリンクだけ開けます", "Only http / https / mailto links can be opened"),
    ("无法打开链接: {}", "リンクを開けません: {}", "Cannot open link: {}"),
    // ---- 查找 / Grep
    ("没有可找的内容", "検索する内容がありません", "Nothing to search for"),
    ("{}+ 处", "{}+ 件", "{}+ hits"),
    ("{} 处", "{} 件", "{} hit(s)"),
    ("已停止, {} 处命中", "停止しました。{} 件ヒット", "Stopped, {} hit(s)"),
    ("找不到搜索位置", "検索場所が見つかりません", "Cannot find a place to search"),
    ("请输入要搜索的内容", "検索する内容を入力してください", "Enter something to search for"),
    ("还没有搜索结果", "検索結果がまだありません", "No search results yet"),
    ("{} 处命中 (已达 {} 上限)", "{} 件ヒット (上限 {} に到達)", "{} hit(s) (limit {} reached)"),
    ("{} 处命中", "{} 件ヒット", "{} hit(s)"),
    ("搜索中…  (Esc 停止)", "検索中…  (Esc で停止)", "Searching...  (Esc to stop)"),
    ("搜索中… {} 处  (Esc 停止)", "検索中… {} 件  (Esc で停止)", "Searching... {} hit(s)  (Esc to stop)"),
    ("置换 {} 处, 改写 {} 个文件 (已备份为 .bak){}", "{} 件を置換、{} 個のファイルを書き換えました (.bak に退避){}", "Replaced {} in {} file(s) (backed up as .bak){}"),
    (", 跳过 {} 个有未保存修改的已打开文件", ", 未保存の変更がある開いたファイル {} 個をスキップ", ", skipped {} open file(s) with unsaved changes"),
    (", {} 个失败", ", {} 個失敗", ", {} failed"),
    ("{}; 已重新载入 {} 个打开的标签", "{}; 開いているタブ {} 個を再読み込みしました", "{}; reloaded {} open tab(s)"),
    // ---- 侧栏 / Git
    ("名字不能为空", "名前を空にできません", "The name cannot be empty"),
    ("名字里不能有 / \\ : * ? \" < > |", "名前に / \\ : * ? \" < > | は使えません", "The name cannot contain / \\ : * ? \" < > |"),
    ("名字不能以点或空格结尾", "名前の末尾にドットや空白は使えません", "The name cannot end with a dot or a space"),
    ("同名文件或文件夹已存在", "同名のファイルまたはフォルダーが既にあります", "A file or folder with that name already exists"),
    ("新建失败: {}", "作成に失敗: {}", "Create failed: {}"),
    ("重命名失败: {}", "名前の変更に失敗: {}", "Rename failed: {}"),
    ("删除失败: {}", "削除に失敗: {}", "Delete failed: {}"),
    ("不是 Git 仓库", "Git リポジトリではありません", "Not a Git repository"),
    ("无法运行 git: {}", "git を実行できません: {}", "Cannot run git: {}"),
    ("这个仓库的本地配置定义了 filter / diff 外部命令, 高速版不自动运行 git (请在终端里操作)", "このリポジトリのローカル設定に filter / diff の外部コマンドがあるため、高速版は git を自動実行しません (ターミナルで操作してください)", "This repository's local config defines external filter / diff commands, so the fast edition will not run git automatically (use the terminal)"),
    ("分支 {}", "ブランチ {}", "Branch {}"),
    // ---- 插件 / 其它
    ("插件命令失败 (编辑器不受影响): {}", "プラグインコマンドが失敗しました (エディターには影響なし): {}", "Plugin command failed (the editor is unaffected): {}"),
    ("插件宿主已停止", "プラグインホストが停止しました", "The plugin host has stopped"),
    ("无法启动 shell: {}", "シェルを起動できません: {}", "Cannot start shell: {}"),
];

struct Tpl {
    pieces: Vec<&'static str>,
    ja: &'static str,
    en: &'static str,
    /// 字面部分总长, 越长越具体, 先匹配
    weight: usize,
}

fn index() -> &'static [Tpl] {
    static IDX: OnceLock<Vec<Tpl>> = OnceLock::new();
    IDX.get_or_init(|| {
        let mut v: Vec<Tpl> = MSG_T
            .iter()
            .map(|(zh, ja, en)| {
                let pieces: Vec<&str> = zh.split("{}").collect();
                Tpl { weight: pieces.iter().map(|p| p.len()).sum(), pieces, ja, en }
            })
            .collect();
        v.sort_by(|a, b| b.weight.cmp(&a.weight));
        v
    })
}

/// 按模板取出各 `{}` 的内容; 不匹配 = None
fn capture<'a>(pieces: &[&str], s: &'a str) -> Option<Vec<&'a str>> {
    let (first, rest) = pieces.split_first()?;
    if rest.is_empty() {
        return (s == *first).then(Vec::new);
    }
    let mut tail = s.strip_prefix(first)?;
    let last = *rest.last()?;
    tail = tail.strip_suffix(last)?;
    let mut caps = vec![];
    for mid in &rest[..rest.len() - 1] {
        if mid.is_empty() {
            return None; // 相邻的 {}{} 无法切分
        }
        let at = tail.find(mid)?;
        caps.push(&tail[..at]);
        tail = &tail[at + mid.len()..];
    }
    caps.push(tail);
    Some(caps)
}

fn fill(tpl: &str, caps: &[String]) -> String {
    let mut out = String::new();
    let mut it = caps.iter();
    let mut rest = tpl;
    while let Some(p) = rest.find("{}") {
        out.push_str(&rest[..p]);
        out.push_str(it.next().map(|s| s.as_str()).unwrap_or(""));
        rest = &rest[p + 2..];
    }
    out.push_str(rest);
    out
}

fn translate(s: &str, depth: u8) -> Option<String> {
    for t in index() {
        let Some(caps) = capture(&t.pieces, s) else { continue };
        let caps: Vec<String> = caps.iter().map(|c| if depth < 2 { translate(c, depth + 1).unwrap_or_else(|| (*c).to_string()) } else { (*c).to_string() }).collect();
        return Some(fill(if cur() == Lang::Ja { t.ja } else { t.en }, &caps));
    }
    None
}

/// 把一条中文消息翻成当前界面语言; 中文界面 / 没有模板 = 原样返回
pub fn msg(s: &str) -> Cow<'_, str> {
    if cur() == Lang::Zh || s.is_empty() {
        return Cow::Borrowed(s);
    }
    match translate(s, 0) {
        Some(t) => Cow::Owned(t),
        None => Cow::Borrowed(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{set, TEST_LOCK};

    #[test]
    fn templates_translate_and_capture_values() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set(Lang::En);
        assert_eq!(msg("已保存 3 个文件"), "Saved 3 file(s)");
        assert_eq!(msg("已停止, 12 处命中"), "Stopped, 12 hit(s)");
        assert_eq!(msg("5 处命中 (已达 5000 上限)"), "5 hit(s) (limit 5000 reached)");
        assert_eq!(msg("置换 4 处, 改写 2 个文件 (已备份为 .bak), 1 个失败"), "Replaced 4 in 2 file(s) (backed up as .bak), 1 failed");
        assert_eq!(msg("1/9 (已折返)"), "1/9 (wrapped)");
        assert_eq!(msg("置换了 1 处; 1/3"), "Replaced 1; 1/3");
        assert_eq!(msg("置换了 1 处; 没有找到: x"), "Replaced 1; Not found: x", "嵌套的提示也会翻译");
        assert_eq!(msg("没有任何模板的文字"), "没有任何模板的文字");
        set(Lang::Ja);
        assert_eq!(msg("没有找到: abc"), "見つかりません: abc");
        set(Lang::Zh);
        assert_eq!(msg("已保存 3 个文件"), "已保存 3 个文件", "中文界面原样");
    }

    #[test]
    fn every_template_has_matching_placeholders_in_all_languages() {
        for (zh, ja, en) in MSG_T {
            let n = zh.matches("{}").count();
            assert_eq!(ja.matches("{}").count(), n, "{zh}");
            assert_eq!(en.matches("{}").count(), n, "{zh}");
            assert!(!ja.is_empty() && !en.is_empty());
        }
    }

    #[test]
    fn matching_is_anchored_at_both_ends() {
        assert_eq!(capture(&["打不开: ", ""], "打不开: x.txt"), Some(vec!["x.txt"]));
        assert_eq!(capture(&["", " 处"], "5 处命中"), None);
        assert_eq!(capture(&["a", "b", "c"], "a1b2c"), Some(vec!["1", "2"]));
        assert_eq!(capture(&["无标题"], "无标题 2"), None);
    }
}
