use holochain::prelude::{
    kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::tokio::time::Instant as TokioInstant,
    metrics::RoundMetric,
};
use human_repr::{HumanCount, HumanThroughput};
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
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::*,
    Frame,
};

mod layout;
mod widgets;

// 999, 99, or 9
const MAX_COUNT: usize = 999;

const YELLOW_THRESHOLD: usize = MAX_COUNT / 5;
const RED_THRESHOLD: usize = MAX_COUNT / 2;

#[derive(Clone, Debug)]
pub struct Node {
    pub conductor: Arc<SweetConductor>,
    pub zome: SweetZome,
    pub diagnostics: GossipDiagnostics,
}

impl Node {
    pub async fn new(conductor: Arc<SweetConductor>, zome: SweetZome) -> Self {
        let dna_hash = zome.cell_id().dna_hash().clone();
        let diagnostics = conductor
            .holochain_p2p()
            .get_diagnostics(dna_hash)
            .await
            .unwrap();
        Self {
            conductor,
            zome,
            diagnostics,
        }
    }

    pub fn agent(&self) -> AgentPubKey {
        self.zome.cell_id().agent_pubkey().clone()
    }
}

pub struct State<const N: usize, const B: usize> {
    pub commits: [usize; B],
    pub counts: [[(usize, Instant); B]; N],
    pub list_state: ListState,
    pub filter_zero_rounds: bool,
    pub done_time: Option<Instant>,
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

pub type NodeInfoList<'a, Id> = Vec<(Id, &'a NodeInfo)>;

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
        let layout = layout::layout(N, B, f);

        let (selected, filter_zeroes, done_time) = self.state.share_mut(|state| {
            let metrics: Vec<_> = self
                .nodes
                .iter()
                .map(|n| n.diagnostics.metrics.read())
                .collect();
            let infos: Vec<_> = metrics.iter().map(|m| self.node_infos(&m)).collect();
            f.render_stateful_widget(
                widgets::ui_node_list(infos.as_slice()),
                layout.node_list,
                &mut state.list_state,
            );
            f.render_widget(
                widgets::ui_basis_table(self.refresh_rate * 4, state),
                layout.basis_table,
            );
            let selected = state.selected_node();
            if selected.is_none() {
                f.render_widget(widgets::ui_keymap(), layout.table_extras);
                f.render_widget(
                    widgets::ui_global_stats(self.start_time, state),
                    layout.bottom,
                );
            }
            (selected, state.filter_zero_rounds, state.done_time)
        });
        if let Some(selected) = selected {
            let node = &self.nodes[selected];
            let metrics = node.diagnostics.metrics.read();
            let infos = self.node_infos(&metrics);
            f.render_widget(
                widgets::ui_gossip_info_table(&infos, selected),
                layout.table_extras,
            );
            f.render_widget(
                widgets::gossip_round_table(&infos, self.start_time, filter_zeroes),
                layout.bottom,
            );
        }

        let z = if filter_zeroes { "(0)" } else { "   " };
        let (t, style) = done_time
            .map(|t| {
                (
                    t.duration_since(self.start_time),
                    Style::default().add_modifier(Modifier::REVERSED),
                )
            })
            .unwrap_or_else(|| (self.start_time.elapsed(), Style::default()));
        let t_widget = Paragraph::new(format!("{}  T={:<.2?}", z, t)).style(style);
        f.render_widget(t_widget, layout.time);
    }

    fn node_infos<'a>(&self, metrics: &'a Metrics) -> NodeInfoList<'a, usize> {
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
