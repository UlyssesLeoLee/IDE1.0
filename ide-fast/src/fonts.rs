//! 字体系统: 为主题挑选美观、免费商用的优质字体 (支持中日英三语与等宽编程)。
//! 支持 Sakura 样式的字体设置 (字号、粗体、斜体、行间距、字符间距、全角半角等宽网格对齐、自定义 CJK fallback)。
//! 在后台线程并行预加载字体字节, 毫秒级极速冷启动。

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// 嵌入式 16-Bit 街机点阵全字形字体 (包含 31,753 字符，覆盖 ASCII、全量 CJK 汉字与假名及符号，免除外部字体文件依赖)
pub const EMBEDDED_PIXEL_FONT: &[u8] =
    include_bytes!("../../crates/ide-shell-web/src/PixelFontMulti-12.ttf");

/// 判断是否为 16-Bit 像素字体
pub fn is_pixel_font(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("pixel")
}

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
        || s.contains("WINGDINGS")
        || s.contains("WEBDINGS")
        || s.contains("SYMBOL")
        || s.contains("MARLETT")
        || s.contains("OUTLOOK")
        || s.contains("MDL2")
        || s.contains("HOLOMDL2")
        || s.contains("MTEXTRA")
}

/// 确保字体适用于正文与代码阅读，若传入非法或艺术扭曲字体则安全回退至主题默认等宽编程字体 (空字符串保持为空，代表主题默认)
pub fn sanitize_font_family(font: &str, theme_id: &str) -> String {
    let clean = font.trim();
    if theme_id == "theme-retro16" {
        return "PixelFontMulti-12.ttf".to_string();
    }
    if clean.is_empty() || is_pixel_font(clean) {
        String::new()
    } else if is_illegible_display_font(clean) {
        theme_default_font(theme_id)
            .unwrap_or("CascadiaCode.ttf")
            .to_string()
    } else {
        clean.to_string()
    }
}

/// 计算各主题在当前运行环境下的最终有效主字体文件名
/// - Retro 16-Bit (theme-retro16) 核心视觉语言强制使用 PixelFont 12 点阵像素字体
/// - 其他主题绝不使用像素字体；若未指定字体、仍残留像素字体或传入不可读艺术花体，则回退到默认等宽编程字体
pub fn effective_font_family(font_family: &str, theme_id: &str) -> String {
    let clean = font_family.trim();
    if theme_id == "theme-retro16" {
        return "PixelFontMulti-12.ttf".to_string();
    }
    if clean.is_empty() || is_pixel_font(clean) || is_illegible_display_font(clean) {
        theme_default_font(theme_id)
            .unwrap_or("CascadiaCode.ttf")
            .to_string()
    } else {
        clean.to_string()
    }
}

/// 为各主题匹配最贴合风格的免费商用美观字体 (除 16 位像素主题外均挑选美观且高辨识度字体)
pub fn theme_default_font(theme_id: &str) -> Option<&'static str> {
    match theme_id {
        "theme-retro16" => Some("PixelFontMulti-12.ttf"), // 16位复古像素风主题专属
        "theme-crystal" => Some("CascadiaCode.ttf"),      // 统一透明水晶现代几何通透感
        "theme-comic" => Some("CascadiaCode.ttf"),        // 美漫 3渲2 粗线条高辨识度
        "theme-clear" => Some("CascadiaCode.ttf"),        // 与 theme-crystal 统一使用同一默认字体
        "theme-dark" => Some("CascadiaCode.ttf"),         // 暗色专业代码
        "theme-light" => Some("CascadiaMono.ttf"),        // 纸质明朗舒适
        "theme-high-contrast" => Some("CascadiaCode.ttf"), // 高对比度锐利大字怀
        _ => None,
    }
}

