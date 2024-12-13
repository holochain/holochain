use holochain_integrity_types::{CapClaimEntry, CapGrantEntry};

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreEntry`](holochain_integrity_types::op::Op::StoreEntry)
/// operation.
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
        /// The [`Create`] action that creates this entry
        action: Create,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for an
    /// [`AgentPubKey`].
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates this agent's key
        action: Create,
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
        /// The [`Update`] action that updates this entry
        action: Update,
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
        /// The [`Update`] action that updates this entry
        action: Update,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for a CapGrant
    CreateCapGrant {
        /// The cap grant entry data.
        entry: CapGrantEntry,
        /// The [`Create`] action that creates this cap grant
        action: Create,
    },
    /// This operation stores the [`Entry`](holochain_integrity_types::entry::Entry) for a CapClaim
    CreateCapClaim {
        /// The cap claim entry data.
        entry: CapClaimEntry,
        /// The [`Create`] action that creates this cap claim
        action: Create,
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
        /// The [`Update`] action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        action: Update,
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
        /// The [`Update`] action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        action: Update,
        /// The new entry to store
        entry: CapClaimEntry,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterUpdate`](holochain_integrity_types::op::Op::RegisterUpdate)
/// operation.
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
        action: Update,
    },
    /// This operation registers an update from the original private
    /// [`Entry`](holochain_integrity_types::entry::Entry).
    PrivateEntry {
        /// The hash of the original original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The unit version of the app defined entry type for the new entry.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The action that updates this entry
        action: Update,
    },
    /// This operation registers an update from the original [`AgentPubKey`].
    Agent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the agent's key
        action: Update,
    },
    /// This operation registers an update from a Capability Claim.
    CapClaim {
        /// The hash of the original original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        action: Update,
    },
    /// This operation registers an update from a Capability Grant.
    CapGrant {
        /// The hash of the original original
        /// [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        action: Update,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterDelete`](holochain_integrity_types::op::Op::RegisterDelete)
/// operation.
pub struct OpDelete {
    /// The [`Delete`] action that deletes this entry
    pub action: Delete,
}
