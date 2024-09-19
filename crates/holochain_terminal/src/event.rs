use crate::app::App;
use crossterm::event::{self, Event, KeyCode};

#[derive(Debug)]
pub enum ScreenEvent {
    Refresh,
    SwitchNetwork,
    NavDown,
    NavUp,
}

pub fn handle_events(app: &mut App) -> anyhow::Result<()> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == event::KeyEventKind::Press {
                if key.code == KeyCode::Tab {
                    app.incr_tab_index();
                } else if key.code == KeyCode::BackTab {
                    app.decr_tab_index();
                } else if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    app.stop();
                } else if key.code == KeyCode::Char('r') {
                    app.push_event(ScreenEvent::Refresh);
                } else if key.code == KeyCode::Char('n') {
                    app.push_event(ScreenEvent::SwitchNetwork);
                } else if key.code == KeyCode::Down {
                    app.push_event(ScreenEvent::NavDown)
                } else if key.code == KeyCode::Up {
                    app.push_event(ScreenEvent::NavUp)
                }
            }
        }
    }
    Ok(())
}
