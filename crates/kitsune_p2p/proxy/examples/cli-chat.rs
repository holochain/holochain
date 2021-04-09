use crossterm::ExecutableCommand;
use std::io::Write;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    crossterm::terminal::enable_raw_mode().unwrap();

    tokio::task::spawn_blocking(|| {
        let mut stdout = std::io::stdout();
        loop {
            if crossterm::event::poll(std::time::Duration::from_millis(500))? {
                use crossterm::event::Event::*;
                match crossterm::event::read()? {
                    Key(event) => {
                        if event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL)
                        {
                            if event.code == crossterm::event::KeyCode::Char('c')
                                || event.code == crossterm::event::KeyCode::Char('c')
                                || event.code == crossterm::event::KeyCode::Esc
                            {
                                stdout.execute(crossterm::cursor::MoveTo(0, 0)).unwrap();
                                stdout
                                    .execute(crossterm::terminal::Clear(
                                        crossterm::terminal::ClearType::All,
                                    ))
                                    .unwrap();
                                crossterm::terminal::disable_raw_mode().unwrap();
                                println!("DONE");
                                std::process::exit(0);
                            }
                        }
                        println!("{:?}", event);
                    }
                    Mouse(event) => println!("{:?}", event),
                    Resize(w, h) => println!("resize {}x{}", w, h),
                }
            }
        }
        #[allow(unreachable_code)]
        <crossterm::Result<()>>::Ok(())
    });

    let mut stdout = std::io::stdout();
    stdout
        .execute(crossterm::terminal::Clear(
            crossterm::terminal::ClearType::All,
        ))
        .unwrap();
    stdout.execute(crossterm::cursor::MoveTo(5, 5)).unwrap();
    write!(stdout, "{}", "[---------------------------]").unwrap();
    stdout.flush().unwrap();
    stdout.execute(crossterm::cursor::MoveTo(8, 8)).unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
}
