//! 集成测试：Sakura 18 页配置数据模型、INI 解析与避头尾禁则体系 (WBS-08).

use editcore::sakura_settings::{
    CustomMenuItem, ExternalCmdSlot, MacroTrigger, PrintOrientation, SakuraCommonSettings,
    TabDoubleClickAction, TabPosition, WrapMode, CUSTOM_MENU_SLOT_COUNT, MACRO_SLOT_COUNT,
    PALETTE_COLOR_COUNT,
};

#[test]
fn test_default_settings_and_to_ini_roundtrip() {
    let default_cfg = SakuraCommonSettings::default();
    let ini_output = default_cfg.to_ini();

    // 验证 18 个段落全部出现在 INI 输出中
    let expected_sections = [
        "[Window]",
        "[TabBar]",
        "[Edit]",
        "[File]",
        "[Search]",
        "[Format]",
        "[Assist]",
        "[Macro]",
        "[StatusBar]",
        "[Plugin]",
        "[Toolbar]",
        "[KeySearch]",
        "[ExecCmd]",
        "[ColorPalette]",
        "[CustomMenu]",
        "[MainMenu]",
        "[Font]",
        "[Print]",
    ];

    for sec in &expected_sections {
        assert!(
            ini_output.contains(sec),
            "INI output must contain section {sec}"
        );
    }

    // 从导出的 INI 字符串解析回来
    let parsed_cfg = SakuraCommonSettings::from_ini(&ini_output)
        .expect("Failed to parse default config to_ini output");

    // 验证往返完全一致
    assert_eq!(default_cfg.window, parsed_cfg.window);
    assert_eq!(default_cfg.tab_bar, parsed_cfg.tab_bar);
    assert_eq!(default_cfg.edit, parsed_cfg.edit);
    assert_eq!(default_cfg.file, parsed_cfg.file);
    assert_eq!(default_cfg.search, parsed_cfg.search);
    assert_eq!(default_cfg.format, parsed_cfg.format);
    assert_eq!(default_cfg.assist, parsed_cfg.assist);
    assert_eq!(default_cfg.macro_cfg, parsed_cfg.macro_cfg);
    assert_eq!(default_cfg.status_bar, parsed_cfg.status_bar);
    assert_eq!(default_cfg.plugin, parsed_cfg.plugin);
    assert_eq!(default_cfg.toolbar, parsed_cfg.toolbar);
    assert_eq!(default_cfg.key_search, parsed_cfg.key_search);
    assert_eq!(default_cfg.exec_cmd, parsed_cfg.exec_cmd);
    assert_eq!(default_cfg.color_palette, parsed_cfg.color_palette);
    assert_eq!(default_cfg.custom_menu, parsed_cfg.custom_menu);
    assert_eq!(default_cfg.main_menu, parsed_cfg.main_menu);
    assert_eq!(default_cfg.font, parsed_cfg.font);
    assert_eq!(default_cfg.print, parsed_cfg.print);
    assert_eq!(default_cfg, parsed_cfg);
}

