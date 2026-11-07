//! IDE1.0 高速版 —— 原生 Rust (egui) 的 Sakura 风格文本编辑器。
//!
//! 为什么单做一个: Tauri 版的界面是网页, Windows 上每次启动都要先拉起 WebView2 (实测约 0.7–1.2 s, 不在我们代码里)。
//! 这一版不用 WebView: 进程一起来就直接建原生窗口 + OpenGL 上下文, 编辑 / 移动 / 查找的语义仍然全部走 `editcore`
//! (和 Tauri 版同一套 Rust 代码), 所以 Sakura 的键位行为一致。Tauri 版继续保留, 作为移植到别的 agent / 网页版的基底。
//!
//! 启动耗时追踪: 设环境变量 `IDE_FAST_TRACE=1`, 在 stderr 打印 "进程入口 -> 窗口创建 -> 首帧" 各点的毫秒数。

// Windows 发行版走 GUI 子系统: 否则双击启动会多出一个控制台窗口 (调试构建保留控制台, 方便看 IDE_FAST_TRACE 输出)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "256"]
#![allow(
    deprecated,
    dead_code,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::incompatible_msrv,
    clippy::enum_variant_names,
    clippy::field_reassign_with_default,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::assertions_on_constants
)]

mod ai;
mod app;
mod appicon;
mod backup;
mod btnstyle;
mod ckbox;
mod cmds;
mod cmdui;
mod complete;
mod curlx;
mod diff;
mod doc;
mod find;
mod fonts;
mod fsx;
mod ghplugin;
mod glassfx;
mod grep;
mod help;
mod i18n;
mod icons;
mod instance;
mod keymap;
mod kinsoku;
mod macros;
mod markdown;
mod md_view;
mod mdtools;
mod msgtr;
mod panels;
mod pick;
mod plugin_host;
mod plugin_ui;
mod plugins;
mod prefs;
mod print;
mod relaunch;
mod rowview;
mod scm;
mod scrollx;
mod settings;
mod sidebar;
mod split;
mod svg_lite;
mod svgicon;
mod tabs;
mod tagjump;
mod terminal;
mod textconv;
mod textdrag;
mod tools;
mod ui_ext;
mod updapply;
mod update;
mod updkey;
mod updnet;
mod view;

use std::time::Instant;

/// 启动耗时追踪。`IDE_FAST_TRACE=1` 打到 stderr; 值是文件路径 (含 `/` `\` 或 `.`) 就追加写到该文件
/// (发行版是 GUI 子系统没有控制台, 基准脚本 `tests/perf.ps1` 用文件)。不设变量时 `mark` 只是一次布尔判断。
#[derive(Clone)]
pub struct Trace {
    t0: Instant,
    on: bool,
    file: Option<std::path::PathBuf>,
}
impl Trace {
    pub fn new() -> Self {
        let v = std::env::var_os("IDE_FAST_TRACE");
        let file = v
            .as_ref()
            .filter(|s| s.to_string_lossy().contains(['/', '\\', '.']))
            .map(std::path::PathBuf::from);
        Trace {
            t0: Instant::now(),
            on: v.is_some(),
            file,
        }
    }
}
impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}
impl Trace {
    pub fn mark(&self, what: &str) {
        if !self.on {
            return;
        }
        // 微秒精度: 取整到毫秒会把 <1ms 的阶段吞掉
        let line = format!(
            "[fast +{:.1}ms] {what}",
            self.t0.elapsed().as_secs_f64() * 1000.0
        );
        match &self.file {
            Some(p) => {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(p)
                {
                    let _ = writeln!(f, "{line}");
                }
            }
            None => eprintln!("{line}"),
        }
    }
}

/// 命令行里的 `--new`: 取出并告诉调用方 (其余原样当文件参数)
fn split_new_flag(args: Vec<String>) -> (bool, Vec<String>) {
    let new = args.iter().any(|a| a == "--new");
    (new, args.into_iter().filter(|a| a != "--new").collect())
}

