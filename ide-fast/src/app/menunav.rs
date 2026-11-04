//! 菜单键盘导航的纯逻辑 (不碰 egui 上下文, 可单测): 方向键状态机 + 原始输入事件清洗。
//! 状态机: 第一次 ↓ 高亮第一项 (↑ 高亮最后一项), 之后在项间循环; ← → 切换顶层菜单 (循环) 并清掉高亮。

use eframe::egui::{Event, Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavKey { Up, Down, Left, Right }

pub fn nav_key(key: Key, m: Modifiers) -> Option<NavKey> {
    if m.any() { return None; }
    Some(match key { Key::ArrowUp => NavKey::Up, Key::ArrowDown => NavKey::Down, Key::ArrowLeft => NavKey::Left, Key::ArrowRight => NavKey::Right, _ => return None })
}

/// 返回 (顶层菜单下标, 高亮项下标)。`n_items == 0` 时 ↑ ↓ 不动 (菜单项还没登记)
pub fn step(menu: usize, item: Option<usize>, n_menus: usize, n_items: usize, key: NavKey) -> (usize, Option<usize>) {
    match key {
        NavKey::Right if n_menus > 0 => ((menu + 1) % n_menus, None),
        NavKey::Left if n_menus > 0 => ((menu + n_menus - 1) % n_menus, None),
        NavKey::Down if n_items > 0 => (menu, Some(item.map_or(0, |i| (i + 1) % n_items))),
        NavKey::Up if n_items > 0 => (menu, Some(item.map_or(n_items - 1, |i| (i + n_items - 1) % n_items))),
        _ => (menu, item),
    }
}

/// 这一批原始事件里有没有 Alt+字母 (没有 Ctrl / Command)
fn alt_letter(events: &[Event]) -> Option<char> {
    events.iter().find_map(|e| match e {
        Event::Key { key, pressed: true, modifiers, .. } if modifiers.alt && !modifiers.ctrl && !modifiers.command && key.name().len() == 1 && key.name().chars().all(|c| c.is_ascii_alphabetic()) => key.name().chars().next(),
        _ => None,
    })
}

/// 清洗一批原始事件: Alt+字母 在同一帧和下一帧里附带的 Text 事件一律吃掉 (winit 常把字母晚一帧送来);
/// 同一批里有 Esc 且菜单开着 (或同批有 Alt+字母) 时, 这一批的全部 Text 都吃掉, 免得 Esc 关菜单时漏出一个字母。
/// `swallow` 跨帧保存「还要吃哪个字母、几帧」。
pub fn scrub(events: &mut Vec<Event>, swallow: &mut Option<(char, u8)>, menu_open: bool) {
    let alt = alt_letter(events);
    let esc = events.iter().any(|e| matches!(e, Event::Key { key: Key::Escape, pressed: true, .. }));
    if esc && (menu_open || alt.is_some()) {
        events.retain(|e| !matches!(e, Event::Text(_)));
    }
    if let Some(c) = alt { *swallow = Some((c, 2)); }
    if let Some((c, left)) = *swallow {
        events.retain(|e| !matches!(e, Event::Text(t) if t.len() == 1 && t.chars().next().is_some_and(|x| x.eq_ignore_ascii_case(&c))));
        *swallow = if left > 1 { Some((c, left - 1)) } else { None };
    }
}

/// 单按 Alt (按下再松开, 中间没有任何别的键 / 鼠标 / 文字 / 失焦) 的识别器。
/// egui 0.37 没有 `RawInput.modifiers`, 修饰键变化以 `Event::ModifiersChanged` 按顺序送来; 按事件顺序处理, 同一帧里按下又松开也认得。
/// Alt 按下沿「武装」, 之后任何键盘 / 鼠标按键 / 滚轮 / 文字 / 失焦事件, 或又按了 Ctrl / Shift / Cmd, 都解除武装; 松开沿且仍武装才算一次单按。
/// (照 Tauri `altArmed`: Alt+字母、Alt+点击、Alt+拖选 (列块) 都不算。)
#[derive(Default, Debug)]
pub struct AltTap { armed: bool, alt: bool }

impl AltTap {
    /// 喂一帧的原始事件; 返回「这一帧里松开了一次单按的 Alt」
    pub fn feed(&mut self, events: &[Event]) -> bool {
        let mut fire = false;
        for e in events {
            match e {
                Event::ModifiersChanged(m) => {
                    let plain = !m.ctrl && !m.shift && !m.command && !m.mac_cmd;
                    if m.alt && !self.alt { self.armed = plain; }
                    if m.alt && !plain { self.armed = false; }
                    if !m.alt && self.alt { fire |= self.armed; self.armed = false; }
                    self.alt = m.alt;
                }
                // Alt 键本身也作为 Event::Key (AltLeft / AltRight) 送来, 不算打断
                Event::Key { key: Key::AltLeft | Key::AltRight, .. } => {}
                Event::Key { .. } | Event::Text(_) | Event::PointerButton { .. } | Event::MouseWheel { .. } | Event::WindowFocused(_)
                | Event::Copy | Event::Cut | Event::Paste(_) | Event::Ime(_) | Event::Touch { .. } => self.armed = false,
                _ => {}
            }
        }
        fire
    }
}

/// 菜单栏拿到焦点 (单按 Alt 之后、还没展开菜单) 时一次按键的结果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarOut {
    /// 与菜单栏无关: 退出菜单栏焦点, 按键照常交给别处
    Pass,
    /// 退出菜单栏焦点, 按键吃掉 (Esc)
    Leave,
    /// 焦点移到另一个顶层菜单
    Move(usize),
    /// 展开该菜单; 带的方向键给菜单状态机 (↓ / Enter / 空格 = 第一项, ↑ = 最后一项)
    Open(usize, NavKey),
}

