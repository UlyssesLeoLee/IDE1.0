//! Sakura Editor 18 页共通与类型别设置及 INI 映射体系 (WBS-08).
//!
//! 包含完整 18 个页面配置分类：
//! 1. WindowConfig (窗口)
//! 2. TabBarConfig (标签栏)
//! 3. EditConfig (编辑/智能缩进/Tab宽/自动保存)
//! 4. FileConfig (文件/独占/历史/备份/编码默认)
//! 5. SearchConfig (检索/环视/历史/高亮)
//! 6. FormatConfig (格式书式/折行方式/禁则字符/避头尾)
//! 7. AssistConfig (支援功能/补全/候选)
//! 8. MacroConfig (宏/20个槽位/触发/参数)
//! 9. StatusBarConfig (状态栏/字段定制)
//! 10. PluginConfig (插件/扫描/开关)
//! 11. ToolbarConfig (工具栏/按钮布局)
//! 12. KeySearchConfig (关键字帮助)
//! 13. ExecCmdConfig (外部命令)
//! 14. ColorPaletteConfig (颜色配色/24色方案)
//! 15. CustomMenuConfig (自定义菜单/24个用户菜单槽位)
//! 16. MainMenuConfig (主菜单定制)
//! 17. FontConfig (字体/字号/行间距)
//! 18. PrintConfig (打印排版)
//!
//! 零依赖、纯 safe Rust.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

/// INI 解析错误类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniParseError(pub String);

impl fmt::Display for IniParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "INI parse error: {}", self.0)
    }
}

impl std::error::Error for IniParseError {}

// ============================================================================
// 1. 窗口配置 (WindowConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub center_window: bool,
    pub remember_window_size: bool,
    pub remember_window_pos: bool,
    pub always_on_top: bool,
    pub opacity: u8, // 0..=100
    pub minimize_to_tray: bool,
    pub multi_window_mode: bool,
    pub full_screen: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 768,
            pos_x: 100,
            pos_y: 100,
            center_window: false,
            remember_window_size: true,
            remember_window_pos: true,
            always_on_top: false,
            opacity: 100,
            minimize_to_tray: false,
            multi_window_mode: true,
            full_screen: false,
        }
    }
}

// ============================================================================
// 2. 标签栏配置 (TabBarConfig)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabPosition {
    Top,
    Bottom,
    Left,
    Right,
}

impl TabPosition {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "0" | "top" => Self::Top,
            "1" | "bottom" => Self::Bottom,
            "2" | "left" => Self::Left,
            "3" | "right" => Self::Right,
            _ => Self::Top,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabDoubleClickAction {
    None,
    Close,
    NewTab,
    Maximize,
}

impl TabDoubleClickAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Close => "Close",
            Self::NewTab => "NewTab",
            Self::Maximize => "Maximize",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "0" | "none" => Self::None,
            "1" | "close" => Self::Close,
            "2" | "newtab" | "new_tab" => Self::NewTab,
            "3" | "maximize" => Self::Maximize,
            _ => Self::Close,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabBarConfig {
    pub visible: bool,
    pub position: TabPosition,
    pub multi_row: bool,
    pub show_close_button: bool,
    pub double_click_action: TabDoubleClickAction,
    pub close_on_middle_click: bool,
    pub close_on_right_click: bool,
    pub wheel_tab_switch: bool,
    pub drag_drop_reorder: bool,
    pub distinguish_same_name: bool,
    pub tab_width_max: u32,
    pub tab_width_min: u32,
}

impl Default for TabBarConfig {
    fn default() -> Self {
        Self {
            visible: true,
            position: TabPosition::Top,
            multi_row: false,
            show_close_button: true,
            double_click_action: TabDoubleClickAction::Close,
            close_on_middle_click: true,
            close_on_right_click: false,
            wheel_tab_switch: true,
            drag_drop_reorder: true,
            distinguish_same_name: true,
            tab_width_max: 200,
            tab_width_min: 50,
        }
    }
}

// ============================================================================
// 3. 编辑配置 (EditConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditConfig {
    pub tab_width: u32,
    pub indent_width: u32,
    pub use_spaces_for_tab: bool,
    pub smart_indent: bool,
    pub auto_indent: bool,
    pub auto_save_enabled: bool,
    pub auto_save_interval_sec: u32,
    pub overwrite_mode: bool,
    pub drag_drop_edit: bool,
    pub copy_line_when_empty_selection: bool,
    pub scroll_smooth: bool,
    pub scroll_past_end_of_line: bool,
}

impl Default for EditConfig {
    fn default() -> Self {
        Self {
            tab_width: 4,
            indent_width: 4,
            use_spaces_for_tab: false,
            smart_indent: true,
            auto_indent: true,
            auto_save_enabled: false,
            auto_save_interval_sec: 300,
            overwrite_mode: false,
            drag_drop_edit: true,
            copy_line_when_empty_selection: true,
            scroll_smooth: true,
            scroll_past_end_of_line: false,
        }
    }
}

// ============================================================================
// 4. 文件配置 (FileConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileConfig {
    pub default_encoding: String,
    pub default_eol: String,
    pub file_lock_exclusive: bool,
    pub recent_files_max: usize,
    pub recent_folders_max: usize,
    pub auto_reload_changed: bool,
    pub alert_file_deleted: bool,
    pub backup_enabled: bool,
    pub backup_folder: String,
    pub backup_extension: String,
    pub backup_count_max: u32,
    pub restore_cursor_pos: bool,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            default_encoding: "UTF-8".to_string(),
            default_eol: "CRLF".to_string(),
            file_lock_exclusive: false,
            recent_files_max: 30,
            recent_folders_max: 20,
            auto_reload_changed: true,
            alert_file_deleted: true,
            backup_enabled: true,
            backup_folder: ".sakura-backup".to_string(),
            backup_extension: ".bak".to_string(),
            backup_count_max: 5,
            restore_cursor_pos: true,
        }
    }
}

// ============================================================================
// 5. 检索配置 (SearchConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchConfig {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex_enabled: bool,
    pub lookaround_enabled: bool,
    pub search_wrap: bool,
    pub search_backwards: bool,
    pub highlight_all_matches: bool,
    pub search_history_max: usize,
    pub replace_history_max: usize,
    pub auto_close_dialog: bool,
    pub use_migemo: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            regex_enabled: false,
            lookaround_enabled: true,
            search_wrap: true,
            search_backwards: false,
            highlight_all_matches: true,
            search_history_max: 30,
            replace_history_max: 30,
            auto_close_dialog: false,
            use_migemo: false,
        }
    }
}

// ============================================================================
// 6. 格式书式与避头尾禁则字符 (FormatConfig & KinsokuChars)
// ============================================================================

/// 默认行头禁则字符 (JIS X 4051 避头尾禁则标准 + 中文常用标点)
pub const DEFAULT_HEAD_FORBIDDEN: &str = "、。，．・：；？！ヽヾゝゞ々ー）〕］｝〉》」』】’”!?)]}:;,.‰℃℉′″¢";

/// 默认行尾禁则字符 (各类前置括弧与前置引号、前置货币符号)
pub const DEFAULT_TAIL_FORBIDDEN: &str = "（〔［｛〈《「『【‘“([{\\$#";

/// 避头尾禁则字符集
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KinsokuChars {
    head: BTreeSet<char>,
    tail: BTreeSet<char>,
}

impl Default for KinsokuChars {
    fn default() -> Self {
        Self {
            head: DEFAULT_HEAD_FORBIDDEN.chars().collect(),
            tail: DEFAULT_TAIL_FORBIDDEN.chars().collect(),
        }
    }
}

impl KinsokuChars {
    pub fn new() -> Self {
        Self::default()
    }

    /// 行头禁止字符判定
    pub fn is_head_forbidden(&self, c: char) -> bool {
        self.head.contains(&c)
    }

    /// 行尾禁止字符判定
    pub fn is_tail_forbidden(&self, c: char) -> bool {
        self.tail.contains(&c)
    }

    /// 增加行头禁止字符
    pub fn add_head_char(&mut self, c: char) {
        self.head.insert(c);
    }

    /// 移除行头禁止字符
    pub fn remove_head_char(&mut self, c: char) -> bool {
        self.head.remove(&c)
    }

    /// 增加行尾禁止字符
    pub fn add_tail_char(&mut self, c: char) {
        self.tail.insert(c);
    }

    /// 移除行尾禁止字符
    pub fn remove_tail_char(&mut self, c: char) -> bool {
        self.tail.remove(&c)
    }

    /// 获取行头禁止字符字符串
    pub fn head_chars(&self) -> String {
        self.head.iter().collect()
    }

    /// 获取行尾禁止字符字符串
    pub fn tail_chars(&self) -> String {
        self.tail.iter().collect()
    }

    /// 重设行头禁止字符
    pub fn set_head_chars(&mut self, s: &str) {
        self.head = s.chars().collect();
    }

