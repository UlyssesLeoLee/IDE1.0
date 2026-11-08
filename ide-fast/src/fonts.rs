//! 字体系统: 为主题挑选美观、免费商用的优质字体 (支持中日英三语与等宽编程)。
//! 支持 Sakura 样式的字体设置 (字号、粗体、斜体、行间距、字符间距、全角半角等宽网格对齐、自定义 CJK fallback)。
//! 在后台线程并行预加载字体字节, 毫秒级极速冷启动。

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// 精选免费商用与高质量系统字体项
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecommendedFont {
    pub file: &'static str,
    pub name_zh: &'static str,
    pub name_ja: &'static str,
    pub name_en: &'static str,
    pub license: &'static str,
    pub is_free_commercial: bool,
}

pub const RECOMMENDED_FONTS: &[RecommendedFont] = &[
    RecommendedFont {
        file: "CascadiaCode.ttf",
        name_zh: "Cascadia Code (现代编程等宽, 合字/连字)",
        name_ja: "Cascadia Code (合字対応プログラミング等幅)",
        name_en: "Cascadia Code (Modern Coding Monospace with ligatures)",
        license: "SIL OFL (免费商用)",
        is_free_commercial: true,
    },
    RecommendedFont {
        file: "CascadiaMono.ttf",
        name_zh: "Cascadia Mono (现代编程等宽, 标准无连字)",
        name_ja: "Cascadia Mono (標準プログラミング等幅)",
        name_en: "Cascadia Mono (Standard Coding Monospace)",
        license: "SIL OFL (免费商用)",
        is_free_commercial: true,
    },
    RecommendedFont {
        file: "BIZ-UDGothicR.ttc",
        name_zh: "BIZ UD Gothic (森泽高辨识度通用设计 CJK & 英文)",
        name_ja: "BIZ UDゴシック (モリサワ UD等幅・日中英高可読)",
        name_en: "BIZ UD Gothic (Morisawa Universal Design CJK)",
        license: "SIL OFL (免费商用)",
        is_free_commercial: true,
    },
    RecommendedFont {
        file: "NotoSansSC-VF.ttf",
        name_zh: "Noto Sans SC (思源黑体 简中优美高颜值无衬线)",
        name_ja: "Noto Sans SC (思源黒体・簡体字現代サンセリフ)",
        name_en: "Noto Sans SC (Source Han Sans Simplified Chinese)",
        license: "SIL OFL (免费商用)",
        is_free_commercial: true,
    },
    RecommendedFont {
        file: "NotoSansJP-VF.ttf",
        name_zh: "Noto Sans JP (思源黑体 日文优美典雅无衬线)",
        name_ja: "Noto Sans JP (思源黒体・日本語美文字サンセリフ)",
        name_en: "Noto Sans JP (Source Han Sans Japanese)",
        license: "SIL OFL (免费商用)",
        is_free_commercial: true,
    },
    RecommendedFont {
        file: "consola.ttf",
        name_zh: "Consolas (经典清爽代码等宽)",
        name_ja: "Consolas (定番クリーン等幅)",
        name_en: "Consolas (Classic Clean Monospace)",
        license: "Windows System",
        is_free_commercial: false,
    },
    RecommendedFont {
        file: "segoeui.ttf",
        name_zh: "Segoe UI (现代几何无衬线, 适合 Comic 风格)",
        name_ja: "Segoe UI (モダンサンセリフ, Comic向け)",
        name_en: "Segoe UI (Modern Clean Sans, ideal for Comic)",
        license: "Windows System",
        is_free_commercial: false,
    },
    RecommendedFont {
        file: "PixelFontMulti-12.ttf",
        name_zh: "PixelFont 12 (16位街机点阵像素, 适合 Retro16 主题)",
        name_ja: "PixelFont 12 (レトロ16-Bitドット文字, Retro16専用)",
        name_en: "PixelFont 12 (Retro 16-Bit Arcade Pixel, for Retro16)",
        license: "SIL OFL (免费商用)",
        is_free_commercial: true,
    },
];

