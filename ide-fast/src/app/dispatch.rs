use super::App;
use crate::i18n::tr;
use crate::keymap::App as KApp;
use crate::tabs::CloseKind;
use eframe::egui;

impl App {
    /// 窗口级动作 (菜单、快捷键、命令面板都走这里)
    pub(super) fn run_app(&mut self, a: KApp, ctx: &egui::Context) {
        let ai = self.active.min(self.docs.len() - 1);
        match a {
            KApp::NewDoc => self.new_doc(),
            KApp::OpenFile => self.open_dialog(),
            KApp::OpenFolder => self.open_folder_dialog(),
            KApp::Save => {
                self.save(false);
            }
            KApp::SaveAs => {
                self.save(true);
            }
            KApp::SaveAll => self.save_all(),
            KApp::SaveClose => self.save_close(),
            KApp::CloseTab => {
                let t = self.active;
                self.close_tab(t);
            }
            KApp::CloseAll => self.close_many(CloseKind::All),
            KApp::CloseOpen => self.close_open(),
            KApp::Reload => self.reload_current(),
            KApp::RecentClear => self.clear_recent(),
            KApp::Print => self.print_cur(),
            KApp::PrintSetup => self.print_ui.setup_open = true,
            KApp::PrintPreview => self.print_ui.preview = Some(0),
            KApp::Browse => self.browse_current(),
            KApp::Props => self.props = Some(self.cur().properties()),
            KApp::NewWindow => self.new_window(),
            KApp::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            KApp::Undo => self.cur().undo(),
            KApp::Redo => self.cur().redo(),
            KApp::Cut => {
                if let Some(c) = self.cur().edit("cut", None) {
                    self.put_clip(ctx, c);
                }
            }
            KApp::Copy => {
                if let Some(c) = self.cur().edit("copy", None) {
                    self.put_clip(ctx, c);
                }
            }
            KApp::Paste => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
            KApp::SelectAll => self.cur().select_all(),
            KApp::Find => self.open_find(false),
            KApp::Replace => self.open_find(true),
            KApp::Grep => self.focus_grep(),
            KApp::MdStruct => self.md_struct_open(),
            KApp::FindNext => self.find_next(true),
            KApp::FindPrev => self.find_next(false),
            KApp::FindWord => self.find.word_search(&mut self.docs[ai], true),
            KApp::FindWordPrev => self.find.word_search(&mut self.docs[ai], false),
            KApp::ReplaceAll => self.find.replace_all(&mut self.docs[ai]),
            KApp::DiffTab => self.diff_pick = true,
            KApp::DiffFile => self.diff_with_file(),
            KApp::DiffList => self.diff_list_open = true,
            KApp::DiffNext => self.diff_step(true),
            KApp::DiffPrev => self.diff_step(false),
            KApp::DiffClear => {
                self.diff = None;
                self.diff_list_open = false;
            }
            KApp::FoldToggle => {
                let d = self.cur();
                let r = d.cursor[0];
                d.toggle_fold(r);
            }
            KApp::UnfoldAll => self.cur().folds.clear(),
            KApp::BmToggle => self.cur().bookmark("toggle"),
            KApp::BmNext => {
                self.jump_mark();
                self.cur().bookmark("next");
            }
            KApp::BmPrev => {
                self.jump_mark();
                self.cur().bookmark("prev");
            }
            KApp::BmView => self.bm_open = !self.bm_open,
            KApp::BmReset => {
                self.cur().bookmarks.clear();
                self.cur().tip = tr("dlg.bm-cleared").into();
            }
            KApp::BracketJump => self.bracket_jump(),
            KApp::HistPrev => self.jump_go(-1),
            KApp::HistNext => self.jump_go(1),
            KApp::HistMark => {
                self.jump_mark();
                self.cur().tip = tr("dlg.hist-marked").into();
            }
            KApp::Split => self.toggle_split(),
            KApp::SplitRows => self.toggle_split_kind(crate::split::SplitKind::Rows),
            KApp::SplitQuad => self.toggle_split_kind(crate::split::SplitKind::Quad),
            KApp::NextTab => self.next_tab(true),
            KApp::PrevTab => self.next_tab(false),
            KApp::TabLeft => self.move_active_tab(true),
            KApp::TabRight => self.move_active_tab(false),
            KApp::CloseOthers => self.close_many(CloseKind::Others),
            KApp::CloseLeft => self.close_many(CloseKind::Left),
            KApp::CloseRight => self.close_many(CloseKind::Right),
            KApp::Minimize => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
            KApp::Maximize => {
                let m = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!m));
            }
            KApp::Fullscreen => self.toggle_fullscreen(ctx),
            KApp::Topmost => self.toggle_topmost(ctx),
            KApp::MacroToggle => self.cur().macro_toggle(),
            KApp::MacroPlay => self.cur().macro_play(),
            KApp::MacroRunDoc => self.run_mac_doc(ctx),
            KApp::MacroRunFile => self.run_mac_file(ctx),
            KApp::MacroRunPpa => self.run_ppa_doc(),
            KApp::Terminal => self.toggle_dock_open(crate::panels::Dock::Bottom),
            KApp::AiChat => self.ai_toggle(),
            KApp::MdPreview => {
                if super::nemaopts::is_nema_doc(&self.docs[ai]) {
                    self.nema_preview = !self.nema_preview;
                } else {
                    self.md_preview = !self.md_preview;
                }
            }
            KApp::NemaConvert => self.nema_convert(),
            KApp::Dark => self.dark = !self.dark,
            KApp::Highlight => self.highlight = !self.highlight,
            KApp::Wrap => self.wrap = !self.wrap,
            KApp::Sidebar => self.toggle_dock_open(crate::panels::Dock::Side),
            KApp::SideCollapse => self.collapse_dock(crate::panels::Dock::Side),
            KApp::RightCollapse => self.collapse_dock(crate::panels::Dock::Right),
            KApp::BottomCollapse => self.collapse_dock(crate::panels::Dock::Bottom),
            KApp::SystemTitleBar => self.toggle_system_title_bar(),
            KApp::LineNumbers => self.cfg.view.line_numbers = !self.cfg.view.line_numbers,
            KApp::StatusBar => self.cfg.view.status_bar = !self.cfg.view.status_bar,
            KApp::Whitespace => self.cfg.view.whitespace = !self.cfg.view.whitespace,
            KApp::Ruler => self.cfg.view.ruler = !self.cfg.view.ruler,
            KApp::CurLine => self.cfg.view.cur_line = !self.cfg.view.cur_line,
            KApp::Minimap => self.cfg.view.minimap = !self.cfg.view.minimap,
            KApp::ToolbarToggle => self.cfg.view.toolbar = !self.cfg.view.toolbar,
            KApp::FunckeyToggle => self.cfg.view.funckey = !self.cfg.view.funckey,
            KApp::RailToggle => self.cfg.view.rail = !self.cfg.view.rail,
            KApp::MinimalMode => self.set_minimal(),
            KApp::FullMode => self.set_full(),
            KApp::ZoomIn => self.font_px = (self.font_px + 1.0).min(40.0),
            KApp::ZoomOut => self.font_px = (self.font_px - 1.0).max(8.0),
            KApp::ZoomReset => self.font_px = self.default_font_px(),
            KApp::Prefs => self.open_prefs(),
            KApp::PickSyntax => self.open_syntax_pick(),
            KApp::TypeConfig => self.open_prefs_tab(crate::prefs::PrefsTab::Types),
            KApp::Font => self.open_prefs_tab(crate::prefs::PrefsTab::Font),
            KApp::ExportCfg => self.export_settings(),
            KApp::ImportCfg => self.import_settings(),
            KApp::ImportSakura => self.import_sakura(),
            KApp::ResetCfg => self.reset_settings(),
            KApp::KeyConfig => self.cmdui.open_keycfg(),
            KApp::KeyList => self.cmdui.open_keylist(),
            KApp::CopyFname => self.copy_doc_name(ctx, false),
            KApp::CopyPath => self.copy_doc_name(ctx, true),
            KApp::SelParaPrev => self.cur().select_para(false),
            KApp::SelParaNext => self.cur().select_para(true),
            KApp::ModNext => self.step_modified(true),
            KApp::ModPrev => self.step_modified(false),
            KApp::MacroSave => self.macro_save(),
            KApp::MacroLoad => self.macro_load(),
            KApp::MacroRecMac => self.macro_rec_mac(),
            KApp::FuncNext => self.step_function(true),
            KApp::CopyTag => self.copy_tag(ctx),
            KApp::GroupNext => self.group_switch(true),
            KApp::GroupPrev => self.group_switch(false),
            KApp::TabSeparate => self.group_separate(),
            KApp::TabJointNext => self.group_joint(true),
            KApp::TabJointPrev => self.group_joint(false),
            KApp::BindWindow => self.group_bind_all(),
            KApp::GroupClose => self.group_close(),
            KApp::SpaceIndent => self.space_indent_cmd(false),
            KApp::SpaceUnindent => self.space_indent_cmd(true),
            KApp::CurCenter => self.center_cursor(),
            KApp::BackupNow => self.backup_now(),
            KApp::Completion => self.show_completion(),
            KApp::MacroRegister => self.register_macro_doc(),
            KApp::MacroSlot(n) => self.run_macro_slot(n as usize, ctx),
            KApp::GotoDef => self.goto_definition(),
            KApp::ExecCmd => self.open_exec_terminal(),
            KApp::SelCountMode => self.cycle_sel_count(),
            KApp::FindMarks => self.toggle_find_marks(),
            KApp::FindStart => self.jump_find_start(),
            KApp::CopyWrapped => self.copy_wrapped(ctx),
            KApp::OpenHc => self.open_counterpart(),
            KApp::ModNextSel => self.step_modified_sel(true),
            KApp::ModPrevSel => self.step_modified_sel(false),
            KApp::FuncPrev => self.step_function(false),
            KApp::MacroToMac => self.macro_to_mac(),
            KApp::About => self.about_open = true,
            KApp::CheckUpdate => self.upd_open(),
            KApp::ColDelete
            | KApp::ColReplace
            | KApp::SelMode
            | KApp::TagJump
            | KApp::TagBack
            | KApp::TagKeyword
            | KApp::TagsMake
            | KApp::IsFwd
            | KApp::IsBack
            | KApp::IsRegexFwd
            | KApp::IsRegexBack
            | KApp::IsMigemoFwd
            | KApp::IsMigemoBack
            | KApp::Base64Decode
            | KApp::UuDecode
            | KApp::CopyHtml
            | KApp::CopyHtmlLn
            | KApp::ClipHistory
            | KApp::CtrlCodes
            | KApp::InsDate
            | KApp::InsTime
            | KApp::InsStamp
            | KApp::StampCfg
            | KApp::WrapCol
            | KApp::WrapWindow
            | KApp::WrapNone => self.run_extra(a, ctx),
            KApp::WindowList
            | KApp::PickRecent
            | KApp::PickFolders
            | KApp::PickFavorite
            | KApp::InsRecentFile
            | KApp::InsRecentDir
            | KApp::PickCharset
            | KApp::PickReopen
            | KApp::Help
            | KApp::HelpSearch
            | KApp::ExtHelp
            | KApp::KeySearch
            | KApp::PaneOutline
            | KApp::PaneFiles => self.run_pick(a),
            KApp::Palette => {
                self.cmdui.toggle_palette();
                self.pal_plugins = Self::plugin_cmds();
            }
            KApp::StickyNotes => {
                self.sticky_notes.open = !self.sticky_notes.open;
                if self.sticky_notes.open {
                    self.sticky_notes.focus_needed = true;
                }
                self.sticky_notes.mark_dirty();
            }
        }
    }
}