#[test]
fn test_parse_real_sakura_ini() {
    let real_sakura_ini = r#"
; Sakura Editor Configuration File (Compatibility Mock)
# Header Comment

[Window]
nWinSizeCX=1280
nWinSizeCY=800
nWinPosX=150
nWinPosY=80
bCenterWin=1
bSaveSize=1
bSavePos=1
bAlwaysTop=0
nAlpha=95
bTaskTray=1
bMultiWin=0
bFullScreen=0

[TabBar]
bTabVisible=1
nTabPosition=Bottom
bTabMultiRow=1
bTabCloseBtn=1
nTabDoubleClickAction=NewTab
bCloseMiddle=1
bCloseRight=1
bWheelTab=1
bTabDrag=1
bSameName=1
nTabWidthMax=240
nTabWidthMin=60

[Edit]
nTabWidth=8
nIndentWidth=8
bSpaceForTab=1
bSmartIndent=1
bAutoIndent=1
bAutoSave=1
nAutoSaveInterval=180
bOverWrite=0
bDragDrop=1
bCopyLine=1
bSmoothScroll=1
bFreeCaret=0

[File]
szEncoding=UTF-8
szEol=LF
bFileLock=1
nRecentMax=50
nRecentFoldersMax=30
bAutoReload=1
bAlertDeleted=1
bBackup=1
szBackupFolder=.my-backups
szBackupExt=.bak2
nBackupMax=10
bRestoreCursor=1

[Search]
bCaseSensitive=1
bWholeWord=1
bRegex=1
bLookAround=1
bSearchWrap=1
bSearchBackwards=0
bHighlightAll=1
nSearchHistoryMax=40
nReplaceHistoryMax=40
bAutoCloseDialog=1
bMigemo=1

[Format]
nWrapMode=ByWidth
nWrapColumns=120
bKinsokuHead=1
bKinsokuTail=1
bKinsokuPush=1
szKinsokuHead=、。，！？）〕］｝〉》」』】
szKinsokuTail=（〔［｛〈《「『【
bShowLineNums=1
bShowRuler=0

[Assist]
bAutoComplete=1
nMinTrigger=3
bShowCandidates=1
szDictPath=C:\dict\words.txt
bUseWordsInDoc=1
bMatchParen=1
bAutoCloseBracket=1
nCandidateMax=15

[Macro]
Slot01_Name=FormatCode
Slot01_Path=C:\macros\format.mac
Slot01_Reload=1
Slot01_Trigger=OnSave
Slot01_Params=--mode=release

Slot02_Name=LintDoc
Slot02_Path=C:\macros\lint.mac
Slot02_Reload=0
Slot02_Trigger=OnOpen
Slot02_Params=-v

[StatusBar]
bStatusVisible=1
bCaretPos=1
bCharCode=1
bEncoding=1
bEol=1
bModified=1
bInsOvr=1
bFileSize=1
bZoom=1
bLineCount=1

[Plugin]
bAutoScan=1
szPluginDir=C:\sakura\plugins
EnabledPlugins=git_helper,outline_tree,diff_viewer
nSecurityLevel=1
bAllowNetwork=1

[Toolbar]
bToolbarVisible=1
nButtonSize=24
bFlatStyle=0
bShowTooltips=1
Buttons=new,open,save,separator,undo,redo,separator,find

[KeySearch]
bEnabled=1
szLookupUrl=https://rust-lang.org/search?q=%s
szHotkey=Ctrl+F1
bSearchSelected=1
bExternalBrowser=1

[ExecCmd]
szShell=pwsh.exe
bCaptureOutput=1
bBackground=0
bShowConsole=0
Command_Count=2
Cmd01_Id=1
Cmd01_Name=Cargo Check
Cmd01_Cmd=cargo check
Cmd01_WorkDir=crates/editcore
Cmd01_Stdin=0
Cmd02_Id=2
Cmd02_Name=Git Status
Cmd02_Cmd=git status
Cmd02_WorkDir=.
Cmd02_Stdin=0

[ColorPalette]
szSchemeName=Solarized Dark
Color01_Name=Text
Color01_Fore=#839496
Color01_Back=#002B36
Color01_Bold=0
Color01_Underline=0
Color08_Name=Keyword
Color08_Fore=#859900
Color08_Back=#002B36
Color08_Bold=1
Color08_Underline=0

[CustomMenu]
Menu01_Title=Git Quick Menu
Menu01_ItemCount=2
Menu01_Item01=Status,git status
Menu01_Item02=Diff,git diff

[MainMenu]
bAcceleratorKeys=1
bHideEmptyRecent=1
bShowIcons=1
Customized_Items=File,Edit,Search,View,Tool,Help

[Font]
szFontName=Source Code Pro
nFontSize=12.0
nLineSpacing=4
nLetterSpacing=1
bAntialias=1
bBold=1
bItalic=0

[Print]
szHeaderLeft=&f (page &p)
szHeaderCenter=Confidential
szHeaderRight=&d
szFooterLeft=
szFooterCenter=- &p -
szFooterRight=
nMarginTop=25
nMarginBottom=25
nMarginLeft=20
nMarginRight=20
bPrintLineNums=0
Orientation=Landscape
"#;

    let settings = SakuraCommonSettings::from_ini(real_sakura_ini)
        .expect("Must parse real sakura.ini successfully");

    // 1. Window
    assert_eq!(settings.window.width, 1280);
    assert_eq!(settings.window.height, 800);
    assert_eq!(settings.window.pos_x, 150);
    assert_eq!(settings.window.pos_y, 80);
    assert!(settings.window.center_window);
    assert_eq!(settings.window.opacity, 95);
    assert!(settings.window.minimize_to_tray);
    assert!(!settings.window.multi_window_mode);

    // 2. TabBar
    assert!(settings.tab_bar.visible);
    assert_eq!(settings.tab_bar.position, TabPosition::Bottom);
    assert!(settings.tab_bar.multi_row);
    assert_eq!(settings.tab_bar.double_click_action, TabDoubleClickAction::NewTab);
    assert_eq!(settings.tab_bar.tab_width_max, 240);
    assert_eq!(settings.tab_bar.tab_width_min, 60);

    // 3. Edit
    assert_eq!(settings.edit.tab_width, 8);
    assert_eq!(settings.edit.indent_width, 8);
    assert!(settings.edit.use_spaces_for_tab);
    assert!(settings.edit.smart_indent);
    assert!(settings.edit.auto_save_enabled);
    assert_eq!(settings.edit.auto_save_interval_sec, 180);

    // 4. File
    assert_eq!(settings.file.default_encoding, "UTF-8");
    assert_eq!(settings.file.default_eol, "LF");
    assert!(settings.file.file_lock_exclusive);
    assert_eq!(settings.file.recent_files_max, 50);
    assert_eq!(settings.file.backup_folder, ".my-backups");
    assert_eq!(settings.file.backup_extension, ".bak2");
    assert_eq!(settings.file.backup_count_max, 10);

    // 5. Search
    assert!(settings.search.case_sensitive);
    assert!(settings.search.whole_word);
    assert!(settings.search.regex_enabled);
    assert!(settings.search.lookaround_enabled);
    assert!(settings.search.auto_close_dialog);
    assert!(settings.search.use_migemo);

    // 6. Format
    assert_eq!(settings.format.wrap_mode, WrapMode::ByWidth);
    assert_eq!(settings.format.wrap_columns, 120);
    assert!(settings.format.kinsoku_head_enabled);
    assert!(settings.format.kinsoku_tail_enabled);
    assert!(settings.format.kinsoku_push_enabled);
    assert!(settings.format.kinsoku_chars.is_head_forbidden('、'));
    assert!(settings.format.kinsoku_chars.is_head_forbidden('。'));
    assert!(settings.format.kinsoku_chars.is_tail_forbidden('（'));
    assert!(settings.format.kinsoku_chars.is_tail_forbidden('「'));
    assert!(!settings.format.kinsoku_chars.is_tail_forbidden('。'));
    assert!(!settings.format.show_ruler);

    // 7. Assist
    assert!(settings.assist.auto_complete_enabled);
    assert_eq!(settings.assist.min_char_trigger, 3);
    assert_eq!(settings.assist.dictionary_path, r"C:\dict\words.txt");
    assert!(settings.assist.auto_close_brackets);
    assert_eq!(settings.assist.candidate_max_count, 15);

    // 8. Macro
    assert_eq!(settings.macro_cfg.slots[0].name, "FormatCode");
    assert_eq!(settings.macro_cfg.slots[0].path, r"C:\macros\format.mac");
    assert!(settings.macro_cfg.slots[0].reload_on_run);
    assert_eq!(settings.macro_cfg.slots[0].trigger, MacroTrigger::OnSave);
    assert_eq!(settings.macro_cfg.slots[0].params, "--mode=release");

    assert_eq!(settings.macro_cfg.slots[1].name, "LintDoc");
    assert_eq!(settings.macro_cfg.slots[1].path, r"C:\macros\lint.mac");
    assert!(!settings.macro_cfg.slots[1].reload_on_run);
    assert_eq!(settings.macro_cfg.slots[1].trigger, MacroTrigger::OnOpen);
    assert_eq!(settings.macro_cfg.slots[1].params, "-v");

    // 9. StatusBar
    assert!(settings.status_bar.visible);
    assert!(settings.status_bar.show_caret_pos);
    assert!(settings.status_bar.show_file_size);

    // 10. Plugin
    assert!(settings.plugin.auto_scan_plugins);
    assert_eq!(settings.plugin.plugin_dir, r"C:\sakura\plugins");
    assert_eq!(
        settings.plugin.enabled_plugins,
        vec!["git_helper", "outline_tree", "diff_viewer"]
    );
    assert_eq!(settings.plugin.security_level, 1);
    assert!(settings.plugin.allow_network);

    // 11. Toolbar
    assert!(settings.toolbar.visible);
    assert_eq!(settings.toolbar.button_size, 24);
    assert!(!settings.toolbar.flat_style);
    assert_eq!(
        settings.toolbar.buttons,
        vec!["new", "open", "save", "separator", "undo", "redo", "separator", "find"]
    );

    // 12. KeySearch
    assert!(settings.key_search.enabled);
    assert_eq!(
        settings.key_search.dictionary_lookup_url,
        "https://rust-lang.org/search?q=%s"
    );
    assert_eq!(settings.key_search.hotkey, "Ctrl+F1");

    // 13. ExecCmd
    assert_eq!(settings.exec_cmd.default_shell, "pwsh.exe");
    assert_eq!(settings.exec_cmd.commands.len(), 2);
    assert_eq!(settings.exec_cmd.commands[0].name, "Cargo Check");
    assert_eq!(settings.exec_cmd.commands[0].cmd, "cargo check");
    assert_eq!(settings.exec_cmd.commands[0].work_dir, "crates/editcore");

    // 14. ColorPalette
    assert_eq!(settings.color_palette.scheme_name, "Solarized Dark");
    assert_eq!(settings.color_palette.colors[0].name, "Text");
    assert_eq!(settings.color_palette.colors[0].foreground, "#839496");
    assert_eq!(settings.color_palette.colors[0].background, "#002B36");
    assert_eq!(settings.color_palette.colors[7].name, "Keyword");
    assert_eq!(settings.color_palette.colors[7].foreground, "#859900");
    assert!(settings.color_palette.colors[7].bold);

    // 15. CustomMenu
    assert_eq!(settings.custom_menu.slots[0].title, "Git Quick Menu");
    assert_eq!(settings.custom_menu.slots[0].items.len(), 2);
    assert_eq!(settings.custom_menu.slots[0].items[0].label, "Status");
    assert_eq!(settings.custom_menu.slots[0].items[0].command, "git status");
    assert_eq!(settings.custom_menu.slots[0].items[1].label, "Diff");
    assert_eq!(settings.custom_menu.slots[0].items[1].command, "git diff");

    // 16. MainMenu
    assert!(settings.main_menu.show_accelerator_keys);
    assert!(settings.main_menu.hide_empty_recent);
    assert_eq!(
        settings.main_menu.customized_items,
        vec!["File", "Edit", "Search", "View", "Tool", "Help"]
    );

    // 17. Font
    assert_eq!(settings.font.font_family, "Source Code Pro");
    assert_eq!(settings.font.font_size_pt, 12.0);
    assert_eq!(settings.font.line_spacing_px, 4);
    assert_eq!(settings.font.letter_spacing_px, 1);
    assert!(settings.font.antialias);
    assert!(settings.font.bold);
    assert!(!settings.font.italic);

    // 18. Print
    assert_eq!(settings.print.header_left, "&f (page &p)");
    assert_eq!(settings.print.header_center, "Confidential");
    assert_eq!(settings.print.margin_top_mm, 25);
    assert!(!settings.print.print_line_numbers);
    assert_eq!(settings.print.orientation, PrintOrientation::Landscape);
}

