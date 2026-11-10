use super::menuui::{self, menu_check};
use super::App;
use crate::view::ThemeColors;
use crate::i18n::{lk, tr, trz, Lang};
use crate::keymap::App as KApp;
use crate::tabs::CloseKind;
use editcore::document_io::Charset;
use eframe::egui;
use std::path::PathBuf;

impl App {
    pub(super) fn menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use menuui::menu_check;
        let tc = self.tc;
        let hide = self.sys.mn_hide;
        let mn = move |k: &str| if hide { ' ' } else { menuui::mnemonic(k) };
        let mut mids: Vec<(&'static str, egui::Id)> = Vec::new();
        let bf = self.sys.bar_focus;
        ui.data_mut(|d| d.insert_temp(menuui::bar_focus_id(), bf));
        egui::MenuBar::new()
            .style(menuui::bar_style(tc))
            .config(egui::containers::menu::MenuConfig::new().style(menuui::style(tc)))
            .ui(ui, |ui| {
                menuui::top(
                    ui,
                    &mut mids,
                    "menu.file",
                    menuui::menu_title(&tc, tr("menu.file"), mn("menu.file")),
                    |ui| {
                        menuui::scroll(ui, |ui| {
                            if menuui::mbtn(ui, lk("c.new-file", "Ctrl+N")).clicked() {
                                self.new_doc();
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.open-file", "Ctrl+O")).clicked() {
                                ui.close();
                                self.open_dialog();
                            }
                            if menuui::mbtn(ui, lk("c.open-folder", "")).clicked() {
                                ui.close();
                                self.open_folder_dialog();
                            }
                            ui.menu_button(tr("sub.recent"), |ui| {
                                if self.recent.is_empty() {
                                    ui.label(tr("ui.none"));
                                }
                                let mut pick: Option<String> = None;
                                for r in &self.recent {
                                    if ui.button(r).clicked() {
                                        pick = Some(r.clone());
                                        ui.close();
                                    }
                                }
                                if let Some(r) = pick {
                                    self.open_path(std::path::Path::new(&r));
                                }
                                ui.separator();
                                if menuui::mbtn(ui, lk("c.recent-clear", "")).clicked() {
                                    self.clear_recent();
                                    ui.close();
                                }
                            });
                            ui.menu_button(tr("sub.recent-dirs"), |ui| {
                                if self.recent_dirs.is_empty() {
                                    ui.label(tr("ui.none"));
                                }
                                let mut pick: Option<String> = None;
                                for r in &self.recent_dirs {
                                    if ui.button(r).clicked() {
                                        pick = Some(r.clone());
                                        ui.close();
                                    }
                                }
                                if let Some(r) = pick {
                                    self.open_folder(PathBuf::from(r));
                                }
                            });
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.save", "Ctrl+S")).clicked() {
                                ui.close();
                                self.save(false);
                            }
                            if menuui::mbtn(ui, lk("c.save-as", "Ctrl+Shift+S")).clicked() {
                                ui.close();
                                self.save(true);
                            }
                            if menuui::mbtn(ui, lk("c.save-all", "Ctrl+Alt+Shift+S")).clicked() {
                                ui.close();
                                self.save_all();
                            }
                            if menuui::mbtn(ui, lk("c.save-close", "")).clicked() {
                                ui.close();
                                self.save_close();
                            }
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.close-tab", "Ctrl+W")).clicked() {
                                ui.close();
                                let a = self.active;
                                self.close_tab(a);
                            }
                            if menuui::mbtn(ui, lk("c.close-all", "Ctrl+Shift+W")).clicked() {
                                ui.close();
                                self.close_many(CloseKind::All);
                            }
                            if menuui::mbtn(ui, lk("c.close-open", "Ctrl+Shift+F4")).clicked() {
                                ui.close();
                                self.close_open();
                            }
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.reload", "")).clicked() {
                                ui.close();
                                self.reload_current();
                            }
                            ui.menu_button(tr("sub.reopen"), |ui| {
                                for cs in Charset::ALL {
                                    if ui.button(cs.label()).clicked() {
                                        ui.close();
                                        self.reopen_as(cs);
                                    }
                                }
                            });
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.print", "Ctrl+P")).clicked() {
                                ui.close();
                                self.print_cur();
                            }
                            if menuui::mbtn(ui, lk("c.print-setup", "")).clicked() {
                                ui.close();
                                self.print_ui.setup_open = true;
                            }
                            if menuui::mbtn(ui, lk("c.print-preview", "")).clicked() {
                                ui.close();
                                self.print_ui.preview = Some(0);
                            }
                            if menuui::mbtn(ui, lk("c.browse", "")).clicked() {
                                ui.close();
                                self.browse_current();
                            }
                            if menuui::mbtn(ui, lk("c.file-properties", "Alt+Enter")).clicked() {
                                ui.close();
                                self.props = Some(self.cur().properties());
                            }
                            if menuui::mbtn(ui, lk("c.new-window", "")).clicked() {
                                ui.close();
                                self.new_window();
                            }
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.exit", "")).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        })
                    },
                );
                menuui::top(
                    ui,
                    &mut mids,
                    "menu.edit",
                    menuui::menu_title(&tc, tr("menu.edit"), mn("menu.edit")),
                    |ui| {
                        menuui::scroll(ui, |ui| {
                            if menuui::mbtn(ui, lk("c.undo", "Ctrl+Z")).clicked() {
                                self.cur().undo();
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.redo", "Ctrl+Y")).clicked() {
                                self.cur().redo();
                                ui.close();
                            }
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.cut", "Ctrl+X")).clicked() {
                                if let Some(c) = self.cur().edit("cut", None) {
                                    self.put_clip(ctx, c);
                                }
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.copy", "Ctrl+C")).clicked() {
                                if let Some(c) = self.cur().edit("copy", None) {
                                    self.put_clip(ctx, c);
                                }
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.paste", "Ctrl+V")).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.select-all", "Ctrl+A")).clicked() {
                                self.cur().select_all();
                                ui.close();
                            }
                            ui.separator();
                            self.cmd_menu(ui, ctx);
                            ui.separator();
                            for (z, a) in [
                                ("选到上一段落", KApp::SelParaPrev),
                                ("选到下一段落", KApp::SelParaNext),
                                ("范围选择模式 (开始 / 结束)", KApp::SelMode),
                                ("复制文件名", KApp::CopyFname),
                                ("复制完整路径", KApp::CopyPath),
                                ("代码补全", KApp::Completion),
                                ("SPACE 缩进", KApp::SpaceIndent),
                                ("SPACE 反缩进", KApp::SpaceUnindent),
                                ("光标行居中", KApp::CurCenter),
                                ("切换选区统计方式 (字符 / 字节 / 行)", KApp::SelCountMode),
                                ("复制路径与光标位置", KApp::CopyTag),
                                ("在折行处加换行复制", KApp::CopyWrapped),
                                ("列块删除", KApp::ColDelete),
                                ("列块替换…", KApp::ColReplace),
                                ("复制为彩色 HTML", KApp::CopyHtml),
                                ("复制为彩色 HTML (带行号)", KApp::CopyHtmlLn),
                                ("剪贴板历史…", KApp::ClipHistory),
                            ] {
                                if ui.button(Self::app_label(a, z)).clicked() {
                                    self.run_app(a, ctx);
                                    ui.close();
                                }
                            }
                            ui.separator();
                            ui.menu_button(trz("插入"), |ui| self.insert_menu(ui, ctx));
                            for (z, a) in [
                                ("Base64 解码", KApp::Base64Decode),
                                ("uudecode 解码", KApp::UuDecode),
                            ] {
                                if ui.button(Self::app_label(a, z)).clicked() {
                                    self.run_app(a, ctx);
                                    ui.close();
                                }
                            }
                        })
                    },
                );
                menuui::top(
                    ui,
                    &mut mids,
                    "menu.search",
                    menuui::menu_title(&tc, tr("menu.search"), mn("menu.search")),
                    |ui| {
                        menuui::scroll(ui, |ui| {
                            if menuui::mbtn(ui, lk("c.find", "Ctrl+F")).clicked() {
                                self.open_find(false);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.replace", "Ctrl+H / Ctrl+R")).clicked() {
                                self.open_find(true);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.grep", "Ctrl+G")).clicked() {
                                self.focus_grep();
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.md-struct", "")).clicked() {
                                ui.close();
                                self.md_struct_open();
                            }
                            if menuui::mbtn(ui, lk("c.find-next", "F3")).clicked() {
                                self.find_next(true);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.find-prev", "Shift+F3")).clicked() {
                                self.find_next(false);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.find-word", "Ctrl+F3")).clicked() {
                                let i = self.active.min(self.docs.len() - 1);
                                self.find.word_search(&mut self.docs[i], true);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.replace-all", "Ctrl+Alt+R")).clicked() {
                                let i = self.active.min(self.docs.len() - 1);
                                self.find.replace_all(&mut self.docs[i]);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.goto-line", "Ctrl+J")).clicked() {
                                self.goto_open = true;
                                self.goto_text.clear();
                                ui.close();
                            }
                            ui.separator();
                            ui.menu_button(tr("sub.isearch"), |ui| {
                                for (z, a) in [
                                    ("增量查找: 向后", KApp::IsFwd),
                                    ("增量查找: 向前", KApp::IsBack),
                                    ("增量查找: 正则 向后", KApp::IsRegexFwd),
                                    ("增量查找: 正则 向前", KApp::IsRegexBack),
                                    ("增量查找: Migemo 向后", KApp::IsMigemoFwd),
                                    ("增量查找: Migemo 向前", KApp::IsMigemoBack),
                                ] {
                                    if ui.button(Self::app_label(a, z)).clicked() {
                                        self.run_app(a, ctx);
                                        ui.close();
                                    }
                                }
                            });
                            ui.menu_button(tr("sub.tags"), |ui| {
                                for (z, a) in [
                                    ("标签跳转", KApp::TagJump),
                                    ("标签跳转: 返回", KApp::TagBack),
                                    ("指定关键字的标签跳转…", KApp::TagKeyword),
                                    ("生成标签文件 (tags)", KApp::TagsMake),
                                    ("打开同名 C/C++ 头 / 源文件", KApp::OpenHc),
                                    ("上一个函数", KApp::FuncPrev),
                                    ("下一个函数", KApp::FuncNext),
                                ] {
                                    if ui.button(Self::app_label(a, z)).clicked() {
                                        self.run_app(a, ctx);
                                        ui.close();
                                    }
                                }
                            });
                            ui.separator();
                            for (z, a) in [
                                ("上一处改动", KApp::ModPrev),
                                ("下一处改动", KApp::ModNext),
                                ("上一个函数", KApp::FuncPrev),
                                ("下一个函数", KApp::FuncNext),
                                ("(选择) 上一处改动", KApp::ModPrevSel),
                                ("(选择) 下一处改动", KApp::ModNextSel),
                                ("打开同名 C/C++ 头 / 源文件", KApp::OpenHc),
                                ("切换查找标记 (全部强调)", KApp::FindMarks),
                                ("回到查找起点", KApp::FindStart),
                                ("转到定义 (本文件)", KApp::GotoDef),
                            ] {
                                if ui.button(Self::app_label(a, z)).clicked() {
                                    self.run_app(a, ctx);
                                    ui.close();
                                }
                            }
                            ui.menu_button(trz("比较"), |ui| {
                                if ui.button(trz("与其他标签比较…")).clicked() {
                                    self.diff_pick = true;
                                    ui.close();
                                }
                                if ui.button(trz("与文件比较…")).clicked() {
                                    self.diff_with_file();
                                    ui.close();
                                }
                                if ui.button(trz("差异列表")).clicked() {
                                    self.diff_list_open = true;
                                    ui.close();
                                }
                                if ui.button(trz("下一处差异 (Alt+↓)")).clicked() {
                                    self.diff_step(true);
                                    ui.close();
                                }
                                if ui.button(trz("上一处差异 (Alt+↑)")).clicked() {
                                    self.diff_step(false);
                                    ui.close();
                                }
                                if ui.button(trz("清除比较")).clicked() {
                                    self.diff = None;
                                    self.diff_list_open = false;
                                    ui.close();
                                }
                            });
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.bracket-jump", "Ctrl+[")).clicked() {
                                self.bracket_jump();
                                ui.close();
                            }
                            ui.menu_button(tr("sub.bookmark"), |ui| {
                                if menuui::mbtn(ui, lk("c.bookmark-toggle", "Ctrl+F2")).clicked() {
                                    self.cur().bookmark("toggle");
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.bookmark-next", "F2")).clicked() {
                                    self.jump_mark();
                                    self.cur().bookmark("next");
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.bookmark-prev", "Shift+F2")).clicked() {
                                    self.jump_mark();
                                    self.cur().bookmark("prev");
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.bookmark-view", "Alt+F2")).clicked() {
                                    self.bm_open = true;
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.bookmark-reset", "Ctrl+Shift+F2")).clicked() {
                                    self.cur().bookmarks.clear();
                                    self.cur().tip = tr("dlg.bm-cleared").into();
                                    ui.close();
                                }
                            });
                            ui.menu_button(trz("移动履历"), |ui| {
                                if menuui::mbtn(ui, lk("c.hist-prev", "Alt+P")).clicked() {
                                    self.jump_go(-1);
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.hist-next", "Alt+N")).clicked() {
                                    self.jump_go(1);
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.hist-mark", "")).clicked() {
                                    self.jump_mark();
                                    self.cur().tip = tr("dlg.hist-marked").into();
                                    ui.close();
                                }
                            });
                        })
                    },
                );
                menuui::top(
                    ui,
                    &mut mids,
                    "menu.view",
                    menuui::menu_title(&tc, tr("menu.view"), mn("menu.view")),
                    |ui| {
                        menuui::scroll(ui, |ui| {
                            // 1. 界面布局 (Layout)
                            ui.menu_button(tr("sub.layout"), |ui| {
                                if menuui::mbtn(ui, lk("c.full-mode", "")).clicked() {
                                    self.set_full();
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.minimal-mode", "")).clicked() {
                                    self.set_minimal();
                                    ui.close();
                                }
                                ui.separator();
                                if menu_check(
                                    ui,
                                    &tc,
                                    self.cfg.system_title_bar,
                                    Self::app_label(KApp::SystemTitleBar, "使用系统标题栏"),
                                )
                                .clicked()
                                {
                                    self.run_app(KApp::SystemTitleBar, ctx);
                                    ui.close();
                                }
                            });

                            // 2. 面板与折叠 (Panels & Docks)
                            ui.menu_button(tr("sub.dock-panels"), |ui| {
                                if menu_check(ui, &tc, self.cfg.view.rail, lk("c.rail", "")).clicked() {
                                    self.cfg.view.rail = !self.cfg.view.rail;
                                }
                                if menu_check(ui, &tc, self.side.open, lk("c.sidebar", "Ctrl+B")).clicked() {
                                    self.side.open = !self.side.open;
                                }
                                if menu_check(ui, &tc, self.ai.open, lk("c.ai-chat", "")).clicked() {
                                    self.ai.open = !self.ai.open;
                                }
                                if menu_check(ui, &tc, self.term.open, lk("c.terminal", "Ctrl+`")).clicked() {
                                    self.term.open = !self.term.open;
                                }
                                if menu_check(ui, &tc, self.sticky_notes.open, lk("c.sticky-notes", "Ctrl+Alt+N")).clicked() {
                                    self.sticky_notes.open = !self.sticky_notes.open;
                                    if self.sticky_notes.open {
                                        self.sticky_notes.focus_needed = true;
                                    }
                                    self.sticky_notes.mark_dirty();
                                }
                                ui.separator();
                                for (a, zh, on) in [
                                    (KApp::SideCollapse, "折叠左侧栏", self.cfg.view.side_collapsed),
                                    (KApp::RightCollapse, "折叠右侧面板", self.cfg.view.right_collapsed),
                                    (KApp::BottomCollapse, "折叠底部面板", self.cfg.view.bottom_collapsed),
                                ] {
                                    if menu_check(ui, &tc, on, Self::app_label(a, zh)).clicked() {
                                        self.run_app(a, ctx);
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                for (t, zh) in [
                                    (super::sysopts::BottomTab::Terminal, "底部: 终端 (TERMINAL)"),
                                    (super::sysopts::BottomTab::Output, "底部: 输出 (OUTPUT)"),
                                    (super::sysopts::BottomTab::Problems, "底部: 问题 (PROBLEMS)"),
                                    (super::sysopts::BottomTab::CollabReview, "底部: 🛡️ 协同审查"),
                                ] {
                                    let on = self.term.open && !self.cfg.view.bottom_collapsed && self.sys.bottom == t;
                                    if menu_check(ui, &tc, on, trz(zh)).clicked() {
                                        self.sys.bottom = t;
                                        self.term.open = true;
                                        self.expand_dock(crate::panels::Dock::Bottom);
                                        ui.close();
                                    }
                                }
                                if menuui::mbtn(ui, trz("+ 新建终端会话 (PowerShell)")).clicked() {
                                    self.new_terminal_session();
                                    ui.close();
                                }
                            });

                            // 3. 界面元素 (UI Elements)
                            ui.menu_button(tr("sub.ui-bars"), |ui| {
                                if menu_check(ui, &tc, self.cfg.view.status_bar, lk("c.status-bar", "")).clicked() {
                                    self.cfg.view.status_bar = !self.cfg.view.status_bar;
                                }
                                if menu_check(ui, &tc, self.cfg.view.line_numbers, lk("c.line-numbers", "")).clicked() {
                                    self.cfg.view.line_numbers = !self.cfg.view.line_numbers;
                                }
                                if menu_check(ui, &tc, self.cfg.view.minimap, lk("c.minimap", "")).clicked() {
                                    self.cfg.view.minimap = !self.cfg.view.minimap;
                                }
                                if menu_check(ui, &tc, self.cfg.view.ruler, lk("c.ruler", "")).clicked() {
                                    self.cfg.view.ruler = !self.cfg.view.ruler;
                                }
                                if menu_check(ui, &tc, self.cfg.view.funckey, lk("c.funckey", "")).clicked() {
                                    self.cfg.view.funckey = !self.cfg.view.funckey;
                                }
                                if menu_check(ui, &tc, self.cfg.view.toolbar, lk("c.toolbar", "")).clicked() {
                                    self.cfg.view.toolbar = !self.cfg.view.toolbar;
                                }
                                ui.separator();
                                if menu_check(ui, &tc, !self.cfg.view.md_bar_collapsed, trz("Markdown 工具条")).clicked() {
                                    self.cfg.view.md_bar_collapsed = !self.cfg.view.md_bar_collapsed;
                                    self.cfg.save();
                                }
                                if menu_check(ui, &tc, !self.cfg.view.nema_bar_collapsed, trz("NEMA 工具条")).clicked() {
                                    self.cfg.view.nema_bar_collapsed = !self.cfg.view.nema_bar_collapsed;
                                    self.cfg.save();
                                }
                            });

                            // 4. 文本显示 (Text Display)
                            ui.menu_button(tr("sub.text-view"), |ui| {
                                if menu_check(ui, &tc, self.wrap, lk("c.wrap", "Alt+Z")).clicked() {
                                    self.wrap = !self.wrap;
                                }
                                ui.menu_button(tr("sub.wrap-mode"), |ui| {
                                    self.wrap_mode_menu_items(ui, &tc, ctx);
                                });
                                ui.separator();
                                if menu_check(ui, &tc, self.cfg.view.whitespace, lk("c.whitespace", "")).clicked() {
                                    self.cfg.view.whitespace = !self.cfg.view.whitespace;
                                }
                                if menu_check(ui, &tc, self.cfg.view.cur_line, lk("c.cur-line", "")).clicked() {
                                    self.cfg.view.cur_line = !self.cfg.view.cur_line;
                                }
                                let ro = self.cur().readonly;
                                if menu_check(ui, &tc, ro, trz("只读模式")).clicked() {
                                    if let Some(d) = self.docs.get_mut(self.active) { d.readonly = !ro; }
                                    ui.close();
                                }
                            });

                            // 5. 缩放 (Zoom)
                            ui.menu_button(tr("sub.zoom"), |ui| {
                                if menuui::mbtn(ui, lk("c.zoom-in", "Ctrl+=")).clicked() {
                                    self.font_px = (self.font_px + 1.0).min(40.0);
                                }
                                if menuui::mbtn(ui, lk("c.zoom-out", "Ctrl+-")).clicked() {
                                    self.font_px = (self.font_px - 1.0).max(8.0);
                                }
                                if menuui::mbtn(ui, lk("c.zoom-reset", "Ctrl+0")).clicked() {
                                    self.font_px = self.default_font_px();
                                }
                                ui.separator();
                                ui.menu_button(tr("sub.tabwidth"), |ui| {
                                    for w in [2usize, 3, 4, 8] {
                                        if menu_check(ui, &tc, self.tab == w, format!("{w}")).clicked() {
                                            self.tab = w;
                                            ui.close();
                                        }
                                    }
                                });
                            });

                            ui.separator();
                            if menuui::mbtn(ui, lk("c.redraw", "F5")).clicked() {
                                ctx.request_repaint();
                                ui.close();
                            }
                        })
                    },
                );
                menuui::top(
                    ui,
                    &mut mids,
                    "menu.tool",
                    menuui::menu_title(&tc, tr("menu.tool"), mn("menu.tool")),
                    |ui| menuui::scroll(ui, |ui| self.tool_items(ui, ctx)),
                );
                menuui::top(
                    ui,
                    &mut mids,
                    "menu.setting",
                    menuui::menu_title(&tc, tr("menu.setting"), mn("menu.setting")),
                    |ui| {
                        menuui::scroll(ui, |ui| {
                            if menuui::mbtn(ui, lk("c.common-config", "Ctrl+6")).clicked() {
                                self.open_prefs();
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.type-config", "Ctrl+5")).clicked() {
                                self.open_prefs_tab(crate::prefs::PrefsTab::Types);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.font", "Ctrl+7")).clicked() {
                                self.open_prefs_tab(crate::prefs::PrefsTab::Font);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.language", "Ctrl+4")).clicked() {
                                self.open_syntax_pick();
                                ui.close();
                            }
                            ui.menu_button(tr("sub.wrap-mode"), |ui| {
                                self.wrap_mode_menu_items(ui, &tc, ctx);
                            });
                            if menu_check(ui, &tc, self.cfg.view.minimap, tr("c.minimap")).clicked() {
                                self.cfg.view.minimap = !self.cfg.view.minimap;
                                self.cfg.save();
                                ui.close();
                            }
                            ui.menu_button(trz("语法类型"), |ui| self.syntax_items(ui));
                            ui.menu_button(tr("sub.theme"), |ui| {
                                if menuui::mbtn(ui, lk("c.theme-select", "")).clicked() {
                                    self.open_prefs();
                                    ui.close();
                                }
                                ui.separator();
                                for (id, label_key) in [
                                    ("theme-crystal", "c.theme-crystal"),
                                    ("theme-clear", "c.theme-clear"),
                                    ("theme-dark", "c.theme-dark"),
                                    ("theme-light", "c.theme-light"),
                                    ("theme-high-contrast", "c.theme-high-contrast"),
                                    ("theme-retro16", "c.theme-retro16"),
                                    ("theme-comic", "c.theme-comic"),
                                ] {
                                    let active = if self.theme_id.is_empty() {
                                        if self.dark { id == "theme-dark" } else { id == "theme-light" }
                                    } else {
                                        self.theme_id == id
                                    };
                                    if menu_check(ui, &tc, active, tr(label_key)).clicked() {
                                        if id == "theme-dark" {
                                            self.dark = true;
                                            self.set_theme_plugin(id);
                                        } else if id == "theme-light" {
                                            self.dark = false;
                                            self.set_theme_plugin(id);
                                        } else {
                                            self.set_theme_plugin(id);
                                        }
                                        ui.close();
                                    }
                                }
                            });
                            ui.menu_button(tr("sub.uilang"), |ui| {
                                for l in Lang::ALL {
                                    if menu_check(ui, &tc, crate::i18n::cur() == l, l.native())
                                        .clicked()
                                    {
                                        let mut n = self.current_settings();
                                        n.lang = l.code().into();
                                        self.apply_settings(&n);
                                        ui.close();
                                    }
                                }
                            });
                            ui.separator();
                            ui.menu_button(tr("sub.charset"), |ui| {
                                let cur_cs = self.cur().charset;
                                for cs in editcore::document_io::Charset::ALL {
                                    if menu_check(ui, &tc, cur_cs == cs, cs.label()).clicked() {
                                        self.cur().set_charset(cs);
                                        ui.close();
                                    }
                                }
                            });
                            ui.menu_button(tr("sub.eolmode"), |ui| {
                                let cur_eol = self.cur().eol;
                                for e in [
                                    editcore::document_io::Eol::Crlf,
                                    editcore::document_io::Eol::Lf,
                                    editcore::document_io::Eol::Cr,
                                ] {
                                    if menu_check(ui, &tc, cur_eol == e, e.label()).clicked() {
                                        self.cur().set_eol(e);
                                        ui.close();
                                    }
                                }
                            });
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.export-config", "")).clicked() {
                                self.export_settings();
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.import-config", "")).clicked() {
                                self.import_settings();
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.import-sakura", "")).clicked() {
                                self.import_sakura();
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.reset-settings", "")).clicked() {
                                self.reset_settings();
                                ui.close();
                            }
                            if ui.button(tr("c.open-autosave")).clicked() {
                                self.open_autosave();
                                ui.close();
                            }
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.key-config", "")).clicked() {
                                self.cmdui.open_keycfg();
                                ui.close();
                            }
                        })
                    },
                );
