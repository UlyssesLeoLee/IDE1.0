//! 命令面板 / 键位设置 / 键位一览的界面状态和窗口。命令表本身在 keymap.rs; 这里不执行命令,
//! 只把用户的选择 (`Item`) 或改好的键位表还给 app.rs, 由它去跑、去落盘。

use crate::i18n::{tr, trf};
use crate::keymap::{self, Combo, Overrides};
use eframe::egui::{self, Key, Modifiers};

/// 命令面板里的一行: 注册表里的命令, 或者脚本插件贡献的命令 (下标指向传进来的插件列表)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Cmd(usize),
    Plugin(usize),
}

/// (插件 id, 命令名, 标题)
pub type PluginCmd = (String, String, String);

/// 面板里最多列出这么多行 (全部命令也就一百多条, 这只是防御)
const MAX_ROWS: usize = 200;

/// 过滤 + 排序: 注册表命令和插件命令一起按模糊匹配分数排; 同分时注册表命令在前, 其余保持原来的顺序
pub fn palette_items(query: &str, plugins: &[PluginCmd]) -> Vec<Item> {
    let mut hits: Vec<(i32, u8, usize)> = vec![];
    for (i, e) in keymap::entries().iter().enumerate() {
        if let Some(s) = keymap::fuzzy_score(query, &keymap::haystack(e)) {
            hits.push((s, 0, i));
        }
    }
    for (i, (id, cmd, title)) in plugins.iter().enumerate() {
        if let Some(s) =
            keymap::fuzzy_score(query, &format!("{title} {id} {cmd} {}", tr("pal.plugin")))
        {
            hits.push((s, 1, i));
        }
    }
    hits.sort_by_key(|&(s, g, i)| (std::cmp::Reverse(s), g, i));
    hits.into_iter()
        .take(MAX_ROWS)
        .map(|(_, g, i)| {
            if g == 0 {
                Item::Cmd(i)
            } else {
                Item::Plugin(i)
            }
        })
        .collect()
}

struct PaletteState {
    query: String,
    sel: usize,
    /// 选中行刚被方向键改过: 这一帧要把它滚进可见范围
    moved: bool,
    /// 仅在打开首帧请求一次焦点，避免每帧重复 request_focus 导致 Windows 输入法 (如搜狗拼音) 反复重置上下文并狂闪
    focus_needed: bool,
}

struct Pending {
    id: String,
    combo: Combo,
    /// 占着这个键的命令的名字 (给用户看)
    taken: Vec<String>,
}

struct KeyCfgState {
    filter: String,
    /// 正在等用户按下新键的命令 id
    capture: Option<String>,
    pending: Option<Pending>,
    msg: String,
}

#[derive(Default)]
pub struct CmdUi {
    palette: Option<PaletteState>,
    keycfg: Option<KeyCfgState>,
    keylist: Option<String>,
}

enum Captured {
    Cancel,
    Key(Combo),
}

/// 一帧事件里第一个「真正的键」(修饰键本身跳过; Esc 且不带修饰键 = 取消)
fn pick(events: &[egui::Event]) -> Option<Captured> {
    events.iter().find_map(|e| match e {
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } if !keymap::is_modifier_key(*key) => {
            if *key == Key::Escape && !modifiers.command && !modifiers.alt && !modifiers.shift {
                return Some(Captured::Cancel);
            }
            let mut m = Modifiers::NONE;
            if modifiers.command || modifiers.ctrl {
                m |= Modifiers::COMMAND;
            }
            if modifiers.alt {
                m |= Modifiers::ALT;
            }
            if modifiers.shift {
                m |= Modifiers::SHIFT;
            }
            Some(Captured::Key((m, *key)))
        }
        _ => None,
    })
}

/// 这一帧里用户按下的键; 取到之后这一帧的按键事件到此为止 (否则同一帧稍后的快捷键分发会把刚绑好的键当成一次按下, 立刻执行那条命令)
fn captured(ctx: &egui::Context) -> Option<Captured> {
    let got = ctx.input(|i| pick(&i.events));
    if got.is_some() {
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                !matches!(
                    e,
                    egui::Event::Key { .. }
                        | egui::Event::Text(_)
                        | egui::Event::Ime(_)
                        | egui::Event::Paste(_)
                )
            })
        });
    }
    got
}

impl CmdUi {
    /// 打开命令面板; 已经开着就关掉 (快捷键再按一次 = 收起)
    pub fn toggle_palette(&mut self) {
        self.palette = if self.palette.is_some() {
            None
        } else {
            Some(PaletteState {
                query: String::new(),
                sel: 0,
                moved: true,
                focus_needed: true,
            })
        };
    }