#[test]
fn test_kinsoku_forbidden_characters_and_mutation() {
    let mut kinsoku = editcore::sakura_settings::KinsokuChars::default();

    // 默认禁止检查
    // 行头禁止字符：句号、逗号、感叹号、右括弧等
    assert!(kinsoku.is_head_forbidden('。'));
    assert!(kinsoku.is_head_forbidden('、'));
    assert!(kinsoku.is_head_forbidden('！'));
    assert!(kinsoku.is_head_forbidden('？'));
    assert!(kinsoku.is_head_forbidden('）'));
    assert!(kinsoku.is_head_forbidden('】'));
    assert!(kinsoku.is_head_forbidden('”'));
    assert!(!kinsoku.is_head_forbidden('a'));
    assert!(!kinsoku.is_head_forbidden('中'));

    // 行尾禁止字符：左括弧、左引号等
    assert!(kinsoku.is_tail_forbidden('（'));
    assert!(kinsoku.is_tail_forbidden('【'));
    assert!(kinsoku.is_tail_forbidden('“'));
    assert!(kinsoku.is_tail_forbidden('《'));
    assert!(kinsoku.is_tail_forbidden('['));
    assert!(!kinsoku.is_tail_forbidden('。'));
    assert!(!kinsoku.is_tail_forbidden('z'));

    // 动态添加自定义字符
    kinsoku.add_head_char('@');
    assert!(kinsoku.is_head_forbidden('@'));

    kinsoku.add_tail_char('~');
    assert!(kinsoku.is_tail_forbidden('~'));

    // 动态移除字符
    assert!(kinsoku.remove_head_char('@'));
    assert!(!kinsoku.is_head_forbidden('@'));

    assert!(kinsoku.remove_tail_char('~'));
    assert!(!kinsoku.is_tail_forbidden('~'));

    // 整体重置字符集
    kinsoku.set_head_chars(".,!");
    assert!(kinsoku.is_head_forbidden('.'));
    assert!(kinsoku.is_head_forbidden(','));
    assert!(kinsoku.is_head_forbidden('!'));
    assert!(!kinsoku.is_head_forbidden('。')); // 原来的被替换

    kinsoku.set_tail_chars("([{");
    assert!(kinsoku.is_tail_forbidden('('));
    assert!(kinsoku.is_tail_forbidden('['));
    assert!(kinsoku.is_tail_forbidden('{'));
    assert!(!kinsoku.is_tail_forbidden('（')); // 原来的被替换
}

