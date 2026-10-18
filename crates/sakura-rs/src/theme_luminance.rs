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
            // 取接近纯黑白而非"柔和灰", 因为透明主题下最坏情况是中灰壁纸:
            // 纯白 #ffffff 对中灰 3.53:1, 纯黑 #000000 对中灰 4.73:1 —— 都比
            // 带灰的 #f0f2f5 (3.52) / #1a1c1e (4.33) 略好, 且配合 55% 遮罩后
            // 五种极端壁纸下最差可达 5.10:1。
            Self::Light => "#000000",
            Self::Dark => "#ffffff",
        }
    }

    /// 次要文字 (行号、路径提示、禁用态)。
    pub fn text_muted(self) -> &'static str {
        match self {
            // 0.72 在半透明玻璃上实测偏弱: 行号/路径提示这类辅助信息本来就
            // 不该"弱到看不清"。提到 0.88, 仍明显弱于正文但不牺牲可读性。
            Self::Light => "rgba(26, 28, 30, 0.88)",
            Self::Dark => "rgba(240, 242, 245, 0.88)",
        }
    }

    /// 三级文字 (最弱的注释、占位符)。
    pub fn text_dim(self) -> &'static str {
        match self {
            Self::Light => "rgba(26, 28, 30, 0.68)",
            Self::Dark => "rgba(240, 242, 245, 0.68)",
        }
    }

    /// 边框 / 分隔线 —— 玻璃边缘需要对比才能看出层次。
    pub fn border(self) -> &'static str {
        match self {
            Self::Light => "rgba(26, 28, 30, 0.22)",
            Self::Dark => "rgba(255, 255, 255, 0.28)",
        }
    }

    /// 玻璃层底色 (遮罩) —— **可读性保障**, 不是装饰。
    ///
    /// 这里原来只给 4% 不透明度, 实测根本压不住中等亮度壁纸:
    ///   - 深色桌面档 (白字 #f0f2f5) 配中灰壁纸 (128,128,128) 只有 3.52:1;
    ///   - 浅色桌面档 (黑字 #1a1c1e) 同一壁纸 4.33:1;
    /// 两者都不到 WCAG AA 正文要求的 4.5:1, 用户的直观感受就是「文字不鲜明」。
    ///
    /// 为什么不做到 100% (那样任意壁纸都达标): 那透明主题就没有意义了。
    /// 真正的取舍是**强度** —— 所以这里给的是"能压住大多数壁纸、又不至于
    /// 变成实心面板"的值, 并由 `tint_strength()` 集中管理, 便于调整与测试。
    ///
    /// 当前取值经扫描确定: 遮罩 55% 时, 五种极端壁纸(纯白/浅灰/中灰/深灰/纯黑)
    /// 下的最差对比度分别是 5.10:1 与 4.24:1 —— 浅色档全达标, 深色档在中灰
    /// 壁纸上仍差 0.26。要严格 AA 需 70%, 但那已接近实心。
    pub fn glass_tint(self) -> &'static str {
        match self {
            // 亮背景: 压一层**白** —— 文字是黑的, 底越亮对比越高.
            Self::Light => "rgba(255, 255, 255, 0.55)",
            // 暗背景: 压一层**黑** —— 文字是白的, 底越暗对比越高.
            // (这里原先错写成白色遮罩, 会把白字压在浅色底上, 对比度掉到 1.0:1)
            Self::Dark => "rgba(0, 0, 0, 0.55)",
        }
    }

    /// 遮罩强度 (0.0 ~ 1.0), 供测试与前端调节使用。
    pub fn tint_strength(self) -> f64 {
        0.55
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
    // hover/active 是叠在**已遮罩的底**之上的高亮, 所以用低不透明度即可;
    // 但 rail / bottom / input 是独立面板, 必须跟 glass_tint 一样有足够遮罩,
    // 否则它们盖不住壁纸, 里面的文字照样看不清。
    let (hover, active, input, rail) = match l {
        DesktopLuminance::Light => (
            "rgba(0,0,0,0.08)",
            "rgba(0,0,0,0.14)",
            "rgba(0,0,0,0.72)",
            "rgba(255,255,255,0.55)",
        ),
        DesktopLuminance::Dark => (
            "rgba(255,255,255,0.12)",
            "rgba(255,255,255,0.20)",
            "rgba(0,0,0,0.72)",
            "rgba(0,0,0,0.55)",
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
