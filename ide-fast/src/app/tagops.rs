//! `App` 的标签跳转: 光标下的词 (或选区) 找定义, 多个候选弹列表, 可返回、可指定关键字、可生成 tags 文件。
//! 查找与生成的逻辑在 `tagjump` (纯函数, 有单测)。

use super::*;
use crate::tagjump::{self, Cand, Ev};

impl App {
    /// 光标处的词 (有单行选区优先用选区)
    fn tag_word(&mut self) -> Option<String> {
        let d = self.cur();
        d.selected_text_single().or_else(|| d.word_at_cursor())
    }

    /// 查找并跳转: 一个候选直接去, 多个弹列表, 没有就提示
    pub(super) fn tag_lookup(&mut self, name: &str) {
        let doc_dir = self
            .cur()
            .path
            .as_deref()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());
        let root = self.side.root.clone();
        let mut cands = tagjump::lookup(name, doc_dir.as_deref(), root.as_deref());
        match cands.len() {
            0 => self.cur().tip = trf("dlg.tag-none", &[name]),
            1 => self.tag_go(cands.remove(0)),
            _ => self.tags.pick = Some(cands),
        }
    }

    pub(super) fn tag_go(&mut self, c: Cand) {
        let (id, pos) = {
            let d = self.cur();
            (d.id, d.cursor)
        };
        self.tags.push(id, pos);
        self.open_at(&c.path, c.line, 0);
        self.cur().tip = trf("dlg.tag-jumped", &[&c.name, &c.line.to_string()]);
    }

    fn tag_back(&mut self) {
        let Some((id, pos)) = self.tags.stack.pop() else {
            self.cur().tip = tr("dlg.tag-empty").into();
            return;
        };
        match self.docs.iter().position(|d| d.id == id) {
            Some(i) => {
                self.active = i;
                self.docs[i].goto(pos[0], pos[1]);
            }
            None => self.cur().tip = tr("dlg.tag-closed").into(),
        }
    }

    fn tags_make(&mut self) {
        let Some(root) = self.side.root.clone() else {
            self.cur().tip = tr("dlg.tag-noroot").into();
            return;
        };
        self.cur().tip = match tagjump::generate(&root) {
            Ok((n, p)) => trf("dlg.tag-made", &[&n.to_string(), &p.display().to_string()]),
            Err(e) => trf("dlg.tag-fail", &[&e]),
        };
    }

    pub(super) fn run_tag(&mut self, a: KApp) {
        match a {
            KApp::TagJump => match self.tag_word() {
                Some(w) => self.tag_lookup(&w),
                None => self.cur().tip = tr("dlg.tag-noword").into(),
            },
            KApp::TagBack => self.tag_back(),
            KApp::TagKeyword => self.tags.open_keyword(),
            KApp::TagsMake => self.tags_make(),
            _ => {}
        }
    }

    pub(super) fn tag_windows(&mut self, ctx: &egui::Context) {
        for ev in self.tags.windows(ctx) {
            match ev {
                Ev::Lookup(n) => self.tag_lookup(&n),
                Ev::Go(c) => self.tag_go(c),
            }
        }
    }
}