/// 菜单栏焦点在第 `cur` 个菜单 (共 `n` 个) 时按下 `key` (无修饰键): ← → 循环切换, ↓ Enter 空格 展开并高亮第一项, ↑ 展开并高亮最后一项, Esc 退出
pub fn bar_key(cur: usize, n: usize, key: Key, m: Modifiers) -> BarOut {
    if m.any() { return BarOut::Pass; }
    match key {
        Key::ArrowLeft => BarOut::Move(step(cur, None, n, 0, NavKey::Left).0),
        Key::ArrowRight => BarOut::Move(step(cur, None, n, 0, NavKey::Right).0),
        Key::ArrowDown | Key::Enter | Key::Space => BarOut::Open(cur, NavKey::Down),
        Key::ArrowUp => BarOut::Open(cur, NavKey::Up),
        Key::Escape => BarOut::Leave,
        _ => BarOut::Pass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: Key, m: Modifiers) -> Event {
        Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m }
    }

    #[test]
    fn first_down_highlights_the_first_item_then_moves_one_at_a_time() {
        let (m, i) = step(2, None, 7, 5, NavKey::Down);
        assert_eq!((m, i), (2, Some(0)));
        let (_, i) = step(m, i, 7, 5, NavKey::Down);
        assert_eq!(i, Some(1));
        let (_, i) = step(m, i, 7, 5, NavKey::Down);
        assert_eq!(i, Some(2), "第三次 ↓ 才到第三项");
        assert_eq!(step(0, Some(4), 7, 5, NavKey::Down).1, Some(0), "到底回到第一项");
    }

    #[test]
    fn up_starts_from_the_last_item_and_wraps() {
        assert_eq!(step(0, None, 7, 5, NavKey::Up).1, Some(4));
        assert_eq!(step(0, Some(0), 7, 5, NavKey::Up).1, Some(4));
        assert_eq!(step(0, Some(3), 7, 5, NavKey::Up).1, Some(2));
    }

    #[test]
    fn left_right_switch_menu_even_with_nothing_highlighted() {
        assert_eq!(step(0, None, 7, 5, NavKey::Right), (1, None), "没高亮时 → 也能切");
        assert_eq!(step(6, Some(3), 7, 5, NavKey::Right), (0, None), "最后一个再 → 回到第一个, 高亮清掉");
        assert_eq!(step(0, None, 7, 5, NavKey::Left), (6, None));
        assert_eq!(step(3, Some(2), 7, 5, NavKey::Left), (2, None));
    }

    #[test]
    fn empty_menu_does_not_move_highlight() {
        assert_eq!(step(1, None, 7, 0, NavKey::Down), (1, None));
        assert_eq!(step(1, None, 0, 0, NavKey::Right), (1, None));
    }

    #[test]
    fn only_bare_arrows_navigate() {
        assert_eq!(nav_key(Key::ArrowDown, Modifiers::NONE), Some(NavKey::Down));
        assert_eq!(nav_key(Key::ArrowDown, Modifiers::SHIFT), None);
        assert_eq!(nav_key(Key::Enter, Modifiers::NONE), None);
    }

    #[test]
    fn esc_with_alt_letter_in_one_frame_types_nothing() {
        let mut ev = vec![key(Key::V, Modifiers::ALT), key(Key::Escape, Modifiers::NONE), Event::Text("v".into()), Event::Text("x".into())];
        let mut sw = None;
        scrub(&mut ev, &mut sw, true);
        assert!(!ev.iter().any(|e| matches!(e, Event::Text(_))));
        // 字母晚一帧到也不漏
        let mut next = vec![Event::Text("v".into()), Event::Text("a".into())];
        scrub(&mut next, &mut sw, false);
        assert_eq!(next, vec![Event::Text("a".into())]);
    }

    #[test]
    fn esc_alone_with_menu_open_eats_text_but_closed_menu_keeps_it() {
        let mut ev = vec![key(Key::Escape, Modifiers::NONE), Event::Text("q".into())];
        scrub(&mut ev, &mut None, true);
        assert_eq!(ev.len(), 1);
        let mut ev = vec![key(Key::Escape, Modifiers::NONE), Event::Text("q".into())];
        scrub(&mut ev, &mut None, false);
        assert_eq!(ev.len(), 2);
    }

    #[test]
    fn ordinary_typing_is_untouched() {
        let mut ev = vec![key(Key::V, Modifiers::NONE), Event::Text("v".into())];
        let mut sw = None;
        scrub(&mut ev, &mut sw, false);
        assert_eq!(ev.len(), 2);
        assert!(sw.is_none());
    }

    fn mc(alt: bool, shift: bool) -> Event {
        Event::ModifiersChanged(Modifiers { alt, shift, ..Default::default() })
    }
    fn click(pressed: bool) -> Event {
        Event::PointerButton { pos: eframe::egui::pos2(0.0, 0.0), button: eframe::egui::PointerButton::Primary, pressed, modifiers: Modifiers::ALT }
    }

    #[test]
    fn alt_alone_fires_on_release() {
        let mut t = AltTap::default();
        assert!(!t.feed(&[mc(true, false), key(Key::AltLeft, Modifiers::ALT)]), "Alt 键自己的 Key 事件不打断");
        assert!(!t.feed(&[Event::PointerMoved(eframe::egui::pos2(1.0, 1.0))]), "只动鼠标不打断");
        assert!(t.feed(&[mc(false, false)]));
        assert!(!t.feed(&[mc(false, false)]), "只触发一次");
    }

    #[test]
    fn alt_pressed_and_released_in_one_frame_still_counts() {
        let mut t = AltTap::default();
        assert!(t.feed(&[mc(true, false), mc(false, false)]));
    }

    #[test]
    fn alt_with_a_key_click_text_or_wheel_does_not_fire() {
        for ev in [
            key(Key::V, Modifiers::ALT),
            Event::Text("v".into()),
            click(true),
            click(false),
            Event::MouseWheel { unit: eframe::egui::MouseWheelUnit::Line, delta: eframe::egui::vec2(0.0, 1.0), phase: eframe::egui::TouchPhase::Move, source: eframe::egui::MouseWheelSource::Wheel, modifiers: Modifiers::ALT },
            Event::WindowFocused(false),
        ] {
            let mut t = AltTap::default();
            t.feed(&[mc(true, false)]);
            t.feed(std::slice::from_ref(&ev));
            assert!(!t.feed(&[mc(false, false)]), "{ev:?}");
        }
        // 按键和 Alt 按下在同一帧也不算
        let mut t = AltTap::default();
        t.feed(&[mc(true, false), key(Key::V, Modifiers::ALT)]);
        assert!(!t.feed(&[mc(false, false)]));
        // Alt+点击: 点击先于 Alt 松开
        let mut t = AltTap::default();
        assert!(!t.feed(&[mc(true, false), click(true), click(false), mc(false, false)]));
    }

    #[test]
    fn alt_after_ctrl_or_shift_does_not_fire_and_next_tap_works_again() {
        let mut t = AltTap::default();
        t.feed(&[mc(false, true), mc(true, true)]); // Shift 已按着时按 Alt
        assert!(!t.feed(&[mc(false, false)]));
        t.feed(&[mc(true, false)]);
        t.feed(&[mc(true, true)]); // 按住 Alt 时又按了 Shift
        assert!(!t.feed(&[mc(false, false)]));
        t.feed(&[mc(true, false)]);
        assert!(t.feed(&[mc(false, false)]), "下一次干净的单按照常触发");
    }

    #[test]
    fn bar_focus_keys_move_open_and_leave() {
        let n = Modifiers::NONE;
        assert_eq!(bar_key(0, 7, Key::ArrowRight, n), BarOut::Move(1));
        assert_eq!(bar_key(6, 7, Key::ArrowRight, n), BarOut::Move(0), "循环");
        assert_eq!(bar_key(0, 7, Key::ArrowLeft, n), BarOut::Move(6));
        assert_eq!(bar_key(2, 7, Key::ArrowDown, n), BarOut::Open(2, NavKey::Down));
        assert_eq!(bar_key(2, 7, Key::Enter, n), BarOut::Open(2, NavKey::Down));
        assert_eq!(bar_key(2, 7, Key::Space, n), BarOut::Open(2, NavKey::Down));
        assert_eq!(bar_key(2, 7, Key::ArrowUp, n), BarOut::Open(2, NavKey::Up));
        assert_eq!(bar_key(2, 7, Key::Escape, n), BarOut::Leave);
        assert_eq!(bar_key(2, 7, Key::A, n), BarOut::Pass);
        assert_eq!(bar_key(2, 7, Key::ArrowRight, Modifiers::SHIFT), BarOut::Pass);
    }
}
