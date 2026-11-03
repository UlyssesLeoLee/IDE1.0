//! 窗口 / 任务栏图标: 取 `packaging/ide-fast.ico` 里最合适的 PNG 帧解码成 RGBA (图标本身编进可执行文件)。

const ICO: &[u8] = include_bytes!("../packaging/ide-fast.ico");
/// 窗口图标要的边长 (64 兼顾高分屏标题栏和任务栏)
const WANT: u32 = 64;
/// 单帧 PNG 的上限 (防畸形图标目录)
const MAX_FRAME: usize = 1 << 20;

/// 在 ICO 目录里找边长最接近 `want` 的 PNG 帧, 返回它的字节
fn pick_png(ico: &[u8], want: u32) -> Option<&[u8]> {
    if ico.len() < 6 || ico[0..4] != [0, 0, 1, 0] { return None; }
    let n = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let mut best: Option<(u32, &[u8])> = None;
    for i in 0..n.min(32) {
        let e = ico.get(6 + 16 * i..22 + 16 * i)?;
        let side = if e[0] == 0 { 256 } else { e[0] as u32 };
        let size = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
        let Some(data) = off.checked_add(size).and_then(|end| ico.get(off..end)) else { continue };
        if size > MAX_FRAME || !data.starts_with(b"\x89PNG") { continue; }
        let d = side.abs_diff(want);
        if best.is_none_or(|(bd, _)| d < bd) { best = Some((d, data)); }
    }
    best.map(|b| b.1)
}

fn decode(png_bytes: &[u8]) -> Option<eframe::egui::IconData> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut rd = dec.read_info().ok()?;
    let mut buf = vec![0; rd.output_buffer_size()];
    let info = rd.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    if w == 0 || h == 0 || w > 512 || h > 512 { return None; }
    let px = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (rgba.len() == (w * h * 4) as usize).then_some(eframe::egui::IconData { rgba, width: w, height: h })
}

pub fn window_icon() -> Option<eframe::egui::IconData> {
    decode(pick_png(ICO, WANT)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_icon_decodes_at_the_wanted_size() {
        let i = window_icon().expect("图标能解码");
        assert_eq!((i.width, i.height), (64, 64));
        assert_eq!(i.rgba.len(), 64 * 64 * 4);
    }

    #[test]
    fn broken_icon_directories_are_rejected() {
        assert!(pick_png(&[], 64).is_none());
        assert!(pick_png(&[0, 0, 1, 0, 1, 0, 16, 16, 0, 0, 1, 0, 32, 0, 255, 255, 255, 127, 0, 0, 0, 0], 64).is_none());
        assert!(decode(b"nope").is_none());
    }
}