#[test]
fn test_24_custom_menus_and_20_macro_slots_roundtrip() {
    let mut settings = SakuraCommonSettings::default();

    // 配置全部 24 个自定义菜单槽位
    for i in 1..=CUSTOM_MENU_SLOT_COUNT {
        let slot = &mut settings.custom_menu.slots[i - 1];
        slot.title = format!("Custom Menu Slot {i}");
        slot.items = vec![
            CustomMenuItem {
                label: format!("Action {i}_A"),
                command: format!("cmd_a_{i}"),
            },
            CustomMenuItem {
                label: format!("Action {i}_B"),
                command: format!("cmd_b_{i}"),
            },
        ];
    }

    // 配置全部 20 个宏槽位
    for i in 1..=MACRO_SLOT_COUNT {
        let slot = &mut settings.macro_cfg.slots[i - 1];
        slot.name = format!("Macro Name {i}");
        slot.path = format!("scripts/macro_{i}.mac");
        slot.reload_on_run = (i % 2) == 0;
        slot.trigger = match i % 5 {
            1 => MacroTrigger::OnOpen,
            2 => MacroTrigger::OnSave,
            3 => MacroTrigger::BeforeClose,
            4 => MacroTrigger::OnActivate,
            _ => MacroTrigger::None,
        };
        slot.params = format!("--slot={i} --flag");
    }

    // 序列化为 INI 字符串
    let ini_str = settings.to_ini();

    // 解析回来
    let parsed = SakuraCommonSettings::from_ini(&ini_str)
        .expect("Failed to parse roundtrip custom menu & macro slots");

    // 校验 24 个菜单槽位
    assert_eq!(parsed.custom_menu.slots.len(), CUSTOM_MENU_SLOT_COUNT);
    for i in 1..=CUSTOM_MENU_SLOT_COUNT {
        let orig = &settings.custom_menu.slots[i - 1];
        let restored = &parsed.custom_menu.slots[i - 1];
        assert_eq!(orig.id, restored.id, "Menu id mismatch at slot {i}");
        assert_eq!(orig.title, restored.title, "Menu title mismatch at slot {i}");
        assert_eq!(orig.items.len(), restored.items.len(), "Menu items len mismatch at slot {i}");
        for j in 0..orig.items.len() {
            assert_eq!(orig.items[j], restored.items[j], "Menu item mismatch at slot {i}, item {j}");
        }
    }

    // 校验 20 个宏槽位
    assert_eq!(parsed.macro_cfg.slots.len(), MACRO_SLOT_COUNT);
    for i in 1..=MACRO_SLOT_COUNT {
        let orig = &settings.macro_cfg.slots[i - 1];
        let restored = &parsed.macro_cfg.slots[i - 1];
        assert_eq!(orig.id, restored.id, "Macro id mismatch at slot {i}");
        assert_eq!(orig.name, restored.name, "Macro name mismatch at slot {i}");
        assert_eq!(orig.path, restored.path, "Macro path mismatch at slot {i}");
        assert_eq!(orig.reload_on_run, restored.reload_on_run, "Macro reload mismatch at slot {i}");
        assert_eq!(orig.trigger, restored.trigger, "Macro trigger mismatch at slot {i}");
        assert_eq!(orig.params, restored.params, "Macro params mismatch at slot {i}");
    }

    // 总体相等性验证
    assert_eq!(settings.custom_menu, parsed.custom_menu);
    assert_eq!(settings.macro_cfg, parsed.macro_cfg);
}

