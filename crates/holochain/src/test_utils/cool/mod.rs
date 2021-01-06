//! A wrapper around ConductorHandle which provides useful methods for setup
//! and zome calling, as well as some helpful references to Cells and Zomes
//! which make zome interaction much less verbose

mod cool_agents;
mod cool_app;
mod cool_cell;
mod cool_conductor;
mod cool_dna;
mod cool_network;
mod cool_zome;

pub use cool_agents::*;
pub use cool_app::*;
pub use cool_cell::*;
pub use cool_conductor::*;
pub use cool_dna::*;
pub use cool_network::*;
pub use cool_zome::*;

use hdk3::prelude::Element;
use holochain_serialized_bytes::prelude::*;

/// Necessary for parsing the output of a simple "get entry"
// TODO: remove once host fns remove SerializedBytes constraint
#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes)]
#[serde(transparent)]
#[repr(transparent)]
pub struct MaybeElement(pub Option<Element>);