fn main() -> eframe::Result<()> {
    let trace = Trace::new();
    trace.mark("main enter");
    // 便携版自更新的帮手模式: 本进程是校验过的 `<exe>.new`, 只负责覆盖旧 exe 并启动新版 (不开窗口、不碰单实例)
    if std::env::args().nth(1).as_deref() == Some(updapply::APPLY_FLAG) {
        updapply::run_apply();
        return Ok(());
    }
    let (force_new, args) = split_new_flag(std::env::args().skip(1).collect());
    // `--relaunch` = 「立即重启」拉起来的新进程: 旧进程这时正在退出, 不能把文件转交给它
    let (relaunched, args) = relaunch::take_flag(args);
    // 单实例: 已经有一个在运行就把文件交给它, 自己退出 (`--new` = 就是要一个独立的新窗口, 也不去抢监听端口)
    if !force_new && !relaunched && instance::forward_to_running(&args) {
        trace.mark("handed over to the running instance");
        return Ok(());
    }
    // 上次的窗口位置 / 大小 / 最大化状态 (没有就用默认的 1100x720)
    let st = settings::Settings::load();
    trace.mark("settings loaded");
    let font_file = if st.font_family.is_empty() {
        fonts::theme_default_font(&st.theme_plugin).unwrap_or("").to_string()
    } else {
        st.font_family.clone()
    };
    // 字体文件在后台双线程并行读 (主字体 + 核心 CJK fallback, 按需加载粗体), 和下面窗口 / OpenGL 的创建 (约 230 ms) 并行
    let fonts_job = fonts::prefetch_ext(&font_file, &st.font_cjk_fallback, &st.lang, st.font_bold);
    let inbox = if force_new { None } else { instance::listen() };
    // 半透明 (玻璃) 主题: 窗口必须在创建时就是透明的; 只有保存的主题是半透明的才付这个代价 (没选主题插件时不读插件仓库)
    let glass = plugins::is_translucent(&st.theme_plugin);
    let blur = glass && st.glass_blur;
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_visible(true)
        .with_title("IDE1.0 Fast")
        .with_inner_size([1100.0, 720.0])
        .with_min_inner_size([480.0, 320.0])
        .with_decorations(st.system_title_bar); // 默认自绘标题栏 (app/titlebar.rs); 设置里可换回系统的
    if let Some(icon) = appicon::window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    if let Some(g) = st.view.geom {
        viewport = viewport
            .with_inner_size([g.w, g.h])
            .with_position([g.x, g.y])
            .with_maximized(g.maximized);
    }
    let options = eframe::NativeOptions {
        viewport: viewport.with_transparent(true), // 一律透明窗口: 运行中切到玻璃主题才不用重启 (清屏色在不透明主题下是不透明的)
        // 不画抗锯齿多重采样、不要深度 / 模板缓冲: 文本编辑器用不上, 少一点 GPU 初始化
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        persist_window: false,
        // 细分「进程入口 -> 窗口就绪」的几百毫秒: 事件循环建好 / 窗口即将创建 (winit 窗口 + GL 上下文都在这之后、`window + GL context ready` 之前)
        event_loop_builder: Some({
            let t = trace.clone();
            Box::new(move |_| t.mark("event loop builder"))
        }),
        window_builder: Some({
            let t = trace.clone();
            Box::new(move |vb| {
                t.mark("window builder (window + GL creation starts)");
                vb
            })
        }),
        ..Default::default()
    };
    trace.mark("before run_native");
    let result = eframe::run_native(
        "IDE1.0 Fast",
        options,
        Box::new(move |cc| {
            trace.mark("window + GL context ready");
            match fonts_job.map(|h| h.join()) {
                Some(Ok(loaded)) => fonts::install_loaded(&cc.egui_ctx, loaded),
                _ => fonts::install(&cc.egui_ctx, &font_file, &st.font_cjk_fallback, &st.lang), // 线程没起来 / 读取线程 panic: 同步读
            }
            trace.mark("fonts installed");
            #[cfg(windows)]
            if blur {
                // 窗口背后加 DWM 模糊; 失败 (旧系统 / 驱动) 就只剩 alpha 叠色, 不影响使用
                let _ = glassfx::apply(cc);
            }
            #[cfg(not(windows))]
            let _ = blur;
            let t = trace.clone();
            // 直接复用已读出的 st, 避免在 App::new 内部重复读盘解析 settings.json (启动提速)
            let app = app::App::with_settings(args, inbox, trace, st);
            t.mark("App::new done");
            Ok(Box::new(app))
        }),
    );
    // 「安装并重启」(自更新): 窗口已经关完、设置已经落盘; msi 交给 msiexec, 便携版运行 `<exe>.new` 完成替换
    if let Some(a) = updapply::take() {
        let _ = updapply::launch(&a);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::split_new_flag;

    #[test]
    fn new_flag_is_taken_out_of_the_file_args() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            split_new_flag(v(&["--new", "a.txt"])),
            (true, v(&["a.txt"]))
        );
        assert_eq!(split_new_flag(v(&["a.txt"])), (false, v(&["a.txt"])));
    }
}
