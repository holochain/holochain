//! Sweetest = Streamlined Holochain test utils with lots of added sugar
//!
//! A wrapper around ConductorHandle which provides useful methods for setup
//! and zome calling, as well as some helpful references to Cells and Zomes
//! which make zome interaction much less verbose

mod sweet_agents;
mod sweet_app;
mod sweet_cell;
mod sweet_conductor;
mod sweet_dna;
mod sweet_network;
mod sweet_zome;

pub use sweet_agents::*;
pub use sweet_app::*;
pub use sweet_cell::*;
pub use sweet_conductor::*;
pub use sweet_dna::*;
pub use sweet_network::*;
pub use sweet_zome::*;

use hdk3::prelude::Element;
use holochain_serialized_bytes::prelude::*;

/// Necessary for parsing the output of a simple "get entry"
// TODO: remove once host fns remove SerializedBytes constraint
#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes)]
#[serde(transparent)]
#[repr(transparent)]
pub struct MaybeElement(pub Option<Element>);
