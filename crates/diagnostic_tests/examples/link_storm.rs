use crossterm::event::{self, Event, KeyCode};
use holochain_diagnostics::{
    holo_hash::ActionHash,
    holochain::{conductor::conductor::RwShare, prelude::*, sweettest::*},
    *,
};
use std::{
    error::Error,
    io::{self},
    sync::Arc,
    time::{Duration, Instant},
};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::*,
    Frame, Terminal,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app = setup_app().await;

    task_commit(app.clone());
    task_get(app.clone());

    tui_crossterm_setup(|t| run_app(t, app))?;

    Ok(())
}

const NODES: usize = 10;
const BASES: usize = 4;

const ENTRY_SIZE: usize = 10_000_000;
const MAX_COMMITS: usize = 100;

const APP_REFRESH_RATE: Duration = Duration::from_millis(50);
const COMMIT_RATE: Duration = Duration::from_millis(1000);
const GET_RATE: Duration = Duration::from_millis(10);

const YELLOW_THRESHOLD: usize = 5;
const RED_THRESHOLD: usize = 15;

#[derive(Clone)]
struct App {
    state: RwShare<State>,
    start_time: Instant,
    nodes: [Node; NODES],
    bases: [AnyLinkableHash; BASES],
}

struct State {
    commits: [usize; BASES],
    counts: [[(usize, Instant); BASES]; NODES],
    list_state: ListState,
}

impl State {
    fn done_committing(&self) -> bool {
        self.commits.iter().sum::<usize>() >= MAX_COMMITS
    }

    fn total_commits(&self) -> usize {
        self.commits.iter().sum()
    }

    fn total_discrepancy(&self) -> usize {
        self.counts
            .iter()
            .map(|r| r.iter().map(|(c, _)| c).copied().sum::<usize>())
            .sum()
    }

    fn node_selector(&mut self, i: isize) {
        if let Some(s) = self.list_state.selected() {
            let n = (s as isize + i).min(NODES as isize - 1).max(0);
            self.list_state.select(Some(n as usize));
        }
    }
}

#[derive(Clone, Debug)]
struct Node {
    conductor: Arc<SweetConductor>,
    zome: SweetZome,
    diagnostics: GossipDiagnostics,
}

async fn setup_app() -> App {
    assert!(BASES <= NODES);
    let config = config_historical_and_agent_gossip_only();

    let (conductors, zomes) = diagnostic_tests::setup_conductors_single_zome(
        NODES,
        config,
        diagnostic_tests::basic_zome(),
    )
    .await;

    let now = Instant::now();
    let commits = [0; BASES];
    let counts = [[(0, now); BASES]; NODES];
    let bases = zomes
        .iter()
        .take(BASES)
        .map(|z| z.cell_id().agent_pubkey().clone().into())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    conductors.exchange_peer_info().await;
    println!("Peer info exchanged. Starting UI.");

    let mut nodes = vec![];

    for (conductor, zome) in std::iter::zip(conductors.into_iter().map(Arc::new), zomes.into_iter())
    {
        let dna_hash = zome.cell_id().dna_hash().clone();
        let diagnostics = conductor
            .holochain_p2p()
            .get_diagnostics(dna_hash)
            .await
            .unwrap();
        nodes.push(Node {
            conductor,
            zome,
            diagnostics,
        });
    }
    let nodes = nodes.try_into().unwrap();

    let mut list_state: ListState = Default::default();
    list_state.select(Some(0));

    App {
        nodes,
        bases,
        start_time: Instant::now(),
        state: RwShare::new(State {
            commits,
            counts,
            list_state,
        }),
    }
}

