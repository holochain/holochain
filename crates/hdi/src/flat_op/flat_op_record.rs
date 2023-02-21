use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreRecord`] operation.
pub enum OpRecord<ET: UnitEnum, LT> {
    /// This operation stores the [`Record`] for an
    /// app defined entry type.
    CreateEntry {
        /// The app defined entry type with the deserialized
        /// [`Entry`] data.
        app_entry: ET,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`] for an
    /// app defined private entry type.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type.
        /// Note it is not possible to deserialize the full
        /// entry type here because we don't have the [`Entry`] data.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`] for an
    /// [`AgentPubKey`] that has been created.
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`] for a
    /// Capability Claim that has been created.
    CreateCapClaim {
        /// The [`Create`] action that creates the [`crate::CapClaim`]
        action: Create,
    },
    /// This operation stores the [`Record`] for a
    /// Capability Grant that has been created.
    CreateCapGrant {
        /// The [`Create`] action that creates the [`crate::CapGrant`]
        action: Create,
    },
    /// This operation stores the [`Record`] for an
    /// updated app defined entry type.
    UpdateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data from the new entry.
        /// Note the new entry type is always the same as the
        /// original entry type however the data may have changed.
        app_entry: ET,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated app defined private entry type.
    UpdatePrivateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// Note the new entry type is always the same as the
        /// original entry type however the data may have changed.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The hash of the [`Action`] that created the original key
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated Capability Claim.
    UpdateCapClaim {
        /// The hash of the [`Action`] that created the original [`crate::CapClaim`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapClaim`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapClaim`]
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated Capability Grant.
    UpdateCapGrant {
        /// The hash of the [`Action`] that created the original [`crate::CapGrant`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapGrant`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapGrant`]
        action: Update,
    },
    /// This operation stores the [`Record`] for a
    /// deleted app defined entry type.
    DeleteEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The [`Delete`] action that creates the entry
        action: Delete,
    },
    /// This operation stores the [`Record`] for a
    /// new link.
    CreateLink {
        /// The base address of the link.
        base_address: AnyLinkableHash,
        /// The target address of the link.
        target_address: AnyLinkableHash,
        /// The link's tag.
        tag: LinkTag,
        /// The app defined link type of this link.
        link_type: LT,
        /// The [`CreateLink`] action that creates this link
        action: CreateLink,
    },
    /// This operation stores the [`Record`] for a
    /// deleted link and contains the original link's
    /// [`Action`] hash.
    DeleteLink {
        /// The deleted links [`CreateLink`] [`Action`].
        original_action_hash: ActionHash,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The [`DeleteLink`] action that deletes the link
        action: DeleteLink,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::Dna`].
    Dna {
        /// The hash of the DNA
        dna_hash: DnaHash,
        /// The [`Dna`] action
        action: Dna,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::OpenChain`] and contains the previous
    /// chains's [`DnaHash`].
    OpenChain {
        /// Hash of the prevous DNA that we are migrating from
        previous_dna_hash: DnaHash,
        /// The [`OpenChain`] action
        action: OpenChain,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::CloseChain`] and contains the new
    /// chains's [`DnaHash`].
    CloseChain {
        /// Hash of the new DNA that we are migrating to
        new_dna_hash: DnaHash,
        /// The [`CloseChain`] action
        action: CloseChain,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::AgentValidationPkg`] and contains
    /// the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA
        membrane_proof: Option<MembraneProof>,
        /// The [`AgentValidationPkg`] action
        action: AgentValidationPkg,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::InitZomesComplete`].
    InitZomesComplete {
        /// The [`InitZomesComplete`] action
        action: InitZomesComplete,
    },
}
