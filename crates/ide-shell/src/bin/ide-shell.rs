//! IDE1.0 IDE Shell demo binary.
//!
//! Stage 3.0 brief v0.1 — 最小 demo: PowerShell-like prompt + Vim normal/insert
//! + 鼠标 + AI placeholder. 跑 `cargo run -p ide-shell` 启动。

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ide_shell::{App, AppConfig};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

fn main() -> io::Result<()> {
    let mut stdout = setup_terminal()?;
    let result = run(&mut stdout);
    restore_terminal(&mut stdout)?;
    result
}

fn setup_terminal() -> io::Result<Stdout> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    Ok(stdout)
}

fn restore_terminal(stdout: &mut Stdout) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;
    Ok(())
}

fn run(stdout: &mut Stdout) -> io::Result<()> {
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(AppConfig::default());

    loop {
        terminal.draw(|f| ide_shell::render::ratatui_render::render(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            let evt = event::read()?;
            match evt {
                Event::Key(k) => {
                    app.on_key(k);
                }
                Event::Mouse(m) => {
                    app.on_mouse(m);
                }
                _ => {}
            }
            if app.should_quit() {
                return Ok(());
            }
        } else {
            app.tick();
        }
    }
}
