//! v2 of [`FlatOp`](crate::flat_op::FlatOp), expressed over the v2
//! `holochain_integrity_types::dht_v2::Action`. Transitional staging module;
//! promoted to replace `flat_op` in the legacy-deletion phase.

use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash};
use holochain_integrity_types::dht_v2::Action;
use holochain_integrity_types::{LinkTag, MembraneProof, UnitEnum};

mod flat_op_activity;
mod flat_op_entry;
mod flat_op_record;
pub use flat_op_activity::*;
pub use flat_op_entry::*;
pub use flat_op_record::*;
