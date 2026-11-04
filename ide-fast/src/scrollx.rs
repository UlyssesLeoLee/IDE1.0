//! 侧栏 / 终端的细滚动条 (Tauri `#side-body` / `#terminal-body`: 10px 宽, 滑块 `rgba(127,127,127,.45)` 全宽圆角)。
//! 全局滚动条是 12px 款 (`tauri_scroll`); 这两处用 `show` 代替 `ScrollArea::show`: 在外层 scope 里换滚动条样式和滑块色,
//! 进到内容之前把样式还原, 内容里的按钮 / 文字颜色不受影响。

use eframe::egui::{self, Color32, ScrollArea, Ui};

/// 细滚动条: 10px, 无边距, 滑块半透明灰
pub fn thin() -> egui::style::ScrollStyle {
    egui::style::ScrollStyle {
        bar_width: 10.0, bar_inner_margin: 0.0, bar_outer_margin: 0.0,
        dormant_handle_opacity: 0.45, active_handle_opacity: 0.45, interact_handle_opacity: 0.7,
        ..egui::style::ScrollStyle::solid()
    }
}

/// 滑块底色 (Tauri `rgba(127,127,127,.45)` 的 RGB 部分)
pub const HANDLE: Color32 = Color32::from_rgb(127, 127, 127);

pub fn show<R>(ui: &mut Ui, area: ScrollArea, add: impl FnOnce(&mut Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let orig = ui.style().clone();
    ui.scope(|ui| {
        ui.spacing_mut().scroll = thin();
        let v = ui.visuals_mut();
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] { w.fg_stroke.color = HANDLE; w.bg_fill = HANDLE; }
        area.show(ui, |ui| { ui.set_style(orig); add(ui) })
    }).inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thin_bar_is_ten_wide_with_no_margins() {
        let s = thin();
        assert_eq!(s.allocated_width(), 10.0);
    }
}