    /// 命令面板是否处于打开状态
    pub fn is_palette_open(&self) -> bool {
        self.palette.is_some()
    }

    pub fn open_keycfg(&mut self) {
        if self.keycfg.is_none() {
            self.keycfg = Some(KeyCfgState {
                filter: String::new(),
                capture: None,
                pending: None,
                msg: String::new(),
            });
        }
    }

    pub fn open_keylist(&mut self) {
        if self.keylist.is_none() {
            self.keylist = Some(String::new());
        }
    }

    /// 正在等用户按新键: 这期间快捷键分发和编辑区都不能吃键盘
    pub fn capturing(&self) -> bool {
        self.keycfg.as_ref().is_some_and(|k| k.capture.is_some())
    }

    /// 面板开着时, 文本框拿着焦点; 返回用户选中的命令
    pub fn palette_window(&mut self, ctx: &egui::Context, plugins: &[PluginCmd]) -> Option<Item> {
        let st = self.palette.as_mut()?;
        let items = palette_items(&st.query, plugins);
        // 先把方向键 / Enter / Esc 拿走, 单行文本框就不会拿它们去移动光标
        let (down, up, enter, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if esc {
            self.palette = None;
            return None;
        }
        let last = items.len().saturating_sub(1);
        if down {
            st.sel = (st.sel + 1).min(last);
            st.moved = true;
        }
        if up {
            st.sel = st.sel.saturating_sub(1);
            st.moved = true;
        }
        st.sel = st.sel.min(last);
        let mut picked: Option<Item> = if enter {
            items.get(st.sel).copied()
        } else {
            None
        };
        egui::Window::new("cmd-palette")
            .title_bar(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 32.0])
            .fixed_size([580.0, 360.0])
            .show(ctx, |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut st.query)
                        .hint_text(tr("pal.hint"))
                        .desired_width(f32::INFINITY),
                );
                if st.focus_needed {
                    r.request_focus();
                    st.focus_needed = false;
                }
                if r.changed() {
                    st.sel = 0;
                    st.moved = true;
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if items.is_empty() {
                            ui.weak(tr("pal.none"));
                        }
                        for (n, it) in items.iter().enumerate() {
                            let (label, cat, keys) = describe(*it, plugins);
                            ui.horizontal(|ui| {
                                let resp = ui.selectable_label(n == st.sel, label);
                                if resp.clicked() {
                                    picked = Some(*it);
                                }
                                if n == st.sel && st.moved {
                                    resp.scroll_to_me(None);
                                }
                                ui.weak(cat);
                                if !keys.is_empty() {
                                    ui.monospace(keys);
                                }
                            });
                        }
                        st.moved = false;
                    });
            });
        if picked.is_some() {
            self.palette = None;
        }
        picked
    }

    /// 键位设置: 列出全部命令和键, 「改键」后按下新组合; 冲突时问是否抢占。键位表有改动时返回新表 (调用方套用并落盘)
    pub fn keycfg_window(&mut self, ctx: &egui::Context, ov: &Overrides) -> Option<Overrides> {
        let st = self.keycfg.as_mut()?;
        let mut work = ov.clone();
        let mut changed = false;
        if let Some(id) = st.capture.clone() {
            match captured(ctx) {
                Some(Captured::Cancel) => st.capture = None,
                Some(Captured::Key(c)) => {
                    st.capture = None;
                    let taken = keymap::find_conflicts(&work, &id, c);
                    let name = keymap::entry_by_id(&id)
                        .map(|e| e.label.text())
                        .unwrap_or_default();
                    if let Err(e) = keymap::validate_binding(c.0, c.1) {
                        st.msg = e;
                    } else if taken.is_empty() || taken.iter().any(|t| t.fixed) {
                        match keymap::assign(&mut work, &id, c) {
                            Ok(_) => {
                                changed = true;
                                st.msg =
                                    trf("kc.assigned", &[name, &keymap::format_combo(c.0, c.1)]);
                            }
                            Err(e) => st.msg = e,
                        }
                    } else {
                        st.pending = Some(Pending {
                            id,
                            combo: c,
                            taken: taken.iter().map(|t| t.label.text().to_string()).collect(),
                        });
                    }
                }
                None => {}
            }
        }
        let mut open = true;
        let mut close = false;
        egui::Window::new(tr("kc.title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(640.0)
            .default_height(460.0)
            .default_pos(egui::pos2(24.0, 56.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut st.filter)
                            .hint_text(tr("kc.filter"))
                            .desired_width(240.0),
                    );
                    if ui.button(tr("kc.reset-all")).clicked() {
                        work.clear();
                        changed = true;
                        st.pending = None;
                    }
                    ui.weak(trf("kc.changed", &[&work.len().to_string()]));
                });
                if st.capture.is_some() {
                    ui.colored_label(ui.visuals().warn_fg_color, tr("kc.capture"));
                }
                if let Some(p) = st.pending.take() {
                    let name = keymap::entry_by_id(&p.id)
                        .map(|e| e.label.text())
                        .unwrap_or_default();
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        trf(
                            "kc.conflict",
                            &[
                                &keymap::format_combo(p.combo.0, p.combo.1),
                                &p.taken.join(", "),
                            ],
                        ),
                    );
                    let mut keep = true;
                    ui.horizontal(|ui| {
                        if ui.button(tr("kc.steal")).clicked() {
                            if keymap::assign(&mut work, &p.id, p.combo).is_ok() {
                                changed = true;
                                st.msg = trf(
                                    "kc.assigned",
                                    &[name, &keymap::format_combo(p.combo.0, p.combo.1)],
                                );
                            }
                            keep = false;
                        }
                        if ui.button(tr("kc.cancel")).clicked() {
                            keep = false;
                        }
                    });
                    if keep {
                        st.pending = Some(p);
                    }
                } else if !st.msg.is_empty() {
                    ui.weak(&st.msg);
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(ui.available_height() - 36.0)
                    .show(ui, |ui| {
                        egui::Grid::new("kc-grid")
                            .striped(true)
                            .num_columns(4)
                            .show(ui, |ui| {
                                for e in keymap::entries() {
                                    let keys = keymap::effective(&work, e).to_string();
                                    let shown = keys
                                        .split('|')
                                        .filter(|s| !s.is_empty())
                                        .collect::<Vec<_>>()
                                        .join(" / ");
                                    if keymap::fuzzy_score(
                                        &st.filter,
                                        &format!("{} {}", keymap::haystack(e), shown),
                                    )
                                    .is_none()
                                    {
                                        continue;
                                    }
                                    let overridden = work.iter().any(|(i, _)| *i == e.id);
                                    ui.weak(e.cat.text());
                                    ui.label(e.label.text());
                                    if shown.is_empty() {
                                        ui.weak(tr("kc.unbound"));
                                    } else if overridden {
                                        ui.strong(egui::RichText::new(&shown).monospace());
                                    } else {
                                        ui.monospace(&shown);
                                    }
                                    ui.horizontal(|ui| {
                                        if e.fixed {
                                            ui.weak(tr("kc.fixed"));
                                        } else {
                                            if ui.small_button(tr("kc.rebind")).clicked() {
                                                st.capture = Some(e.id.clone());
                                                st.pending = None;
                                                st.msg.clear();
                                            }
                                            if !shown.is_empty()
                                                && ui.small_button(tr("kc.clear")).clicked()
                                            {
                                                keymap::set_binding(&mut work, &e.id, "");
                                                changed = true;
                                            }
                                            if overridden
                                                && ui.small_button(tr("kc.default")).clicked()
                                            {
                                                work.retain(|(i, _)| *i != e.id);
                                                changed = true;
                                            }
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
                ui.separator();
                if ui.button(tr("kc.close")).clicked() {
                    close = true;
                }
            });
        if !open || close {
            self.keycfg = None;
        }
        if changed {
            Some(work)
        } else {
            None
        }
    }

    /// 键位一览 (只读, 可筛选, 可复制全部); 点了复制返回提示文字
    pub fn keylist_window(&mut self, ctx: &egui::Context) -> Option<String> {
        let filter = self.keylist.as_mut()?;
        let mut open = true;
        let mut tip = None;
        egui::Window::new(tr("kc.list-title"))
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .default_height(420.0)
            .default_pos(egui::pos2(24.0, 56.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(filter)
                            .hint_text(tr("kc.filter"))
                            .desired_width(220.0),
                    );
                    if ui.button(tr("kc.copy")).clicked() {
                        ctx.copy_text(keymap::list_text());
                        tip = Some(tr("kc.copied").to_string());
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("kl-grid")
                            .striped(true)
                            .num_columns(3)
                            .show(ui, |ui| {
                                for e in keymap::entries() {
                                    let Some(keys) =
                                        keymap::display(&e.id).filter(|k| !k.is_empty())
                                    else {
                                        continue;
                                    };
                                    if keymap::fuzzy_score(
                                        filter,
                                        &format!("{} {}", keymap::haystack(e), keys),
                                    )
                                    .is_none()
                                    {
                                        continue;
                                    }
                                    ui.weak(e.cat.text());
                                    ui.label(e.label.text());
                                    ui.monospace(keys);
                                    ui.end_row();
                                }
                            });
                    });
            });
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.keylist = None;
        }
        tip
    }
}

/// (标签, 分类, 键) —— 面板一行的显示内容
fn describe(it: Item, plugins: &[PluginCmd]) -> (String, String, String) {
    match it {
        Item::Cmd(i) => {
            let e = &keymap::entries()[i];
            (
                e.label.text().to_string(),
                e.cat.text().to_string(),
                keymap::display(&e.id).unwrap_or_default(),
            )
        }
        Item::Plugin(i) => {
            let (_, _, title) = &plugins[i];
            (
                format!("▶ {title}"),
                tr("pal.plugin").to_string(),
                String::new(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plug() -> Vec<PluginCmd> {
        vec![
            ("doc-graph".into(), "show".into(), "Document Graph".into()),
            ("ppa".into(), "run".into(), "Run PPA".into()),
        ]
    }

    #[test]
    fn empty_query_lists_everything_commands_first() {
        let items = palette_items("", &plug());
        assert_eq!(items.len(), (keymap::entries().len() + 2).min(MAX_ROWS));
        assert!(matches!(items[0], Item::Cmd(0)));
    }

    #[test]
    fn plugin_commands_are_searchable() {
        let items = palette_items("graph", &plug());
        assert!(items.contains(&Item::Plugin(0)));
        assert!(!items.contains(&Item::Plugin(1)));
    }

    #[test]
    fn query_with_no_match_is_empty() {
        assert!(palette_items("qqqqzzzz", &plug()).is_empty());
    }

    #[test]
    fn describe_shows_the_active_key() {
        keymap::set_overrides(&[]);
        let i = keymap::entries()
            .iter()
            .position(|e| e.id == "c.find-next")
            .unwrap();
        let (_, _, keys) = describe(Item::Cmd(i), &[]);
        assert_eq!(keys, "F3");
        let (label, _, _) = describe(Item::Plugin(1), &plug());
        assert_eq!(label, "▶ Run PPA");
    }

    fn key_ev(key: Key, m: Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: m,
        }
    }

    #[test]
    fn capture_skips_modifier_presses_and_reads_the_real_key() {
        let ctrl_alt = Modifiers::COMMAND | Modifiers::ALT;
        let evs = vec![
            key_ev(Key::ControlLeft, Modifiers::COMMAND),
            key_ev(Key::AltLeft, ctrl_alt),
            key_ev(Key::K, ctrl_alt),
        ];
        match pick(&evs) {
            Some(Captured::Key((m, k))) => {
                assert_eq!(k, Key::K);
                assert!(m.command && m.alt && !m.shift);
            }
            _ => panic!("应当取到 Ctrl+Alt+K"),
        }
        assert!(pick(&evs[..2]).is_none(), "只有修饰键 = 还没按完");
        assert!(matches!(
            pick(&[key_ev(Key::Escape, Modifiers::NONE)]),
            Some(Captured::Cancel)
        ));
        assert!(
            matches!(
                pick(&[key_ev(Key::Escape, Modifiers::SHIFT)]),
                Some(Captured::Key(_))
            ),
            "带修饰键的 Esc 当普通键 (之后会被校验拒绝)"
        );
    }

    #[test]
    fn toggle_and_capture_flags() {
        let mut ui = CmdUi::default();
        assert!(ui.palette.is_none());
        ui.toggle_palette();
        assert!(ui.palette.is_some());
        ui.toggle_palette();
        assert!(ui.palette.is_none());
        assert!(!ui.capturing());
        ui.open_keycfg();
        assert!(!ui.capturing());
        ui.keycfg.as_mut().unwrap().capture = Some("c.save".into());
        assert!(ui.capturing());
    }

    #[test]
    fn test_palette_focus_needed_single_shot_invariant() {
        let mut ui = CmdUi::default();
        assert!(!ui.is_palette_open());

        // 打开命令面板：必须标记 focus_needed = true
        ui.toggle_palette();
        assert!(ui.is_palette_open());
        assert!(ui.palette.as_ref().unwrap().focus_needed);

        let ctx = egui::Context::default();
        let mut out1 = ctx.run_ui(Default::default(), |_| {
            let _ = ui.palette_window(&ctx, &[]);
        });
        out1.textures_delta.clear();

        // 经过第一帧渲染后，focus_needed 必须被一次性消耗重置为 false，杜绝后续每帧反复 request_focus 导致搜狗拼音等 IME 狂闪
        assert!(ui.is_palette_open());
        assert!(!ui.palette.as_ref().unwrap().focus_needed);

        // 第二帧渲染，仍然保持 false
        let mut out2 = ctx.run_ui(Default::default(), |_| {
            let _ = ui.palette_window(&ctx, &[]);
        });
        out2.textures_delta.clear();
        assert!(!ui.palette.as_ref().unwrap().focus_needed);
    }
}
