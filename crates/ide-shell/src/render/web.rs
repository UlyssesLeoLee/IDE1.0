//! Web render backend — 返回结构化 `WebFrame` 数据 (Playwright e2e 友好).
//!
//! 设计要点:
//! - 不依赖 ratatui / crossterm,纯数据 → 便于序列化 (JSON) + 测试
//! - 与 ratatui_render 共享同一份 &App 数据,保证 visual parity
//! - Playwright 通过 HTTP /api/frame 拿 WebFrame JSON,然后驱动 DOM
//!
//! 输出模型 (per §3 brief 双外观):
//! - 3 段垂直布局: status (3 行) / history (N 行) / prompt (3 行)
//! - 每行 = Vec<WebCell { ch, style: WebStyle }>
//! - WebStyle 仅描述视觉属性,不绑定 CSS (前端自由映射)

use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::mode::Mode;

/// 单个字符 + 视觉属性。Playwright 渲染时映射成 DOM <span class="..." style="...">。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebCell {
    pub ch: char,
    pub style: WebStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WebStyle {
    /// "normal" / "bold"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    /// "blue" / "green" / "yellow" / "gray" / "dark_gray" / "black"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
}

impl WebStyle {
    pub fn bold() -> Self {
        Self {
            weight: Some("bold".into()),
            ..Default::default()
        }
    }

    pub fn fg(color: &str) -> Self {
        Self {
            fg: Some(color.into()),
            ..Default::default()
        }
    }

    pub fn bg(color: &str) -> Self {
        Self {
            bg: Some(color.into()),
            ..Default::default()
        }
    }
}

/// 一个 WebFrame = 三段布局,每段是若干行,每行是若干 WebCell。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebFrame {
    /// 顶部 status 段 (mode badge + kernel banner)
    pub status: Vec<Vec<WebCell>>,
    /// 中部 history 段 (已执行命令的输出)
    pub history: Vec<Vec<WebCell>>,
    /// 底部 prompt 段 (prefix + buffer)
    pub prompt: Vec<Vec<WebCell>>,
    /// 当前 mode — Playwright 用 `[data-mode="..."]` selector 验证
    pub mode: Mode,
    /// 当前编辑 buffer 内容 (Insert/Normal) — Playwright 用 `[data-buffer="..."]` 验证
    pub buffer: String,
    /// 当前活动 buffer 的 cursor (Command mode = cmd_buffer cursor, 其余 = 编辑 buffer cursor)
    pub cursor: usize,
    /// Command mode 专用 cmd buffer (`:` 后输入的内部命令名)
    pub cmd_buffer: String,
}

const FRAME_WIDTH: usize = 80;
const STATUS_HEIGHT: usize = 3;
const PROMPT_HEIGHT: usize = 3;
const PADDING_CHAR: char = ' ';

fn pad_row(mut row: Vec<WebCell>, width: usize) -> Vec<WebCell> {
    while row.len() < width {
        row.push(WebCell {
            ch: PADDING_CHAR,
            style: WebStyle::default(),
        });
    }
    row.truncate(width);
    row
}

fn border_row(width: usize, title: &str) -> Vec<WebCell> {
    // 简易 box-drawing: ── status ──────...
    let mut row: Vec<WebCell> = Vec::with_capacity(width);
    row.push(WebCell {
        ch: '─',
        style: WebStyle::default(),
    });
    row.push(WebCell {
        ch: ' ',
        style: WebStyle::default(),
    });
    for c in title.chars() {
        if row.len() >= width - 2 {
            break;
        }
        row.push(WebCell {
            ch: c,
            style: WebStyle::fg("dark_gray"),
        });
    }
    row.push(WebCell {
        ch: ' ',
        style: WebStyle::default(),
    });
    while row.len() < width - 1 {
        row.push(WebCell {
            ch: '─',
            style: WebStyle::default(),
        });
    }
    if row.len() < width {
        row.push(WebCell {
            ch: '─',
            style: WebStyle::default(),
        });
    }
    row
}

fn render_status_web(app: &App) -> Vec<Vec<WebCell>> {
    let (badge_bg, badge_text) = match app.mode() {
        Mode::Normal => ("blue", "NORMAL"),
        Mode::Insert => ("green", "INSERT"),
        Mode::Command => ("yellow", "COMMAND"),
    };

    // 行 1: 边框 ── status ──...
    let mut rows = Vec::with_capacity(STATUS_HEIGHT);
    rows.push(border_row(FRAME_WIDTH, "status"));

    // 行 2: 实际内容 — mode badge (反色 bold) + 空格 + kernel banner (gray)
    let mut content: Vec<WebCell> = Vec::with_capacity(FRAME_WIDTH);
    for ch in format!(" {} ", badge_text).chars() {
        content.push(WebCell {
            ch,
            style: WebStyle {
                weight: Some("bold".into()),
                fg: Some("black".into()),
                bg: Some(badge_bg.into()),
            },
        });
    }
    content.push(WebCell {
        ch: ' ',
        style: WebStyle::default(),
    });
    content.push(WebCell {
        ch: ' ',
        style: WebStyle::default(),
    });
    let banner = format!("v{} | {}", env!("CARGO_PKG_VERSION"), app.kernel_banner());
    for ch in banner.chars() {
        if content.len() >= FRAME_WIDTH {
            break;
        }
        content.push(WebCell {
            ch,
            style: WebStyle::fg("dark_gray"),
        });
    }
    rows.push(pad_row(content, FRAME_WIDTH));

    // 行 3: 底边
    rows.push(border_row(FRAME_WIDTH, ""));

    rows
}

