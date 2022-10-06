const YELLOW_THRESHOLD: usize = 5;
const RED_THRESHOLD: usize = 15;

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
            let n = (s as isize + i).min(N as isize - 1).max(0);
            self.list_state.select(Some(n as usize));
        }
    }
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
                    _ => {}
                }
            }
        };
        false
    }

    pub fn render<K: Backend>(&self, f: &mut Frame<K>) {
        let [rect_list, rect_table, rect_gossip, rect_stats] = self.ui_layout(f);

        let selected = self.state.share_mut(|state| {
            f.render_stateful_widget(self.ui_node_list(), rect_list, &mut state.list_state);
            f.render_widget(self.ui_basis_table(state), rect_table);
            f.render_widget(self.ui_global_stats(state), rect_stats);
            state.list_state.selected()
        });
        f.render_widget(self.ui_gossip_info_table(selected.unwrap()), rect_gossip);
    }

    fn ui_node_list(&self) -> List<'static> {
        List::new(
            self.nodes
                .iter()
                .enumerate()
                .map(|(i, _)| format!("C{:<2}", i))
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

    fn ui_gossip_detail(&self, n: usize) {
        let node = &self.nodes[n];
        let metrics = node.diagnostics.metrics.read();
        let mut infos: Vec<_> = self
            .node_infos(&metrics)
            .into_iter()
            .flat_map(|(_, i)| i.complete_rounds.clone())
            .collect();
        infos.sort_unstable_by(|a, b| b.cmp(a));
    }

    fn ui_gossip_info_row(&self, info: &NodeInfo, own: bool) -> Row<'static> {
        let active = if info.current_round { "*" } else { " " }.to_string();
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

    fn ui_layout<K: Backend>(&self, f: &mut Frame<K>) -> [Rect; 4] {
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
