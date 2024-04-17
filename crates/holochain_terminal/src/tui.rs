use crate::app::App;
use crate::components::bootstrap::render_bootstrap_widget;
use crate::components::network_info::render_network_info_widget;
use crossterm::terminal;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::Backend;
use ratatui::layout::{Alignment, Constraint};
use ratatui::prelude::{Color, Direction, Layout, Line, Style};
use ratatui::symbols::DOT;
use ratatui::widgets::{Block, Tabs};
use ratatui::{Frame, Terminal};
use std::io;
use std::panic;

#[derive(Debug)]
pub struct Tui<B: Backend> {
    /// Interface to the Terminal.
    terminal: Terminal<B>,
}

impl<B: Backend> Tui<B> {
    /// Constructs a new instance of [`Tui`].
    pub fn new(terminal: Terminal<B>) -> Self {
        Self { terminal }
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        terminal::enable_raw_mode()?;
        crossterm::execute!(io::stderr(), EnterAlternateScreen)?;

        // Define a custom panic hook to reset the terminal properties.
        // This way, you won't have your terminal messed up if an unexpected error happens.
        let panic_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic| {
            Self::reset().expect("failed to reset the terminal");
            panic_hook(panic);
        }));

        self.terminal.hide_cursor()?;
        self.terminal.clear()?;
        Ok(())
    }

    /// Draw the terminal interface by [`rendering`] the widgets.
    pub fn draw(&mut self, app: &mut App) -> anyhow::Result<()> {
        self.terminal.draw(|frame| render(app, frame))?;
        Ok(())
    }

    /// Resets the terminal interface.
    fn reset() -> anyhow::Result<()> {
        terminal::disable_raw_mode()?;
        crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
        Ok(())
    }

    /// Exits the terminal interface.
    pub fn exit(&mut self) -> anyhow::Result<()> {
        Self::reset()?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

fn render(app: &mut App, frame: &mut Frame) {
    let root_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(frame.size());

    let titles = ["Network", "Bootstrap"].iter().cloned().map(Line::from);
    let tabs = Tabs::new(titles)
        .select(app.tab_index())
        .block(
            Block::default()
                .title("Holochain terminal")
                .title_alignment(Alignment::Center),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Blue))
        .divider(DOT);

    frame.render_widget(tabs, root_layout[0]);

    let events = app.drain_events();

    match app.tab_index() {
        0 => {
            let app_client = app.app_client();
            render_network_info_widget(app.args(), app_client, events, frame, root_layout[1]);
        }
        1 => {
            render_bootstrap_widget(app.args(), events, frame, root_layout[1]);
        }
        _ => {
            panic!("Page not implemented");
        }
    }
}
