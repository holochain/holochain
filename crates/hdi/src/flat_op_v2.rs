//! [`FlatOp`] flattens a v2 [`Op`](holochain_integrity_types::dht_v2::Op)
//! into a flatter, more accessible shape than
//! [`Op`](holochain_integrity_types::dht_v2::Op)'s deeply nested variants,
//! expressed over the v2 `holochain_integrity_types::dht_v2::Action`.

use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash};
use holochain_integrity_types::dht_v2::Action;
use holochain_integrity_types::{LinkTag, MembraneProof, UnitEnum};

mod flat_op_activity;
mod flat_op_entry;
mod flat_op_record;
pub use flat_op_activity::*;
pub use flat_op_entry::*;
pub use flat_op_record::*;

/// A flattened view of a v2 [`Op`](holochain_integrity_types::dht_v2::Op),
/// grouped by authority (record, entry, agent activity, link) rather than by
/// the underlying action variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatOp<ET, LT>
where
    ET: UnitEnum,
{
    /// Received by the action authority; see [`OpRecord`].
    StoreRecord(OpRecord<ET, LT>),
    /// Received by the entry authority; see [`OpEntry`].
    StoreEntry(OpEntry<ET>),
    /// Received by the chain authority for every action; see [`OpActivity`].
    RegisterAgentActivity(OpActivity<<ET as UnitEnum>::Unit, LT>),
    /// A link create or delete operation, grouped into [`OpLink`] to mirror the
    /// [`OpRecord`]/[`OpEntry`]/[`OpActivity`] sub-types.
    RegisterLink(OpLink<LT>),
    /// Received by the entry authority when an entry is updated; see [`OpUpdate`].
    RegisterUpdate(OpUpdate<ET>),
    /// Received by the entry authority when an entry is deleted; see [`OpDelete`].
    RegisterDelete(OpDelete),
}

/// The link operations of [`FlatOp`], grouped into a sub-type to mirror
/// [`OpRecord`]/[`OpEntry`]/[`OpActivity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpLink<LT> {
    /// A link was created (`ActionData::CreateLink`).
    CreateLink {
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
    /// A link was deleted (`ActionData::DeleteLink`).
    DeleteLink {
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
}
