//! A wrapper around ConductorHandle which provides useful methods for setup
//! and zome calling, as well as some helpful references to Cells and Zomes
//! which make zome interaction much less verbose

mod test_agents;
mod test_cell;
mod test_handle;
mod test_set;
mod test_zome;

pub use test_agents::*;
pub use test_cell::*;
pub use test_handle::*;
pub use test_set::*;
pub use test_zome::*;

use hdk3::prelude::Element;
use holochain_serialized_bytes::prelude::*;

/// Necessary for parsing the output of a simple "get entry"
// TODO: remove once host fns remove SerializedBytes constraint
#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes)]
#[serde(transparent)]
#[repr(transparent)]
pub struct MaybeElement(pub Option<Element>);
