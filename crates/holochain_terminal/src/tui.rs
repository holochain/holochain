use crate::app::App;
use crate::components::bootstrap::BootstrapWidget;
use crate::components::network_info::NetworkInfoWidget;
use crossterm::{terminal, ExecutableCommand};
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::Backend;
use ratatui::layout::{Alignment, Constraint};
use ratatui::prelude::{Color, Direction, Layout, Line, Style};
use ratatui::symbols::DOT;
use ratatui::widgets::{Block, Tabs};
use ratatui::{Frame, Terminal};
use std::io;
use std::io::stdout;
use std::panic;
use crate::components::home::HomeWidget;

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
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;

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
        .split(frame.area());

    let titles = ["Home", "Network", "Bootstrap"].iter().cloned().map(Line::from);
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
            let home_widget = HomeWidget::new(app.args());
            frame.render_widget(home_widget, root_layout[1]);
        }
        1 => {
            let app_client = app.app_client();
            let network_info_widget = NetworkInfoWidget::new(app.args(), app_client, events);
            frame.render_widget(network_info_widget, root_layout[1]);
        }
        2 => {
            let bootstrap_widget = BootstrapWidget::new(app.args(), events);
            frame.render_widget(bootstrap_widget, root_layout[1]);
        }
        _ => {
            panic!("Page not implemented");
        }
    }
}