/// 过滤并校正明显影响阅读的紧缩装饰/艺术/标题/哥特/书法字体 (如 Old English, Bodoni Poster, Chiller, Castellar 等)
pub fn is_illegible_display_font(file_or_name: &str) -> bool {
    let s = file_or_name.to_ascii_uppercase();
    s.contains("BOD_PSTC")
        || s.contains("BODONI")
        || s.contains("PLAYBILL")
        || s.contains("CHILLER")
        || s.contains("CURLZ")
        || s.contains("OLDENG")
        || s.contains("FRAKTUR")
        || s.contains("BLACKLETTER")
        || s.contains("PAPYRUS")
        || s.contains("SHOWG")
        || s.contains("SNAP")
        || s.contains("ENGR")
        || s.contains("CASTELAR")
        || s.contains("EDWARDIAN")
        || s.contains("KUNSTLER")
        || s.contains("HARRINGTON")
        || s.contains("JOKERMAN")
        || s.contains("MATURA")
        || s.contains("MAGNETO")
        || s.contains("PRISTINA")
        || s.contains("RAVIE")
        || s.contains("VIVALDI")
        || s.contains("VLADIMIR")
        || s.contains("FREESCPT")
        || s.contains("BRUSH")
        || s.contains("COMIC")
        || s.contains("CLOISTER")
        || s.contains("GOUDY")
        || s.contains("HARLOW")
        || s.contains("COLONNA")
        || s.contains("ONYX")
        || s.contains("PARCHMENT")
        || s.contains("STENCIL")
        || s.contains("INFORMAL")
        || s.contains("CHANCERY")
}

/// 确保字体适用于正文与代码阅读，若传入非法或艺术扭曲字体则安全回退至主题默认等宽编程字体 (空字符串保持为空，代表主题默认)
pub fn sanitize_font_family(font: &str, theme_id: &str) -> String {
    let clean = font.trim();
    if clean.is_empty() {
        String::new()
    } else if is_illegible_display_font(clean) {
        theme_default_font(theme_id).unwrap_or("CascadiaCode.ttf").to_string()
    } else {
        clean.to_string()
    }
}

/// 为各主题匹配最贴合风格的免费商用美观字体 (除 16 位像素主题外均挑选美观且高辨识度字体)
pub fn theme_default_font(theme_id: &str) -> Option<&'static str> {
    match theme_id {
        "theme-retro16" => Some("PixelFontMulti-12.ttf"), // 16位复古像素风主题专属
        "theme-crystal" => Some("CascadiaCode.ttf"),      // 水晶现代几何通透感
        "theme-comic" => Some("CascadiaCode.ttf"),        // 美漫 3渲2 粗线条高辨识度
        "theme-clear" => Some("CascadiaMono.ttf"),        // 玻璃极简清透
        "theme-dark" => Some("CascadiaCode.ttf"),         // 暗色专业代码
        "theme-light" => Some("CascadiaMono.ttf"),        // 纸质明朗舒适
        "theme-high-contrast" => Some("CascadiaCode.ttf"), // 高对比度锐利大字怀
        _ => None,
    }
}

/// 所有支持的 CJK Fallback 候选清单 (按字符集与语言环境排序)
const CJK_CANDIDATES_ALL: &[(&str, u32)] = &[
    ("NotoSansSC-VF.ttf", 0),  // 思源黑体 简中 (SIL OFL 免费商用)
    ("NotoSansJP-VF.ttf", 0),  // 思源黑体 日文 (SIL OFL 免费商用)
    ("BIZ-UDGothicR.ttc", 0),  // BIZ UD Gothic (SIL OFL 免费商用)
    ("msyh.ttc", 0),          // 微软雅黑: 简体 + 常用汉字 + 假名
    ("YuGothM.ttc", 0),       // 游ゴシック: 日文假名 / 汉字
    ("meiryo.ttc", 0),        // Meiryo
    ("msgothic.ttc", 0),      // MS Gothic: 老系统上的日文等宽
    ("malgun.ttf", 0),        // 韩文
];

