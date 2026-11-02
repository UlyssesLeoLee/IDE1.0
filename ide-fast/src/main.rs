//! IDE1.0 高速版 —— 原生 Rust (egui) 的 Sakura 风格文本编辑器。
//!
//! 为什么单做一个: Tauri 版的界面是网页, Windows 上每次启动都要先拉起 WebView2 (实测约 0.7–1.2 s, 不在我们代码里)。
//! 这一版不用 WebView: 进程一起来就直接建原生窗口 + OpenGL 上下文, 编辑 / 移动 / 查找的语义仍然全部走 `editcore`
//! (和 Tauri 版同一套 Rust 代码), 所以 Sakura 的键位行为一致。Tauri 版继续保留, 作为移植到别的 agent / 网页版的基底。
//!
//! 启动耗时追踪: 设环境变量 `IDE_FAST_TRACE=1`, 在 stderr 打印 "进程入口 -> 窗口创建 -> 首帧" 各点的毫秒数。

mod app;
mod doc;
mod fonts;
mod settings;

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

fn main() -> eframe::Result<()> {
    let trace = Trace::new();
    trace.mark("main enter");
    let files: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("IDE1.0 Fast")
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([480.0, 320.0]),
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
            fonts::install(&cc.egui_ctx);
            trace.mark("fonts installed");
            Ok(Box::new(app::App::new(files, trace)))
        }),
    )
}
