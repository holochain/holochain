//! An alternative to [`Op`] using a flatter structure, and user-defined deserialized
//! entry included where appropriate

use crate::{
    AgentValidationPkg, CloseChain, Create, CreateLink, Delete, DeleteLink, Dna,
    EntryCreationAction, InitZomesComplete, LinkTag, MembraneProof, OpenChain, UnitEnum, Update,
};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash};

mod flat_op_activity;
mod flat_op_entry;
mod flat_op_record;
pub use flat_op_activity::*;
pub use flat_op_entry::*;
pub use flat_op_record::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A convenience type for validation [`Op`]s.
pub enum FlatOp<ET, LT>
where
    ET: UnitEnum,
{
    /// The [`Op::StoreRecord`] which is validated by the authority
    /// for the [`ActionHash`] of this record.
    ///
    /// This operation stores a [`Record`] on the DHT and is
    /// returned when the authority receives a request
    /// on the [`ActionHash`].
    StoreRecord(OpRecord<ET, LT>),
    /// The [`Op::StoreEntry`] which is validated by the authority
    /// for the [`EntryHash`] of this entry.
    ///
    /// This operation stores an [`Entry`] on the DHT and is
    /// returned when the authority receives a request
    /// on the [`EntryHash`].
    StoreEntry(OpEntry<ET>),
    /// The [`Op::RegisterAgentActivity`] which is validated by
    /// the authority for the [`AgentPubKey`] for the author of this [`Action`].
    ///
    /// This operation registers an [`Action`] to an agent's chain
    /// on the DHT and is returned when the authority receives a request
    /// on the [`AgentPubKey`] for chain data.
    ///
    /// Note that [`Op::RegisterAgentActivity`] is the only operation
    /// that is validated by all zomes regardless of entry or link types.
    RegisterAgentActivity(OpActivity<<ET as UnitEnum>::Unit, LT>),
    /// The [`Op::RegisterCreateLink`] which is validated by
    /// the authority for the [`AnyLinkableHash`] in the base address
    /// of this link.
    ///
    /// This operation register's a link to the base address
    /// on the DHT and is returned when the authority receives a request
    /// on the base [`AnyLinkableHash`] for links.
    RegisterCreateLink {
        /// The base address where this link is stored.
        base_address: AnyLinkableHash,
        /// The target address of this link.
        target_address: AnyLinkableHash,
        /// The link's tag data.
        tag: LinkTag,
        /// The app defined link type of this link.
        link_type: LT,
        /// The [`CreateLink`] action that creates the link
        action: CreateLink,
    },
    /// The [`Op::RegisterDeleteLink`] which is validated by
    /// the authority for the [`AnyLinkableHash`] in the base address
    /// of the link that is being deleted.
    ///
    /// This operation registers a deletion of a link to the base address
    /// on the DHT and is returned when the authority receives a request
    /// on the base [`AnyLinkableHash`] for the link that is being deleted.
    RegisterDeleteLink {
        /// The original [`CreateLink`] [`Action`] that created the link.
        original_action: CreateLink,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The target address of the link being deleted.
        target_address: AnyLinkableHash,
        /// The deleted links tag data.
        tag: LinkTag,
        /// The app defined link type of the deleted link.
        link_type: LT,
        /// The [`DeleteLink`] action that deletes the link
        action: DeleteLink,
    },
    /// The [`Op::RegisterUpdate`] which is validated by
    /// the authority for the [`ActionHash`] of the original entry
    /// and the authority for the [`EntryHash`] of the original entry.
    ///
    /// This operation registers an update from the original entry on
    /// the DHT and is returned when the authority receives a request
    /// for the [`ActionHash`] of the original entry [`Action`] or the
    /// [`EntryHash`] of the original entry.
    RegisterUpdate(OpUpdate<ET>),
    /// The [`Op::RegisterDelete`] which is validated by
    /// the authority for the [`ActionHash`] of the deleted entry
    /// and the authority for the [`EntryHash`] of the deleted entry.
    ///
    /// This operation registers a deletion to the original entry on
    /// the DHT and is returned when the authority receives a request
    /// for the [`ActionHash`] of the deleted entry [`Action`] or the
    /// [`EntryHash`] of the deleted entry.
    RegisterDelete(OpDelete<ET>),
}

#[deprecated = "use the name FlatOp instead"]
/// Alias for `FlatOp` for backward compatibility
pub type OpType<ET, LT> = FlatOp<ET, LT>;