/// 编辑区用的字体族
pub fn editor_family() -> FontFamily {
    FontFamily::Name("editor".into())
}

/// 编辑区用的粗体字体族 (Sakura 样式粗体配置)
pub fn editor_bold_family() -> FontFamily {
    FontFamily::Name("editor-bold".into())
}

fn fonts_dir() -> std::path::PathBuf {
    std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| "C:/Windows/Fonts".into())
}

/// 系统字体目录里能直接用的字体文件名 (.ttf / .otf, 排序去重并过滤掉不适宜编程阅读的艺术花体字); 目录读不到 = 空
pub fn system_fonts() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(fonts_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| is_font_file(n) && !is_illegible_display_font(n))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|n| n.to_lowercase());
    v.dedup();
    v
}

pub fn is_font_file(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".ttf") || l.ends_with(".otf") || l.ends_with(".ttc")
}

/// 查找指定字体对应的伴生粗体文件
fn find_bold_file(dir: &std::path::Path, file: &str) -> Option<String> {
    let lower = file.to_lowercase();
    if lower == "cascadiacode.ttf" || lower == "cascadiamono.ttf" {
        for candidate in ["CascadiaCode-Bold.otf", "CascadiaMono-Bold.otf", "CascadiaCode.ttf"] {
            if dir.join(candidate).is_file() {
                return Some(candidate.to_string());
            }
        }
    } else if lower == "consola.ttf" {
        if dir.join("consolab.ttf").is_file() {
            return Some("consolab.ttf".to_string());
        }
    } else if lower.starts_with("biz-udgothic") {
        if dir.join("BIZ-UDGothicB.ttc").is_file() {
            return Some("BIZ-UDGothicB.ttc".to_string());
        }
    } else if lower.starts_with("msyh") {
        if dir.join("msyhbd.ttc").is_file() {
            return Some("msyhbd.ttc".to_string());
        }
    } else if lower.starts_with("segoeui") {
        if dir.join("segoeuib.ttf").is_file() {
            return Some("segoeuib.ttf".to_string());
        }
    }
    None
}

/// 已从磁盘读出的字体字节: (CJK fallback 的 (文件名, TTC 序号, 字节), 编辑器常规字体字节, 编辑器粗体字节)
pub struct Loaded {
    pub fallbacks: Vec<(String, u32, Vec<u8>)>,
    pub editor: Option<Vec<u8>>,
    pub editor_bold: Option<Vec<u8>>,
}

/// 根据语言环境获取最优 CJK 候选顺序
fn get_cjk_candidates(custom_cjk: &str, lang: &str) -> Vec<(&'static str, u32)> {
    let mut list = Vec::new();
    // 用户自定义优先
    if !custom_cjk.is_empty() {
        if let Some(&cand) = CJK_CANDIDATES_ALL.iter().find(|(f, _)| *f == custom_cjk) {
            list.push(cand);
        }
    }
    match lang {
        "ja" => {
            // 日文环境: 优先日文字形
            for &c in &[
                ("BIZ-UDGothicR.ttc", 0),
                ("NotoSansJP-VF.ttf", 0),
                ("YuGothM.ttc", 0),
                ("meiryo.ttc", 0),
                ("msgothic.ttc", 0),
                ("msyh.ttc", 0),
                ("malgun.ttf", 0),
            ] {
                if !list.iter().any(|(f, _)| *f == c.0) {
                    list.push(c);
                }
            }
        }
        _ => {
            // 中文及其他环境: 优先思源/微黑
            for &c in &[
                ("NotoSansSC-VF.ttf", 0),
                ("msyh.ttc", 0),
                ("BIZ-UDGothicR.ttc", 0),
                ("YuGothM.ttc", 0),
                ("NotoSansJP-VF.ttf", 0),
                ("msgothic.ttc", 0),
                ("malgun.ttf", 0),
            ] {
                if !list.iter().any(|(f, _)| *f == c.0) {
                    list.push(c);
                }
            }
        }
    }
    list
}

