use holochain::prelude::{
    dht::region::Region,
    gossip::sharded_gossip::RegionDiffs,
    kitsune_p2p::dependencies::kitsune_p2p_types::dependencies::tokio::time::Instant as TokioInstant,
    metrics::{CompletedRound, CurrentRound, PeerNodeHistory},
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
};
use tui::{
    backend::Backend,
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::*,
    Frame,
};

use self::widgets::{
    gossip_region_table::{gossip_region_table, GossipRegionTableState},
    gossip_round_table::{gossip_round_table, GossipRoundTableState},
    ui_gossip_progress_gauge,
};

mod input;
mod layout;
mod render;
mod state;
mod widgets;

pub use input::*;

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
    fn node_rounds_sorted<'a>(
        &self,
        metrics: &'a Metrics,
        agent: &AgentPubKey,
    ) -> NodeRounds<'a, usize>;
}

/// Distinct modes of input handling and display
#[derive(Debug, Clone)]
pub enum Focus {
    /// Nothing is selected
    Empty,
    /// We've drilled into a particular Node, now we can select one of its gossip rounds
    Node(usize),
    /// We've drilled into a Round, now we can see more detailed info about it
    Round {
        node: usize,
        round: RoundInfo,
        ours: bool,
    },
}

#[derive(Debug, Clone)]
pub struct RoundInfo {
    our_diff: Vec<Region>,
    their_diff: Vec<Region>,
}

impl Default for Focus {
    fn default() -> Self {
        Focus::Empty
    }
}

/// State specific to the UI
#[derive(Default)]
pub struct LocalState {
    pub node_list_state: ListState,
    pub round_table_state: TableState,
    pub region_table_state: TableState,
    pub focus: Focus,
    pub filter_zeroes: bool,
    pub done_time: Option<Instant>,
}

impl LocalState {
    pub fn node_selector(&mut self, i: isize, max: usize) {
        if let Some(s) = self.node_list_state.selected() {
            let n = (s as isize + i).min(max as isize).max(0);
            self.node_list_state.select(Some(n as usize));
        }
    }

    pub fn round_selector(&mut self, i: isize) {
        if let Some(s) = self.round_table_state.selected() {
            let n = (s as isize + i).max(0);
            self.round_table_state.select(Some(n as usize));
        }
    }

    pub fn region_selector(&mut self, i: isize) {
        if let Some(s) = self.region_table_state.selected() {
            let n = (s as isize + i).max(0);
            self.region_table_state.select(Some(n as usize));
        }
    }

    pub fn selected_node(&self) -> Option<usize> {
        self.node_list_state
            .selected()
            .and_then(|s| (s > 0).then(|| s - 1))
    }
}

/// Outer vec for nodes, inner vec for bases
pub type LinkCounts = Vec<Vec<(usize, Instant)>>;
pub type LinkCountsRef<'a> = &'a [Vec<(usize, Instant)>];
pub struct NodeRounds<'a, Id> {
    currents: Vec<(Id, &'a CurrentRound)>,
    completed: Vec<(Id, &'a CompletedRound)>,
}

impl<'a, Id: Clone> NodeRounds<'a, Id> {
    pub fn new(items: Vec<(Id, &'a PeerNodeHistory)>) -> Self {
        let mut currents: Vec<_> = items
            .iter()
            .filter_map(|(n, i)| i.current_round.as_ref().map(|r| (n.clone(), r)))
            .collect();

        let mut completed: Vec<_> = items
            .iter()
            .flat_map(|(n, info)| info.completed_rounds.iter().map(|r| (n.clone(), r)))
            .collect();

        currents.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        completed.sort_unstable_by(|a, b| b.1.cmp(&a.1));

        Self {
            currents,
            completed,
        }
    }

    /// Get RegionDiffs by index, given that completed rounds
    /// immediately follow current rounds in sequence
    pub fn round_regions(&self, index: usize) -> &RegionDiffs {
        let num_current = self.currents.len();
        if index < num_current {
            &self.currents[index].1.region_diffs
        } else {
            &self.completed[num_current + index].1.region_diffs
        }
    }
}

#[derive(Clone)]
pub struct GossipDashboard {
    refresh_rate: Duration,
    start_time: Instant,
    local_state: RwShare<LocalState>,
}

impl GossipDashboard {
    pub fn new(selected_node: Option<usize>, start_time: Instant, refresh_rate: Duration) -> Self {
        let mut state = LocalState::default();
        state.node_list_state.select(selected_node);
        Self {
            start_time,
            refresh_rate,
            local_state: RwShare::new(state),
        }
    }

    pub fn clear<K: Backend>(&self, f: &mut Frame<K>) {
        f.render_widget(tui::widgets::Clear, f.size())
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
