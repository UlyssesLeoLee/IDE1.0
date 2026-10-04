//! ratatui 渲染 — TTY 终端 backend (与 web backend 并存, 共享 &App 数据)。

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::mode::Mode;

/// 渲染入口 — App 不持有 ratatui handle,所有渲染数据从 &App 拿。
pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // status
            Constraint::Min(3),    // history
            Constraint::Length(3), // prompt
        ])
        .split(area);

    render_status(f, chunks[0], app);
    render_history(f, chunks[1], app);
    render_prompt(f, chunks[2], app);
}

fn render_status(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let mode_color = match app.mode() {
        Mode::Normal => Color::Blue,
        Mode::Insert => Color::Green,
        Mode::Command => Color::Yellow,
    };
    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", app.mode()),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("v{} | {}", env!("CARGO_PKG_VERSION"), app.kernel_banner()),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    let p = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_history(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let items: Vec<ListItem> = app
        .history()
        .iter()
        .map(|line| ListItem::new(Line::from(Span::raw(line.clone()))))
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" history "))
        .style(Style::default().fg(Color::Gray));
    f.render_widget(list, area);
}

fn render_prompt(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let prefix = match app.mode() {
        Mode::Normal => "▌ ",
        Mode::Insert => "> ",
        Mode::Command => ":",
    };
    let buffer = app.active_buffer().as_str();
    let prompt_text = format!("{}{}", prefix, buffer);
    let p =
        Paragraph::new(prompt_text).block(Block::default().borders(Borders::ALL).title(" prompt "));
    f.render_widget(p, area);
}
