use crossterm::event::{self, Event, KeyCode};
use holochain_diagnostics::{
    holo_hash::ActionHash,
    holochain::{conductor::conductor::RwShare, sweettest::SweetConductorBatch, sweettest::*},
    seeded_rng, standard_config, tui_crossterm_setup, Rng, *,
};
use std::{
    error::Error,
    io::{self, Write},
    iter,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tui::{
    backend::Backend,
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Cell, Row, Table},
    Frame, Terminal,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = setup_app().await;

    task_commit(app.clone());

    tui_crossterm_setup(|t| run_app(t, app))?;

    // // let get_task = task_get();
    // let commit_task = task_commit();

    Ok(())
}

const NODES: usize = 10;
const BASES: usize = 4;
static FLUSH: AtomicBool = AtomicBool::new(true);

#[derive(Clone)]
struct App {
    state: RwShare<State>,
    conductors: Arc<SweetConductorBatch>,
    zomes: [SweetZome; NODES],
    bases: [AnyLinkableHash; BASES],
}

struct State {
    commits: [usize; BASES],
    counts: [[(usize, Instant); BASES]; NODES],
}

async fn setup_app() -> App {
    assert!(BASES <= NODES);
    let config = standard_config();

    let start = Instant::now();

    let mut conductors = SweetConductorBatch::from_config(NODES, config).await;
    println!("Conductors created (t={:3.1?}).", start.elapsed());

    let (dna, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("zome", diagnostic_tests::basic_zome())).await;
    let apps = conductors.setup_app("basic", &[dna]).await.unwrap();
    let cells = apps.cells_flattened().clone();
    println!("Apps setup (t={:3.1?}).", start.elapsed());

    let zomes = cells
        .iter()
        .map(|c| c.zome("zome"))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let now = Instant::now();
    let commits = [0; BASES];
    let counts = [[(0, now); BASES]; NODES];
    let bases = cells
        .iter()
        .take(BASES)
        .map(|c| c.agent_pubkey().clone().into())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    App {
        conductors: Arc::new(conductors),
        zomes,
        bases,
        state: RwShare::new(State { commits, counts }),
    }
}

fn task_get() -> tokio::task::JoinHandle<()> {
    todo!()
}

fn task_commit(app: App) -> tokio::task::JoinHandle<()> {
    let entry_size = 10_000;
    let max_commits: usize = 100;

    tokio::spawn(async move {
        let mut rng = seeded_rng(None);

        loop {
            let n = rng.gen_range(0..NODES);
            let b = rng.gen_range(0..BASES);

            let base = app.bases[b].clone();
            let _: ActionHash = app.conductors[n]
                .call(
                    &app.zomes[n],
                    "create",
                    (base, random_vec::<u8>(&mut rng, entry_size)),
                )
                .await;

            let done = app.state.share_mut(|state| {
                state.commits[b] += 1;
                // state.counts[i][j].1 =Instant::now();

                state.commits.iter().sum::<usize>() > max_commits
            });

            if done {
                println!("\nNo more links will be created after this point.");
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: App) -> io::Result<()> {
    loop {
        if true || FLUSH.load(Ordering::Relaxed) {
            app.state.share_ref(|state| {
                terminal.draw(|f| ui(f, &state)).unwrap();
            });
            FLUSH.swap(false, Ordering::Relaxed);
        }
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    return Ok(());
                }
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, state: &State) {
    // fn cell(s: &str) -> Cell {
    //     let mut cell = Cell::default();
    //     cell.set_symbol(s);
    //     cell
    // }

    // Wrapping block for a group
    // Just draw the block and the group on the same area and build the group
    // with at least a margin of 1
    let size = f.size();

    let header = Row::new(
        ["exp:".to_string()]
            .into_iter()
            .chain(state.commits.iter().map(|c| c.to_string())),
    )
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
    );
    let rows = state.counts.iter().enumerate().map(|(i, r)| {
        let cells = r.into_iter().enumerate().map(|(j, (c, t))| {
            let val = (state.commits[j].saturating_sub(*c)).min(15);
            let mut style = if val == 0 {
                Style::default().fg(Color::Green)
            } else if val < 3 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Red)
            };
            if t.elapsed() < Duration::from_secs(3) {
                style = style.add_modifier(Modifier::RAPID_BLINK);
            }
            Cell::from(format!("{:1x}", val)).style(style)
        });
        let front = Cell::from(format!("C{:<2}:", i));
        let row = [front].into_iter().chain(cells);
        Row::new(row)
    });
    let widths: Vec<_> = [Constraint::Length(4)]
        .into_iter()
        .chain([Constraint::Min(1); NODES].into_iter())
        .collect();
    let table = Table::new(rows)
        .header(header)
        // .block(Block::default().borders(Borders::ALL).title("Table"))
        .widths(&widths);

    f.render_widget(table, size);

    // // Surrounding block
    // let block = Block::default()
    //     .borders(Borders::ALL)
    //     .title("Main block with round corners")
    //     .title_alignment(Alignment::Center)
    //     .border_type(BorderType::Rounded);
    // f.render_widget(block, size);

    // let chunks = Layout::default()
    //     .direction(Direction::Vertical)
    //     .margin(4)
    //     .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
    //     .split(f.size());

    // // Top two inner blocks
    // let top_chunks = Layout::default()
    //     .direction(Direction::Horizontal)
    //     .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
    //     .split(chunks[0]);

    // // Top left inner block with green background
    // let block = Block::default()
    //     .title(vec![
    //         Span::styled("With", Style::default().fg(Color::Yellow)),
    //         Span::from(" background"),
    //     ])
    //     .style(Style::default().bg(Color::Green));
    // f.render_widget(block, top_chunks[0]);

    // // Top right inner block with styled title aligned to the right
    // let block = Block::default()
    //     .title(Span::styled(
    //         "Styled title",
    //         Style::default()
    //             .fg(Color::White)
    //             .bg(Color::Red)
    //             .add_modifier(Modifier::BOLD),
    //     ))
    //     .title_alignment(Alignment::Right);
    // f.render_widget(block, top_chunks[1]);

    // // Bottom two inner blocks
    // let bottom_chunks = Layout::default()
    //     .direction(Direction::Horizontal)
    //     .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
    //     .split(chunks[1]);

    // // Bottom left block with all default borders
    // let block = Block::default().title("With borders").borders(Borders::ALL);
    // f.render_widget(block, bottom_chunks[0]);

    // // Bottom right block with styled left and right border
    // let block = Block::default()
    //     .title("With styled borders and doubled borders")
    //     .border_style(Style::default().fg(Color::Cyan))
    //     .borders(Borders::LEFT | Borders::RIGHT)
    //     .border_type(BorderType::Double);
    // f.render_widget(block, bottom_chunks[1]);
}
