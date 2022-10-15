const YELLOW_THRESHOLD: usize = 5;
const RED_THRESHOLD: usize = 15;

use holochain::prelude::{
    kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::tokio::time::Instant as TokioInstant,
    metrics::RoundMetric,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode};
use holochain::{
    conductor::conductor::RwShare,
    prelude::{
        metrics::{Metrics, NodeInfo},
        *,
    },
    sweettest::*,
    test_utils::itertools::Itertools,
};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::*,
    Frame,
};

#[derive(Clone, Debug)]
pub struct Node {
    pub conductor: Arc<SweetConductor>,
    pub zome: SweetZome,
    pub diagnostics: GossipDiagnostics,
}

impl Node {
    pub fn agent(&self) -> AgentPubKey {
        self.zome.cell_id().agent_pubkey().clone()
    }
}

pub struct State<const N: usize, const B: usize> {
    pub commits: [usize; B],
    pub counts: [[(usize, Instant); B]; N],
    pub list_state: ListState,
    pub filter_zero_rounds: bool,
}

impl<const N: usize, const B: usize> State<N, B> {
    pub fn total_commits(&self) -> usize {
        self.commits.iter().sum()
    }

    pub fn total_discrepancy(&self) -> usize {
        self.counts
            .iter()
            .map(|r| r.iter().map(|(c, _)| c).copied().sum::<usize>())
            .sum()
    }

    pub fn node_selector(&mut self, i: isize) {
        if let Some(s) = self.list_state.selected() {
            let n = (s as isize + i).min(N as isize).max(0);
            self.list_state.select(Some(n as usize));
        }
    }

    pub fn selected_node(&self) -> Option<usize> {
        self.list_state
            .selected()
            .and_then(|s| (s > 0).then(|| s - 1))
    }
}

struct UiLayout {
    node_list: Rect,
    get_table: Rect,
    gossip_table: Rect,
    stats: Rect,
    time: Rect,
}

#[derive(Clone)]
pub struct Ui<const N: usize, const B: usize> {
    pub refresh_rate: Duration,
    pub start_time: Instant,
    pub nodes: [Node; N],
    pub state: RwShare<State<N, B>>,
    pub agent_node_index: HashMap<AgentPubKey, usize>,
}

