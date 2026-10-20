//! 透明模式的「背后真实颜色」采样 (见 `editcore::theme_luminance` 的模块文档).
//!
//! 流程 (每次约 50~80 ms, 前端每秒调一次):
//!   1. `set_content_protected(true)`  → 本窗口不参与屏幕捕获 (WDA_EXCLUDEFROMCAPTURE),
//!      截图里窗口所在位置是它背后的壁纸 / 其它窗口, 不是我们自己画的字;
//!   2. 等一帧 DWM 合成, 截窗口所在的显示器;
//!   3. 无论成功与否都 `set_content_protected(false)` 恢复;
//!   4. 取窗口矩形内的网格采样点, 求平均亮度 (sRGB 加权, 0..1)。
//!
//! 用户屏幕上看到的内容不受影响 (WDA_EXCLUDEFROMCAPTURE 只影响截图, 不影响显示)。

use std::time::Duration;

/// 窗口背后的平均亮度 0..1。任何一步失败返回 Err, 调用方退回注册表探测。
pub fn sample_backdrop_luma(window: &tauri::WebviewWindow) -> Result<f64, String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    if size.width == 0 || size.height == 0 {
        return Err("窗口尺寸为 0 (最小化?)".into());
    }
    if window.is_minimized().unwrap_or(false) {
        return Err("窗口已最小化".into());
    }
    let cx = pos.x + (size.width / 2) as i32;
    let cy = pos.y + (size.height / 2) as i32;
    let mon = xcap::Monitor::from_point(cx, cy).map_err(|e| e.to_string())?;
    let (mx, my) = (mon.x().map_err(|e| e.to_string())?, mon.y().map_err(|e| e.to_string())?);

    window.set_content_protected(true).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(45));
    let shot = mon.capture_image().map_err(|e| e.to_string());
    let _ = window.set_content_protected(false);       // 先恢复, 再处理截图结果
    let img = shot?;

    let (iw, ih) = (img.width() as i64, img.height() as i64);
    // 窗口矩形换算成截图坐标并夹在截图范围内
    let x0 = ((pos.x - mx) as i64).clamp(0, iw - 1);
    let y0 = ((pos.y - my) as i64).clamp(0, ih - 1);
    let x1 = ((pos.x - mx) as i64 + size.width as i64).clamp(x0 + 1, iw);
    let y1 = ((pos.y - my) as i64 + size.height as i64).clamp(y0 + 1, ih);

    const GX: i64 = 40;
    const GY: i64 = 30;
    let mut sum = 0.0f64;
    let mut n = 0u32;
    for gy in 0..GY {
        for gx in 0..GX {
            let x = (x0 + (x1 - x0) * (2 * gx + 1) / (2 * GX)).clamp(0, iw - 1) as u32;
            let y = (y0 + (y1 - y0) * (2 * gy + 1) / (2 * GY)).clamp(0, ih - 1) as u32;
            let p = img.get_pixel(x, y).0;
            sum += (0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64) / 255.0;
            n += 1;
        }
    }
    if n == 0 {
        return Err("没有采样点".into());
    }
    Ok(sum / n as f64)
}
