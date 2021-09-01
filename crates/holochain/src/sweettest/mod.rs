//! sweettest = Streamlined Holochain test utils with lots of added sugar
//!
//! Features:
//!
//! ### SweetConductor
//! A wrapper around ConductorHandle which provides useful methods for app setup
//! and zome calling, as well as some helpful references to Cells and Zomes
//! which make zome interaction much less verbose.
//!
//! ### SweetApp
//! A handy collection of cells installed under the same app.
//! Makes it easy to destructure the result of a SweetConductor::setup_app call
//! into a collection of SweetCells which can be used for zome calls.

mod sweet_agents;
mod sweet_app;
mod sweet_cell;
mod sweet_conductor;
mod sweet_conductor_batch;
mod sweet_conductor_handle;
mod sweet_conductor_sharded_scenario;
mod sweet_dna_file;
mod sweet_network;
mod sweet_zome;

pub use sweet_agents::*;
pub use sweet_app::*;
pub use sweet_cell::*;
pub use sweet_conductor::*;
pub use sweet_conductor_batch::*;
pub use sweet_conductor_handle::*;
pub use sweet_conductor_sharded_scenario::*;
pub use sweet_dna_file::*;
pub use sweet_network::*;
pub use sweet_zome::*;

/// Re-exports of ScenarioDef-related types form kitsune_p2p
pub mod scenario {
    pub use kitsune_p2p::test_util::scenario_def::{
        PeerMatrix, ScenarioDef, ScenarioDefAgent as Agent, ScenarioDefNode as Node,
    };
}

pub use crate::test_utils::inline_zomes::unit_dna;