    /// 重设行尾禁止字符
    pub fn set_tail_chars(&mut self, s: &str) {
        self.tail = s.chars().collect();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WrapMode {
    NoWrap,
    ByChar,
    ByWindow,
    ByWidth,
}

impl WrapMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoWrap => "NoWrap",
            Self::ByChar => "ByChar",
            Self::ByWindow => "ByWindow",
            Self::ByWidth => "ByWidth",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "0" | "nowrap" | "none" => Self::NoWrap,
            "1" | "bychar" | "char" => Self::ByChar,
            "2" | "bywindow" | "window" => Self::ByWindow,
            "3" | "bywidth" | "width" => Self::ByWidth,
            _ => Self::ByChar,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatConfig {
    pub wrap_mode: WrapMode,
    pub wrap_columns: u32,
    pub kinsoku_head_enabled: bool,
    pub kinsoku_tail_enabled: bool,
    pub kinsoku_push_enabled: bool,
    pub kinsoku_chars: KinsokuChars,
    pub show_line_numbers: bool,
    pub show_ruler: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            wrap_mode: WrapMode::ByChar,
            wrap_columns: 80,
            kinsoku_head_enabled: true,
            kinsoku_tail_enabled: true,
            kinsoku_push_enabled: true,
            kinsoku_chars: KinsokuChars::default(),
            show_line_numbers: true,
            show_ruler: true,
        }
    }
}

// ============================================================================
// 7. 支援功能/补全 (AssistConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistConfig {
    pub auto_complete_enabled: bool,
    pub min_char_trigger: u32,
    pub show_candidate_list: bool,
    pub dictionary_path: String,
    pub use_words_in_doc: bool,
    pub matching_paren_highlight: bool,
    pub auto_close_brackets: bool,
    pub candidate_max_count: u32,
}

impl Default for AssistConfig {
    fn default() -> Self {
        Self {
            auto_complete_enabled: true,
            min_char_trigger: 2,
            show_candidate_list: true,
            dictionary_path: String::new(),
            use_words_in_doc: true,
            matching_paren_highlight: true,
            auto_close_brackets: false,
            candidate_max_count: 10,
        }
    }
}

// ============================================================================
// 8. 宏配置/20槽位 (MacroConfig)
// ============================================================================

pub const MACRO_SLOT_COUNT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroTrigger {
    None,
    OnOpen,
    OnSave,
    BeforeClose,
    OnActivate,
}

impl MacroTrigger {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::OnOpen => "OnOpen",
            Self::OnSave => "OnSave",
            Self::BeforeClose => "BeforeClose",
            Self::OnActivate => "OnActivate",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "0" | "none" => Self::None,
            "1" | "onopen" | "open" => Self::OnOpen,
            "2" | "onsave" | "save" => Self::OnSave,
            "3" | "beforeclose" | "close" => Self::BeforeClose,
            "4" | "onactivate" | "activate" => Self::OnActivate,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroSlot {
    pub id: u8, // 1..=20
    pub name: String,
    pub path: String,
    pub reload_on_run: bool,
    pub trigger: MacroTrigger,
    pub params: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroConfig {
    pub slots: [MacroSlot; MACRO_SLOT_COUNT],
}

impl Default for MacroConfig {
    fn default() -> Self {
        let mut slots = Vec::with_capacity(MACRO_SLOT_COUNT);
        for i in 1..=(MACRO_SLOT_COUNT as u8) {
            slots.push(MacroSlot {
                id: i,
                name: String::new(),
                path: String::new(),
                reload_on_run: false,
                trigger: MacroTrigger::None,
                params: String::new(),
            });
        }
        Self {
            slots: slots
                .try_into()
                .unwrap_or_else(|_| panic!("size mismatch")),
        }
    }
}

// ============================================================================
// 9. 状态栏配置 (StatusBarConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusBarConfig {
    pub visible: bool,
    pub show_caret_pos: bool,
    pub show_char_code: bool,
    pub show_encoding: bool,
    pub show_eol: bool,
    pub show_modified: bool,
    pub show_ins_ovr: bool,
    pub show_file_size: bool,
    pub show_zoom: bool,
    pub show_line_count: bool,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self {
            visible: true,
            show_caret_pos: true,
            show_char_code: true,
            show_encoding: true,
            show_eol: true,
            show_modified: true,
            show_ins_ovr: true,
            show_file_size: true,
            show_zoom: true,
            show_line_count: true,
        }
    }
}

// ============================================================================
// 10. 插件配置 (PluginConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginConfig {
    pub auto_scan_plugins: bool,
    pub plugin_dir: String,
    pub enabled_plugins: Vec<String>,
    pub security_level: u8,
    pub allow_network: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            auto_scan_plugins: true,
            plugin_dir: "plugins".to_string(),
            enabled_plugins: Vec::new(),
            security_level: 0,
            allow_network: false,
        }
    }
}

// ============================================================================
// 11. 工具栏配置 (ToolbarConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolbarConfig {
    pub visible: bool,
    pub button_size: u32,
    pub flat_style: bool,
    pub show_tooltips: bool,
    pub buttons: Vec<String>,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            visible: true,
            button_size: 16,
            flat_style: true,
            show_tooltips: true,
            buttons: vec![
                "new".into(),
                "open".into(),
                "save".into(),
                "cut".into(),
                "copy".into(),
                "paste".into(),
                "undo".into(),
                "redo".into(),
                "find".into(),
                "grep".into(),
            ],
        }
    }
}

// ============================================================================
// 12. 关键字帮助配置 (KeySearchConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySearchConfig {
    pub enabled: bool,
    pub dictionary_lookup_url: String,
    pub hotkey: String,
    pub auto_search_selected: bool,
    pub open_in_external_browser: bool,
}

impl Default for KeySearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dictionary_lookup_url: "https://www.google.com/search?q=%s".to_string(),
            hotkey: "F1".to_string(),
            auto_search_selected: true,
            open_in_external_browser: true,
        }
    }
}

// ============================================================================
// 13. 外部命令配置 (ExecCmdConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCmdSlot {
    pub id: u32,
    pub name: String,
    pub cmd: String,
    pub work_dir: String,
    pub send_stdin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecCmdConfig {
    pub default_shell: String,
    pub capture_output: bool,
    pub run_in_background: bool,
    pub show_console_window: bool,
    pub commands: Vec<ExternalCmdSlot>,
}

impl Default for ExecCmdConfig {
    fn default() -> Self {
        Self {
            default_shell: "cmd.exe".to_string(),
            capture_output: true,
            run_in_background: false,
            show_console_window: false,
            commands: Vec::new(),
        }
    }
}

// ============================================================================
// 14. 颜色配色/24色方案 (ColorPaletteConfig)
// ============================================================================