menuui::top(
                    ui,
                    &mut mids,
                    "menu.window",
                    menuui::menu_title(&tc, tr("menu.window"), mn("menu.window")),
                    |ui| {
                        menuui::scroll(ui, |ui| {
                            if menu_check(
                                ui,
                                &tc,
                                self.panes.kind == crate::split::SplitKind::Cols,
                                lk("c.split", "Ctrl+\\"),
                            )
                            .clicked()
                            {
                                self.toggle_split();
                                ui.close();
                            }
                            if menu_check(
                                ui,
                                &tc,
                                self.panes.kind == crate::split::SplitKind::Rows,
                                lk("c.split-rows", ""),
                            )
                            .clicked()
                            {
                                self.toggle_split_kind(crate::split::SplitKind::Rows);
                                ui.close();
                            }
                            if menu_check(
                                ui,
                                &tc,
                                self.panes.kind == crate::split::SplitKind::Quad,
                                lk("c.split-quad", ""),
                            )
                            .clicked()
                            {
                                self.toggle_split_kind(crate::split::SplitKind::Quad);
                                ui.close();
                            }
                            if self.split_on {
                                if menuui::mbtn(ui, lk("c.split-off", "")).clicked() {
                                    self.end_split();
                                    ui.close();
                                }
                            }
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.next-tab", "Ctrl+Tab")).clicked() {
                                self.next_tab(true);
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.prev-tab", "Ctrl+Shift+Tab")).clicked() {
                                self.next_tab(false);
                                ui.close();
                            }
                            ui.menu_button(tr("sub.tabops"), |ui| {
                                if menuui::mbtn(ui, lk("c.tab-move-left", "")).clicked() {
                                    self.move_active_tab(true);
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.tab-move-right", "")).clicked() {
                                    self.move_active_tab(false);
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.close-others", "")).clicked() {
                                    self.close_many(CloseKind::Others);
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.close-left", "")).clicked() {
                                    self.close_many(CloseKind::Left);
                                    ui.close();
                                }
                                if menuui::mbtn(ui, lk("c.close-right", "")).clicked() {
                                    self.close_many(CloseKind::Right);
                                    ui.close();
                                }
                            });
                            ui.menu_button(trz("标签分组"), |ui| {
                                for (z, a) in [
                                    ("下一个分组", KApp::GroupNext),
                                    ("上一个分组", KApp::GroupPrev),
                                    ("移到新分组", KApp::TabSeparate),
                                    ("移到下一个分组", KApp::TabJointNext),
                                    ("移到上一个分组", KApp::TabJointPrev),
                                    ("合并为一组", KApp::BindWindow),
                                    ("关闭当前分组", KApp::GroupClose),
                                ] {
                                    if ui.button(Self::app_label(a, z)).clicked() {
                                        self.run_app(a, ctx);
                                        ui.close();
                                    }
                                }
                            });
                            if ui
                                .button(Self::app_label(KApp::WindowList, "窗口一览…"))
                                .clicked()
                            {
                                self.run_app(KApp::WindowList, ctx);
                                ui.close();
                            }
                            ui.menu_button(tr("sub.winlist"), |ui| {
                                let mut pick = None;
                                for (i, d) in self.docs.iter().enumerate() {
                                    if ui
                                        .selectable_label(
                                            i == self.active,
                                            format!(
                                                "{}{}",
                                                if d.dirty { "● " } else { "" },
                                                d.name
                                            ),
                                        )
                                        .clicked()
                                    {
                                        pick = Some(i);
                                        ui.close();
                                    }
                                }
                                if let Some(i) = pick {
                                    self.select_tab(i);
                                }
                            });
                            ui.separator();
                            if menuui::mbtn(ui, lk("c.minimize-window", "")).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                                ui.close();
                            }
                            if menuui::mbtn(ui, lk("c.maximize", "")).clicked() {
                                let m = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!m));
                                ui.close();
                            }
                            if menu_check(ui, &tc, self.fullscreen, lk("c.fullscreen", "F11"))
                                .clicked()
                            {
                                self.toggle_fullscreen(ctx);
                                ui.close();
                            }
                            if menu_check(ui, &tc, self.topmost, lk("c.topmost", "")).clicked() {
                                self.toggle_topmost(ctx);
                                ui.close();
                            }
                        })
                    },
                );
