mod app;
mod tui;

use crate::app::App;
use crate::tui::Tui;
use crossterm::{
    event::{self, Event, KeyCode},
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};
use std::io::{self};

fn main() -> anyhow::Result<()> {
    let mut app = App::new(2);

    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let mut tui = Tui::new(terminal);
    tui.init()?;

    while app.is_running() {
        tui.draw(&mut app)?;
        handle_events(&mut app)?;
    }

    tui.exit()?;
    Ok(())
}

fn handle_events(app: &mut App) -> anyhow::Result<()> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press && key.code == KeyCode::Tab {
                app.incr_tab_index();
            } else if key.kind == event::KeyEventKind::Press
                && (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
            {
                app.stop();
            }
        }
    }
    Ok(())
}
