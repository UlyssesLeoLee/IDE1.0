# IDE1.0 标题栏分屏对比与折叠联动详细设计书

**文档编号**: DD-TITLEBAR-DIFF-REBALANCE-2026-V1.0  
**制定组织**: IDE1.0 客户端核心组  
**基准版本**: v1.0  
**关联设计**:
- [BD-004_Titlebar-WinMergeDiff-and-PanelDocks.md](../01_基本设计/BD-004_Titlebar-WinMergeDiff-and-PanelDocks.md)

---

## 1. 标题栏布局规划与降级策略 (`plan_bar`)

标题栏自适应布局计算函数必须保证在高 DPI、窗口窄屏或菜单文本变长时严禁与右侧控制按钮重叠：
```rust
pub fn plan_bar(total: f32, menu_mn: f32, menu_plain: f32, title_w: f32) -> BarPlan {
    let avail = total - BTN_W * 3.0 - LEFT_FIXED - GAP;
    let mut rest = avail;
    let mut take = |w: f32| -> bool {
        if rest >= w { rest -= w; true } else { false }
    };
    // 优先级撤退顺序：
    // 1. 先撤快捷图标钮
    // 2. 再撤多 Agent 协同入口
    // 3. 再撤差异对比入口
    // 4. 再撤窗口标题文字
    // 5. 最后撤菜单的助记快捷键字母
    let mnemonics = rest >= menu_mn;
    let _ = if mnemonics { take(menu_mn) } else { take(menu_plain) };
    let title = take(title_w.min(TITLE_MAX) + 8.0);
    let diff = take(DIFF_BTN_W);
    let collab = take(COLLAB_BTN_W);
    let mut quick = 0;
    while quick < QUICK.len() && take(QUICK_BTN) {
        quick += 1;
    }
    BarPlan { mnemonics, title, diff, collab, quick }
}
```

---

## 2. 停靠折叠自适应压缩算法 (`rebalance_on_dock_open`)

```rust
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
    const MIN_EDITOR_W: f32 = 240.0;
    const MIN_EDITOR_H: f32 = 140.0;
    
    match opened {
        Dock::Side | Dock::Right => {
            // 后打开左侧或右侧: 自动约束先打开的下方停靠区
            if bottom_active {
                let comfortable_h = (viewport_h * 0.35).clamp(120.0, 240.0);
                if *term_h > comfortable_h {
                    *term_h = comfortable_h;
                }
                let max_h = (viewport_h - MIN_EDITOR_H - BOTTOM_HEAD).max(120.0);
                if *term_h > max_h {
                    *term_h = max_h;
                }
            }
            // 确保中央编辑区保留至少 MIN_EDITOR_W 宽度
            let mut total_w = 0.0;
            if side_active { total_w += *side_w; }
            if right_active { total_w += *right_w; }
            let max_total_w = (viewport_w - MIN_EDITOR_W).max(120.0);
            if total_w > max_total_w {
                let excess = total_w - max_total_w;
                if opened == Dock::Side && right_active {
                    *right_w = (*right_w - excess).max(160.0);
                } else if opened == Dock::Right && side_active {
                    *side_w = (*side_w - excess).max(160.0);
                }
            }
        }
        Dock::Bottom => {
            // 打开下方: 确保不超过剩余高度极限
            let max_h = (viewport_h - MIN_EDITOR_H - BOTTOM_HEAD).max(120.0);
            if *term_h > max_h {
                *term_h = max_h;
            }
        }
    }
}
```
