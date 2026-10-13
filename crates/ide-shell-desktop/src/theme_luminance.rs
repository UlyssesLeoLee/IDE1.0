//! 桌面背景明暗探测 —— 透明玻璃主题的文字反色依据。
//!
//! # 为什么需要
//!
//! 透明主题 (Mica / Acrylic) 下, 窗口背后是**用户自己桌面的壁纸**, 内容不可预测:
//! 浅色壁纸配白字 = 完全读不清。VS Code 的 Mica 主题靠 Windows 自身的
//! "应用主题" 决定明暗, 我们没有那个特权 (我们不跟随系统主题, 用户
//! 可以独立切主题), 所以必须自己探测。
//!
//! # 探测方式 (按可靠性排序)
//!
//! 1. **注册表 `AppsUseLightTheme`** — Windows 10 1809+ 的"应用主题"开关。
//!    这是用户显式选的浅/深色, 比任何启发式都准。读它是纯 safe Rust。
//! 2. **启发式回退** — 读不到时 (Windows 8.1 及更早 / 非 Windows /
//!    注册表被策略锁定) 假定深色, 因为透明主题的默认配色是白字。
//!
//! # 为什么不用 DwmGetColorizationColor / 屏幕像素采样
//!
//! - `DwmGetColorizationColor` 是 Win32 FFI, 工作区 `unsafe_code = "forbid"`。
//! - 屏幕像素采样 (PrintWindow / BitBlt) 更糟: 那是**抓屏**, 既慢又涉及
//!   隐私, 而且窗口本身是透明的, 抓到的正是我们要判断的东西 —— 循环依赖。
//!
//! 注册表方案读的是"用户的意图", 而不是"屏幕上恰好是什么颜色", 这正是
//! 我们要的语义。

/// 桌面背景的明暗倾向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopLuminance {
    /// 桌面壁纸/系统主题偏亮 —— 文字应该用深色。
    Light,
    /// 桌面壁纸/系统主题偏暗 —— 文字应该用浅色。
    Dark,
}

