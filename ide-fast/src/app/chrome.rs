//! 窗口外围: 工具栏 / 功能键栏 / 活动栏。三者默认都关 (极简布局 = 单个文本框), 在「视图」菜单里打开; 命令都走注册表, 键位改了会跟着变。

use super::*;
use egui::Key;

/// 工具栏按钮: 命令 id; 空串 = 分隔线
const TOOLBAR: &[&str] = &[
    "c.new-file", "c.open-file", "c.save", "",
    "c.undo", "c.redo", "",
    "c.cut", "c.copy", "c.paste", "",
    "c.find", "c.replace", "c.grep", "",
    "c.split", "c.terminal", "c.palette",
];

const FKEYS: [Key; 12] = [Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12];

/// 命令 id 在注册表里的下标
fn index_of(id: &str) -> Option<usize> {
    keymap::entries().iter().position(|e| e.id == id)
}

/// F1..F12 各自 (不带修饰键) 绑定的命令下标
pub fn function_key_cmds(bound: &[(usize, egui::Modifiers, Key)]) -> [Option<usize>; 12] {
    let mut out = [None; 12];
    for (n, k) in FKEYS.iter().enumerate() {
        out[n] = bound.iter().find(|(_, m, key)| key == k && *m == egui::Modifiers::NONE).map(|b| b.0);
    }
    out
}

impl App {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut run: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for id in TOOLBAR {
                if id.is_empty() { ui.separator(); continue; }
                let Some(i) = index_of(id) else { continue };
                let e = &keymap::entries()[i];
                if ui.small_button(e.label.text()).on_hover_text(crate::i18n::lk_for(e)).clicked() { run = Some(i); }
            }
        });
        if let Some(i) = run { self.run_entry(i, ctx); }
    }

    pub(super) fn funckey_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let cmds = function_key_cmds(&keymap::bound());
        let mut run: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (n, c) in cmds.iter().enumerate() {
                let (label, tip) = match c {
                    Some(i) => (keymap::entries()[*i].label.text(), crate::i18n::lk_for(&keymap::entries()[*i])),
                    None => ("", String::new()),
                };
                let b = ui.add_enabled(c.is_some(), egui::Button::new(format!("F{} {}", n + 1, label)).small());
                if c.is_some() && b.on_hover_text(tip).clicked() { run = *c; }
            }
        });
        if let Some(i) = run { self.run_entry(i, ctx); }
    }

    /// 活动栏: 点哪个就在侧栏里显示哪个面板; 再点已经显示的那个 = 收起侧栏
    pub(super) fn activity_rail(&mut self, ui: &mut egui::Ui) {
        for (t, name) in [(Tab::Files, "文件"), (Tab::Outline, "大纲"), (Tab::Search, "搜索"), (Tab::Scm, "Git")] {
            let on = self.side.open && self.side.tab == t;
            if ui.selectable_label(on, trz(name)).clicked() {
                if on { self.side.open = false; } else { self.side.open = true; self.side.tab = t; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_commands_all_exist_in_the_registry() {
        for id in TOOLBAR.iter().filter(|s| !s.is_empty()) {
            assert!(index_of(id).is_some(), "工具栏命令 {id} 不在注册表里");
        }
    }

    #[test]
    fn function_keys_pick_unmodified_bindings_only() {
        let b = vec![(3, egui::Modifiers::NONE, Key::F3), (4, egui::Modifiers::CTRL, Key::F4), (5, egui::Modifiers::NONE, Key::F5)];
        let m = function_key_cmds(&b);
        assert_eq!((m[2], m[3], m[4], m[0]), (Some(3), None, Some(5), None));
    }
}