menuui::top(
                    ui,
                    &mut mids,
                    "menu.help",
                    menuui::menu_title(&tc, tr("menu.help"), mn("menu.help")),
                    |ui| {
                        if menuui::mbtn(ui, lk("c.palette", "")).clicked() {
                            ui.close();
                            self.run_app(KApp::Palette, ctx);
                        }
                        if menuui::mbtn(ui, lk("c.key-list", "")).clicked() {
                            ui.close();
                            self.cmdui.open_keylist();
                        }
                        if menuui::mbtn(ui, lk("c.key-config", "")).clicked() {
                            ui.close();
                            self.cmdui.open_keycfg();
                        }
                        ui.separator();
                        for (zh, a) in [
                            ("帮助手册", KApp::Help),
                            ("关键字帮助检索…", KApp::HelpSearch),
                            ("关键字帮助: 光标处的词", KApp::ExtHelp),
                        ] {
                            if ui.button(Self::app_label(a, zh)).clicked() {
                                ui.close();
                                self.run_app(a, ctx);
                            }
                        }
                        if menu_check(
                            ui,
                            &tc,
                            self.cfg.view.key_search,
                            Self::app_label(KApp::KeySearch, "关键字帮助自动显示"),
                        )
                        .clicked()
                        {
                            ui.close();
                            self.run_app(KApp::KeySearch, ctx);
                        }
                        ui.separator();
                        if menuui::mbtn(ui, lk("c.wiki", "")).clicked() {
                            ui.close();
                            let _ = crate::md_view::open_url("https://github.com/UlyssesLeoLee/IDE1.0/wiki");
                        }
                        if ui.button(trz("欢迎与安装向导…")).clicked() {
                            ui.close();
                            self.installer.open = true;
                        }
                        if menuui::mbtn(ui, lk("c.check-update", "")).clicked() {
                            ui.close();
                            self.upd_open();
                        }
                        if menuui::mbtn(ui, lk("c.about", "")).clicked() {
                            ui.close();
                            self.about_open = true;
                        }
                    },
                );
            });
        self.sys.menu_ids = mids;
    }

    /// 宏菜单的内容 (「宏」和「工具 > 宏」共用)
    fn macro_items(&mut self, ui: &mut egui::Ui) {
        let rec = self.cur().rec.is_some();
        if menuui::mbtn(
            ui,
            lk(
                if rec { "c.macro-stop" } else { "c.macro-start" },
                "Ctrl+Shift+M",
            ),
        )
        .clicked()
        {
            self.cur().macro_toggle();
            ui.close();
        }
        if menuui::mbtn(ui, lk("c.macro-play", "Ctrl+Shift+L")).clicked() {
            self.cur().macro_play();
            ui.close();
        }
        ui.separator();
        if ui.button(trz("保存键盘宏…")).clicked() {
            ui.close();
            self.macro_save();
        }
        if ui.button(trz("载入键盘宏…")).clicked() {
            ui.close();
            self.macro_load();
        }
        ui.separator();
        let rec_mac = self.cur().rec.is_some();
        if ui
            .button(trz(if rec_mac {
                "停止录制并生成 .mac"
            } else {
                "宏录制 (.mac)"
            }))
            .clicked()
        {
            ui.close();
            self.macro_rec_mac();
        }
        if ui.button(trz("把刚录的键盘宏转成 .mac")).clicked() {
            ui.close();
            self.macro_to_mac();
        }
        ui.separator();
        let ctx = ui.ctx().clone();
        let mut run_slot: Option<usize> = None;
        ui.menu_button(trz("宏登记"), |ui| {
            let mut any = false;
            for (i, s) in self
                .cfg
                .macro_slots
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.is_empty())
            {
                any = true;
                if ui
                    .button(format!(
                        "{}: {}",
                        i + 1,
                        if s.name.is_empty() {
                            crate::msgtr::msg("(无名)")
                        } else {
                            std::borrow::Cow::Borrowed(s.name.as_str())
                        }
                    ))
                    .clicked()
                {
                    run_slot = Some(i);
                    ui.close();
                }
            }
            if !any {
                ui.label(tr("ui.none"));
            }
            ui.separator();
            if ui.button(trz("把当前文档 / 选区登记为宏")).clicked() {
                self.register_macro_doc();
                ui.close();
            }
        });
        if let Some(i) = run_slot {
            self.run_macro_slot(i, &ctx);
        }
        if menuui::mbtn(ui, lk("c.macro-run-doc", "")).clicked() {
            ui.close();
            self.run_mac_doc(&ctx);
        }
        if menuui::mbtn(ui, lk("c.macro-run-file", "")).clicked() {
            ui.close();
            self.run_mac_file(&ctx);
        }
        if menuui::mbtn(ui, lk("c.macro-run-ppa", "")).clicked() {
            ui.close();
            self.run_ppa_doc();
        }
    }

    /// 语法类型子菜单的内容 (「设置」和「工具」共用)
    fn syntax_items(&mut self, ui: &mut egui::Ui) {
        let tc = self.tc;
        use menuui::menu_check;
        let i = self.active.min(self.docs.len() - 1);
        let cur = self.docs[i].syntax.clone();
        if menu_check(ui, &tc, cur.is_none(), trz("自动 (按扩展名)")).clicked() {
            self.docs[i].syntax = None;
            ui.close();
        }
        ui.separator();
        let names: Vec<String> = self.registry.all().iter().map(|t| t.name.clone()).collect();
        for n in names {
            if menu_check(ui, &tc, cur.as_deref() == Some(n.as_str()), &n).clicked() {
                self.docs[i].syntax = Some(n);
                ui.close();
            }
        }
    }

    /// 「工具」菜单 (Tauri `MENU_DEF` 的 tool): 大纲 / 语法 / 类型别设定 / 设置导出导入 / 自动备份 / 文件加锁 / 宏 / 插件 / 执行外部命令,
    /// 每项接到 ide-fast 已有的功能 (Tauri 的 SQL*Plus 与「历史管理」ide-fast 没有对应功能, 不列)
    fn tool_items(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        if ui.button(tr("sd.outline")).clicked() {
            self.side.open = true;
            self.side.tab = crate::sidebar::Tab::Outline;
            self.expand_dock(crate::panels::Dock::Side);
            ui.close();
        }
        ui.menu_button(trz("语法类型"), |ui| self.syntax_items(ui));
        if menuui::mbtn(ui, lk("c.type-config", "Ctrl+5")).clicked() {
            self.open_prefs_tab(crate::prefs::PrefsTab::Types);
            ui.close();
        }
        if menuui::mbtn(ui, lk("c.common-config", "Ctrl+6")).clicked() {
            self.open_prefs();
            ui.close();
        }
        if menuui::mbtn(ui, lk("c.export-config", "")).clicked() {
            ui.close();
            self.export_settings();
        }
        if menuui::mbtn(ui, lk("c.import-config", "")).clicked() {
            ui.close();
            self.import_settings();
        }
        ui.separator();
        if ui.button(tr("c.open-autosave")).clicked() {
            ui.close();
            self.open_autosave();
        }
        if ui.button(trz("立即备份")).clicked() {
            ui.close();
            self.backup_now();
        }
        let ro = self.cur().readonly;
        if menuui::menu_check(ui, &self.tc, ro, tr("c.file-lock-toggle")).clicked() {
            let d = self.cur();
            d.readonly = !ro;
            d.tip = tr(if d.readonly {
                "dlg.file-locked"
            } else {
                "dlg.file-unlocked"
            })
            .into();
            ui.close();
        }
        ui.separator();
        ui.menu_button(tr("menu.macro"), |ui| self.macro_items(ui));
        ui.menu_button(tr("menu.plugin"), |ui| self.plugin_panel(ui));
        ui.separator();
        if menuui::mbtn(ui, Self::app_label(KApp::ExecCmd, "执行外部命令 (终端)")).clicked()
        {
            self.open_exec_terminal();
            ui.close();
        }
        ui.separator();
        if ui.button("多 Agent 协同工作台…").clicked() {
            self.collab.open = true;
            ui.close();
        }
    }

    /// Sakura 风格换行方式子菜单项 (不折行、按窗口宽度、按指定列数、禁则、悬挂与详细设置)
    pub(super) fn wrap_mode_menu_items(
        &mut self,
        ui: &mut egui::Ui,
        tc: &ThemeColors,
        ctx: &egui::Context,
    ) {
        let col = self.cfg.view.wrap_col;
        if menu_check(
            ui,
            tc,
            !self.wrap,
            Self::app_label(KApp::WrapNone, "不折行"),
        )
        .clicked()
        {
            self.run_app(KApp::WrapNone, ctx);
            ui.close();
        }
        if menu_check(
            ui,
            tc,
            self.wrap && col == 0,
            Self::app_label(KApp::WrapWindow, "按窗口宽度折行"),
        )
        .clicked()
        {
            self.run_app(KApp::WrapWindow, ctx);
            ui.close();
        }
        let label = if col > 0 {
            format!(
                "{} ({col})",
                Self::app_label(KApp::WrapCol, "按指定列数折行…")
            )
        } else {
            Self::app_label(KApp::WrapCol, "按指定列数折行…")
        };
        if menu_check(ui, tc, self.wrap && col > 0, label).clicked() {
            self.run_app(KApp::WrapCol, ctx);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(
                col > 0,
                crate::ckbox::Check::new(
                    &mut self.cfg.view.kinsoku,
                    trz("行头 / 行尾禁则 (指定列数折行时)"),
                ),
            )
            .clicked()
        {
            self.cfg.save();
        }
        if ui
            .add_enabled(
                col > 0 && self.cfg.view.kinsoku,
                crate::ckbox::Check::new(
                    &mut self.cfg.view.hang,
                    trz("句读挂在行尾 (ぶら下げ)"),
                ),
            )
            .clicked()
        {
            self.cfg.save();
        }
        ui.separator();
        if menuui::mbtn(ui, tr("pf.wrap-col-setting")).clicked() {
            self.open_prefs();
            ui.close();
        }
    }
}