/// 所有支持的 CJK Fallback 候选清单 (按字符集与语言环境排序)
const CJK_CANDIDATES_ALL: &[(&str, u32)] = &[
    ("NotoSansSC-VF.ttf", 0), // 思源黑体 简中 (SIL OFL 免费商用)
    ("NotoSansJP-VF.ttf", 0), // 思源黑体 日文 (SIL OFL 免费商用)
    ("BIZ-UDGothicR.ttc", 0), // BIZ UD Gothic (SIL OFL 免费商用)
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

/// 用户级安装的专属字体目录 (Windows: %LOCALAPPDATA%\Microsoft\Windows\Fonts)
pub fn user_fonts_dir() -> Option<std::path::PathBuf> {
    if let Some(l) = std::env::var_os("LOCALAPPDATA") {
        let p = std::path::PathBuf::from(l)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts");
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(h) = std::env::var_os("HOME") {
        let p1 = std::path::PathBuf::from(&h).join(".local/share/fonts");
        if p1.is_dir() {
            return Some(p1);
        }
        let p2 = std::path::PathBuf::from(&h).join("Library/Fonts");
        if p2.is_dir() {
            return Some(p2);
        }
    }
    None
}

/// 用户在操作系统中自主安装的字体文件列表 (过滤艺术花体，字母排序去重)
pub fn user_fonts() -> Vec<String> {
    let Some(dir) = user_fonts_dir() else {
        return Vec::new();
    };
    let mut v: Vec<String> = std::fs::read_dir(dir)
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

pub fn fonts_dir() -> std::path::PathBuf {
    std::env::var_os("WINDIR")
        .map(|w| std::path::PathBuf::from(w).join("Fonts"))
        .unwrap_or_else(|| "C:/Windows/Fonts".into())
}

/// 获取全部有效字体搜索目录 (OnceLock 进程级缓存, 避免启动阶段重复读取环境变量与目录探测)
fn font_search_dirs_cached() -> &'static [std::path::PathBuf] {
    static DIRS: std::sync::OnceLock<Vec<std::path::PathBuf>> = std::sync::OnceLock::new();
    DIRS.get_or_init(|| {
        let mut dirs = Vec::with_capacity(2);
        if let Some(ud) = user_fonts_dir() {
            dirs.push(ud);
        }
        dirs.push(fonts_dir());
        dirs
    })
    .as_slice()
}

/// 获取全部有效字体搜索目录 (优先用户字体目录，次之系统字体目录)
pub fn font_search_dirs() -> Vec<std::path::PathBuf> {
    font_search_dirs_cached().to_vec()
}

/// 系统与用户字体目录里能直接用的字体文件名 (.ttf / .otf / .ttc);
/// 优先排布用户自主安装的新字体，随后是系统公共字体，保证用户新装字体优先展示
pub fn system_fonts() -> Vec<String> {
    let mut user = user_fonts();
    let user_set: std::collections::HashSet<String> =
        user.iter().map(|s| s.to_lowercase()).collect();
    let mut sys: Vec<String> = std::fs::read_dir(fonts_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| {
                    is_font_file(n)
                        && !is_illegible_display_font(n)
                        && !user_set.contains(&n.to_lowercase())
                })
                .collect()
        })
        .unwrap_or_default();
    sys.sort_by_key(|n| n.to_lowercase());
    sys.dedup();
    user.extend(sys);
    user
}

pub fn is_font_file(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".ttf") || l.ends_with(".otf") || l.ends_with(".ttc") || is_pixel_font(name)
}

#[inline]
fn has_font_ext(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 4 {
        return false;
    }
    let tail = &name[bytes.len() - 4..];
    tail.eq_ignore_ascii_case(".ttf")
        || tail.eq_ignore_ascii_case(".otf")
        || tail.eq_ignore_ascii_case(".ttc")
}

/// 从所有字体候选目录中按优先级读取指定字体字节 (优先读取用户安装字体)
pub fn read_font_bytes(filename: &str) -> Option<Vec<u8>> {
    let try_with_ttf = !has_font_ext(filename);
    for dir in font_search_dirs_cached() {
        if let Ok(b) = std::fs::read(dir.join(filename)) {
            return Some(b);
        }
        if try_with_ttf {
            if let Ok(b) = std::fs::read(dir.join(format!("{filename}.ttf"))) {
                return Some(b);
            }
        }
    }
    if let Some(p) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(filename)))
    {
        if let Ok(b) = std::fs::read(p) {
            return Some(b);
        }
    }
    None
}