pub const PALETTE_COLOR_COUNT: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaletteColor {
    pub name: String,
    pub foreground: String,
    pub background: String,
    pub bold: bool,
    pub underline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorPaletteConfig {
    pub scheme_name: String,
    pub colors: [PaletteColor; PALETTE_COLOR_COUNT],
}

impl Default for ColorPaletteConfig {
    fn default() -> Self {
        let colors = [
            PaletteColor { name: "Text".into(), foreground: "#000000".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Background".into(), foreground: "#000000".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Selection".into(), foreground: "#FFFFFF".into(), background: "#3399FF".into(), bold: false, underline: false },
            PaletteColor { name: "CurrentLine".into(), foreground: "#000000".into(), background: "#E8F2FE".into(), bold: false, underline: false },
            PaletteColor { name: "LineNumber".into(), foreground: "#808080".into(), background: "#F0F0F0".into(), bold: false, underline: false },
            PaletteColor { name: "Comment".into(), foreground: "#008000".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "String".into(), foreground: "#A31515".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Keyword".into(), foreground: "#0000FF".into(), background: "#FFFFFF".into(), bold: true, underline: false },
            PaletteColor { name: "Keyword2".into(), foreground: "#267F99".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Keyword3".into(), foreground: "#795E26".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Number".into(), foreground: "#098658".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Operator".into(), foreground: "#000000".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Function".into(), foreground: "#795E26".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Type".into(), foreground: "#267F99".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "Preprocessor".into(), foreground: "#AF00DB".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "URL".into(), foreground: "#0066CC".into(), background: "#FFFFFF".into(), bold: false, underline: true },
            PaletteColor { name: "MailAddress".into(), foreground: "#0066CC".into(), background: "#FFFFFF".into(), bold: false, underline: true },
            PaletteColor { name: "MatchingParen".into(), foreground: "#FF0000".into(), background: "#FFFF99".into(), bold: true, underline: false },
            PaletteColor { name: "Ruler".into(), foreground: "#C0C0C0".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "WrapMark".into(), foreground: "#999999".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "EolMark".into(), foreground: "#0099FF".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "TabMark".into(), foreground: "#C0C0C0".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "ZenkakuSpace".into(), foreground: "#CCCCCC".into(), background: "#FFFFFF".into(), bold: false, underline: false },
            PaletteColor { name: "HalfWidthSpace".into(), foreground: "#E0E0E0".into(), background: "#FFFFFF".into(), bold: false, underline: false },
        ];
        Self {
            scheme_name: "Sakura Standard".to_string(),
            colors,
        }
    }
}

// ============================================================================
// 15. 自定义菜单/24个用户菜单槽位 (CustomMenuConfig)
// ============================================================================

pub const CUSTOM_MENU_SLOT_COUNT: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomMenuItem {
    pub label: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomMenuSlot {
    pub id: u8, // 1..=24
    pub title: String,
    pub items: Vec<CustomMenuItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomMenuConfig {
    pub slots: [CustomMenuSlot; CUSTOM_MENU_SLOT_COUNT],
}

impl Default for CustomMenuConfig {
    fn default() -> Self {
        let mut slots = Vec::with_capacity(CUSTOM_MENU_SLOT_COUNT);
        for i in 1..=(CUSTOM_MENU_SLOT_COUNT as u8) {
            slots.push(CustomMenuSlot {
                id: i,
                title: format!("Custom Menu {i}"),
                items: Vec::new(),
            });
        }
        Self {
            slots: slots
                .try_into()
                .unwrap_or_else(|_| panic!("size mismatch")),
        }
    }
}

// ============================================================================
// 16. 主菜单定制 (MainMenuConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainMenuConfig {
    pub show_accelerator_keys: bool,
    pub hide_empty_recent: bool,
    pub show_icons: bool,
    pub customized_items: Vec<String>,
}

impl Default for MainMenuConfig {
    fn default() -> Self {
        Self {
            show_accelerator_keys: true,
            hide_empty_recent: false,
            show_icons: true,
            customized_items: Vec::new(),
        }
    }
}

// ============================================================================
// 17. 字体/字号/行间距 (FontConfig)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontConfig {
    pub font_family: String,
    pub font_size_pt: f32,
    pub line_spacing_px: i32,
    pub letter_spacing_px: i32,
    pub antialias: bool,
    pub bold: bool,
    pub italic: bool,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            font_family: "Consolas".to_string(),
            font_size_pt: 10.5,
            line_spacing_px: 2,
            letter_spacing_px: 0,
            antialias: true,
            bold: false,
            italic: false,
        }
    }
}

// ============================================================================
// 18. 打印排版 (PrintConfig)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintOrientation {
    Portrait,
    Landscape,
}

impl PrintOrientation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Portrait => "Portrait",
            Self::Landscape => "Landscape",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "0" | "portrait" => Self::Portrait,
            "1" | "landscape" => Self::Landscape,
            _ => Self::Portrait,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintConfig {
    pub header_left: String,
    pub header_center: String,
    pub header_right: String,
    pub footer_left: String,
    pub footer_center: String,
    pub footer_right: String,
    pub margin_top_mm: u32,
    pub margin_bottom_mm: u32,
    pub margin_left_mm: u32,
    pub margin_right_mm: u32,
    pub print_line_numbers: bool,
    pub orientation: PrintOrientation,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            header_left: "&f".to_string(),
            header_center: String::new(),
            header_right: "&d".to_string(),
            footer_left: String::new(),
            footer_center: "- &p -".to_string(),
            footer_right: String::new(),
            margin_top_mm: 20,
            margin_bottom_mm: 20,
            margin_left_mm: 15,
            margin_right_mm: 15,
            print_line_numbers: true,
            orientation: PrintOrientation::Portrait,
        }
    }
}

// ============================================================================
// 组合设置：SakuraCommonSettings
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SakuraCommonSettings {
    pub window: WindowConfig,
    pub tab_bar: TabBarConfig,
    pub edit: EditConfig,
    pub file: FileConfig,
    pub search: SearchConfig,
    pub format: FormatConfig,
    pub assist: AssistConfig,
    pub macro_cfg: MacroConfig,
    pub status_bar: StatusBarConfig,
    pub plugin: PluginConfig,
    pub toolbar: ToolbarConfig,
    pub key_search: KeySearchConfig,
    pub exec_cmd: ExecCmdConfig,
    pub color_palette: ColorPaletteConfig,
    pub custom_menu: CustomMenuConfig,
    pub main_menu: MainMenuConfig,
    pub font: FontConfig,
    pub print: PrintConfig,
}

impl Default for SakuraCommonSettings {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            tab_bar: TabBarConfig::default(),
            edit: EditConfig::default(),
            file: FileConfig::default(),
            search: SearchConfig::default(),
            format: FormatConfig::default(),
            assist: AssistConfig::default(),
            macro_cfg: MacroConfig::default(),
            status_bar: StatusBarConfig::default(),
            plugin: PluginConfig::default(),
            toolbar: ToolbarConfig::default(),
            key_search: KeySearchConfig::default(),
            exec_cmd: ExecCmdConfig::default(),
            color_palette: ColorPaletteConfig::default(),
            custom_menu: CustomMenuConfig::default(),
            main_menu: MainMenuConfig::default(),
            font: FontConfig::default(),
            print: PrintConfig::default(),
        }
    }
}

// ============================================================================
// INI 存储与双向映射实现
// ============================================================================

struct IniMap {
    // section (lowercase) -> key (lowercase) -> value
    sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl IniMap {
    fn parse(ini_str: &str) -> Result<Self, IniParseError> {
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut cur_section = String::new();

        for (line_idx, raw_line) in ini_str.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') {
                if let Some(close_idx) = line.find(']') {
                    cur_section = line[1..close_idx].trim().to_ascii_lowercase();
                    sections.entry(cur_section.clone()).or_default();
                    continue;
                } else {
                    return Err(IniParseError(format!(
                        "Line {}: Unclosed section header: {}",
                        line_idx + 1,
                        line
                    )));
                }
            }

            if let Some(eq_idx) = line.find('=') {
                let key = line[..eq_idx].trim().to_ascii_lowercase();
                let mut val = line[eq_idx + 1..].trim().to_string();

                // Strip quotes if wrapped
                if val.len() >= 2
                    && ((val.starts_with('"') && val.ends_with('"'))
                        || (val.starts_with('\'') && val.ends_with('\'')))
                {
                    val = val[1..val.len() - 1].to_string();
                }

                sections
                    .entry(cur_section.clone())
                    .or_default()
                    .insert(key, val);
            }
        }

        Ok(Self { sections })
    }

    fn get_str(&self, section: &str, keys: &[&str]) -> Option<&str> {
        let sec_key = section.to_ascii_lowercase();
        // 1. Check specified section
        if let Some(m) = self.sections.get(&sec_key) {
            for &k in keys {
                if let Some(v) = m.get(&k.to_ascii_lowercase()) {
                    return Some(v.as_str());
                }
            }
        }
        // 2. Check "common" section as fallback
        if sec_key != "common" {
            if let Some(m) = self.sections.get("common") {
                for &k in keys {
                    if let Some(v) = m.get(&k.to_ascii_lowercase()) {
                        return Some(v.as_str());
                    }
                }
            }
        }
        // 3. Check global/root section as fallback
        if let Some(m) = self.sections.get("") {
            for &k in keys {
                if let Some(v) = m.get(&k.to_ascii_lowercase()) {
                    return Some(v.as_str());
                }
            }
        }
        None
    }

    fn get_bool(&self, section: &str, keys: &[&str], default: bool) -> bool {
        match self.get_str(section, keys) {
            Some(s) => match s.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => true,
                "0" | "false" | "no" | "off" => false,
                _ => default,
            },
            None => default,
        }
    }

    fn get_u32(&self, section: &str, keys: &[&str], default: u32) -> u32 {
        self.get_str(section, keys)
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(default)
    }

    fn get_i32(&self, section: &str, keys: &[&str], default: i32) -> i32 {
        self.get_str(section, keys)
            .and_then(|s| s.trim().parse::<i32>().ok())
            .unwrap_or(default)
    }

    fn get_f32(&self, section: &str, keys: &[&str], default: f32) -> f32 {
        self.get_str(section, keys)
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(default)
    }

    fn get_u8(&self, section: &str, keys: &[&str], default: u8) -> u8 {
        self.get_str(section, keys)
            .and_then(|s| s.trim().parse::<u8>().ok())
            .unwrap_or(default)
    }

    fn get_usize(&self, section: &str, keys: &[&str], default: usize) -> usize {
        self.get_str(section, keys)
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(default)
    }

    fn get_string(&self, section: &str, keys: &[&str], default: &str) -> String {
        self.get_str(section, keys)
            .map(|s| s.to_string())
            .unwrap_or_else(|| default.to_string())
    }
}

