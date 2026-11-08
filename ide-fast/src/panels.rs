//! 面板折叠的状态逻辑 (纯函数, 不碰 egui): 左侧栏 / 右侧面板 (AI 对话、Markdown 预览) / 底部面板 (终端 / 输出 / 问题)
//! 各有「显示 (open)」和「折成细条 (collapsed)」两个布尔量。折叠只在显示时才有意义; 折叠时界面只画一条细条 + 展开按钮,
//! 面板里的内容这一帧一行都不布局。持久化的字段在 `settings::ViewState`, 这里只管怎么翻转、怎么规整。

/// 三处可折叠的停靠区
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dock {
    Side,
    Right,
    Bottom,
}

/// 一个停靠区这一帧该怎么画
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Hidden,
    Strip,
    Full,
}

/// 细条的粗细 (逻辑点): 左右两侧是宽, 底部是高
pub const STRIP: f32 = 16.0;
/// 细条里展开 / 折叠按钮的高 (撑满细条宽)
pub const STRIP_BTN_H: f32 = 40.0;
/// 底部面板头的高度 (折叠后头条保留)
pub const BOTTOM_HEAD: f32 = 28.0;

pub fn mode(open: bool, collapsed: bool) -> Mode {
    match (open, collapsed) {
        (false, _) => Mode::Hidden,
        (true, true) => Mode::Strip,
        (true, false) => Mode::Full,
    }
}

/// 底部面板这一帧怎么画: 无论极简界面还是完整界面, 没展开时都常驻保留 28px 的底部面板头
/// (对齐 Tauri `#bottom-head`: TERMINAL / OUTPUT / PROBLEMS / 协同审查 + 新建终端 + 展开/折叠), 头条状态不启动 shell
pub fn bottom_mode(open: bool, collapsed: bool, _full_ui: bool) -> Mode {
    match mode(open, collapsed) {
        Mode::Hidden => Mode::Strip,
        m => m,
    }
}

/// 折叠命令 / 箭头按钮: 显示着就翻转折叠; 没显示就先展开显示出来。返回新的 (open, collapsed)
pub fn toggle_collapse(open: bool, collapsed: bool) -> (bool, bool) {
    if open {
        (true, !collapsed)
    } else {
        (true, false)
    }
}

/// 「显示 / 隐藏」命令 (Ctrl+B 等): 折成细条时先展开, 展开着才真正隐藏
pub fn toggle_open(open: bool, collapsed: bool) -> (bool, bool) {
    match mode(open, collapsed) {
        Mode::Full => (false, false),
        Mode::Strip | Mode::Hidden => (true, false),
    }
}

/// 隐藏之后折叠标记没有意义 (再次显示应当是展开的)
pub fn normalize(open: bool, collapsed: bool) -> bool {
    open && collapsed
}

/// 右侧停靠区这一帧有没有东西可画
pub fn right_visible(ai_open: bool, md_preview: bool, is_md: bool, overlay: bool) -> bool {
    ai_open || (md_preview && is_md && !overlay)
}

/// 面板尺寸夹到合法范围 (设置文件里读到的值不可信)
pub fn clamp_size(dock: Dock, v: f32) -> f32 {
    if !v.is_finite() {
        return default_size(dock);
    }
    match dock {
        Dock::Side => v.clamp(120.0, 900.0),
        Dock::Right => v.clamp(160.0, 1200.0),
        Dock::Bottom => v.clamp(60.0, 900.0),
    }
}

pub fn default_size(dock: Dock) -> f32 {
    match dock {
        Dock::Side => 250.0,
        Dock::Right => 340.0,
        Dock::Bottom => 180.0,
    }
}

