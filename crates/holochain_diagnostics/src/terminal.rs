use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use tui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

pub fn tui_crossterm_setup<
    T,
    F: FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<T>,
>(
    run: F,
) -> anyhow::Result<T> {
    // setup terminal
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    enter_tui(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = run(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    exit_tui(terminal.backend_mut())?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen, // DisableMouseCapture
    )?;

    terminal.show_cursor()?;

    if let Err(ref err) = res {
        println!("{:?}", err)
    }

    Ok(res?)
}

pub fn enter_tui(stdout: &mut io::Stdout) -> Result<(), crossterm::ErrorKind> {
    execute!(stdout, EnterAlternateScreen /* , EnableMouseCapture */)
}

pub fn exit_tui<B: Backend + io::Write>(backend: &mut B) -> Result<(), crossterm::ErrorKind> {
    execute!(backend, LeaveAlternateScreen)
}