impl<const N: usize, const B: usize> Ui<N, B> {
    pub fn new(
        nodes: [Node; N],
        start_time: Instant,
        refresh_rate: Duration,
        state: RwShare<State<N, B>>,
    ) -> Self {
        let agent_node_index = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.agent(), i))
            .collect();

        Self {
            nodes,
            start_time,
            refresh_rate,
            state,
            agent_node_index,
        }
    }

    pub fn input(&self) -> bool {
        if event::poll(self.refresh_rate).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                match key.code {
                    KeyCode::Char('q') => {
                        return true;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.state.share_mut(|s| s.node_selector(-1))
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.state.share_mut(|s| s.node_selector(1))
                    }
                    KeyCode::Char('0') => self
                        .state
                        .share_mut(|s| s.filter_zero_rounds = !s.filter_zero_rounds),
                    _ => {}
                }
            }
        };
        false
    }

    pub fn render<K: Backend>(&self, f: &mut Frame<K>) {
        let layout = self.ui_layout(f);

        let (selected, filter_zeroes) = self.state.share_mut(|state| {
            f.render_stateful_widget(self.ui_node_list(), layout.node_list, &mut state.list_state);
            f.render_widget(self.ui_basis_table(state), layout.get_table);
            let selected = state.selected_node();
            if selected.is_none() {
                f.render_widget(self.ui_keymap(), layout.gossip_table);
                f.render_widget(self.ui_global_stats(state), layout.stats);
            }
            (selected, state.filter_zero_rounds)
        });
        if let Some(selected) = selected {
            f.render_widget(self.ui_gossip_info_table(selected), layout.gossip_table);
            f.render_widget(self.ui_gossip_detail(selected, filter_zeroes), layout.stats);
        }

        let z = if filter_zeroes { "(0)" } else { "   " };
        let t = Paragraph::new(format!("{}  T={:<.2?}", z, self.start_time.elapsed()));
        f.render_widget(t, layout.time);
    }

    fn ui_node_list(&self) -> List<'static> {
        let nodes = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, _)| format!("C{:<2}", i));
        List::new(
            ["<G>".to_string()]
                .into_iter()
                .chain(nodes)
                .map(ListItem::new)
                .collect::<Vec<_>>(),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    }

    fn ui_basis_table(&self, state: &State<N, B>) -> Table<'static> {
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
                if t.elapsed() < self.refresh_rate * B as u32 {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
                Cell::from(format!("{:1x}", val)).style(style)
            });
            Row::new(cells)
        });
        Table::new(rows)
            .header(header)
            .block(Block::default().borders(Borders::union(Borders::LEFT, Borders::RIGHT)))
            .widths(&[Constraint::Min(1); N])
    }

    fn ui_global_stats(&self, state: &State<N, B>) -> List<'static> {
        List::new(
            [
                format!("T:           {:<.2?}", self.start_time.elapsed()),
                format!("Commits:     {}", state.total_commits()),
                format!("Discrepancy: {}", state.total_discrepancy()),
            ]
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<_>>(),
        )
        .block(Block::default().borders(Borders::TOP).title("Stats"))
    }

    fn ui_keymap(&self) -> List<'static> {
        List::new(
            [
                format!("up/down/j/k : select node"),
                format!("          0 : toggle empty gossip rounds"),
                format!("          q : Quit"),
            ]
            .into_iter()
            .map(ListItem::new)
            .collect::<Vec<_>>(),
        )
        .block(Block::default().borders(Borders::TOP).title("Keymap"))
    }

    fn ui_gossip_info_table(&self, n: usize) -> Table<'static> {
        let node = &self.nodes[n];
        let metrics = node.diagnostics.metrics.read();
        let infos = self.node_infos(&metrics);

        let header = Row::new(["A", "ini", "rmt", "cmp", "err"])
            .style(Style::default().add_modifier(Modifier::UNDERLINED));

        Table::new(
            infos
                .into_iter()
                .map(|(i, info)| self.ui_gossip_info_row(info, n == i))
                .collect::<Vec<_>>(),
        )
        .header(header)
        .widths(&[
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            // Constraint::Length(5),
            Constraint::Percentage(100),
        ])
    }

    fn ui_gossip_detail(&self, n: usize, filter_zeroes: bool) -> Table<'static> {
        let node = &self.nodes[n];
        let metrics = node.diagnostics.metrics.read();
        let infos = self.node_infos(&metrics);
        let mut currents: Vec<_> = infos
            .iter()
            .filter_map(|(n, i)| i.current_round.clone().map(|r| (*n, r)))
            .collect();

        let mut metrics: Vec<_> = infos
            .iter()
            .flat_map(|(n, info)| {
                info.complete_rounds
                    .clone()
                    .into_iter()
                    .map(move |r| (n, r))
            })
            .collect();

        currents.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        metrics.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        let header = Row::new(["n", "time", "t", "dur", "#in", "#out", "in", "out"])
            .style(Style::default().add_modifier(Modifier::UNDERLINED));

        let mut rows = vec![];

        // Add current round info

        rows.extend(
            currents
                .into_iter()
                .map(|(n, metric)| render_gossip_metric_row(n, metric, self.start_time, true)),
        );

        // Add past round info

        rows.extend(metrics.into_iter().filter_map(|(n, info)| {
            let zero = info
                .round
                .as_ref()
                .map(|r| {
                    r.throughput.op_count.incoming
                        + r.throughput.op_count.outgoing
                        + r.throughput.op_bytes.incoming
                        + r.throughput.op_bytes.outgoing
                        == 0
                })
                .unwrap_or(false);
            if filter_zeroes && zero {
                None
            } else {
                Some(render_gossip_metric_row(*n, info, self.start_time, false))
            }
        }));

        Table::new(rows).header(header).widths(&[
            Constraint::Percentage(100 / 8),
            Constraint::Percentage(100 / 8),
            Constraint::Percentage(100 / 8),
            Constraint::Percentage(100 / 8),
            Constraint::Percentage(100 / 8),
            Constraint::Percentage(100 / 8),
            Constraint::Percentage(100 / 8),
            Constraint::Percentage(100 / 8),
        ])
    }

    fn ui_gossip_info_row(&self, info: &NodeInfo, own: bool) -> Row<'static> {
        let active = if info.current_round.is_some() {
            "*"
        } else {
            " "
        }
        .to_string();
        let rounds = info
            .complete_rounds
            .iter()
            .map(|i| format!("{}", i.duration().as_millis()))
            .rev()
            .join(" ");
        // let latency = format!("{:3}", *info.latency_micros / 1000.0);
        if own {
            Row::new(vec![
                active,
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                // latency,
                rounds,
            ])
        } else {
            Row::new(vec![
                active,
                info.initiates.len().to_string(),
                info.remote_rounds.len().to_string(),
                info.complete_rounds.len().to_string(),
                info.errors.len().to_string(),
                // latency,
                rounds,
            ])
        }
    }

    fn ui_layout<K: Backend>(&self, f: &mut Frame<K>) -> UiLayout {
        let list_len = 3;
        let table_len = B as u16 * 2 + 2;
        let stats_height = 5;
        let mut vsplit = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length((N + 1) as u16),
                Constraint::Length(stats_height),
            ])
            .vertical_margin(1)
            .split(f.size());

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Length(list_len),
                    Constraint::Length(table_len),
                    Constraint::Percentage(100),
                ]
                .as_ref(),
            )
            .split(vsplit[0]);

        vsplit[1].y += 1;
        vsplit[1].height -= 1;

        let w = f.size().width;
        let tw = 16;
        let time = Rect {
            x: w - tw,
            y: 0,
            width: tw,
            height: 1,
        };

        UiLayout {
            node_list: top_chunks[0],
            get_table: top_chunks[1],
            gossip_table: top_chunks[2],
            stats: vsplit[1],
            time,
        }
    }

    fn node_infos<'a>(&self, metrics: &'a Metrics) -> Vec<(usize, &'a NodeInfo)> {
        let mut infos: Vec<_> = metrics
            .node_info()
            .iter()
            .map(|(agent, info)| {
                (
                    *self
                        .agent_node_index
                        .get(&AgentPubKey::from_kitsune(agent))
                        .unwrap(),
                    info,
                )
            })
            .collect();
        infos.sort_unstable_by_key(|(i, _)| *i);
        infos
    }
}