fn task_get(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut i = 0;
        let mut last_zero = None;

        loop {
            let n = (i / BASES) % NODES;
            let b = i % BASES;

            let base = app.bases[b].clone();
            let links: usize = app.nodes[n]
                .conductor
                .call(&app.nodes[n].zome, "link_count", base)
                .await;

            let is_zero = app.state.share_mut(|state| {
                let val = state.commits[b] - links;
                state.counts[n][b].0 = val;
                state.counts[n][b].1 = Instant::now();
                val == 0
            });

            if is_zero {
                if let Some(last) = last_zero {
                    if i - last > NODES * BASES * 2 {
                        // If we've gone through two cycles of consistent zeros, then we can stop running get.
                        break;
                    }
                } else {
                    last_zero = Some(i);
                }
            } else {
                last_zero = None;
            }

            i += 1;

            tokio::time::sleep(GET_RATE).await;
        }
    })
}

fn task_commit(app: App) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut rng = seeded_rng(None);

        loop {
            let n = rng.gen_range(0..NODES);
            let b = rng.gen_range(0..BASES);

            let base = app.bases[b].clone();
            let _: ActionHash = app.nodes[n]
                .conductor
                .call(
                    &app.nodes[n].zome,
                    "create",
                    (base, random_vec::<u8>(&mut rng, ENTRY_SIZE)),
                )
                .await;

            let done = app.state.share_mut(|state| {
                state.commits[b] += 1;
                state.done_committing()
            });
            if done {
                break;
            }
            tokio::time::sleep(COMMIT_RATE).await;
        }
    })
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app)).unwrap();
        if event::poll(APP_REFRESH_RATE)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.state.share_mut(|s| s.node_selector(-1))
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.state.share_mut(|s| s.node_selector(1))
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let table_len = BASES as u16 * 2 + 5 + 2;
    let mut chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(4),
                Constraint::Length(table_len),
                Constraint::Min(20),
            ]
            .as_ref(),
        )
        .split(f.size());
    chunks[0].y += 1;
    chunks[0].height -= 1;

    let chunks2 = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Max(50)])
        .split(chunks[2]);

    let selected = app.state.share_mut(|state| {
        let header = Row::new(state.commits.iter().enumerate().map(|(i, _)| i.to_string())).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        );

        let rows = state.counts.iter().enumerate().map(|(_i, r)| {
            let cells = r.into_iter().enumerate().map(|(_, (c, t))| {
                let val = (*c).min(15);
                let mut style = if val == 0 {
                    Style::default().fg(Color::Green)
                } else if val < YELLOW_THRESHOLD {
                    Style::default().fg(Color::Yellow)
                } else if val < RED_THRESHOLD {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Magenta)
                };
                if t.elapsed() < GET_RATE * BASES as u32 {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                Cell::from(format!("{:1x}", val)).style(style)
            });
            Row::new(cells)
        });
        let table = Table::new(rows)
            .header(header)
            // .block(Block::default().borders(Borders::ALL).title("Table"))
            .widths(&[Constraint::Min(1); NODES]);

        let node_list = List::new(
            app.nodes
                .iter()
                .enumerate()
                .map(|(i, _)| format!("C{:<2}", i))
                .map(ListItem::new)
                .collect::<Vec<_>>(),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let global_stats = List::new(
            [
                format!("T:           {:<.2?}", app.start_time.elapsed()),
                format!("Commits:     {}", state.total_commits()),
                format!("Discrepancy: {}", state.total_discrepancy()),
            ]
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<_>>(),
        );

        f.render_stateful_widget(node_list, chunks[0], &mut state.list_state);
        f.render_widget(table, chunks[1]);
        f.render_widget(global_stats, chunks2[0]);
        state.list_state.selected()
    });
    let gossip_info = ui_gossip_info(&app.nodes[selected.unwrap()]);
    f.render_widget(gossip_info, chunks2[1]);
}

fn ui_gossip_info(node: &Node) -> List {
    let metrics = node.diagnostics.metrics.read();
    let rows: Vec<_> = metrics
        .node_info()
        .iter()
        .map(|(agent, info)| format!("{:?}", info))
        .map(ListItem::new)
        .collect();
    // let rows: Vec<_> = metrics
    //     .dump_historical()
    //     .into_iter()
    //     .map(|r| format!("{:?}", r))
    //     .collect();
    List::new(rows)
}
