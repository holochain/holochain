use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterAgentActivity`] operation.
pub enum OpActivity<UnitType, LT> {
    /// This operation registers the [`Action`] for an
    /// app defined entry type to the author's chain.
    CreateEntry {
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation registers the [`Action`] for an
    /// app defined private entry type to the author's chain.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation registers the [`Action`] for an
    /// [`AgentPubKey`] to the author's chain.
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation registers the [`Action`] for a
    /// Capability Claim to the author's chain.
    CreateCapClaim {
        /// The [`Create`] action that creates the [`crate::CapClaim`]
        action: Create,
    },
    /// This operation registers the [`Action`] for a
    /// Capability Grant to the author's chain.
    CreateCapGrant {
        /// The [`Create`] action that creates the [`crate::CapGrant`]
        action: Create,
    },
    /// This operation registers the [`Action`] for an
    /// updated app defined entry type to the author's chain.
    UpdateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated app defined private entry type to the author's chain.
    UpdatePrivateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated [`AgentPubKey`] to the author's chain.
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the agent's key
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated Capability Claim to the author's chain.
    UpdateCapClaim {
        /// The hash of the [`Action`] that created the original [`crate::CapClaim`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapClaim`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapClaim`]
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated Capability Grant to the author's chain.
    UpdateCapGrant {
        /// The hash of the [`Action`] that created the original [`crate::CapGrant`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapGrant`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapGrant`]
        action: Update,
    },
    /// This operation registers the [`Action`] for a
    /// deleted app defined entry type to the author's chain.
    DeleteEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The action that deletes the original entry
        action: Delete,
    },
    /// This operation registers the [`Action`] for a
    /// new link to the author's chain.
    CreateLink {
        /// The base address of the link.
        base_address: AnyLinkableHash,
        /// The target address of the link.
        target_address: AnyLinkableHash,
        /// The link's tag.
        tag: LinkTag,
        /// The app defined link type of this link.
        /// If this is [`None`] then the link type is defined
        /// in a different zome.
        link_type: Option<LT>,
        /// The action that creates this link
        action: CreateLink,
    },
    /// This operation registers the [`Action`] for a
    /// deleted link to the author's chain and contains
    /// the original link's [`Action`] hash.
    DeleteLink {
        /// The deleted links [`CreateLink`] [`Action`].
        original_action_hash: ActionHash,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The [`DeleteLink`] action that deletes the link
        action: DeleteLink,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::Dna`] to the author's chain.
    Dna {
        /// The hash of the DNA
        dna_hash: DnaHash,
        /// The [`Dna`] action
        action: Dna,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::OpenChain`] to the author's chain
    /// and contains the previous chains's [`DnaHash`].
    OpenChain {
        /// Hash of the prevous DNA that we are migrating from
        previous_dna_hash: DnaHash,
        /// The [`OpenChain`] action
        action: OpenChain,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::CloseChain`] to the author's chain
    /// and contains the new chains's [`DnaHash`].
    CloseChain {
        /// Hash of the new DNA that we are migrating to
        new_dna_hash: DnaHash,
        /// The [`CloseChain`] action
        action: CloseChain,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::AgentValidationPkg`] to the author's chain
    /// and contains the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA
        membrane_proof: Option<MembraneProof>,
        /// The [`AgentValidationPkg`] action
        action: AgentValidationPkg,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::InitZomesComplete`] to the author's chain.
    InitZomesComplete {
        /// The [`InitZomesComplete`] action
        action: InitZomesComplete,
    },
}
