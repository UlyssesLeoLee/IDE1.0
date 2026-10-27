//! 编辑命令的请求 / 响应 DTO (前端协议): 字段名、顺序和 serde 属性就是线上格式, 改动要和前端一起改。
//!
//! 响应有一堆可选字段 (列块、多光标、折叠……), 每条命令只用其中几个: 用下面的构造器 + 链式修饰写,
//! 不要手写满字段的结构体字面量 (加一个字段就要改一圈)。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EditReq {
    pub cmd: String,
    pub row: usize,
    pub col: usize,
    /// 选区 [r0, c0, r1, c1] (前端已把"到行尾"解析成具体列)
    #[serde(default)]
    pub sel: Option<[usize; 4]>,
    pub total_rows: usize,
    /// Enter 自动缩进 / 智能缩进 (new-line 用)
    #[serde(default)]
    pub auto_indent: bool,
    #[serde(default)]
    pub smart_indent: bool,
    /// 粘贴 / 矩形贴付 的文字
    #[serde(default)]
    pub text: Option<String>,
    /// 剪贴板内容来自「无选区的复制 / 剪切」(整行模式)
    #[serde(default)]
    pub line_mode: bool,
    /// 覆盖模式 (Ins): insert-text 时吃掉光标后同样个数的字符
    #[serde(default)]
    pub overwrite: bool,
    /// 矩形(列块)选区 [r0, c0, r1, c1] (未归一化): box-* 命令用
    #[serde(default)]
    pub box_sel: Option<[usize; 4]>,
    /// 多光标 [(行, 列)]: multi-* 命令用
    #[serde(default)]
    pub cursors: Vec<[usize; 2]>,
    /// 需要用到的行: [行号, 文本] (光标行 ±1, 选区各行)
    pub rows: Vec<(usize, String)>,
}

/// 一个行补丁: (起始行, 删几行, 换成的行)
pub type RowPatch = (usize, usize, Vec<String>);

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct EditRes {
    /// false = 什么也没改 (前端只给出提示 / 剪贴板 / 选区)
    pub changed: bool,
    /// 把 [start, start+remove) 行换成 insert
    pub start: usize,
    pub remove: usize,
    pub insert: Vec<String>,
    pub cursor: [usize; 2],
    /// 要写进剪贴板的文字 (剪切类命令)
    pub clip: Option<String>,
    /// 剪贴板内容是整行模式 (再粘贴时插在当前行之前)
    pub clip_line: bool,
    /// 命令产生的选区 [r0, c0, r1, c1] 与锚点 (选词等不改文本的命令用)
    pub sel: Option<[usize; 4]>,
    pub anchor: Option<[usize; 2]>,
    /// 提示文字
    pub tip: Option<String>,
    /// 撤销后光标回到哪里 (缺省 = 命令发出时的光标); 词删除是先选中词再删, 撤销回到词尾
    pub undo_cursor: Option<[usize; 2]>,
    /// box-* 命令之后的列块选区 (None = 列块已清除)
    pub box_sel: Option<[usize; 4]>,
    /// multi-* 命令之后的全部光标 (升序, 已去重; cursor 是第一个)
    pub cursors: Option<Vec<[usize; 2]>>,
    /// fold-* 命令算出的折叠块 [起始行, 结束行] (结束行含); 文字不改
    pub folds: Option<Vec<[usize; 2]>>,
    /// multi-* 命令的补丁, 起始行是改动前的行号; 前端从下往上套。有它时不看 start / remove / insert
    pub patches: Option<Vec<RowPatch>>,
}

impl EditRes {
    /// 什么也没改, 只给一句提示 (`changed == false`)
    pub fn nothing(cursor: [usize; 2], tip: &str) -> Self {
        EditRes { changed: false, tip: Some(tip.to_string()), cursor, ..Default::default() }
    }
    /// 把 [start, start+remove) 行换成 `insert`, 光标落在 `cursor`
    pub fn splice(start: usize, remove: usize, insert: Vec<String>, cursor: [usize; 2]) -> Self {
        EditRes { changed: true, start, remove, insert, cursor, ..Default::default() }
    }
    /// 只换一行
    pub fn replace_line(row: usize, new: String, cursor: [usize; 2]) -> Self {
        Self::splice(row, 1, vec![new], cursor)
    }
    /// 不改文本、只写剪贴板 (复制类)
    pub fn clip_only(clip: String, cursor: [usize; 2]) -> Self {
        EditRes { clip: Some(clip), cursor, ..Default::default() }
    }
    /// 不改文本、只产生一个选区 (选词类)
    pub fn selecting(sel: [usize; 4], anchor: [usize; 2], cursor: [usize; 2]) -> Self {
        EditRes { sel: Some(sel), anchor: Some(anchor), cursor, ..Default::default() }
    }

    pub fn tip(mut self, tip: impl Into<String>) -> Self {
        self.tip = Some(tip.into());
        self
    }
    /// 写剪贴板 (`None` = 不写)
    pub fn clip(mut self, clip: Option<String>) -> Self {
        self.clip = clip;
        self
    }
    pub fn clip_line(mut self) -> Self {
        self.clip_line = true;
        self
    }
    pub fn undo_cursor(mut self, pos: [usize; 2]) -> Self {
        self.undo_cursor = Some(pos);
        self
    }
    pub fn box_sel(mut self, sel: Option<[usize; 4]>) -> Self {
        self.box_sel = sel;
        self
    }
    pub fn cursor(mut self, cursor: [usize; 2]) -> Self {
        self.cursor = cursor;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_set_only_what_they_say() {
        let r = EditRes::replace_line(3, "x".into(), [3, 1]);
        assert_eq!((r.changed, r.start, r.remove, r.insert.clone(), r.cursor), (true, 3, 1, vec!["x".to_string()], [3, 1]));
        assert_eq!((r.clip.is_none(), r.tip.is_none(), r.box_sel.is_none(), r.patches.is_none()), (true, true, true, true));
        let n = EditRes::nothing([2, 2], "t");
        assert_eq!((n.changed, n.tip.as_deref(), n.cursor), (false, Some("t"), [2, 2]));
        let s = EditRes::selecting([0, 1, 0, 3], [0, 1], [0, 3]);
        assert!(!s.changed && s.sel == Some([0, 1, 0, 3]));
    }

    #[test]
    fn wire_format_keeps_all_fields() {
        let v = serde_json::to_value(EditRes::nothing([0, 0], "")).unwrap();
        for k in ["changed", "start", "remove", "insert", "cursor", "clip", "clip_line", "sel", "anchor", "tip", "undo_cursor", "box_sel", "cursors", "folds", "patches"] {
            assert!(v.get(k).is_some(), "响应缺字段 {k}");
        }
    }
}