/// 查找指定字体对应的伴生粗体文件 (直接路径 O(1) 探测, 不遍历 C:\Windows\Fonts)
fn find_bold_file(dirs: &[std::path::PathBuf], file: &str) -> Option<(std::path::PathBuf, String)> {
    let lower = file.to_lowercase();
    let known: &[&str] = if lower == "cascadiacode.ttf" || lower == "cascadiamono.ttf" {
        &[
            "CascadiaCode-Bold.otf",
            "CascadiaMono-Bold.otf",
            "CascadiaCode.ttf",
        ]
    } else if lower == "consola.ttf" {
        &["consolab.ttf"]
    } else if lower.starts_with("biz-udgothic") {
        &["BIZ-UDGothicB.ttc"]
    } else if lower.starts_with("msyh") {
        &["msyhbd.ttc"]
    } else if lower.starts_with("segoeui") {
        &["segoeuib.ttf"]
    } else {
        &[]
    };
    for dir in dirs {
        for cand in known {
            if dir.join(cand).is_file() {
                return Some((dir.clone(), (*cand).to_string()));
            }
        }
    }
    if known.is_empty() {
        let path = std::path::Path::new(file);
        if let (Some(stem), Some(ext)) = (
            path.file_stem().and_then(|s| s.to_str()),
            path.extension().and_then(|e| e.to_str()),
        ) {
            let dyn_cands = [
                format!("{stem}b.{ext}"),
                format!("{stem}-Bold.{ext}"),
                format!("{stem}Bold.{ext}"),
                format!("{stem}_Bold.{ext}"),
            ];
            for dir in dirs {
                for cand in &dyn_cands {
                    if dir.join(cand).is_file() {
                        return Some((dir.clone(), cand.clone()));
                    }
                }
            }
        }
    }
    None
}

