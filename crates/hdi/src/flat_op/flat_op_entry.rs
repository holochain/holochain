use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreEntry`] operation.
pub enum OpEntry<ET>
where
    ET: UnitEnum,
{
    /// This operation stores the [`Entry`] for an
    /// app defined entry type.
    CreateEntry {
        /// The app defined entry with the deserialized
        /// [`Entry`] data.
        app_entry: ET,
        /// The [`Create`] action that creates this entry
        action: Create,
    },
    /// This operation stores the [`Entry`] for an
    /// [`AgentPubKey`].
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates this agent's key
        action: Create,
    },
    /// This operation stores the [`Entry`] for the
    /// newly created entry in an update.
    UpdateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The app defined entry with the deserialized
        /// [`Entry`] data of the new entry.
        app_entry: ET,
        /// The [`Update`] action that updates this entry
        action: Update,
    },
    /// This operation stores the [`Entry`] for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original keys [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates this entry
        action: Update,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterUpdate`] operation.
pub enum OpUpdate<ET>
where
    ET: UnitEnum,
{
    /// This operation registers an update from
    /// the original [`Entry`].
    Entry {
        /// The original [`Create`] or [`Update`] [`Action`].
        original_action: EntryCreationAction,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data of the original entry.
        original_app_entry: ET,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data of the new entry.
        app_entry: ET,
        /// The action that updates this entry
        action: Update,
    },
    /// This operation registers an update from
    /// the original private [`Entry`].
    PrivateEntry {
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The unit version of the app defined entry type
        /// for the original entry.
        original_app_entry_type: <ET as UnitEnum>::Unit,
        /// The unit version of the app defined entry type
        /// for the new entry.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The action that updates this entry
        action: Update,
    },
    /// This operation registers an update from
    /// the original [`AgentPubKey`].
    Agent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the agent's key
        action: Update,
    },
    /// This operation registers an update from
    /// a Capability Claim.
    CapClaim {
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the [`crate::CapClaim`]
        action: Update,
    },
    /// This operation registers an update from
    /// a Capability Grant.
    CapGrant {
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the [`crate::CapGrant`]
        action: Update,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterDelete`] operation.
pub enum OpDelete<ET>
where
    ET: UnitEnum,
{
    /// This operation registers a deletion to the
    /// original [`Entry`].
    Entry {
        /// The entries original [`Create`] or [`Update`] [`Action`].
        original_action: EntryCreationAction,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data from the deleted entry.
        original_app_entry: ET,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to the
    /// original private [`Entry`].
    PrivateEntry {
        /// The entries original [`EntryCreationAction`].
        original_action: EntryCreationAction,
        /// The unit version of the app defined entry type
        /// of the deleted entry.
        original_app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to an
    /// [`AgentPubKey`].
    Agent {
        /// The deleted [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the deleted keys [`Action`].
        original_action: EntryCreationAction,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to a
    /// Capability Claim.
    CapClaim {
        /// The deleted Capability Claim's [`Action`].
        original_action: EntryCreationAction,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to a
    /// Capability Grant.
    CapGrant {
        /// The deleted Capability Claim's [`Action`].
        original_action: EntryCreationAction,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
}
