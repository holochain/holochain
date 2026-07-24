//! [`FlatOp`] flattens an [`Op`](holochain_integrity_types::op::Op)
//! into a flatter, more accessible shape than
//! [`Op`](holochain_integrity_types::op::Op)'s deeply nested variants.
//!
//! Once you've matched a specific variant, read the fields you need directly off
//! `action.data` (e.g. `action.data.base_address`).

use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash};
use holochain_integrity_types::prelude::{
    Action, ActionData, ActionHeader, CreateData, CreateLinkData, DeleteData, DeleteLinkData,
    EntryType, LinkTag, MembraneProof, Timestamp, UnitEnum, UpdateData, WrongActionError,
};

mod flat_op_activity;
mod flat_op_entry;
mod flat_op_record;
mod typed_action;
pub use flat_op_activity::*;
pub use flat_op_entry::*;
pub use flat_op_record::*;
pub use typed_action::*;

/// A flattened view of an [`Op`](holochain_integrity_types::op::Op),
/// grouped by authority (record, entry, agent activity, link) rather than by
/// the underlying action variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatOp<ET, LT>
where
    ET: UnitEnum,
{
    /// Received by the action authority; see [`OpRecord`].
    CreateRecord(OpRecord<ET, LT>),
    /// Received by the entry authority; see [`OpEntry`].
    CreateEntry(OpEntry<ET>),
    /// Received by the chain authority for every action; see [`OpActivity`].
    AgentActivity(OpActivity<<ET as UnitEnum>::Unit, LT>),
    /// A link create or delete operation, grouped into [`OpLink`] to mirror the
    /// [`OpRecord`]/[`OpEntry`]/[`OpActivity`] sub-types.
    Link(OpLink<LT>),
    /// Received by the entry authority when an entry is updated; see [`OpUpdate`].
    Update(OpUpdate<ET>),
    /// Received by the entry authority when an entry is deleted; see [`OpDelete`].
    Delete(OpDelete),
}

/// The link operations of [`FlatOp`], grouped into a sub-type to mirror
/// [`OpRecord`]/[`OpEntry`]/[`OpActivity`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpLink<LT> {
    /// A link was created (`ActionData::CreateLink`).
    CreateLink {
        /// The app defined link type of this link.
        link_type: LT,
        /// The action that creates the link.
        action: TypedAction<CreateLinkData>,
    },
    /// A link was deleted (`ActionData::DeleteLink`).
    DeleteLink {
        /// The original create-link action.
        original_action: TypedAction<CreateLinkData>,
        /// The app defined link type of the deleted link.
        link_type: LT,
        /// The action that deletes the link.
        action: TypedAction<DeleteLinkData>,
    },
}

impl<LT> OpLink<LT> {
    /// The base address of the link this operation touches.
    pub fn base_address(&self) -> &AnyLinkableHash {
        match self {
            OpLink::CreateLink { action, .. } => &action.base_address,
            OpLink::DeleteLink { action, .. } => &action.base_address,
        }
    }

    /// The target address of the link this operation touches.
    pub fn target_address(&self) -> &AnyLinkableHash {
        match self {
            OpLink::CreateLink { action, .. } => &action.target_address,
            OpLink::DeleteLink {
                original_action, ..
            } => &original_action.target_address,
        }
    }

    /// The tag of the link this operation touches.
    pub fn tag(&self) -> &LinkTag {
        match self {
            OpLink::CreateLink { action, .. } => &action.tag,
            OpLink::DeleteLink {
                original_action, ..
            } => &original_action.tag,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::short_hand::{ah, ak, lh};
    use holochain_integrity_types::prelude::{LinkType, ZomeIndex};

    fn header() -> ActionHeader {
        ActionHeader {
            author: ak(1),
            timestamp: Timestamp::from_micros(0),
            action_seq: 3,
            prev_action: Some(ah(2)),
        }
    }

    fn create_link_data() -> CreateLinkData {
        CreateLinkData {
            base_address: lh(3),
            target_address: lh(4),
            zome_index: ZomeIndex(0),
            link_type: LinkType(0),
            tag: LinkTag(vec![]),
        }
    }

    #[test]
    fn create_link_getters_read_action_data() {
        let op = OpLink::CreateLink {
            link_type: (),
            action: TypedAction {
                header: header(),
                data: create_link_data(),
            },
        };
        assert_eq!(op.base_address(), &lh(3));
        assert_eq!(op.target_address(), &lh(4));
        assert_eq!(op.tag(), &LinkTag(vec![]));
    }

    #[test]
    fn delete_link_getters_read_original_action_and_delete_data() {
        let op = OpLink::DeleteLink {
            original_action: TypedAction {
                header: header(),
                data: create_link_data(),
            },
            link_type: (),
            action: TypedAction {
                header: header(),
                data: DeleteLinkData {
                    base_address: lh(3),
                    link_add_address: ah(5),
                },
            },
        };
        assert_eq!(op.base_address(), &lh(3));
        assert_eq!(op.target_address(), &lh(4));
        assert_eq!(op.tag(), &LinkTag(vec![]));
    }
}
