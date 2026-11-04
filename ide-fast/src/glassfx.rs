//! 玻璃主题的窗口背后材质 (Windows): 依次试几种 DWM 材质, 第一个调用成功的生效; 全失败 = 只剩 alpha 透明。
//! `IDE_FAST_BACKDROP=blur|acrylic|mica|tabbed|none` 可强制某一种 (排查 / 截图用)。

use raw_window_handle::HasWindowHandle;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    Blur = 1,
    Acrylic = 2,
    Mica = 3,
    Tabbed = 4,
}

/// 当前生效的材质 (0 = 没有)
static APPLIED: AtomicU8 = AtomicU8::new(0);

pub fn from_name(s: &str) -> Option<Backdrop> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "blur" => Backdrop::Blur,
        "acrylic" => Backdrop::Acrylic,
        "mica" => Backdrop::Mica,
        "tabbed" => Backdrop::Tabbed,
        _ => return None,
    })
}

/// 尝试顺序: 环境变量强制 > 默认顺序 (见 `DEFAULT_ORDER`)
/// Blur (`ACCENT_ENABLE_BLURBEHIND`) 在 Windows 11 10.0.26300 上背后是纯黑, 所以不进默认顺序; Acrylic (Win11 22523+ 走 `DWMWA_SYSTEMBACKDROP_TYPE`, 之前走 SWCA 的 ACRYLICBLURBEHIND) 在这台机器上截图验证有真模糊。
/// Mica 只取桌面壁纸色, 不模糊背后的窗口。全部失败 = 只剩 alpha 透明。
pub const DEFAULT_ORDER: [Backdrop; 1] = [Backdrop::Acrylic];

pub fn order() -> Vec<Backdrop> {
    match std::env::var("IDE_FAST_BACKDROP") {
        Ok(v) if v.eq_ignore_ascii_case("none") => vec![],
        Ok(v) => from_name(&v)
            .map(|b| vec![b])
            .unwrap_or_else(|| DEFAULT_ORDER.to_vec()),
        Err(_) => DEFAULT_ORDER.to_vec(),
    }
}

#[cfg(windows)]
fn try_one(w: &impl HasWindowHandle, b: Backdrop) -> bool {
    use window_vibrancy as v;
    match b {
        Backdrop::Blur => v::apply_blur(w, Some((0, 0, 0, 1))),
        Backdrop::Acrylic => v::apply_acrylic(w, Some((0, 0, 0, 1))),
        Backdrop::Mica => v::apply_mica(w, None),
        Backdrop::Tabbed => v::apply_tabbed(w, None),
    }
    .is_ok()
}

#[cfg(windows)]
pub fn clear(w: &impl HasWindowHandle) {
    use window_vibrancy as v;
    let prev = APPLIED.swap(0, Ordering::Relaxed);
    let _ = match prev {
        1 => v::clear_blur(w),
        2 => v::clear_acrylic(w),
        3 => v::clear_mica(w),
        4 => v::clear_tabbed(w),
        _ => Ok(()),
    };
}

/// 开材质; 返回生效的那一种 (None = 全部失败, 窗口保持纯 alpha 透明)
#[cfg(windows)]
pub fn apply(w: &impl HasWindowHandle) -> Option<Backdrop> {
    clear(w);
    for b in order() {
        if try_one(w, b) {
            APPLIED.store(b as u8, Ordering::Relaxed);
            return Some(b);
        }
    }
    None
}

#[cfg(not(windows))]
pub fn apply(_w: &impl HasWindowHandle) -> Option<Backdrop> {
    None
}
#[cfg(not(windows))]
pub fn clear(_w: &impl HasWindowHandle) {
    let _ = APPLIED.swap(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_backdrops() {
        assert_eq!(from_name("Acrylic"), Some(Backdrop::Acrylic));
        assert_eq!(from_name(" mica "), Some(Backdrop::Mica));
        assert_eq!(from_name("x"), None);
    }
}
