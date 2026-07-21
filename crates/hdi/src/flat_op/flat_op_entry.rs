//! The `OpEntry` / `OpUpdate` / `OpDelete` types; see the description in the [`crate::flat_op`] parent module.
use super::*;
use holochain_integrity_types::prelude::{CapClaimEntry, CapGrantEntry};

/// Data specific to the [`Op::CreateEntry`](holochain_integrity_types::op::Op::CreateEntry)
/// operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpEntry<ET>
where
    ET: UnitEnum,
{
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for an app
    /// defined entry type.
    CreateEntry {
        /// The app defined entry with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data.
        app_entry: ET,
        /// The Create action that creates this entry.
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for an
    /// [`AgentPubKey`].
    CreateAgent {
        /// The Create action that creates this agent's key.
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for the newly
    /// created entry in an update.
    UpdateEntry {
        /// The app defined entry with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data of the new entry.
        app_entry: ET,
        /// The Update action that updates this entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for an updated
    /// [`AgentPubKey`].
    UpdateAgent {
        /// The Update action that updates this entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for a CapGrant
    CreateCapGrant {
        /// The cap grant entry data.
        entry: CapGrantEntry,
        /// The Create action that creates this cap grant.
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for a CapClaim
    CreateCapClaim {
        /// The cap claim entry data.
        entry: CapClaimEntry,
        /// The Create action that creates this cap claim.
        action: TypedAction<CreateData>,
    },
    /// This operation updates the [`Entry`](holochain_integrity_types::entry::Entry) for a
    /// CapGrant
    UpdateCapGrant {
        /// The Update action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant).
        action: TypedAction<UpdateData>,
        /// The new entry to store.
        entry: CapGrantEntry,
    },
    /// This operation updates the [`Entry`](holochain_integrity_types::entry::Entry) for a
    /// CapClaim
    UpdateCapClaim {
        /// The Update action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim).
        action: TypedAction<UpdateData>,
        /// The new entry to store.
        entry: CapClaimEntry,
    },
}

impl<ET: UnitEnum> OpEntry<ET> {
    /// The agent key this action creates, for [`OpEntry::CreateAgent`].
    pub fn agent(&self) -> Option<AgentPubKey> {
        match self {
            OpEntry::CreateAgent { action } => Some(action.data.entry_hash.clone().into()),
            _ => None,
        }
    }

    /// The new agent key this action updates to, for [`OpEntry::UpdateAgent`].
    pub fn new_key(&self) -> Option<AgentPubKey> {
        match self {
            OpEntry::UpdateAgent { action } => Some(action.data.entry_hash.clone().into()),
            _ => None,
        }
    }

    /// The original agent key being updated, for [`OpEntry::UpdateAgent`].
    pub fn original_key(&self) -> Option<AgentPubKey> {
        match self {
            OpEntry::UpdateAgent { action } => {
                Some(action.data.original_entry_address.clone().into())
            }
            _ => None,
        }
    }
}

/// Data specific to the [`Op::Update`](holochain_integrity_types::op::Op::Update)
/// operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpUpdate<ET>
where
    ET: UnitEnum,
{
    /// This operation registers an update from the original
    /// [`Entry`](holochain_integrity_types::entry::Entry).
    Entry {
        /// The app defined entry type with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data of the new entry.
        app_entry: ET,
        /// The action that updates this entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation registers an update from the original private
    /// [`Entry`](holochain_integrity_types::entry::Entry).
    PrivateEntry {
        /// The unit version of the app defined entry type for the new entry.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The action that updates this entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation registers an update from the original [`AgentPubKey`].
    Agent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The Update action that updates the agent's key.
        action: TypedAction<UpdateData>,
    },
    /// This operation registers an update from a Capability Claim.
    CapClaim {
        /// The Update action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim).
        action: TypedAction<UpdateData>,
    },
    /// This operation registers an update from a Capability Grant.
    CapGrant {
        /// The Update action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant).
        action: TypedAction<UpdateData>,
    },
}

impl<ET: UnitEnum> OpUpdate<ET> {
    /// The `Update` action shared by every variant.
    pub fn action(&self) -> &TypedAction<UpdateData> {
        match self {
            OpUpdate::Entry { action, .. }
            | OpUpdate::PrivateEntry { action, .. }
            | OpUpdate::Agent { action, .. }
            | OpUpdate::CapClaim { action, .. }
            | OpUpdate::CapGrant { action, .. } => action,
        }
    }

    /// The hash of the action that created the entry being updated.
    pub fn original_action_hash(&self) -> &ActionHash {
        &self.action().data.original_action_address
    }

    /// The hash of the original entry being updated.
    pub fn original_entry_hash(&self) -> &EntryHash {
        &self.action().data.original_entry_address
    }
}

/// Data specific to the [`Op::Delete`](holochain_integrity_types::op::Op::Delete)
/// operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpDelete {
    /// The Delete action that deletes this entry.
    pub action: TypedAction<DeleteData>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::short_hand::{ah, ak, eh};

    fn header() -> ActionHeader {
        ActionHeader {
            author: ak(1),
            timestamp: Timestamp::from_micros(0),
            action_seq: 0,
            prev_action: None,
        }
    }

    #[test]
    fn op_delete_constructs_and_clones() {
        let op = OpDelete {
            action: TypedAction {
                header: header(),
                data: DeleteData {
                    deletes_address: ah(2),
                    deletes_entry_address: eh(3),
                },
            },
        };
        assert_eq!(op.clone(), op);
    }

    fn update_action(
        original_action_address: ActionHash,
        original_entry_address: EntryHash,
    ) -> TypedAction<UpdateData> {
        TypedAction {
            header: header(),
            data: UpdateData {
                original_action_address,
                original_entry_address,
                entry_type: holochain_integrity_types::action::EntryType::AgentPubKey,
                entry_hash: eh(9),
            },
        }
    }

    #[test]
    fn op_update_action_and_hashes_are_uniform_across_variants() {
        let action = update_action(ah(4), eh(5));
        let op = OpUpdate::<()>::CapClaim {
            action: action.clone(),
        };
        assert_eq!(op.action(), &action);
        assert_eq!(op.original_action_hash(), &ah(4));
        assert_eq!(op.original_entry_hash(), &eh(5));
    }

    #[test]
    fn op_update_entry_has_the_same_accessors_as_every_other_variant() {
        let action = update_action(ah(6), eh(7));
        let op = OpUpdate::<()>::Entry {
            app_entry: (),
            action: action.clone(),
        };
        assert_eq!(op.original_action_hash(), &ah(6));
        assert_eq!(op.original_entry_hash(), &eh(7));
    }

    #[test]
    fn op_entry_create_agent_exposes_agent_key() {
        let action = TypedAction {
            header: header(),
            data: CreateData {
                entry_type: holochain_integrity_types::action::EntryType::AgentPubKey,
                entry_hash: eh(9),
            },
        };
        let op = OpEntry::<()>::CreateAgent { action };
        assert_eq!(op.agent(), Some(AgentPubKey::from(eh(9))));
        assert_eq!(op.new_key(), None);
    }

    #[test]
    fn op_entry_update_agent_exposes_keys() {
        let action = update_action(ah(10), eh(11));
        let op = OpEntry::<()>::UpdateAgent { action };
        assert_eq!(op.new_key(), Some(AgentPubKey::from(eh(9))));
        assert_eq!(op.original_key(), Some(AgentPubKey::from(eh(11))));
    }

    #[test]
    fn op_entry_create_entry_has_no_agent_key() {
        let action = TypedAction {
            header: header(),
            data: CreateData {
                entry_type: holochain_integrity_types::action::EntryType::App(
                    crate::test_utils::short_hand::public_app_entry_def(0, 0),
                ),
                entry_hash: eh(12),
            },
        };
        let op = OpEntry::<()>::CreateEntry {
            app_entry: (),
            action,
        };
        assert_eq!(op.agent(), None);
    }
}
