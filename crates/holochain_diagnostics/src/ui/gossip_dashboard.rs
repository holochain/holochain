use holochain::prelude::{
    kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::tokio::time::Instant as TokioInstant,
    metrics::{PeerNodeHistory, RoundMetric},
};
use human_repr::{HumanCount, HumanThroughput};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode};
use holochain::{
    conductor::conductor::RwShare,
    prelude::{metrics::Metrics, *},
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

use self::widgets::{
    gossip_round_table::{gossip_round_table, GossipRoundTableState},
    ui_gossip_progress_gauge,
};

mod layout;
mod state;
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

/// State shared with the implementor
pub trait ClientState {
    fn time(&self) -> Instant;
    fn num_bases(&self) -> usize;
    fn nodes(&self) -> &[Node];

    fn total_commits(&self) -> usize;
    fn link_counts(&self) -> LinkCountsRef;
    fn node_histories_sorted<'a>(&self, metrics: &'a Metrics) -> NodeHistories<'a, usize>;
}

/// State specific to the UI
#[derive(Default)]
pub struct LocalState {
    pub list_state: ListState,
    pub filter_zero_rounds: bool,
    pub done_time: Option<Instant>,
}

impl LocalState {
    // pub fn total_discrepancy(&self) -> usize {
    //     self.counts
    //         .iter()
    //         .map(|r| r.iter().map(|(c, _)| c).copied().sum::<usize>())
    //         .sum()
    // }

    pub fn node_selector(&mut self, i: isize, max: usize) {
        if let Some(s) = self.list_state.selected() {
            let n = (s as isize + i).min(max as isize).max(0);
            self.list_state.select(Some(n as usize));
        }
    }

    pub fn selected_node(&self) -> Option<usize> {
        self.list_state
            .selected()
            .and_then(|s| (s > 0).then(|| s - 1))
    }
}

/// Outer vec for nodes, inner vec for bases
pub type LinkCounts = Vec<Vec<(usize, Instant)>>;
pub type LinkCountsRef<'a> = &'a [Vec<(usize, Instant)>];
pub type NodeHistories<'a, Id> = Vec<(Id, &'a PeerNodeHistory)>;

#[derive(Clone)]
pub struct GossipDashboard {
    refresh_rate: Duration,
    start_time: Instant,
    local_state: RwShare<LocalState>,
}

pub enum InputCmd {
    Done,
    Clear,
    Exchange,
    AddNode(usize),
}

impl GossipDashboard {
    pub fn new(selected_node: Option<usize>, start_time: Instant, refresh_rate: Duration) -> Self {
        let mut state = LocalState::default();
        state.list_state.select(selected_node);
        Self {
            start_time,
            refresh_rate,
            local_state: RwShare::new(state),
        }
    }

    pub fn input<S: ClientState>(&self, state: RwShare<S>) -> Option<InputCmd> {
        if event::poll(self.refresh_rate).unwrap() {
            if let Event::Key(key) = event::read().unwrap() {
                match key.code {
                    KeyCode::Char('q') => {
                        return Some(InputCmd::Done);
                    }
                    KeyCode::Char('x') => {
                        return Some(InputCmd::Exchange);
                    }
                    KeyCode::Char('c') => {
                        return Some(InputCmd::Clear);
                    }
                    KeyCode::Char('n') => {
                        return Some(InputCmd::AddNode(0));
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.local_state.share_mut(|s| {
                        s.node_selector(-1, state.share_ref(|state| state.nodes().len()))
                    }),
                    KeyCode::Down | KeyCode::Char('j') => self.local_state.share_mut(|s| {
                        s.node_selector(1, state.share_ref(|state| state.nodes().len()))
                    }),
                    KeyCode::Char('0') => self
                        .local_state
                        .share_mut(|s| s.filter_zero_rounds = !s.filter_zero_rounds),
                    _ => {}
                }
            }
        };
        None
    }

    pub fn clear<K: Backend>(&self, f: &mut Frame<K>) {
        f.render_widget(tui::widgets::Clear, f.size())
    }

    pub fn render<K: Backend>(&self, f: &mut Frame<K>, state: &impl ClientState) {
        let layout = layout::layout(state.nodes().len(), state.num_bases(), f);

        let (selected, filter_zeroes, done_time, gauges) = self.local_state.share_mut(|local| {
            let metrics: Vec<_> = state
                .nodes()
                .iter()
                .map(|n| n.diagnostics.metrics.read())
                .collect();
            let activity = metrics
                .iter()
                .map(|m| {
                    state
                        .node_histories_sorted(m)
                        .iter()
                        .any(|i| i.1.current_round.is_some())
                })
                .enumerate();
            f.render_stateful_widget(
                widgets::ui_node_list(activity),
                layout.node_list,
                &mut local.list_state,
            );
            f.render_widget(
                widgets::ui_basis_table(self.refresh_rate * 4, state.link_counts())
                    .block(Block::default().borders(Borders::union(Borders::LEFT, Borders::RIGHT)))
                    // the widths have to be specified here because they are not const
                    // and must be borrowed
                    .widths(&vec![Constraint::Length(3); state.num_bases()]),
                layout.basis_table,
            );
            let selected = local.selected_node();
            if selected.is_none() {
                f.render_widget(widgets::ui_keymap(), layout.bottom);
                f.render_widget(
                    widgets::ui_global_stats(self.start_time, state),
                    layout.table_extras,
                );
            }
            let gauges: Vec<_> = metrics
                .iter()
                .map(|m| ui_gossip_progress_gauge(m.incoming_gossip_progress()))
                .collect();

            (selected, local.filter_zero_rounds, local.done_time, gauges)
        });
        if let Some(selected) = selected {
            // node.conductor.get_agent_infos(Some(node.zome.cell_id().clone()))
            let metrics = &state.nodes()[selected].diagnostics.metrics.read();
            let infos = state.node_histories_sorted(metrics);
            for (i, gauge) in gauges.into_iter().enumerate() {
                f.render_widget(gauge, layout.gauges[i]);
            }
            f.render_widget(
                gossip_round_table(&GossipRoundTableState {
                    infos: &infos,
                    start_time: self.start_time,
                    current_time: state.time(),
                    filter_zeroes,
                }),
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
            .unwrap_or_else(|| {
                (
                    state.time().duration_since(self.start_time),
                    Style::default(),
                )
            });
        let t_widget = Paragraph::new(format!("{}  T={:<.2?}", z, t)).style(style);
        f.render_widget(t_widget, layout.time);
    }
}

// agent_node_index: HashMap<AgentPubKey, usize>,

// let agent_node_index: HashMap<_, _> = agents.enumerate().map(|(i, n)| (n, i)).collect();

// fn node_infos<'a>(&self, metrics: &'a Metrics) -> NodeInfoList<'a, usize> {
//     let mut infos: Vec<_> = metrics
//         .node_info()
//         .iter()
//         .map(|(agent, info)| {
//             (
//                 *self
//                     .agent_node_index
//                     .get(&AgentPubKey::from_kitsune(agent))
//                     .unwrap(),
//                 info,
//             )
//         })
//         .collect();
//     infos.sort_unstable_by_key(|(i, _)| *i);
//     infos
// }
