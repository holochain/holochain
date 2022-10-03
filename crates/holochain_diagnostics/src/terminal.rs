use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use tui::{backend::CrosstermBackend, Terminal};

pub fn tui_crossterm_setup<
    T,
    F: FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<T>,
>(
    run: F,
) -> io::Result<T> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen /* , EnableMouseCapture */)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = run(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen // DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(ref err) = res {
        println!("{:?}", err)
    }

    res
}