impl SakuraCommonSettings {
    /// 新建默认配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 INI 字符串解析完整配置
    pub fn from_ini(ini_str: &str) -> Result<Self, IniParseError> {
        let map = IniMap::parse(ini_str)?;
        let mut cfg = Self::default();

        // 1. Window
        cfg.window.width = map.get_u32("window", &["width", "nwinsizecx", "winsizecx", "cx"], cfg.window.width);
        cfg.window.height = map.get_u32("window", &["height", "nwinsizecy", "winsizecy", "cy"], cfg.window.height);
        cfg.window.pos_x = map.get_i32("window", &["pos_x", "nwinposx", "winposx", "x"], cfg.window.pos_x);
        cfg.window.pos_y = map.get_i32("window", &["pos_y", "nwinposy", "winposy", "y"], cfg.window.pos_y);
        cfg.window.center_window = map.get_bool("window", &["center_window", "bcenterwin", "centerwin"], cfg.window.center_window);
        cfg.window.remember_window_size = map.get_bool("window", &["remember_window_size", "bsavesize", "savesize"], cfg.window.remember_window_size);
        cfg.window.remember_window_pos = map.get_bool("window", &["remember_window_pos", "bsavepos", "savepos"], cfg.window.remember_window_pos);
        cfg.window.always_on_top = map.get_bool("window", &["always_on_top", "balwaystop", "topmost"], cfg.window.always_on_top);
        cfg.window.opacity = map.get_u8("window", &["opacity", "nalpha", "alpha"], cfg.window.opacity);
        cfg.window.minimize_to_tray = map.get_bool("window", &["minimize_to_tray", "btasktray", "tasktray"], cfg.window.minimize_to_tray);
        cfg.window.multi_window_mode = map.get_bool("window", &["multi_window_mode", "bmultiwin", "multiwin"], cfg.window.multi_window_mode);
        cfg.window.full_screen = map.get_bool("window", &["full_screen", "bfullscreen", "fullscreen"], cfg.window.full_screen);

        // 2. TabBar
        cfg.tab_bar.visible = map.get_bool("tabbar", &["visible", "btabvisible", "tabvisible", "disp_tab"], cfg.tab_bar.visible);
        if let Some(pos) = map.get_str("tabbar", &["position", "tab_position", "ntabposition"]) {
            cfg.tab_bar.position = TabPosition::from_str_loose(pos);
        }
        cfg.tab_bar.multi_row = map.get_bool("tabbar", &["multi_row", "btabmultirow", "tabmultirow"], cfg.tab_bar.multi_row);
        cfg.tab_bar.show_close_button = map.get_bool("tabbar", &["show_close_button", "btabclosebtn", "tabclosebtn"], cfg.tab_bar.show_close_button);
        if let Some(act) = map.get_str("tabbar", &["double_click_action", "tab_double_click_action", "ndblclickaction", "ntabdoubleclickaction", "tabdoubleclickaction"]) {
            cfg.tab_bar.double_click_action = TabDoubleClickAction::from_str_loose(act);
        }
        cfg.tab_bar.close_on_middle_click = map.get_bool("tabbar", &["close_on_middle_click", "bclosemiddle", "midclickclose"], cfg.tab_bar.close_on_middle_click);
        cfg.tab_bar.close_on_right_click = map.get_bool("tabbar", &["close_on_right_click", "bcloseright", "rightclickclose"], cfg.tab_bar.close_on_right_click);
        cfg.tab_bar.wheel_tab_switch = map.get_bool("tabbar", &["wheel_tab_switch", "bwheeltab", "wheeltab"], cfg.tab_bar.wheel_tab_switch);
        cfg.tab_bar.drag_drop_reorder = map.get_bool("tabbar", &["drag_drop_reorder", "btabdrag", "tabdrag"], cfg.tab_bar.drag_drop_reorder);
        cfg.tab_bar.distinguish_same_name = map.get_bool("tabbar", &["distinguish_same_name", "bsamename", "samename"], cfg.tab_bar.distinguish_same_name);
        cfg.tab_bar.tab_width_max = map.get_u32("tabbar", &["tab_width_max", "ntabwidthmax", "tabwidthmax"], cfg.tab_bar.tab_width_max);
        cfg.tab_bar.tab_width_min = map.get_u32("tabbar", &["tab_width_min", "ntabwidthmin", "tabwidthmin"], cfg.tab_bar.tab_width_min);

        // 3. Edit
        cfg.edit.tab_width = map.get_u32("edit", &["tab_width", "tabwidth", "ntabwidth", "tab_size"], cfg.edit.tab_width);
        cfg.edit.indent_width = map.get_u32("edit", &["indent_width", "indentwidth", "nindentwidth", "indent_size"], cfg.edit.indent_width);
        cfg.edit.use_spaces_for_tab = map.get_bool("edit", &["use_spaces_for_tab", "bspacefortab", "spacetab"], cfg.edit.use_spaces_for_tab);
        cfg.edit.smart_indent = map.get_bool("edit", &["smart_indent", "bsmartindent", "smartindent"], cfg.edit.smart_indent);
        cfg.edit.auto_indent = map.get_bool("edit", &["auto_indent", "bautoindent", "autoindent"], cfg.edit.auto_indent);
        cfg.edit.auto_save_enabled = map.get_bool("edit", &["auto_save_enabled", "bautosave", "autosave"], cfg.edit.auto_save_enabled);
        cfg.edit.auto_save_interval_sec = map.get_u32("edit", &["auto_save_interval_sec", "nautosaveinterval", "autosaveinterval"], cfg.edit.auto_save_interval_sec);
        cfg.edit.overwrite_mode = map.get_bool("edit", &["overwrite_mode", "boverwrite", "overwritemode"], cfg.edit.overwrite_mode);
        cfg.edit.drag_drop_edit = map.get_bool("edit", &["drag_drop_edit", "bdragdrop", "dragdrop"], cfg.edit.drag_drop_edit);
        cfg.edit.copy_line_when_empty_selection = map.get_bool("edit", &["copy_line_when_empty_selection", "bcopyline", "copyline"], cfg.edit.copy_line_when_empty_selection);
        cfg.edit.scroll_smooth = map.get_bool("edit", &["scroll_smooth", "bsmoothscroll", "smoothscroll"], cfg.edit.scroll_smooth);
        cfg.edit.scroll_past_end_of_line = map.get_bool("edit", &["scroll_past_end_of_line", "bfreecaret", "freecaret"], cfg.edit.scroll_past_end_of_line);

        // 4. File
        cfg.file.default_encoding = map.get_string("file", &["default_encoding", "encoding", "szencoding", "code_page"], &cfg.file.default_encoding);
        cfg.file.default_eol = map.get_string("file", &["default_eol", "eol", "szeol", "line_ending"], &cfg.file.default_eol);
        cfg.file.file_lock_exclusive = map.get_bool("file", &["file_lock_exclusive", "bfilelock", "exclusive_lock"], cfg.file.file_lock_exclusive);
        cfg.file.recent_files_max = map.get_usize("file", &["recent_files_max", "nrecentmax", "recent_max"], cfg.file.recent_files_max);
        cfg.file.recent_folders_max = map.get_usize("file", &["recent_folders_max", "nrecentfoldersmax", "recent_folders_max"], cfg.file.recent_folders_max);
        cfg.file.auto_reload_changed = map.get_bool("file", &["auto_reload_changed", "bautoreload", "autoreload"], cfg.file.auto_reload_changed);
        cfg.file.alert_file_deleted = map.get_bool("file", &["alert_file_deleted", "balertdeleted", "alertdeleted"], cfg.file.alert_file_deleted);
        cfg.file.backup_enabled = map.get_bool("file", &["backup_enabled", "bbackup", "backup"], cfg.file.backup_enabled);
        cfg.file.backup_folder = map.get_string("file", &["backup_folder", "szbackupfolder", "backupdir"], &cfg.file.backup_folder);
        cfg.file.backup_extension = map.get_string("file", &["backup_extension", "szbackupext", "backupext"], &cfg.file.backup_extension);
        cfg.file.backup_count_max = map.get_u32("file", &["backup_count_max", "nbackupmax", "backupmax"], cfg.file.backup_count_max);
        cfg.file.restore_cursor_pos = map.get_bool("file", &["restore_cursor_pos", "brestorecursor", "restorecursor"], cfg.file.restore_cursor_pos);

        // 5. Search
        cfg.search.case_sensitive = map.get_bool("search", &["case_sensitive", "bcasesensitive", "casesensitive"], cfg.search.case_sensitive);
        cfg.search.whole_word = map.get_bool("search", &["whole_word", "bwholeword", "wholeword"], cfg.search.whole_word);
        cfg.search.regex_enabled = map.get_bool("search", &["regex_enabled", "bregex", "regex"], cfg.search.regex_enabled);
        cfg.search.lookaround_enabled = map.get_bool("search", &["lookaround_enabled", "blookaround", "bregexplookaround"], cfg.search.lookaround_enabled);
        cfg.search.search_wrap = map.get_bool("search", &["search_wrap", "bsearchwrap", "wrapsearch"], cfg.search.search_wrap);
        cfg.search.search_backwards = map.get_bool("search", &["search_backwards", "bsearchbackwards", "searchbackwards"], cfg.search.search_backwards);
        cfg.search.highlight_all_matches = map.get_bool("search", &["highlight_all_matches", "bhighlightall", "highlightall"], cfg.search.highlight_all_matches);
        cfg.search.search_history_max = map.get_usize("search", &["search_history_max", "nsearchhistorymax", "searchhistorymax"], cfg.search.search_history_max);
        cfg.search.replace_history_max = map.get_usize("search", &["replace_history_max", "nreplacehistorymax", "replacehistorymax"], cfg.search.replace_history_max);
        cfg.search.auto_close_dialog = map.get_bool("search", &["auto_close_dialog", "bautoclosedialog", "autoclosedialog"], cfg.search.auto_close_dialog);
        cfg.search.use_migemo = map.get_bool("search", &["use_migemo", "bmigemo", "migemo"], cfg.search.use_migemo);

        // 6. Format
        if let Some(wm) = map.get_str("format", &["wrap_mode", "nwrapmode", "wrapmode"]) {
            cfg.format.wrap_mode = WrapMode::from_str_loose(wm);
        }
        cfg.format.wrap_columns = map.get_u32("format", &["wrap_columns", "nwrapcolumns", "wrapcolumns", "max_columns"], cfg.format.wrap_columns);
        cfg.format.kinsoku_head_enabled = map.get_bool("format", &["kinsoku_head_enabled", "bkinsokuhead", "kinsokuhead"], cfg.format.kinsoku_head_enabled);
        cfg.format.kinsoku_tail_enabled = map.get_bool("format", &["kinsoku_tail_enabled", "bkinsokutail", "kinsokutail"], cfg.format.kinsoku_tail_enabled);
        cfg.format.kinsoku_push_enabled = map.get_bool("format", &["kinsoku_push_enabled", "bkinsokupush", "kinsokupush", "burasagari"], cfg.format.kinsoku_push_enabled);
        if let Some(h) = map.get_str("format", &["kinsoku_head", "szkinsokuhead", "kinsokuheadchars"]) {
            cfg.format.kinsoku_chars.set_head_chars(h);
        }
        if let Some(t) = map.get_str("format", &["kinsoku_tail", "szkinsokutail", "kinsokutailchars"]) {
            cfg.format.kinsoku_chars.set_tail_chars(t);
        }
        cfg.format.show_line_numbers = map.get_bool("format", &["show_line_numbers", "bshowlinenums", "showlinenums"], cfg.format.show_line_numbers);
        cfg.format.show_ruler = map.get_bool("format", &["show_ruler", "bshowruler", "showruler"], cfg.format.show_ruler);

        // 7. Assist
        cfg.assist.auto_complete_enabled = map.get_bool("assist", &["auto_complete_enabled", "bautocomplete", "autocomplete"], cfg.assist.auto_complete_enabled);
        cfg.assist.min_char_trigger = map.get_u32("assist", &["min_char_trigger", "nmintrigger", "minchars"], cfg.assist.min_char_trigger);
        cfg.assist.show_candidate_list = map.get_bool("assist", &["show_candidate_list", "bshowcandidates", "showcandidates"], cfg.assist.show_candidate_list);
        cfg.assist.dictionary_path = map.get_string("assist", &["dictionary_path", "szdictpath", "dictpath"], &cfg.assist.dictionary_path);
        cfg.assist.use_words_in_doc = map.get_bool("assist", &["use_words_in_doc", "busewordsindoc", "wordsindoc"], cfg.assist.use_words_in_doc);
        cfg.assist.matching_paren_highlight = map.get_bool("assist", &["matching_paren_highlight", "bmatchparen", "matchparen"], cfg.assist.matching_paren_highlight);
        cfg.assist.auto_close_brackets = map.get_bool("assist", &["auto_close_brackets", "bautoclosebracket", "autoclosebracket"], cfg.assist.auto_close_brackets);
        cfg.assist.candidate_max_count = map.get_u32("assist", &["candidate_max_count", "ncandidatemax", "candidatemax"], cfg.assist.candidate_max_count);

        // 8. Macro (20 Slots)
        for i in 1..=MACRO_SLOT_COUNT {
            let slot = &mut cfg.macro_cfg.slots[i - 1];
            let idx_str = format!("{i:02}");
            let idx_short = format!("{i}");

            let k_name = [
                format!("slot{idx_str}_name"),
                format!("slot{idx_short}_name"),
                format!("macro{idx_str}_name"),
                format!("macro{idx_short}_name"),
            ];
            let k_path = [
                format!("slot{idx_str}_path"),
                format!("slot{idx_short}_path"),
                format!("macro{idx_str}_path"),
                format!("macro{idx_short}_path"),
            ];
            let k_reload = [
                format!("slot{idx_str}_reload"),
                format!("slot{idx_short}_reload"),
                format!("macro{idx_str}_reload"),
                format!("macro{idx_short}_reload"),
            ];
            let k_trigger = [
                format!("slot{idx_str}_trigger"),
                format!("slot{idx_short}_trigger"),
                format!("macro{idx_str}_trigger"),
                format!("macro{idx_short}_trigger"),
            ];
            let k_params = [
                format!("slot{idx_str}_params"),
                format!("slot{idx_short}_params"),
                format!("macro{idx_str}_params"),
                format!("macro{idx_short}_params"),
            ];

            let k_name_slices: Vec<&str> = k_name.iter().map(|s| s.as_str()).collect();
            let k_path_slices: Vec<&str> = k_path.iter().map(|s| s.as_str()).collect();
            let k_reload_slices: Vec<&str> = k_reload.iter().map(|s| s.as_str()).collect();
            let k_trigger_slices: Vec<&str> = k_trigger.iter().map(|s| s.as_str()).collect();
            let k_params_slices: Vec<&str> = k_params.iter().map(|s| s.as_str()).collect();

            slot.name = map.get_string("macro", &k_name_slices, &slot.name);
            slot.path = map.get_string("macro", &k_path_slices, &slot.path);
            slot.reload_on_run = map.get_bool("macro", &k_reload_slices, slot.reload_on_run);
            if let Some(t) = map.get_str("macro", &k_trigger_slices) {
                slot.trigger = MacroTrigger::from_str_loose(t);
            }
            slot.params = map.get_string("macro", &k_params_slices, &slot.params);
        }

        // 9. StatusBar
        cfg.status_bar.visible = map.get_bool("statusbar", &["visible", "bstatusvisible", "statusvisible"], cfg.status_bar.visible);
        cfg.status_bar.show_caret_pos = map.get_bool("statusbar", &["show_caret_pos", "bcaretpos", "caretpos"], cfg.status_bar.show_caret_pos);
        cfg.status_bar.show_char_code = map.get_bool("statusbar", &["show_char_code", "bcharcode", "charcode"], cfg.status_bar.show_char_code);
        cfg.status_bar.show_encoding = map.get_bool("statusbar", &["show_encoding", "bencoding", "encoding"], cfg.status_bar.show_encoding);
        cfg.status_bar.show_eol = map.get_bool("statusbar", &["show_eol", "beol", "eol"], cfg.status_bar.show_eol);
        cfg.status_bar.show_modified = map.get_bool("statusbar", &["show_modified", "bmodified", "modified"], cfg.status_bar.show_modified);
        cfg.status_bar.show_ins_ovr = map.get_bool("statusbar", &["show_ins_ovr", "binsovr", "insovr"], cfg.status_bar.show_ins_ovr);
        cfg.status_bar.show_file_size = map.get_bool("statusbar", &["show_file_size", "bfilesize", "filesize"], cfg.status_bar.show_file_size);
        cfg.status_bar.show_zoom = map.get_bool("statusbar", &["show_zoom", "bzoom", "zoom"], cfg.status_bar.show_zoom);
        cfg.status_bar.show_line_count = map.get_bool("statusbar", &["show_line_count", "blinecount", "linecount"], cfg.status_bar.show_line_count);

        // 10. Plugin
        cfg.plugin.auto_scan_plugins = map.get_bool("plugin", &["auto_scan_plugins", "bautoscan", "autoscan"], cfg.plugin.auto_scan_plugins);
        cfg.plugin.plugin_dir = map.get_string("plugin", &["plugin_dir", "szplugindir", "plugindir"], &cfg.plugin.plugin_dir);
        if let Some(plugins_str) = map.get_str("plugin", &["enabled_plugins", "enabledplugins", "szenabledplugins"]) {
            cfg.plugin.enabled_plugins = plugins_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        cfg.plugin.security_level = map.get_u8("plugin", &["security_level", "nsecuritylevel", "securitylevel"], cfg.plugin.security_level);
        cfg.plugin.allow_network = map.get_bool("plugin", &["allow_network", "ballownetwork", "allownetwork"], cfg.plugin.allow_network);

        // 11. Toolbar
        cfg.toolbar.visible = map.get_bool("toolbar", &["visible", "btoolbarvisible", "toolbarvisible"], cfg.toolbar.visible);
        cfg.toolbar.button_size = map.get_u32("toolbar", &["button_size", "nbuttonsize", "buttonsize"], cfg.toolbar.button_size);
        cfg.toolbar.flat_style = map.get_bool("toolbar", &["flat_style", "bflatstyle", "flatstyle"], cfg.toolbar.flat_style);
        cfg.toolbar.show_tooltips = map.get_bool("toolbar", &["show_tooltips", "bshowtooltips", "tooltips"], cfg.toolbar.show_tooltips);
        if let Some(buttons_str) = map.get_str("toolbar", &["buttons", "szbuttons", "toolbarbuttons"]) {
            cfg.toolbar.buttons = buttons_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // 12. KeySearch
        cfg.key_search.enabled = map.get_bool("keysearch", &["enabled", "benabled"], cfg.key_search.enabled);
        cfg.key_search.dictionary_lookup_url = map.get_string("keysearch", &["dictionary_lookup_url", "szlookupurl", "lookupurl"], &cfg.key_search.dictionary_lookup_url);
        cfg.key_search.hotkey = map.get_string("keysearch", &["hotkey", "szhotkey"], &cfg.key_search.hotkey);
        cfg.key_search.auto_search_selected = map.get_bool("keysearch", &["auto_search_selected", "bsearchselected", "searchselected"], cfg.key_search.auto_search_selected);
        cfg.key_search.open_in_external_browser = map.get_bool("keysearch", &["open_in_external_browser", "bexternalbrowser", "externalbrowser"], cfg.key_search.open_in_external_browser);

        // 13. ExecCmd
        cfg.exec_cmd.default_shell = map.get_string("execcmd", &["default_shell", "szshell", "shell"], &cfg.exec_cmd.default_shell);
        cfg.exec_cmd.capture_output = map.get_bool("execcmd", &["capture_output", "bcaptureoutput", "captureoutput"], cfg.exec_cmd.capture_output);
        cfg.exec_cmd.run_in_background = map.get_bool("execcmd", &["run_in_background", "bbackground", "background"], cfg.exec_cmd.run_in_background);
        cfg.exec_cmd.show_console_window = map.get_bool("execcmd", &["show_console_window", "bshowconsole", "showconsole"], cfg.exec_cmd.show_console_window);

        let cmd_count = map.get_u32("execcmd", &["command_count", "ncommandcount"], 0);
        let mut cmds = Vec::new();
        let max_scan = if cmd_count > 0 { cmd_count } else { 32 };
        for i in 1..=max_scan {
            let idx_str = format!("{i:02}");
            let k_name = [format!("cmd{idx_str}_name"), format!("cmd{i}_name")];
            let k_cmd = [format!("cmd{idx_str}_cmd"), format!("cmd{i}_cmd")];
            let k_dir = [format!("cmd{idx_str}_workdir"), format!("cmd{i}_workdir")];
            let k_stdin = [format!("cmd{idx_str}_stdin"), format!("cmd{i}_stdin")];

            let k_name_s: Vec<&str> = k_name.iter().map(|s| s.as_str()).collect();
            let k_cmd_s: Vec<&str> = k_cmd.iter().map(|s| s.as_str()).collect();
            let k_dir_s: Vec<&str> = k_dir.iter().map(|s| s.as_str()).collect();
            let k_stdin_s: Vec<&str> = k_stdin.iter().map(|s| s.as_str()).collect();

            if let Some(cmd_line) = map.get_str("execcmd", &k_cmd_s) {
                let name = map.get_string("execcmd", &k_name_s, &format!("Command {i}"));
                let work_dir = map.get_string("execcmd", &k_dir_s, "");
                let send_stdin = map.get_bool("execcmd", &k_stdin_s, false);
                cmds.push(ExternalCmdSlot {
                    id: i,
                    name,
                    cmd: cmd_line.to_string(),
                    work_dir,
                    send_stdin,
                });
            } else if cmd_count > 0 {
                // If count was explicit, still check
                continue;
            } else {
                break;
            }
        }
        if !cmds.is_empty() {
            cfg.exec_cmd.commands = cmds;
        }

        // 14. ColorPalette (24 slots)
        cfg.color_palette.scheme_name = map.get_string("colorpalette", &["scheme_name", "schemename", "szschemename"], &cfg.color_palette.scheme_name);
        for i in 1..=PALETTE_COLOR_COUNT {
            let color = &mut cfg.color_palette.colors[i - 1];
            let idx_str = format!("{i:02}");
            let idx_short = format!("{i}");

            let k_name = [format!("color{idx_str}_name"), format!("color{idx_short}_name")];
            let k_fore = [format!("color{idx_str}_fore"), format!("color{idx_short}_fore")];
            let k_back = [format!("color{idx_str}_back"), format!("color{idx_short}_back")];
            let k_bold = [format!("color{idx_str}_bold"), format!("color{idx_short}_bold")];
            let k_under = [format!("color{idx_str}_underline"), format!("color{idx_short}_underline")];

            let k_name_s: Vec<&str> = k_name.iter().map(|s| s.as_str()).collect();
            let k_fore_s: Vec<&str> = k_fore.iter().map(|s| s.as_str()).collect();
            let k_back_s: Vec<&str> = k_back.iter().map(|s| s.as_str()).collect();
            let k_bold_s: Vec<&str> = k_bold.iter().map(|s| s.as_str()).collect();
            let k_under_s: Vec<&str> = k_under.iter().map(|s| s.as_str()).collect();

            color.name = map.get_string("colorpalette", &k_name_s, &color.name);
            color.foreground = map.get_string("colorpalette", &k_fore_s, &color.foreground);
            color.background = map.get_string("colorpalette", &k_back_s, &color.background);
            color.bold = map.get_bool("colorpalette", &k_bold_s, color.bold);
            color.underline = map.get_bool("colorpalette", &k_under_s, color.underline);
        }

        // 15. CustomMenu (24 slots)
        for i in 1..=CUSTOM_MENU_SLOT_COUNT {
            let slot = &mut cfg.custom_menu.slots[i - 1];
            let idx_str = format!("{i:02}");
            let idx_short = format!("{i}");

            let k_title = [format!("menu{idx_str}_title"), format!("menu{idx_short}_title")];
            let k_title_s: Vec<&str> = k_title.iter().map(|s| s.as_str()).collect();
            slot.title = map.get_string("custommenu", &k_title_s, &slot.title);

            let mut items = Vec::new();
            // Try delimited items string first
            let k_items = [format!("menu{idx_str}_items"), format!("menu{idx_short}_items")];
            let k_items_s: Vec<&str> = k_items.iter().map(|s| s.as_str()).collect();
            if let Some(items_str) = map.get_str("custommenu", &k_items_s) {
                for item_chunk in items_str.split(';') {
                    let chunk = item_chunk.trim();
                    if chunk.is_empty() {
                        continue;
                    }
                    if let Some(pos) = chunk.find(',').or_else(|| chunk.find(':')) {
                        items.push(CustomMenuItem {
                            label: chunk[..pos].trim().to_string(),
                            command: chunk[pos + 1..].trim().to_string(),
                        });
                    } else {
                        items.push(CustomMenuItem {
                            label: chunk.to_string(),
                            command: chunk.to_string(),
                        });
                    }
                }
            } else {
                // Try indexed items
                let k_cnt = [format!("menu{idx_str}_itemcount"), format!("menu{idx_short}_itemcount")];
                let k_cnt_s: Vec<&str> = k_cnt.iter().map(|s| s.as_str()).collect();
                let item_cnt = map.get_u32("custommenu", &k_cnt_s, 0);
                let scan_limit = if item_cnt > 0 { item_cnt } else { 32 };
                for j in 1..=scan_limit {
                    let jdx_str = format!("{j:02}");
                    let k_it = [
                        format!("menu{idx_str}_item{jdx_str}"),
                        format!("menu{idx_str}_item{j}"),
                        format!("menu{idx_short}_item{jdx_str}"),
                        format!("menu{idx_short}_item{j}"),
                    ];
                    let k_it_s: Vec<&str> = k_it.iter().map(|s| s.as_str()).collect();
                    if let Some(val) = map.get_str("custommenu", &k_it_s) {
                        let chunk = val.trim();
                        if let Some(pos) = chunk.find(',').or_else(|| chunk.find(':')) {
                            items.push(CustomMenuItem {
                                label: chunk[..pos].trim().to_string(),
                                command: chunk[pos + 1..].trim().to_string(),
                            });
                        } else {
                            items.push(CustomMenuItem {
                                label: chunk.to_string(),
                                command: chunk.to_string(),
                            });
                        }
                    } else if item_cnt > 0 {
                        continue;
                    } else {
                        break;
                    }
                }
            }
            if !items.is_empty() {
                slot.items = items;
            }
        }

        // 16. MainMenu
        cfg.main_menu.show_accelerator_keys = map.get_bool("mainmenu", &["show_accelerator_keys", "bacceleratorkeys", "acceleratorkeys"], cfg.main_menu.show_accelerator_keys);
        cfg.main_menu.hide_empty_recent = map.get_bool("mainmenu", &["hide_empty_recent", "bhideemptyrecent", "hideemptyrecent"], cfg.main_menu.hide_empty_recent);
        cfg.main_menu.show_icons = map.get_bool("mainmenu", &["show_icons", "bshowicons", "showicons"], cfg.main_menu.show_icons);
        if let Some(items_str) = map.get_str("mainmenu", &["customized_items", "customizeditems"]) {
            cfg.main_menu.customized_items = items_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // 17. Font
        cfg.font.font_family = map.get_string("font", &["font_family", "szfontname", "fontname", "family"], &cfg.font.font_family);
        cfg.font.font_size_pt = map.get_f32("font", &["font_size_pt", "nfontsize", "fontsize", "size_pt"], cfg.font.font_size_pt);
        cfg.font.line_spacing_px = map.get_i32("font", &["line_spacing_px", "nlinespacing", "linespacing", "line_space"], cfg.font.line_spacing_px);
        cfg.font.letter_spacing_px = map.get_i32("font", &["letter_spacing_px", "nletterspacing", "letterspacing", "char_space"], cfg.font.letter_spacing_px);
        cfg.font.antialias = map.get_bool("font", &["antialias", "bantialias"], cfg.font.antialias);
        cfg.font.bold = map.get_bool("font", &["bold", "bbold"], cfg.font.bold);
        cfg.font.italic = map.get_bool("font", &["italic", "bitalic"], cfg.font.italic);

        // 18. Print
        cfg.print.header_left = map.get_string("print", &["header_left", "szheaderleft", "headerleft"], &cfg.print.header_left);
        cfg.print.header_center = map.get_string("print", &["header_center", "szheadercenter", "headercenter"], &cfg.print.header_center);
        cfg.print.header_right = map.get_string("print", &["header_right", "szheaderright", "headerright"], &cfg.print.header_right);
        cfg.print.footer_left = map.get_string("print", &["footer_left", "szfooterleft", "footerleft"], &cfg.print.footer_left);
        cfg.print.footer_center = map.get_string("print", &["footer_center", "szfootercenter", "footercenter"], &cfg.print.footer_center);
        cfg.print.footer_right = map.get_string("print", &["footer_right", "szfooterright", "footerright"], &cfg.print.footer_right);
        cfg.print.margin_top_mm = map.get_u32("print", &["margin_top_mm", "nmargintop", "margintop"], cfg.print.margin_top_mm);
        cfg.print.margin_bottom_mm = map.get_u32("print", &["margin_bottom_mm", "nmarginbottom", "marginbottom"], cfg.print.margin_bottom_mm);
        cfg.print.margin_left_mm = map.get_u32("print", &["margin_left_mm", "nmarginleft", "marginleft"], cfg.print.margin_left_mm);
        cfg.print.margin_right_mm = map.get_u32("print", &["margin_right_mm", "nmarginright", "marginright"], cfg.print.margin_right_mm);
        cfg.print.print_line_numbers = map.get_bool("print", &["print_line_numbers", "bprintlinenums", "printlinenums"], cfg.print.print_line_numbers);
        if let Some(ori) = map.get_str("print", &["orientation", "norientation", "page_orientation"]) {
            cfg.print.orientation = PrintOrientation::from_str_loose(ori);
        }

        Ok(cfg)
    }

    /// 序列化为规范的 INI 字符串 (包含完整 18 页面分类)
    pub fn to_ini(&self) -> String {
        let mut out = String::with_capacity(8192);

        out.push_str("; ============================================================================\n");
        out.push_str("; Sakura Editor Common Settings (18 Pages)\n");
        out.push_str("; Generated by editcore::sakura_settings\n");
        out.push_str("; ============================================================================\n\n");

        // 1. Window
        out.push_str("[Window]\n");
        out.push_str(&format!("width = {}\n", self.window.width));
        out.push_str(&format!("height = {}\n", self.window.height));
        out.push_str(&format!("pos_x = {}\n", self.window.pos_x));
        out.push_str(&format!("pos_y = {}\n", self.window.pos_y));
        out.push_str(&format!("center_window = {}\n", self.window.center_window));
        out.push_str(&format!("remember_window_size = {}\n", self.window.remember_window_size));
        out.push_str(&format!("remember_window_pos = {}\n", self.window.remember_window_pos));
        out.push_str(&format!("always_on_top = {}\n", self.window.always_on_top));
        out.push_str(&format!("opacity = {}\n", self.window.opacity));
        out.push_str(&format!("minimize_to_tray = {}\n", self.window.minimize_to_tray));
        out.push_str(&format!("multi_window_mode = {}\n", self.window.multi_window_mode));
        out.push_str(&format!("full_screen = {}\n\n", self.window.full_screen));

        // 2. TabBar
        out.push_str("[TabBar]\n");
        out.push_str(&format!("visible = {}\n", self.tab_bar.visible));
        out.push_str(&format!("position = {}\n", self.tab_bar.position.as_str()));
        out.push_str(&format!("multi_row = {}\n", self.tab_bar.multi_row));
        out.push_str(&format!("show_close_button = {}\n", self.tab_bar.show_close_button));
        out.push_str(&format!("double_click_action = {}\n", self.tab_bar.double_click_action.as_str()));
        out.push_str(&format!("close_on_middle_click = {}\n", self.tab_bar.close_on_middle_click));
        out.push_str(&format!("close_on_right_click = {}\n", self.tab_bar.close_on_right_click));
        out.push_str(&format!("wheel_tab_switch = {}\n", self.tab_bar.wheel_tab_switch));
        out.push_str(&format!("drag_drop_reorder = {}\n", self.tab_bar.drag_drop_reorder));
        out.push_str(&format!("distinguish_same_name = {}\n", self.tab_bar.distinguish_same_name));
        out.push_str(&format!("tab_width_max = {}\n", self.tab_bar.tab_width_max));
        out.push_str(&format!("tab_width_min = {}\n\n", self.tab_bar.tab_width_min));

        // 3. Edit
        out.push_str("[Edit]\n");
        out.push_str(&format!("tab_width = {}\n", self.edit.tab_width));
        out.push_str(&format!("indent_width = {}\n", self.edit.indent_width));
        out.push_str(&format!("use_spaces_for_tab = {}\n", self.edit.use_spaces_for_tab));
        out.push_str(&format!("smart_indent = {}\n", self.edit.smart_indent));
        out.push_str(&format!("auto_indent = {}\n", self.edit.auto_indent));
        out.push_str(&format!("auto_save_enabled = {}\n", self.edit.auto_save_enabled));
        out.push_str(&format!("auto_save_interval_sec = {}\n", self.edit.auto_save_interval_sec));
        out.push_str(&format!("overwrite_mode = {}\n", self.edit.overwrite_mode));
        out.push_str(&format!("drag_drop_edit = {}\n", self.edit.drag_drop_edit));
        out.push_str(&format!("copy_line_when_empty_selection = {}\n", self.edit.copy_line_when_empty_selection));
        out.push_str(&format!("scroll_smooth = {}\n", self.edit.scroll_smooth));
        out.push_str(&format!("scroll_past_end_of_line = {}\n\n", self.edit.scroll_past_end_of_line));

        // 4. File
        out.push_str("[File]\n");
        out.push_str(&format!("default_encoding = {}\n", self.file.default_encoding));
        out.push_str(&format!("default_eol = {}\n", self.file.default_eol));
        out.push_str(&format!("file_lock_exclusive = {}\n", self.file.file_lock_exclusive));
        out.push_str(&format!("recent_files_max = {}\n", self.file.recent_files_max));
        out.push_str(&format!("recent_folders_max = {}\n", self.file.recent_folders_max));
        out.push_str(&format!("auto_reload_changed = {}\n", self.file.auto_reload_changed));
        out.push_str(&format!("alert_file_deleted = {}\n", self.file.alert_file_deleted));
        out.push_str(&format!("backup_enabled = {}\n", self.file.backup_enabled));
        out.push_str(&format!("backup_folder = {}\n", self.file.backup_folder));
        out.push_str(&format!("backup_extension = {}\n", self.file.backup_extension));
        out.push_str(&format!("backup_count_max = {}\n", self.file.backup_count_max));
        out.push_str(&format!("restore_cursor_pos = {}\n\n", self.file.restore_cursor_pos));

        // 5. Search
        out.push_str("[Search]\n");
        out.push_str(&format!("case_sensitive = {}\n", self.search.case_sensitive));
        out.push_str(&format!("whole_word = {}\n", self.search.whole_word));
        out.push_str(&format!("regex_enabled = {}\n", self.search.regex_enabled));
        out.push_str(&format!("lookaround_enabled = {}\n", self.search.lookaround_enabled));
        out.push_str(&format!("search_wrap = {}\n", self.search.search_wrap));
        out.push_str(&format!("search_backwards = {}\n", self.search.search_backwards));
        out.push_str(&format!("highlight_all_matches = {}\n", self.search.highlight_all_matches));
        out.push_str(&format!("search_history_max = {}\n", self.search.search_history_max));
        out.push_str(&format!("replace_history_max = {}\n", self.search.replace_history_max));
        out.push_str(&format!("auto_close_dialog = {}\n", self.search.auto_close_dialog));
        out.push_str(&format!("use_migemo = {}\n\n", self.search.use_migemo));

        // 6. Format
        out.push_str("[Format]\n");
        out.push_str(&format!("wrap_mode = {}\n", self.format.wrap_mode.as_str()));
        out.push_str(&format!("wrap_columns = {}\n", self.format.wrap_columns));
        out.push_str(&format!("kinsoku_head_enabled = {}\n", self.format.kinsoku_head_enabled));
        out.push_str(&format!("kinsoku_tail_enabled = {}\n", self.format.kinsoku_tail_enabled));
        out.push_str(&format!("kinsoku_push_enabled = {}\n", self.format.kinsoku_push_enabled));
        out.push_str(&format!("kinsoku_head = {}\n", self.format.kinsoku_chars.head_chars()));
        out.push_str(&format!("kinsoku_tail = {}\n", self.format.kinsoku_chars.tail_chars()));
        out.push_str(&format!("show_line_numbers = {}\n", self.format.show_line_numbers));
        out.push_str(&format!("show_ruler = {}\n\n", self.format.show_ruler));

        // 7. Assist
        out.push_str("[Assist]\n");
        out.push_str(&format!("auto_complete_enabled = {}\n", self.assist.auto_complete_enabled));
        out.push_str(&format!("min_char_trigger = {}\n", self.assist.min_char_trigger));
        out.push_str(&format!("show_candidate_list = {}\n", self.assist.show_candidate_list));
        out.push_str(&format!("dictionary_path = {}\n", self.assist.dictionary_path));
        out.push_str(&format!("use_words_in_doc = {}\n", self.assist.use_words_in_doc));
        out.push_str(&format!("matching_paren_highlight = {}\n", self.assist.matching_paren_highlight));
        out.push_str(&format!("auto_close_brackets = {}\n", self.assist.auto_close_brackets));
        out.push_str(&format!("candidate_max_count = {}\n\n", self.assist.candidate_max_count));

        // 8. Macro
        out.push_str("[Macro]\n");
        for (i, slot) in self.macro_cfg.slots.iter().enumerate() {
            let idx = i + 1;
            out.push_str(&format!("Slot{:02}_Name = {}\n", idx, slot.name));
            out.push_str(&format!("Slot{:02}_Path = {}\n", idx, slot.path));
            out.push_str(&format!("Slot{:02}_Reload = {}\n", idx, slot.reload_on_run));
            out.push_str(&format!("Slot{:02}_Trigger = {}\n", idx, slot.trigger.as_str()));
            out.push_str(&format!("Slot{:02}_Params = {}\n", idx, slot.params));
        }
        out.push('\n');

        // 9. StatusBar
        out.push_str("[StatusBar]\n");
        out.push_str(&format!("visible = {}\n", self.status_bar.visible));
        out.push_str(&format!("show_caret_pos = {}\n", self.status_bar.show_caret_pos));
        out.push_str(&format!("show_char_code = {}\n", self.status_bar.show_char_code));
        out.push_str(&format!("show_encoding = {}\n", self.status_bar.show_encoding));
        out.push_str(&format!("show_eol = {}\n", self.status_bar.show_eol));
        out.push_str(&format!("show_modified = {}\n", self.status_bar.show_modified));
        out.push_str(&format!("show_ins_ovr = {}\n", self.status_bar.show_ins_ovr));
        out.push_str(&format!("show_file_size = {}\n", self.status_bar.show_file_size));
        out.push_str(&format!("show_zoom = {}\n", self.status_bar.show_zoom));
        out.push_str(&format!("show_line_count = {}\n\n", self.status_bar.show_line_count));

        // 10. Plugin
        out.push_str("[Plugin]\n");
        out.push_str(&format!("auto_scan_plugins = {}\n", self.plugin.auto_scan_plugins));
        out.push_str(&format!("plugin_dir = {}\n", self.plugin.plugin_dir));
        out.push_str(&format!("enabled_plugins = {}\n", self.plugin.enabled_plugins.join(",")));
        out.push_str(&format!("security_level = {}\n", self.plugin.security_level));
        out.push_str(&format!("allow_network = {}\n\n", self.plugin.allow_network));

        // 11. Toolbar
        out.push_str("[Toolbar]\n");
        out.push_str(&format!("visible = {}\n", self.toolbar.visible));
        out.push_str(&format!("button_size = {}\n", self.toolbar.button_size));
        out.push_str(&format!("flat_style = {}\n", self.toolbar.flat_style));
        out.push_str(&format!("show_tooltips = {}\n", self.toolbar.show_tooltips));
        out.push_str(&format!("buttons = {}\n\n", self.toolbar.buttons.join(",")));

        // 12. KeySearch
        out.push_str("[KeySearch]\n");
        out.push_str(&format!("enabled = {}\n", self.key_search.enabled));
        out.push_str(&format!("dictionary_lookup_url = {}\n", self.key_search.dictionary_lookup_url));
        out.push_str(&format!("hotkey = {}\n", self.key_search.hotkey));
        out.push_str(&format!("auto_search_selected = {}\n", self.key_search.auto_search_selected));
        out.push_str(&format!("open_in_external_browser = {}\n\n", self.key_search.open_in_external_browser));

        // 13. ExecCmd
        out.push_str("[ExecCmd]\n");
        out.push_str(&format!("default_shell = {}\n", self.exec_cmd.default_shell));
        out.push_str(&format!("capture_output = {}\n", self.exec_cmd.capture_output));
        out.push_str(&format!("run_in_background = {}\n", self.exec_cmd.run_in_background));
        out.push_str(&format!("show_console_window = {}\n", self.exec_cmd.show_console_window));
        out.push_str(&format!("command_count = {}\n", self.exec_cmd.commands.len()));
        for (i, cmd) in self.exec_cmd.commands.iter().enumerate() {
            let idx = i + 1;
            out.push_str(&format!("Cmd{:02}_Id = {}\n", idx, cmd.id));
            out.push_str(&format!("Cmd{:02}_Name = {}\n", idx, cmd.name));
            out.push_str(&format!("Cmd{:02}_Cmd = {}\n", idx, cmd.cmd));
            out.push_str(&format!("Cmd{:02}_WorkDir = {}\n", idx, cmd.work_dir));
            out.push_str(&format!("Cmd{:02}_Stdin = {}\n", idx, cmd.send_stdin));
        }
        out.push('\n');

        // 14. ColorPalette
        out.push_str("[ColorPalette]\n");
        out.push_str(&format!("scheme_name = {}\n", self.color_palette.scheme_name));
        for (i, color) in self.color_palette.colors.iter().enumerate() {
            let idx = i + 1;
            out.push_str(&format!("Color{:02}_Name = {}\n", idx, color.name));
            out.push_str(&format!("Color{:02}_Fore = {}\n", idx, color.foreground));
            out.push_str(&format!("Color{:02}_Back = {}\n", idx, color.background));
            out.push_str(&format!("Color{:02}_Bold = {}\n", idx, color.bold));
            out.push_str(&format!("Color{:02}_Underline = {}\n", idx, color.underline));
        }
        out.push('\n');

        // 15. CustomMenu
        out.push_str("[CustomMenu]\n");
        for (i, slot) in self.custom_menu.slots.iter().enumerate() {
            let idx = i + 1;
            out.push_str(&format!("Menu{:02}_Title = {}\n", idx, slot.title));
            out.push_str(&format!("Menu{:02}_ItemCount = {}\n", idx, slot.items.len()));
            for (j, item) in slot.items.iter().enumerate() {
                let jdx = j + 1;
                out.push_str(&format!("Menu{:02}_Item{:02} = {},{}\n", idx, jdx, item.label, item.command));
            }
        }
        out.push('\n');

        // 16. MainMenu
        out.push_str("[MainMenu]\n");
        out.push_str(&format!("show_accelerator_keys = {}\n", self.main_menu.show_accelerator_keys));
        out.push_str(&format!("hide_empty_recent = {}\n", self.main_menu.hide_empty_recent));
        out.push_str(&format!("show_icons = {}\n", self.main_menu.show_icons));
        out.push_str(&format!("customized_items = {}\n\n", self.main_menu.customized_items.join(",")));

        // 17. Font
        out.push_str("[Font]\n");
        out.push_str(&format!("font_family = {}\n", self.font.font_family));
        out.push_str(&format!("font_size_pt = {}\n", self.font.font_size_pt));
        out.push_str(&format!("line_spacing_px = {}\n", self.font.line_spacing_px));
        out.push_str(&format!("letter_spacing_px = {}\n", self.font.letter_spacing_px));
        out.push_str(&format!("antialias = {}\n", self.font.antialias));
        out.push_str(&format!("bold = {}\n", self.font.bold));
        out.push_str(&format!("italic = {}\n\n", self.font.italic));

        // 18. Print
        out.push_str("[Print]\n");
        out.push_str(&format!("header_left = {}\n", self.print.header_left));
        out.push_str(&format!("header_center = {}\n", self.print.header_center));
        out.push_str(&format!("header_right = {}\n", self.print.header_right));
        out.push_str(&format!("footer_left = {}\n", self.print.footer_left));
        out.push_str(&format!("footer_center = {}\n", self.print.footer_center));
        out.push_str(&format!("footer_right = {}\n", self.print.footer_right));
        out.push_str(&format!("margin_top_mm = {}\n", self.print.margin_top_mm));
        out.push_str(&format!("margin_bottom_mm = {}\n", self.print.margin_bottom_mm));
        out.push_str(&format!("margin_left_mm = {}\n", self.print.margin_left_mm));
        out.push_str(&format!("margin_right_mm = {}\n", self.print.margin_right_mm));
        out.push_str(&format!("print_line_numbers = {}\n", self.print.print_line_numbers));
        out.push_str(&format!("orientation = {}\n", self.print.orientation.as_str()));

        out
    }

    /// 从文件加载配置
    pub fn load_file(path: impl AsRef<Path>) -> Result<Self, IniParseError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| IniParseError(format!("Failed to read file: {e}")))?;
        Self::from_ini(&content)
    }

    /// 保存配置到文件 (原子写入)
    pub fn save_file(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = self.to_ini();
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, p)?;
        Ok(())
    }
}