fn render_history_web(app: &App) -> Vec<Vec<WebCell>> {
    let mut rows = Vec::new();
    rows.push(border_row(FRAME_WIDTH, "history"));
    for line in app.history() {
        let mut row: Vec<WebCell> = Vec::with_capacity(FRAME_WIDTH);
        for ch in line.chars() {
            if row.len() >= FRAME_WIDTH {
                break;
            }
            row.push(WebCell {
                ch,
                style: WebStyle::fg("gray"),
            });
        }
        rows.push(pad_row(row, FRAME_WIDTH));
    }
    if app.history().is_empty() {
        let row: Vec<WebCell> = (0..FRAME_WIDTH)
            .map(|_| WebCell {
                ch: PADDING_CHAR,
                style: WebStyle::default(),
            })
            .collect();
        rows.push(row);
    }
    rows.push(border_row(FRAME_WIDTH, ""));
    rows
}

fn render_prompt_web(app: &App) -> Vec<Vec<WebCell>> {
    let prefix = match app.mode() {
        Mode::Normal => "▌ ",
        Mode::Insert => "> ",
        Mode::Command => ":",
    };
    let buffer = app.active_buffer().as_str();
    let prompt_text = format!("{}{}", prefix, buffer);

    let mut rows = Vec::with_capacity(PROMPT_HEIGHT);
    rows.push(border_row(FRAME_WIDTH, "prompt"));

    let mut content: Vec<WebCell> = Vec::with_capacity(FRAME_WIDTH);
    for ch in prompt_text.chars() {
        if content.len() >= FRAME_WIDTH {
            break;
        }
        content.push(WebCell {
            ch,
            style: WebStyle::default(),
        });
    }
    rows.push(pad_row(content, FRAME_WIDTH));
    rows.push(border_row(FRAME_WIDTH, ""));
    rows
}

/// 渲染入口 — 返回完整 WebFrame,与 ratatui_render::render 视觉等价。
pub fn render(app: &App) -> WebFrame {
    WebFrame {
        status: render_status_web(app),
        history: render_history_web(app),
        prompt: render_prompt_web(app),
        mode: app.mode(),
        buffer: app.buffer().as_str(),
        cursor: app.active_buffer().cursor(),
        cmd_buffer: app.cmd_buffer().as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppConfig;

    fn app() -> App {
        App::new(AppConfig::default())
    }

    #[test]
    fn test_render_initial_frame() {
        let a = app();
        let frame = render(&a);
        assert_eq!(frame.mode, Mode::Normal);
        assert_eq!(frame.buffer, "");
        assert_eq!(frame.cursor, 0);
        // 3 段每段 ≥ 1 行
        assert!(!frame.status.is_empty());
        assert!(!frame.history.is_empty());
        assert!(!frame.prompt.is_empty());
        // status 第一行包含 "status" 标题
        let status_text: String = frame.status[0].iter().map(|c| c.ch).collect();
        assert!(status_text.contains("status"), "got: {status_text}");
    }

    #[test]
    fn test_render_mode_color_mapping() {
        let mut a = app();
        a.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('i'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let frame = render(&a);
        assert_eq!(frame.mode, Mode::Insert);
        // INSERT badge 在 status[1] (content 行) — 验 background color
        let content = &frame.status[1];
        let has_green_bg = content
            .iter()
            .any(|c| c.style.bg.as_deref() == Some("green"));
        assert!(has_green_bg, "INSERT mode should have green background");
    }

    #[test]
    fn test_render_buffer_in_prompt() {
        let mut a = app();
        a.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('i'),
            crossterm::event::KeyModifiers::NONE,
        ));
        for c in "hello".chars() {
            a.on_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(c),
                crossterm::event::KeyModifiers::NONE,
            ));
        }
        let frame = render(&a);
        assert_eq!(frame.buffer, "hello");
        let prompt_text: String = frame.prompt[1].iter().map(|c| c.ch).collect();
        assert!(
            prompt_text.contains("> hello"),
            "prompt should contain prefix + buffer, got: {prompt_text:?}"
        );
    }

    #[test]
    fn test_render_history_after_command() {
        let mut a = app();
        a.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(':'),
            crossterm::event::KeyModifiers::NONE,
        ));
        for c in "version".chars() {
            a.on_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(c),
                crossterm::event::KeyModifiers::NONE,
            ));
        }
        a.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        let frame = render(&a);
        assert!(!frame.history.is_empty(), "history should not be empty");
        let hist_text: String = frame
            .history
            .iter()
            .flatten()
            .map(|c| c.ch)
            .collect::<String>()
            .trim()
            .to_string();
        assert!(
            hist_text.contains("ide-kernel-core"),
            "history should contain kernel banner, got: {hist_text}"
        );
    }

    #[test]
    fn test_render_command_mode_yellow_badge() {
        let mut a = app();
        a.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(':'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let frame = render(&a);
        assert_eq!(frame.mode, Mode::Command);
        let content = &frame.status[1];
        let has_yellow_bg = content
            .iter()
            .any(|c| c.style.bg.as_deref() == Some("yellow"));
        assert!(has_yellow_bg, "COMMAND mode should have yellow background");
    }

    #[test]
    fn test_pad_row_truncates_long_input() {
        let row = vec![
            WebCell {
                ch: 'x',
                style: WebStyle::default(),
            };
            100
        ];
        let padded = pad_row(row, 50);
        assert_eq!(padded.len(), 50);
    }

    #[test]
    fn test_pad_row_pads_short_input() {
        let row = vec![
            WebCell {
                ch: 'x',
                style: WebStyle::default(),
            };
            5
        ];
        let padded = pad_row(row, 10);
        assert_eq!(padded.len(), 10);
        assert_eq!(padded[5].ch, ' ');
    }

    #[test]
    fn test_border_row_contains_title() {
        let row = border_row(40, "test");
        let text: String = row.iter().map(|c| c.ch).collect();
        assert!(text.contains("test"));
    }
}
