mod app;
mod cli;
mod components;
mod event;
mod tui;

use crate::app::App;
use crate::cli::Args;
use crate::event::handle_events;
use crate::tui::Tui;
use clap::Parser;
use ratatui::prelude::*;
use std::io::{self};

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    args.validate()?;

    let mut app = App::new(args, 2);

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