/// 并行读磁盘上的字体文件 (主字体与 CJK fallback 并行读取, 消除冷启动 I/O 串行阻塞)
pub fn load(editor_file: &str, cjk_file: &str, lang: &str) -> Loaded {
    load_ext(editor_file, cjk_file, lang, true)
}

/// 扩展加载接口: 支持指定是否需要粗体变体 (启动时若未开启粗体则跳过粗体文件扫描与读取)
pub fn load_ext(editor_file: &str, cjk_file: &str, lang: &str, need_bold: bool) -> Loaded {
    let dir = fonts_dir();
    let safe_editor = std::path::Path::new(editor_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let safe_cjk = std::path::Path::new(cjk_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // 线程 1: 并行读取主字体与其粗体变体 (不需要粗体时跳过磁盘搜索与读取)
    let dir_clone = dir.clone();
    let ed_handle = std::thread::spawn(move || {
        let (mut ed, mut ed_bold) = if !safe_editor.is_empty() && is_font_file(&safe_editor) {
            let b1 = std::fs::read(dir_clone.join(&safe_editor)).ok();
            let b2 = if need_bold {
                let bold_file = find_bold_file(&dir_clone, &safe_editor);
                bold_file.and_then(|f| std::fs::read(dir_clone.join(f)).ok())
            } else {
                None
            };
            (b1, b2)
        } else {
            (None, None)
        };
        // 若未指定或未读到主字体，优先尝试加载系统自带的现代编程等宽字体保底
        if ed.is_none() {
            for fallback in ["CascadiaCode.ttf", "CascadiaMono.ttf", "consola.ttf"] {
                if let Ok(b) = std::fs::read(dir_clone.join(fallback)) {
                    if need_bold {
                        let bold_file = find_bold_file(&dir_clone, fallback);
                        ed_bold = bold_file.and_then(|f| std::fs::read(dir_clone.join(f)).ok());
                    }
                    ed = Some(b);
                    break;
                }
            }
        }
        (ed, ed_bold)
    });

    // 线程 0 (当前线程): 并行读取 CJK Fallback 字体
    // 首次冷启动加载 1 个核心字体 (如 NotoSansSC 或 msyh), 已覆盖中日英全量基础字形, 避免 30MB+ 冗余 I/O
    let cjk_cands = get_cjk_candidates(&safe_cjk, lang);
    let mut fallbacks = vec![];
    for (file, index) in cjk_cands {
        let Ok(bytes) = std::fs::read(dir.join(file)) else {
            continue;
        };
        fallbacks.push((file.to_string(), index, bytes));
        if !fallbacks.is_empty() {
            break; // 1 个核心 CJK 字体已足够覆盖中日英文字形, 控制首帧前 I/O 与图集构建开销
        }
    }

    let (editor, editor_bold) = ed_handle.join().unwrap_or((None, None));
    Loaded {
        fallbacks,
        editor,
        editor_bold,
    }
}

/// 兼容接口: 仅指定编辑器字体文件
pub fn load_simple(editor_file: &str) -> Loaded {
    load(editor_file, "", "zh")
}

/// 后台并行预读字体; 在主线程初始化窗口和 OpenGL 期间运行
pub fn prefetch(
    editor_file: &str,
    cjk_file: &str,
    lang: &str,
) -> Option<std::thread::JoinHandle<Loaded>> {
    prefetch_ext(editor_file, cjk_file, lang, true)
}

/// 扩展预读接口: 支持按需忽略粗体
pub fn prefetch_ext(
    editor_file: &str,
    cjk_file: &str,
    lang: &str,
    need_bold: bool,
) -> Option<std::thread::JoinHandle<Loaded>> {
    let ef = editor_file.to_string();
    let cf = cjk_file.to_string();
    let lg = lang.to_string();
    std::thread::Builder::new()
        .name("font-prefetch".into())
        .spawn(move || load_ext(&ef, &cf, &lg, need_bold))
        .ok()
}

/// 同步安装字体到 egui 上下文中
pub fn install(ctx: &egui::Context, editor_file: &str, cjk_file: &str, lang: &str) {
    install_loaded(ctx, load(editor_file, cjk_file, lang));
}

/// 将加载的字体字节安装到 egui 上下文中 (注册 editor 与 editor-bold 字体族)
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
    let mut editor_bold: Vec<String> = vec![];

    if let Some(bytes) = loaded.editor {
        defs.font_data
            .insert("editor-font".into(), Arc::new(FontData::from_owned(bytes)));
        editor.push("editor-font".into());
    }

    if let Some(bytes) = loaded.editor_bold {
        defs.font_data
            .insert("editor-bold-font".into(), Arc::new(FontData::from_owned(bytes)));
        editor_bold.push("editor-bold-font".into());
    } else if let Some(first) = editor.first().cloned() {
        editor_bold.push(first);
    }

    let mono_fallbacks = defs
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    editor.extend(mono_fallbacks.clone());
    editor_bold.extend(mono_fallbacks);

    defs.families.insert(editor_family(), editor);
    defs.families.insert(editor_bold_family(), editor_bold);
    ctx.set_fonts(defs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_file_extension_filtering() {
        assert!(is_font_file("Consola.TTF"));
        assert!(is_font_file("CascadiaCode.ttf"));
        assert!(is_font_file("BIZ-UDGothicR.ttc"));
        assert!(is_font_file("a.otf"));
        assert!(!is_font_file("readme.txt"));
        assert!(!is_font_file("binary.bin"));
    }

    #[test]
    fn theme_default_font_matching() {
        assert_eq!(theme_default_font("theme-retro16"), Some("PixelFontMulti-12.ttf"));
        assert_eq!(theme_default_font("theme-crystal"), Some("CascadiaCode.ttf"));
        assert_eq!(theme_default_font("theme-clear"), Some("CascadiaMono.ttf"));
        assert_eq!(theme_default_font("theme-dark"), Some("CascadiaCode.ttf"));
        assert_eq!(theme_default_font("theme-light"), Some("CascadiaMono.ttf"));
        assert_eq!(theme_default_font("theme-comic"), Some("CascadiaCode.ttf"));
        assert_eq!(theme_default_font("unknown"), None);
    }

    #[test]
    fn cjk_candidate_ordering_by_language() {
        let ja = get_cjk_candidates("", "ja");
        assert!(ja.iter().any(|(f, _)| *f == "BIZ-UDGothicR.ttc"));
        assert!(ja[0].0 == "BIZ-UDGothicR.ttc" || ja[0].0 == "NotoSansJP-VF.ttf");

        let zh = get_cjk_candidates("", "zh");
        assert!(zh.iter().any(|(f, _)| *f == "msyh.ttc" || *f == "NotoSansSC-VF.ttf"));

        let custom = get_cjk_candidates("BIZ-UDGothicR.ttc", "zh");
        assert_eq!(custom[0].0, "BIZ-UDGothicR.ttc");
    }

    #[test]
    fn install_registers_both_editor_and_editor_bold_families() {
        let ctx = egui::Context::default();
        install(&ctx, "", "", "zh");
        install(&ctx, r"..\..\evil.ttf", "", "zh");
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let id_normal = egui::FontId::new(14.0, editor_family());
            let id_bold = egui::FontId::new(14.0, editor_bold_family());
            assert!(ui.ctx().fonts_mut(|f| f.row_height(&id_normal)) > 0.0);
            assert!(ui.ctx().fonts_mut(|f| f.row_height(&id_bold)) > 0.0);
        });
        out.textures_delta.clear();
    }
}
