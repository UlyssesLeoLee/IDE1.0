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
}
