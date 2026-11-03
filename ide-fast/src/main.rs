//! IDE1.0 高速版 —— 原生 Rust (egui) 的 Sakura 风格文本编辑器。
//!
//! 为什么单做一个: Tauri 版的界面是网页, Windows 上每次启动都要先拉起 WebView2 (实测约 0.7–1.2 s, 不在我们代码里)。
//! 这一版不用 WebView: 进程一起来就直接建原生窗口 + OpenGL 上下文, 编辑 / 移动 / 查找的语义仍然全部走 `editcore`
//! (和 Tauri 版同一套 Rust 代码), 所以 Sakura 的键位行为一致。Tauri 版继续保留, 作为移植到别的 agent / 网页版的基底。
//!
//! 启动耗时追踪: 设环境变量 `IDE_FAST_TRACE=1`, 在 stderr 打印 "进程入口 -> 窗口创建 -> 首帧" 各点的毫秒数。

// Windows 发行版走 GUI 子系统: 否则双击启动会多出一个控制台窗口 (调试构建保留控制台, 方便看 IDE_FAST_TRACE 输出)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod doc;
mod find;
mod grep;
mod i18n;
mod prefs;
mod cmds;
mod keymap;
mod cmdui;
mod fonts;
mod instance;
mod plugins;
mod plugin_host;
mod plugin_ui;
mod svg_lite;
mod print;
mod settings;
mod macros;
mod diff;
mod markdown;
mod md_view;
mod scm;
mod terminal;
mod sidebar;
mod tabs;
mod view;
mod textconv;
mod tools;
mod tagjump;
mod kinsoku;
mod textdrag;
mod backup;
mod ghplugin;
mod ai;
mod fsx;
mod curlx;
mod split;
mod appicon;
mod mdtools;

use std::time::Instant;

pub struct Trace {
    t0: Instant,
    on: bool,
}
impl Trace {
    pub fn new() -> Self {
        Trace { t0: Instant::now(), on: std::env::var_os("IDE_FAST_TRACE").is_some() }
    }
    pub fn mark(&self, what: &str) {
        if self.on {
            eprintln!("[fast +{}ms] {what}", self.t0.elapsed().as_millis());
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
    let (force_new, args) = split_new_flag(std::env::args().skip(1).collect());
    // 单实例: 已经有一个在运行就把文件交给它, 自己退出 (`--new` = 就是要一个独立的新窗口, 也不去抢监听端口)
    if !force_new && instance::forward_to_running(&args) {
        trace.mark("handed over to the running instance");
        return Ok(());
    }
    let inbox = if force_new { None } else { instance::listen() };
    // 上次的窗口位置 / 大小 / 最大化状态 (没有就用默认的 1100x720)
    let st = settings::Settings::load();
    let font_file = st.font_family.clone();
    // 半透明 (玻璃) 主题: 窗口必须在创建时就是透明的; 只有保存的主题是半透明的才付这个代价 (没选主题插件时不读插件仓库)
    let glass = plugins::is_translucent(&st.theme_plugin);
    let blur = glass && st.glass_blur;
    let mut viewport = eframe::egui::ViewportBuilder::default().with_title("IDE1.0 Fast").with_inner_size([1100.0, 720.0]).with_min_inner_size([480.0, 320.0]);
    if let Some(icon) = appicon::window_icon() { viewport = viewport.with_icon(std::sync::Arc::new(icon)); }
    if let Some(g) = st.view.geom {
        viewport = viewport.with_inner_size([g.w, g.h]).with_position([g.x, g.y]).with_maximized(g.maximized);
    }
    let options = eframe::NativeOptions {
        viewport: viewport.with_transparent(glass),
        // 不画抗锯齿多重采样、不要深度 / 模板缓冲: 文本编辑器用不上, 少一点 GPU 初始化
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "IDE1.0 Fast",
        options,
        Box::new(move |cc| {
            trace.mark("window + GL context ready");
            fonts::install(&cc.egui_ctx, &font_file);
            trace.mark("fonts installed");
            #[cfg(windows)]
            if blur {
                // 窗口背后加 DWM 模糊; 失败 (旧系统 / 驱动) 就只剩 alpha 叠色, 不影响使用
                let _ = window_vibrancy::apply_blur(cc, Some((0, 0, 0, 1)));
            }
            #[cfg(not(windows))]
            let _ = blur;
            Ok(Box::new(app::App::new(args, inbox, trace)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::split_new_flag;

    #[test]
    fn new_flag_is_taken_out_of_the_file_args() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(split_new_flag(v(&["--new", "a.txt"])), (true, v(&["a.txt"])));
        assert_eq!(split_new_flag(v(&["a.txt"])), (false, v(&["a.txt"])));
    }
}
