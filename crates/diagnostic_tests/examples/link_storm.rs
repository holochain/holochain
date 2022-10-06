use crossterm::event::{self, Event, KeyCode};
use holochain_diagnostics::{
    holochain::{conductor::conductor::RwShare, prelude::*, sweettest::*},
    metrics::*,
    *,
};
use std::{
    collections::HashMap,
    error::Error,
    io::{self},
    sync::Arc,
    time::{Duration, Instant},
};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
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

    agent_node_index: HashMap<AgentPubKey, usize>,
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

impl Node {
    pub fn agent(&self) -> AgentPubKey {
        self.zome.cell_id().agent_pubkey().clone()
    }
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
    let agent_node_index = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.agent(), i))
        .collect();
    let bases = nodes
        .iter()
        .take(BASES)
        .map(|n| n.agent().into())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let nodes = nodes.try_into().unwrap();

    let now = Instant::now();
    let commits = [0; BASES];
    let counts = [[(0, now); BASES]; NODES];

    let mut list_state: ListState = Default::default();
    list_state.select(Some(0));

    App {
        nodes,
        bases,
        start_time: now,
        state: RwShare::new(State {
            commits,
            counts,
            list_state,
        }),
        agent_node_index,
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
    let [rect_list, rect_table, rect_gossip, rect_stats] = ui_layout(f);

    let selected = app.state.share_mut(|state| {
        f.render_stateful_widget(ui_node_list(app), rect_list, &mut state.list_state);
        f.render_widget(ui_basis_table(state), rect_table);
        f.render_widget(ui_global_stats(app, state), rect_stats);
        state.list_state.selected()
    });
    f.render_widget(ui_gossip_info_table(&app, selected.unwrap()), rect_gossip);
}

fn ui_node_list(app: &App) -> List<'static> {
    List::new(
        app.nodes
            .iter()
            .enumerate()
            .map(|(i, _)| format!("C{:<2}", i))
            .map(ListItem::new)
            .collect::<Vec<_>>(),
    )
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
}

fn ui_basis_table(state: &State) -> Table<'static> {
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
    Table::new(rows)
        .header(header)
        .block(Block::default().borders(Borders::union(Borders::LEFT, Borders::RIGHT)))
        .widths(&[Constraint::Min(1); NODES])
}

fn ui_global_stats(app: &App, state: &State) -> List<'static> {
    List::new(
        [
            format!("T:           {:<.2?}", app.start_time.elapsed()),
            format!("Commits:     {}", state.total_commits()),
            format!("Discrepancy: {}", state.total_discrepancy()),
        ]
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<_>>(),
    )
    .block(Block::default().borders(Borders::TOP).title("Stats"))
}

fn ui_gossip_info_table(app: &App, n: usize) -> Table<'static> {
    let node = &app.nodes[n];
    let metrics = node.diagnostics.metrics.read();
    let mut rows: Vec<_> = metrics
        .node_info()
        .iter()
        .map(|(agent, info)| {
            (
                *app.agent_node_index
                    .get(&AgentPubKey::from_kitsune(agent))
                    .unwrap(),
                info,
            )
        })
        .collect();
    rows.sort_unstable_by_key(|(i, _)| *i);

    let header = Row::new(["A", "ini", "rmt", "cmp", "err", "lat"])
        .style(Style::default().add_modifier(Modifier::UNDERLINED));

    Table::new(
        rows.into_iter()
            .map(|(i, info)| ui_gossip_info_row(info, n == i))
            .collect::<Vec<_>>(),
    )
    .header(header)
    .widths(&[
        Constraint::Min(1),
        Constraint::Min(3),
        Constraint::Min(3),
        Constraint::Min(3),
        Constraint::Min(3),
        Constraint::Min(5),
    ])
}

fn ui_gossip_info_row(info: &NodeInfo, own: bool) -> Row<'static> {
    let active = if info.current_round { "*" } else { " " }.to_string();
    if own {
        Row::new(vec![
            active,
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            format!("{:3}", *info.latency_micros / 1000.0),
        ])
    } else {
        Row::new(vec![
            active,
            info.initiates.len().to_string(),
            info.remote_rounds.len().to_string(),
            info.complete_rounds.len().to_string(),
            info.errors.len().to_string(),
            format!("{:3}", *info.latency_micros / 1000.0),
        ])
    }
}

fn ui_layout<B: Backend>(f: &mut Frame<B>) -> [Rect; 4] {
    let list_len = 3;
    let table_len = BASES as u16 * 2 + 2;
    let stats_height = 5;
    let mut vsplit = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((NODES + 1) as u16),
            Constraint::Length(stats_height),
        ])
        .vertical_margin(1)
        .split(f.size());

    let mut top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(list_len),
                Constraint::Length(table_len),
                Constraint::Min(20),
            ]
            .as_ref(),
        )
        .split(vsplit[0]);

    top_chunks[0].y += 1;
    top_chunks[0].height -= 1;

    vsplit[1].y += 1;
    vsplit[1].height -= 1;

    [top_chunks[0], top_chunks[1], top_chunks[2], vsplit[1]]
}
