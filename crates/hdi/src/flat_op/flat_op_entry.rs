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
        /// The Create action that creates this entry
        action: Action,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for an
    /// [`AgentPubKey`].
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The Create action that creates this agent's key
        action: Action,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for the newly
    /// created entry in an update.
    UpdateEntry {
        /// The hash of the [`Action`](holochain_integrity_types::action::Action) that created the
        /// original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The app defined entry with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data of the new entry.
        app_entry: ET,
        /// The Update action that updates this entry
        action: Action,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for an updated
    /// [`AgentPubKey`].
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original keys [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The Update action that updates this entry
        action: Action,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for a CapGrant
    CreateCapGrant {
        /// The cap grant entry data.
        entry: CapGrantEntry,
        /// The Create action that creates this cap grant
        action: Action,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for a CapClaim
    CreateCapClaim {
        /// The cap claim entry data.
        entry: CapClaimEntry,
        /// The Create action that creates this cap claim
        action: Action,
    },
    /// This operation updates the [`Entry`](holochain_integrity_types::entry::Entry) for a
    /// CapGrant
    UpdateCapGrant {
        /// The hash of the [`Action`](holochain_integrity_types::action::Action) that created the
        /// original [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        original_action_hash: ActionHash,
        /// The hash of the original
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        original_entry_hash: EntryHash,
        /// The Update action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        action: Action,
        /// The new entry to store
        entry: CapGrantEntry,
    },
    /// This operation updates the [`Entry`](holochain_integrity_types::entry::Entry) for a
    /// CapClaim
    UpdateCapClaim {
        /// The hash of the [`Action`](holochain_integrity_types::action::Action) that created the
        /// original [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        original_action_hash: ActionHash,
        /// The hash of the original
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        original_entry_hash: EntryHash,
        /// The Update action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        action: Action,
        /// The new entry to store
        entry: CapClaimEntry,
    },
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
        /// The action that updates this entry
        action: Action,
    },
    /// This operation registers an update from the original private
    /// [`Entry`](holochain_integrity_types::entry::Entry).
    PrivateEntry {
        /// The hash of the original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The unit version of the app defined entry type for the new entry.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The action that updates this entry
        action: Action,
    },
    /// This operation registers an update from the original [`AgentPubKey`].
    Agent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The Update action that updates the agent's key
        action: Action,
    },
    /// This operation registers an update from a Capability Claim.
    CapClaim {
        /// The hash of the original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The Update action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        action: Action,
    },
    /// This operation registers an update from a Capability Grant.
    CapGrant {
        /// The hash of the original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The Update action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        action: Action,
    },
}

/// Data specific to the [`Op::Delete`](holochain_integrity_types::op::Op::Delete)
/// operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpDelete {
    /// The Delete action that deletes this entry
    pub action: Action,
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey, EntryHash};
    use holochain_integrity_types::action::{ActionData, ActionHeader, DeleteData};

    fn action_from_data(data: ActionData) -> Action {
        Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_integrity_types::timestamp::Timestamp::from_micros(0),
                action_seq: 0,
                prev_action: None,
            },
            data,
        }
    }

    #[test]
    fn op_delete_constructs_and_clones() {
        let action = action_from_data(ActionData::Delete(DeleteData {
            deletes_address: ActionHash::from_raw_36(vec![2u8; 36]),
            deletes_entry_address: EntryHash::from_raw_36(vec![3u8; 36]),
        }));
        let op = OpDelete {
            action: action.clone(),
        };
        assert_eq!(op.clone(), op);
    }
}
