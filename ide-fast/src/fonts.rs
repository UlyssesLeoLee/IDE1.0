//! 字体: egui 自带字体没有中日文字形, 把系统里的 CJK 字体接成 fallback。
//! 只读磁盘上现成的系统字体文件 (不往程序里打包几十 MB 的字体); 找不到就退回自带字体 (中日文显示成方框, 但不影响编辑)。

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// 候选 (文件名, TTC 里的字体序号), 按顺序都加进去当 fallback: 前一个缺的字形由后一个补
const CANDIDATES: &[(&str, u32)] = &[
    ("msyh.ttc", 0),     // 微软雅黑: 简体 + 常用汉字
    ("YuGothM.ttc", 0),  // 游ゴシック: 日文假名 / 汉字
    ("msgothic.ttc", 0), // MS Gothic: 老系统上的日文等宽
    ("malgun.ttf", 0),   // 韩文
];

pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let dir = std::env::var_os("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts")).unwrap_or_else(|| "C:/Windows/Fonts".into());
    let mut added = 0;
    for (file, index) in CANDIDATES {
        let Ok(bytes) = std::fs::read(dir.join(file)) else { continue };
        let name = format!("sys-{file}");
        let mut data = FontData::from_owned(bytes);
        data.index = *index;
        defs.font_data.insert(name.clone(), Arc::new(data));
        for fam in [FontFamily::Monospace, FontFamily::Proportional] {
            defs.families.entry(fam).or_default().push(name.clone());
        }
        added += 1;
        if added >= 2 {
            break; // 两个就够覆盖中日文; 再多只会拖慢启动
        }
    }
    ctx.set_fonts(defs);
}