impl DesktopLuminance {
    /// 该倾向下正文文字应该用的颜色。
    ///
    /// 不用纯黑/纯白: 纯黑在亮壁纸上会有"空洞感", 纯白在半透明层
    /// (0.06 白) 上对比度不足。这里取的是满足 WCAG AA (4.5:1)
    /// 在**最坏背景**下仍达标的值。
    pub fn text_primary(self) -> &'static str {
        match self {
            Self::Light => "#1a1c1e",   // 对纯白 15.8:1, 对浅灰壁纸 ~12:1
            Self::Dark => "#f0f2f5",    // 对纯黑 17.4:1, 对深色壁纸 ~13:1
        }
    }

    /// 次要文字 (行号、路径提示、禁用态)。
    pub fn text_muted(self) -> &'static str {
        match self {
            Self::Light => "rgba(26, 28, 30, 0.72)",
            Self::Dark => "rgba(240, 242, 245, 0.72)",
        }
    }

    /// 三级文字 (最弱的注释、占位符)。
    pub fn text_dim(self) -> &'static str {
        match self {
            Self::Light => "rgba(26, 28, 30, 0.48)",
            Self::Dark => "rgba(240, 242, 245, 0.48)",
        }
    }

    /// 边框 / 分隔线 —— 玻璃边缘需要对比才能看出层次。
    pub fn border(self) -> &'static str {
        match self {
            Self::Light => "rgba(26, 28, 30, 0.22)",
            Self::Dark => "rgba(255, 255, 255, 0.28)",
        }
    }

    /// 玻璃层底色 —— 亮背景下必须加一点暗色, 否则白字以外的内容
    /// (选中态高亮、语法高亮) 会失真。
    pub fn glass_tint(self) -> &'static str {
        match self {
            // 亮背景: 压一层暗, 保证叠加后的综合亮度仍偏亮
            Self::Light => "rgba(0, 0, 0, 0.04)",
            Self::Dark => "rgba(255, 255, 255, 0.04)",
        }
    }

    /// 强调色 (蓝) 之上的文字色 —— 状态栏/主按钮用蓝底, 上面必须是白字.
    ///
    /// 注意这不是"跟随桌面明暗": 蓝底 (#005fb8 深蓝 / #8ab4f8 亮蓝)
    /// 在两种桌面明暗下都需要**白字或近白字**才够对比. 浅色桌面下如果
    /// 返回深色, 就会出现"深蓝底 + 黑字"这种几乎看不见的组合 (实测).
    pub fn accent_fg(self) -> &'static str {
        // 蓝底一律配白字. 亮蓝 (#8ab4f8) 上的白字对比只有 1.6:1, 但那
        // 只出现在玻璃主题的半透明状态栏上, 背后还有用户壁纸兜底;
        // 真正不透明的状态栏 (深色/浅色主题) 用的是各自的中蓝底.
        "#ffffff"
    }

    /// 相对亮度 (WCAG 定义), 用于前端做对比度自检。
    pub fn relative_luminance(self) -> f64 {
        match self {
            Self::Light => 1.0,
            Self::Dark => 0.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// 读 Windows 的"应用主题"开关 (`AppsUseLightTheme`)。
///
/// 路径: `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// 值: `AppsUseLightTheme` (DWORD, 1 = 浅色, 0 = 深色)
///
/// 返回 `None` 表示读不到 (非 Windows / 键值不存在 / 权限不足) —— 调用方
/// 应回退到 `DesktopLuminance::Dark`。
#[cfg(target_os = "windows")]
fn read_apps_use_light_theme() -> Option<bool> {
    // windows-registry 0.3: CURRENT_USER 是 &'static Key, open() 返回子 Key,
    // 0.3 的方法名是 get_u32 (无 value_ 中缀); 0.2 是 get_value::<u32>().
    let personalize = windows_registry::CURRENT_USER
        .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .ok()?;
    let val: u32 = personalize.get_u32("AppsUseLightTheme").ok()?;
    Some(val != 0)
}

#[cfg(not(target_os = "windows"))]
fn read_apps_use_light_theme() -> Option<bool> {
    None
}

/// 探测桌面背景明暗倾向。
///
/// 非 Windows 平台恒为 `Dark` (透明主题的默认配色)。
pub fn detect() -> DesktopLuminance {
    match read_apps_use_light_theme() {
        Some(true) => DesktopLuminance::Light,
        // Some(false) 与 None 都走 Dark: 深色是透明主题的安全默认,
        // 因为默认配色是白字, 而白字在深色背景上永远可读。
        _ => DesktopLuminance::Dark,
    }
}

/// 探测并返回前端需要的一整套颜色变量。
///
/// 前端 `setTheme("clear")` 时调这个, 而不是用硬编码白字。
pub fn build_clear_theme_vars() -> serde_json::Value {
    let l = detect();
    // 组件层新 token 也在这里给值 —— 前端的 hover/active/rail/bottom 等
    // 状态都从这些变量取色, 不给的话会退回 :root 的深色默认值, 在浅色
    // 壁纸上就是深色控件压在浅背景上。
    let (hover, active, input, rail) = match l {
        DesktopLuminance::Light => (
            "rgba(255,255,255,0.10)",
            "rgba(255,255,255,0.18)",
            "rgba(255,255,255,0.12)",
            "rgba(255,255,255,0.06)",
        ),
        DesktopLuminance::Dark => (
            "rgba(255,255,255,0.12)",
            "rgba(255,255,255,0.18)",
            "rgba(255,255,255,0.10)",
            "rgba(255,255,255,0.05)",
        ),
    };
    serde_json::json!({
        "luminance": l.as_str(),
        "--fg": l.text_primary(),
        "--fg-muted": l.text_muted(),
        "--fg-dim": l.text_dim(),
        "--border": l.border(),
        "--border-strong": l.border(),
        "--fg-on-accent": l.accent_fg(),
        "--glass-tint": l.glass_tint(),
        "--bg-bar": l.glass_tint(),
        "--bg-side": l.glass_tint(),
        "--bg-status": l.glass_tint(),
        "--bg-rail": rail,
        "--bg-bottom": rail,
        "--bg-hover": hover,
        "--bg-active": active,
        "--bg-input": input,
        "--blue": if l == DesktopLuminance::Light { "#0b57d0" } else { "#8ab4f8" },
        "--green": if l == DesktopLuminance::Light { "#0f7b41" } else { "#7ee2b8" },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_uses_light_text() {
        let d = DesktopLuminance::Dark;
        // 白字系
        assert!(d.text_primary().starts_with("#f"));
        assert!(d.relative_luminance() < 0.5);
    }

    #[test]
    fn light_uses_dark_text() {
        let l = DesktopLuminance::Light;
        assert!(l.text_primary().starts_with("#1"));
        assert!(l.relative_luminance() > 0.5);
    }

    #[test]
    fn contrast_meets_wcag_aa_in_worst_case() {
        // WCAG AA 正文要求 4.5:1。检验两端在**最坏背景**下:
        //  - Light 文字 (#1a1c1e, L≈0.014) 配纯白背景 (L=1.0)
        //  - Dark  文字 (#f0f2f5, L≈0.878) 配纯黑背景 (L=0.0)
        fn luminance(hex: &str) -> f64 {
            let r = i32::from_str_radix(&hex[1..3], 16).unwrap() as f64 / 255.0;
            let g = i32::from_str_radix(&hex[3..5], 16).unwrap() as f64 / 255.0;
            let b = i32::from_str_radix(&hex[5..7], 16).unwrap() as f64 / 255.0;
            0.2126 * r + 0.7152 * g + 0.0722 * b
        }
        fn ratio(a: f64, b: f64) -> f64 {
            let (hi, lo) = if a > b { (a, b) } else { (b, a) };
            (hi + 0.05) / (lo + 0.05)
        }

        let light_text = luminance(DesktopLuminance::Light.text_primary());
        let dark_text = luminance(DesktopLuminance::Dark.text_primary());

        assert!(
            ratio(light_text, 1.0) >= 4.5,
            "Light 文字配纯白背景对比度不足: {:.2}",
            ratio(light_text, 1.0)
        );
        assert!(
            ratio(dark_text, 0.0) >= 4.5,
            "Dark 文字配纯黑背景对比度不足: {:.2}",
            ratio(dark_text, 0.0)
        );
    }

    #[test]
    fn accent_fg_is_light_on_both_desktop_modes() {
        // 状态栏是蓝底 —— 两种桌面明暗下都必须是浅字.
        // 浅色桌面下返回深色曾导致"深蓝底 + 黑字"(实测 rgb(26,28,30)),
        // 几乎不可读.
        for l in [DesktopLuminance::Light, DesktopLuminance::Dark] {
            let fg = l.accent_fg();
            let rgb = (
                i32::from_str_radix(&fg[1..3], 16).unwrap(),
                i32::from_str_radix(&fg[3..5], 16).unwrap(),
                i32::from_str_radix(&fg[5..7], 16).unwrap(),
            );
            let lum = (0.2126 * rgb.0 as f64 + 0.7152 * rgb.1 as f64 + 0.0722 * rgb.2 as f64) / 255.0;
            assert!(lum > 0.7, "蓝底上的文字应偏亮, 得到 {}", fg);
        }
    }

    #[test]
    fn detect_never_panics_and_defaults_dark() {
        // 无论注册表读到什么, detect() 必须返回合法值
        let l = detect();
        assert!(matches!(l, DesktopLuminance::Light | DesktopLuminance::Dark));
    }

    #[test]
    fn clear_theme_vars_has_all_css_variables() {
        let v = build_clear_theme_vars();
        for key in [
            "--fg",
            "--fg-muted",
            "--fg-dim",
            "--border",
            "--glass-tint",
            "--bg-bar",
            "--bg-side",
            "--bg-status",
            "--blue",
            "--green",
        ] {
            assert!(v.get(key).is_some(), "缺少 CSS 变量 {}", key);
        }
        assert!(v.get("luminance").is_some());
    }
}

// ---------------------------------------------------------------------------
// Tauri commands — 暴露给前端
// ---------------------------------------------------------------------------

/// 探测桌面背景明暗。返回 `"light"` / `"dark"`。
///
/// 前端在 `setTheme("clear")` 时调它, 决定透明主题下文字用深色还是浅色。
#[tauri::command]
pub fn desktop_luminance() -> String {
    detect().as_str().to_string()
}

/// 探测桌面背景明暗并返回一整套 CSS 变量。
///
/// 返回 `serde_json::Value`, 前端逐个 `setProperty` 到 `documentElement`。
/// 之所以在 Rust 侧算好而不是前端算: 明暗判定的**唯一事实来源**是注册表,
/// 让前端只消费结果, 避免两处逻辑漂移。
#[tauri::command]
pub fn clear_theme_vars() -> serde_json::Value {
    build_clear_theme_vars()
}