#[test]
fn test_24_color_palette_and_external_commands_roundtrip() {
    let mut settings = SakuraCommonSettings::default();
    settings.color_palette.scheme_name = "Nordic Theme".to_string();

    for i in 1..=PALETTE_COLOR_COUNT {
        let c = &mut settings.color_palette.colors[i - 1];
        c.foreground = format!("#{:02X}{:02X}FF", i * 10, i * 5);
        c.background = format!("#2E34{:02X}", (i * 8) % 256);
        c.bold = (i % 3) == 0;
        c.underline = (i % 4) == 0;
    }

    settings.exec_cmd.commands = vec![
        ExternalCmdSlot {
            id: 1,
            name: "Cargo Build".into(),
            cmd: "cargo build --release".into(),
            work_dir: "crates/editcore".into(),
            send_stdin: false,
        },
        ExternalCmdSlot {
            id: 2,
            name: "Run Tests".into(),
            cmd: "cargo test".into(),
            work_dir: ".".into(),
            send_stdin: true,
        },
    ];

    let ini_str = settings.to_ini();
    let parsed = SakuraCommonSettings::from_ini(&ini_str).expect("Parse palette & exec cmds");

    assert_eq!(settings.color_palette, parsed.color_palette);
    assert_eq!(settings.exec_cmd.commands.len(), parsed.exec_cmd.commands.len());
    assert_eq!(settings.exec_cmd.commands[0], parsed.exec_cmd.commands[0]);
    assert_eq!(settings.exec_cmd.commands[1], parsed.exec_cmd.commands[1]);
}

#[test]
fn test_save_and_load_file() {
    let mut settings = SakuraCommonSettings::default();
    settings.window.width = 1920;
    settings.window.height = 1080;
    settings.font.font_family = "JetBrains Mono".to_string();
    settings.font.font_size_pt = 14.0;

    let tmp_dir = std::env::temp_dir().join(format!("sakura-test-{}", std::process::id()));
    let tmp_file = tmp_dir.join("test_sakura.ini");

    settings.save_file(&tmp_file).expect("Must save to file");
    assert!(tmp_file.exists());

    let loaded = SakuraCommonSettings::load_file(&tmp_file).expect("Must load from file");
    assert_eq!(loaded.window.width, 1920);
    assert_eq!(loaded.window.height, 1080);
    assert_eq!(loaded.font.font_family, "JetBrains Mono");
    assert_eq!(loaded.font.font_size_pt, 14.0);

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
