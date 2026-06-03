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

/// v2 of [`FlatOp`](crate::flat_op::FlatOp), over the v2 `dht_v2::Action`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatOp<ET, LT>
where
    ET: UnitEnum,
{
    /// See [`crate::flat_op::FlatOp::StoreRecord`].
    StoreRecord(OpRecord<ET, LT>),
    /// See [`crate::flat_op::FlatOp::StoreEntry`].
    StoreEntry(OpEntry<ET>),
    /// See [`crate::flat_op::FlatOp::RegisterAgentActivity`].
    RegisterAgentActivity(OpActivity<<ET as UnitEnum>::Unit, LT>),
    /// See [`crate::flat_op::FlatOp::RegisterCreateLink`].
    RegisterCreateLink {
        /// The base address where this link is stored.
        base_address: AnyLinkableHash,
        /// The target address of this link.
        target_address: AnyLinkableHash,
        /// The link's tag data.
        tag: LinkTag,
        /// The app defined link type of this link.
        link_type: LT,
        /// The v2 action that creates the link (`ActionData::CreateLink`).
        action: Action,
    },
    /// See [`crate::flat_op::FlatOp::RegisterDeleteLink`].
    RegisterDeleteLink {
        /// The original create-link v2 action (`ActionData::CreateLink`).
        original_action: Action,
        /// The base address where this link is stored.
        base_address: AnyLinkableHash,
        /// The target address of the link being deleted.
        target_address: AnyLinkableHash,
        /// The deleted link's tag data.
        tag: LinkTag,
        /// The app defined link type of the deleted link.
        link_type: LT,
        /// The v2 action that deletes the link (`ActionData::DeleteLink`).
        action: Action,
    },
    /// See [`crate::flat_op::FlatOp::RegisterUpdate`].
    RegisterUpdate(OpUpdate<ET>),
    /// See [`crate::flat_op::FlatOp::RegisterDelete`].
    RegisterDelete(OpDelete),
}
