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
mod sweet_app_installation;
mod sweet_cell;
mod sweet_conductor;
mod sweet_conductor_batch;
mod sweet_conductor_config;
mod sweet_conductor_config_rendezvous;
mod sweet_conductor_handle;
pub mod sweet_consistency;
mod sweet_dna;
/// Generation of network topologies.
pub mod sweet_topos;
mod sweet_zome;

pub use crate::{await_consistency, await_consistency_advanced};
pub use sweet_agents::*;
pub use sweet_app::*;
pub use sweet_app_installation::*;
pub use sweet_cell::*;
pub use sweet_conductor::*;
pub use sweet_conductor_batch::*;
pub use sweet_conductor_config::*;
pub use sweet_conductor_config_rendezvous::*;
pub use sweet_conductor_handle::*;
pub use sweet_dna::*;
pub use sweet_topos::*;
pub use sweet_zome::*;