/// 已从磁盘读出的字体字节: (CJK fallback 的 (文件名, TTC 序号, 字节), 编辑器常规字体字节, 编辑器粗体字节, 是否为 Retro16 像素字体)
pub struct Loaded {
    pub fallbacks: Vec<(String, u32, Vec<u8>)>,
    pub editor: Option<Vec<u8>>,
    pub editor_bold: Option<Vec<u8>>,
    pub is_retro16: bool,
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
    let safe_editor = std::path::Path::new(editor_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let safe_cjk = std::path::Path::new(cjk_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let is_retro = is_pixel_font(&safe_editor);

    // 线程 1: 并行读取主字体与其粗体变体 (优先用户自装字体，不需要粗体时跳过磁盘搜索与读取)
    let ed_handle = std::thread::spawn(move || {
        let search_dirs = font_search_dirs_cached();
        let (mut ed, mut ed_bold) = if !safe_editor.is_empty()
            && is_font_file(&safe_editor)
            && !is_illegible_display_font(&safe_editor)
        {
            let b1 = if is_pixel_font(&safe_editor) {
                if safe_editor.eq_ignore_ascii_case("PixelFontMulti-12.ttf")
                    || safe_editor.eq_ignore_ascii_case("PixelFont 12")
                {
                    // 内嵌的 16-Bit 街机点阵全字形字体, O(1) 零 I/O 直读
                    Some(EMBEDDED_PIXEL_FONT.to_vec())
                } else {
                    read_font_bytes(&safe_editor)
                        .or_else(|| read_font_bytes(&format!("{safe_editor}.ttf")))
                        .or_else(|| Some(EMBEDDED_PIXEL_FONT.to_vec()))
                }
            } else {
                read_font_bytes(&safe_editor)
            };
            let b2 = if need_bold && !is_pixel_font(&safe_editor) {
                find_bold_file(search_dirs, &safe_editor)
                    .and_then(|(d, f)| std::fs::read(d.join(f)).ok())
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
                if let Some(b) = read_font_bytes(fallback) {
                    if need_bold {
                        let bold_file = find_bold_file(search_dirs, fallback);
                        ed_bold = bold_file.and_then(|(d, f)| std::fs::read(d.join(f)).ok());
                    }
                    ed = Some(b);
                    break;
                }
            }
        }
        if need_bold && ed_bold.is_none() && !is_retro {
            for bold_fb in [
                "consolab.ttf",
                "segoeuib.ttf",
                "msyhbd.ttc",
                "CascadiaCode-Bold.otf",
                "DejaVuSansMono-Bold.ttf",
                "LiberationMono-Bold.ttf",
            ] {
                if let Some(b) = read_font_bytes(bold_fb) {
                    ed_bold = Some(b);
                    break;
                }
            }
        }
        (ed, ed_bold)
    });

    // 线程 0 (当前线程): 并行读取 CJK Fallback 字体 (优先在用户自装字体中查找)
    // 首次冷启动加载 1 个核心字体 (如 NotoSansSC 或 msyh), 已覆盖中日英全量基础字形, 避免 30MB+ 冗余 I/O
    let cjk_cands = get_cjk_candidates(&safe_cjk, lang);
    let mut fallbacks = vec![];
    for (file, index) in cjk_cands {
        let Some(bytes) = read_font_bytes(file) else {
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
        is_retro16: is_retro,
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
        defs.font_data.insert(
            "editor-bold-font".into(),
            Arc::new(FontData::from_owned(bytes)),
        );
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

    if loaded.is_retro16 {
        defs.families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "editor-font".into());
        defs.families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "editor-font".into());
    }

    defs.families.insert(editor_family(), editor);
    defs.families.insert(editor_bold_family(), editor_bold);
    ctx.set_fonts(defs);

    if loaded.is_retro16 {
        ctx.global_style_mut(|s| {
            s.text_styles.insert(
                egui::TextStyle::Small,
                egui::FontId::new(12.0, FontFamily::Proportional),
            );
            s.text_styles.insert(
                egui::TextStyle::Body,
                egui::FontId::new(12.0, FontFamily::Proportional),
            );
            s.text_styles.insert(
                egui::TextStyle::Button,
                egui::FontId::new(12.0, FontFamily::Proportional),
            );
            s.text_styles.insert(
                egui::TextStyle::Monospace,
                egui::FontId::new(12.0, FontFamily::Monospace),
            );
            s.text_styles.insert(
                egui::TextStyle::Heading,
                egui::FontId::new(12.0, FontFamily::Proportional),
            );
        });
    } else {
        ctx.global_style_mut(|s| {
            s.text_styles = egui::Style::default().text_styles;
        });
    }
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
        assert_eq!(
            theme_default_font("theme-retro16"),
            Some("PixelFontMulti-12.ttf")
        );
        assert_eq!(
            theme_default_font("theme-crystal"),
            Some("CascadiaCode.ttf")
        );
        assert_eq!(theme_default_font("theme-clear"), Some("CascadiaCode.ttf"));
        assert_eq!(theme_default_font("theme-dark"), Some("CascadiaCode.ttf"));
        assert_eq!(theme_default_font("theme-light"), Some("CascadiaMono.ttf"));
        assert_eq!(theme_default_font("theme-comic"), Some("CascadiaCode.ttf"));
        assert_eq!(theme_default_font("unknown"), None);
    }

    #[test]
    fn effective_font_family_retro16_invariant() {
        assert_eq!(
            effective_font_family("", "theme-retro16"),
            "PixelFontMulti-12.ttf"
        );
        assert_eq!(
            effective_font_family("Consolas", "theme-retro16"),
            "PixelFontMulti-12.ttf"
        );
        assert_eq!(
            effective_font_family("BOD_PSTC.TTF", "theme-retro16"),
            "PixelFontMulti-12.ttf"
        );
        // 非 16-Bit Retro 主题一律不使用像素字体，回退默认字体
        assert_eq!(
            effective_font_family("PixelFont 12", "theme-comic"),
            "CascadiaCode.ttf"
        );
        assert_eq!(
            effective_font_family("pixelfont", "theme-dark"),
            "CascadiaCode.ttf"
        );
        assert_eq!(
            sanitize_font_family("PixelFontMulti-12.ttf", "theme-comic"),
            ""
        );

        assert_eq!(effective_font_family("", "theme-comic"), "CascadiaCode.ttf");
        assert_eq!(effective_font_family("Consolas", "theme-comic"), "Consolas");
        assert_eq!(
            effective_font_family("BOD_PSTC.TTF", "theme-comic"),
            "CascadiaCode.ttf"
        );
    }

    #[test]
    fn pixel_font_detection_and_file_validity() {
        assert!(is_pixel_font("PixelFont 12"));
        assert!(is_pixel_font("pixelfont 12"));
        assert!(is_pixel_font("PixelFontMulti-12.ttf"));
        assert!(is_pixel_font("retro_pixel"));
        assert!(!is_pixel_font("Consolas.ttf"));

        assert!(is_font_file("PixelFont 12"));
        assert!(is_font_file("PixelFontMulti-12.ttf"));
    }

    #[test]
    fn retro16_global_text_styles_and_families_configured() {
        let ctx = egui::Context::default();
        let loaded = load_ext("PixelFontMulti-12.ttf", "", "zh", false);
        assert!(loaded.is_retro16);
        install_loaded(&ctx, loaded);

        let s = ctx.global_style();
        assert_eq!(
            s.text_styles.get(&egui::TextStyle::Small).unwrap().size,
            12.0
        );
        assert_eq!(
            s.text_styles.get(&egui::TextStyle::Body).unwrap().size,
            12.0
        );
        assert_eq!(
            s.text_styles.get(&egui::TextStyle::Button).unwrap().size,
            12.0
        );
        assert_eq!(
            s.text_styles.get(&egui::TextStyle::Monospace).unwrap().size,
            12.0
        );
        assert_eq!(
            s.text_styles.get(&egui::TextStyle::Heading).unwrap().size,
            12.0
        );

        // 切回普通主题 (例如 CascadiaCode)
        let normal_loaded = Loaded {
            fallbacks: vec![],
            editor: Some(vec![]),
            editor_bold: None,
            is_retro16: false,
        };
        install_loaded(&ctx, normal_loaded);
        let s2 = ctx.global_style();
        assert_eq!(
            s2.text_styles.get(&egui::TextStyle::Body).unwrap().size,
            egui::Style::default()
                .text_styles
                .get(&egui::TextStyle::Body)
                .unwrap()
                .size
        );
    }

    #[test]
    fn embedded_pixel_font_loads_and_flags_retro16() {
        let loaded = load_ext("PixelFontMulti-12.ttf", "", "zh", false);
        assert!(
            loaded.editor.is_some(),
            "Embedded PixelFont bytes must load successfully"
        );
        assert!(loaded.is_retro16, "is_retro16 must be set for PixelFont");
    }

    #[test]
    fn cjk_candidate_ordering_by_language() {
        let ja = get_cjk_candidates("", "ja");
        assert!(ja.iter().any(|(f, _)| *f == "BIZ-UDGothicR.ttc"));
        assert!(ja[0].0 == "BIZ-UDGothicR.ttc" || ja[0].0 == "NotoSansJP-VF.ttf");

        let zh = get_cjk_candidates("", "zh");
        assert!(zh
            .iter()
            .any(|(f, _)| *f == "msyh.ttc" || *f == "NotoSansSC-VF.ttf"));

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

    #[test]
    fn test_user_fonts_discovery_and_priority() {
        let user = user_fonts();
        let all = system_fonts();
        // 如果系统上存在用户安装字体，确保它们排列在 system_fonts() 的头部优先展示
        if !user.is_empty() {
            assert_eq!(&all[..user.len()], &user[..]);
        }
    }
}
