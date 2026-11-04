//! 字体: egui 自带字体没有中日文字形, 把系统里的 CJK 字体接成 fallback。
//! 只读磁盘上现成的系统字体文件 (不往程序里打包几十 MB 的字体); 找不到就退回自带字体 (中日文显示成方框, 但不影响编辑)。
//! 编辑器用单独的字体族 `editor`: 默认等于等宽字体; 首选项里选了字体文件就把它排在最前面 (缺的字形仍由后面的 fallback 补)。

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// 候选 (文件名, TTC 里的字体序号), 按顺序都加进去当 fallback: 前一个缺的字形由后一个补
const CANDIDATES: &[(&str, u32)] = &[
    ("msyh.ttc", 0),     // 微软雅黑: 简体 + 常用汉字
    ("YuGothM.ttc", 0),  // 游ゴシック: 日文假名 / 汉字
    ("msgothic.ttc", 0), // MS Gothic: 老系统上的日文等宽
    ("malgun.ttf", 0),   // 韩文
];

/// 编辑区用的字体族
pub fn editor_family() -> FontFamily {
    FontFamily::Name("editor".into())
}

fn fonts_dir() -> std::path::PathBuf {
    std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| "C:/Windows/Fonts".into())
}

/// 系统字体目录里能直接用的字体文件名 (.ttf / .otf, 排序去重); 目录读不到 = 空
pub fn system_fonts() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(fonts_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| is_font_file(n))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|n| n.to_lowercase());
    v.dedup();
    v
}

fn is_font_file(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".ttf") || l.ends_with(".otf")
}

/// 已从磁盘读出的字体字节: (CJK fallback 的 (文件名, TTC 序号, 字节), 编辑器字体字节)
pub struct Loaded {
    fallbacks: Vec<(&'static str, u32, Vec<u8>)>,
    editor: Option<Vec<u8>>,
}

/// 读磁盘上的字体文件 (几十 MB, 冷缓存时是启动里最慢的一段 I/O): 和窗口 / OpenGL 初始化并行, 在后台线程里跑
pub fn load(editor_file: &str) -> Loaded {
    let dir = fonts_dir();
    let mut fallbacks = vec![];
    for (file, index) in CANDIDATES {
        let Ok(bytes) = std::fs::read(dir.join(file)) else {
            continue;
        };
        fallbacks.push((*file, *index, bytes));
        if fallbacks.len() >= 2 {
            break; // 两个就够覆盖中日文; 再多只会拖慢启动
        }
    }
    // 文件名只取最后一段, 免得设置文件里写个 ..\ 去读别处的文件
    let safe = std::path::Path::new(editor_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let editor = if !safe.is_empty() && is_font_file(&safe) {
        std::fs::read(dir.join(&safe)).ok()
    } else {
        None
    };
    Loaded { fallbacks, editor }
}

/// 后台读字体; `install_loaded` 之前 join。线程起不来 (几乎不可能) 就返回 None, 调用方退回同步读取
pub fn prefetch(editor_file: &str) -> Option<std::thread::JoinHandle<Loaded>> {
    let f = editor_file.to_string();
    std::thread::Builder::new()
        .name("font-prefetch".into())
        .spawn(move || load(&f))
        .ok()
}

/// `editor_file` = 编辑器字体的文件名 (空 = 默认等宽); 读不到文件就当没选
pub fn install(ctx: &egui::Context, editor_file: &str) {
    install_loaded(ctx, load(editor_file));
}

pub fn install_loaded(ctx: &egui::Context, loaded: Loaded) {
    let mut defs = FontDefinitions::default();
    for (file, index, bytes) in loaded.fallbacks {
        let name = format!("sys-{file}");
        let mut data = FontData::from_owned(bytes);
        data.index = index;
        defs.font_data.insert(name.clone(), Arc::new(data));
        for fam in [FontFamily::Monospace, FontFamily::Proportional] {
            defs.families.entry(fam).or_default().push(name.clone());
        }
    }
    let mut editor: Vec<String> = vec![];
    if let Some(bytes) = loaded.editor {
        defs.font_data
            .insert("editor-font".into(), Arc::new(FontData::from_owned(bytes)));
        editor.push("editor-font".into());
    }
    editor.extend(
        defs.families
            .get(&FontFamily::Monospace)
            .cloned()
            .unwrap_or_default(),
    );
    defs.families.insert(editor_family(), editor);
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ttf_and_otf_are_listed() {
        assert!(is_font_file("Consola.TTF"));
        assert!(is_font_file("a.otf"));
        assert!(!is_font_file("msyh.ttc"));
        assert!(!is_font_file("readme.txt"));
    }

    #[test]
    fn install_registers_the_editor_family_even_without_a_font_file() {
        let ctx = egui::Context::default();
        install(&ctx, "");
        install(&ctx, r"..\..\evil.ttf"); // 不存在 / 路径被削成文件名: 不 panic, 家族照样在
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let id = egui::FontId::new(14.0, editor_family());
            assert!(ui.ctx().fonts_mut(|f| f.row_height(&id)) > 0.0);
        });
        out.textures_delta.clear(); // 测试里没有渲染器去接纹理增量, 不清会在 drop 时 panic
    }
}