fn render_gossip_metric_row(
    n: usize,
    metric: RoundMetric,
    start_time: Instant,
    current: bool,
) -> Row<'static> {
    let number_cell = |v| {
        let cell = Cell::from(format!("{}", v));
        if v == 0 {
            cell.style(Style::default().fg(Color::DarkGray))
        } else {
            cell
        }
    };

    let (gt, style) = match metric.gossip_type {
        GossipModuleType::ShardedRecent => (
            Cell::from("R".to_string()),
            Style::default().fg(Color::Green),
        ),
        GossipModuleType::ShardedHistorical => (
            Cell::from("H".to_string()),
            Style::default().fg(Color::Blue),
        ),
    };
    let mut cells = vec![
        Cell::from(n.to_string()),
        Cell::from(format!(
            "{:.1?}",
            metric
                .instant
                .duration_since(TokioInstant::from(start_time))
        )),
        gt,
    ];

    cells.push({
        let dur = if current {
            metric.instant.elapsed()
        } else if let Some(round) = &metric.round {
            metric.instant.duration_since(round.start_time)
        } else {
            Duration::ZERO
        };
        let style = if dur.as_millis() >= 1000 {
            Style::default().fg(Color::Red)
        } else if dur.as_millis() >= 100 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        Cell::from(format!("{:3.1?}", dur)).style(style)
    });

    if let Some(round) = metric.round {
        cells.extend([
            number_cell(round.throughput.op_count.incoming),
            number_cell(round.throughput.op_count.outgoing),
            number_cell(round.throughput.op_bytes.incoming),
            number_cell(round.throughput.op_bytes.outgoing),
        ])
    }
    let style = if current {
        style.add_modifier(Modifier::REVERSED)
    } else {
        style
    };
    Row::new(cells).style(style)
}