/// 自动协同重平衡折叠面板尺寸 (对齐 Tauri 原版行为):
/// 当先打开下方折叠、再打开左侧或右侧折叠时 (或先打开侧栏再打开下方)，根据视口可用宽高与「让位原则」，
/// 自动对先打开的面板以及新打开的面板做联动尺寸自适应计算，确保中央编辑区保持充足的可读面积与纵横比。
pub fn rebalance_on_dock_open(
    opened: Dock,
    side_active: bool,
    side_w: &mut f32,
    right_active: bool,
    right_w: &mut f32,
    bottom_active: bool,
    term_h: &mut f32,
    viewport_w: f32,
    viewport_h: f32,
) {
    let vw = if viewport_w.is_finite() && viewport_w > 200.0 {
        viewport_w
    } else {
        1100.0
    };
    let vh = if viewport_h.is_finite() && viewport_h > 200.0 {
        viewport_h
    } else {
        720.0
    };

    const MIN_EDITOR_W: f32 = 240.0;
    const MIN_EDITOR_H: f32 = 140.0;

    match opened {
        Dock::Side | Dock::Right => {
            // 后打开左侧或右侧折叠:
            // 1. 若下方折叠当前已处于展开状态，由于主工作区水平宽度被侧栏挤压，
            // 为了维持黄金可读视野与纵横比，先打开的下方折叠必须自动调整/收缩高度 (对齐 Tauri clamp(120px, 35vh, 240px))
            if bottom_active {
                let comfortable_h = (vh * 0.35).clamp(120.0, 240.0);
                if *term_h > comfortable_h {
                    *term_h = comfortable_h;
                }
                let max_h = (vh - MIN_EDITOR_H - BOTTOM_HEAD).max(120.0);
                if *term_h > max_h {
                    *term_h = max_h;
                }
            }

            // 2. 水平总宽度约束: 保证中央编辑区至少保留 MIN_EDITOR_W
            let mut total_w = 0.0;
            if side_active {
                total_w += *side_w;
            }
            if right_active {
                total_w += *right_w;
            }
            let max_total_w = (vw - MIN_EDITOR_W).max(120.0);
            if total_w > max_total_w {
                let excess = total_w - max_total_w;
                if opened == Dock::Side && right_active {
                    // 后开左侧: 先开的右侧让位
                    *right_w = (*right_w - excess).max(160.0);
                } else if opened == Dock::Right && side_active {
                    // 后开右侧: 先开的左侧让位
                    *side_w = (*side_w - excess).max(120.0);
                }
            }
        }
        Dock::Bottom => {
            // 后打开下方折叠:
            // 1. 下方折叠高度不能盲目撑大，舒适比例取 35vh (不超过 240px)，
            // 且绝对不超过可用高度减去最小编辑区保障高度
            let comfortable_h = (vh * 0.35).clamp(120.0, 240.0);
            if *term_h > comfortable_h {
                *term_h = comfortable_h;
            }
            let max_h = (vh - MIN_EDITOR_H - BOTTOM_HEAD).max(120.0);
            if *term_h > max_h {
                *term_h = max_h;
            }
            // 2. 若左侧栏或右侧栏已在展开状态且过宽 (超过 35% 视口宽)，先打开的侧栏相应缩微让位
            if side_active {
                let max_side = (vw * 0.35).max(180.0);
                if *side_w > max_side {
                    *side_w = max_side;
                }
            }
            if right_active {
                let max_right = (vw * 0.40).max(220.0);
                if *right_w > max_right {
                    *right_w = max_right;
                }
            }
        }
    }
}

/// 左右细条按钮上的箭头 (Tauri 的 `‹` U+2039 / `›` U+203A): 箭头指向「点下去面板会往哪走」
pub fn chevron(dock: Dock, collapsed: bool) -> &'static str {
    match (dock, collapsed) {
        (Dock::Side, false) => "\u{2039}",
        (Dock::Side, true) => "\u{203A}",
        (Dock::Right, false) => "\u{203A}",
        (Dock::Right, true) => "\u{2039}",
        (Dock::Bottom, false) => "\u{25BC}",
        (Dock::Bottom, true) => "\u{25B2}",
    }
}

/// 按钮提示 / 无障碍文案: 随状态变 (折叠时说「展开」, 展开时说「折叠」)
pub fn tip(dock: Dock, collapsed: bool) -> &'static str {
    match (dock, collapsed) {
        (Dock::Side, true) => "展开导航 / 文件",
        (Dock::Side, false) => "折叠导航 / 文件",
        (Dock::Right, true) => "展开 Chat",
        (Dock::Right, false) => "折叠 Chat",
        (Dock::Bottom, true) => "展开底部面板",
        (Dock::Bottom, false) => "折叠底部面板",
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bottom_keeps_its_head_in_both_minimal_and_full_ui_when_closed() {
        assert_eq!(bottom_mode(false, false, true), Mode::Strip);
        assert_eq!(bottom_mode(false, false, false), Mode::Strip);
        assert_eq!(bottom_mode(true, false, true), Mode::Full);
        assert_eq!(bottom_mode(true, false, false), Mode::Full);
        assert_eq!(bottom_mode(true, true, false), Mode::Strip);
    }

    use super::*;

    #[test]
    fn mode_follows_open_and_collapsed() {
        assert_eq!(mode(false, false), Mode::Hidden);
        assert_eq!(mode(false, true), Mode::Hidden);
        assert_eq!(mode(true, true), Mode::Strip);
        assert_eq!(mode(true, false), Mode::Full);
    }

    #[test]
    fn collapse_flips_when_open_and_opens_when_hidden() {
        assert_eq!(toggle_collapse(true, false), (true, true));
        assert_eq!(toggle_collapse(true, true), (true, false));
        assert_eq!(
            toggle_collapse(false, false),
            (true, false),
            "没显示: 先展开显示"
        );
        assert_eq!(toggle_collapse(false, true), (true, false));
    }

    #[test]
    fn open_toggle_expands_a_strip_before_hiding() {
        assert_eq!(toggle_open(true, false), (false, false));
        assert_eq!(
            toggle_open(true, true),
            (true, false),
            "细条状态下按 Ctrl+B = 展开"
        );
        assert_eq!(toggle_open(false, false), (true, false));
    }

    #[test]
    fn hidden_panels_forget_their_collapse_flag() {
        assert!(!normalize(false, true));
        assert!(normalize(true, true));
        assert!(!normalize(true, false));
    }

    #[test]
    fn right_dock_needs_chat_or_a_markdown_preview() {
        assert!(!right_visible(false, false, true, false));
        assert!(
            !right_visible(false, true, false, false),
            "预览开着但当前不是 md"
        );
        assert!(
            !right_visible(false, true, true, true),
            "覆盖模式预览不占右侧停靠区"
        );
        assert!(right_visible(true, true, true, true));
        assert!(right_visible(false, true, true, false));
        assert!(right_visible(true, false, false, false));
    }

    #[test]
    fn sizes_are_clamped_and_nan_falls_back() {
        assert_eq!(clamp_size(Dock::Side, 5.0), 120.0);
        assert_eq!(clamp_size(Dock::Side, 5000.0), 900.0);
        assert_eq!(clamp_size(Dock::Bottom, f32::NAN), 180.0);
        assert_eq!(clamp_size(Dock::Right, 400.0), 400.0);
    }

    #[test]
    fn tips_follow_the_state() {
        for d in [Dock::Side, Dock::Right, Dock::Bottom] {
            assert_ne!(tip(d, true), tip(d, false));
            assert!(tip(d, true).starts_with('展') && tip(d, false).starts_with('折'));
        }
        assert_eq!(STRIP, 16.0);
    }

    #[test]
    fn chevrons_point_toward_where_the_panel_goes() {
        assert_ne!(chevron(Dock::Side, false), chevron(Dock::Side, true));
        assert_eq!(chevron(Dock::Side, false), chevron(Dock::Right, true));
    }

    #[test]
    fn rebalance_coordinated_resizing_on_open() {
        let mut side_w = 300.0;
        let mut right_w = 340.0;
        let mut term_h = 320.0;

        // 1. 先打开下方折叠 (term_h=320.0)，后打开左侧折叠 (Dock::Side):
        // 先打开的下方折叠尺寸自动适配收缩至舒适高度 clamp(120.0, 720*0.35=252->240, 240.0)
        rebalance_on_dock_open(
            Dock::Side,
            true,
            &mut side_w,
            false,
            &mut right_w,
            true,
            &mut term_h,
            1100.0,
            720.0,
        );
        assert_eq!(term_h, 240.0, "先打开的底部高度自动收缩至舒适比例 240px 以保全垂直可读视野");

        // 2. 窄视口下双侧栏超宽让位:
        let mut wide_side = 500.0;
        let mut wide_right = 500.0;
        let mut normal_term = 180.0;
        rebalance_on_dock_open(
            Dock::Side,
            true,
            &mut wide_side,
            true,
            &mut wide_right,
            false,
            &mut normal_term,
            1000.0,
            600.0,
        );
        assert!(wide_side + wide_right <= 1000.0 - 240.0 + 1.0, "中央编辑区宽度必须保证至少 240px");

        // 3. 先打开侧栏，后打开下方折叠 (Dock::Bottom):
        let mut large_side = 500.0;
        let mut large_term = 400.0;
        rebalance_on_dock_open(
            Dock::Bottom,
            true,
            &mut large_side,
            false,
            &mut right_w,
            true,
            &mut large_term,
            1000.0,
            600.0,
        );
        assert!(large_term <= 210.0, "底部高度受限不超过 35vh");
        assert!(large_side <= 350.0, "先打开的侧栏让位不超过 35% 视口宽");
    }
}
