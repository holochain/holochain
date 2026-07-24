//! The `OpActivity` type; see the description in the [`crate::flat_op`] parent module.
use super::*;
use holochain_integrity_types::prelude::{
    AgentValidationPkgData, CloseChainData, DnaData, InitZomesCompleteData, MigrationTarget,
    OpenChainData,
};

/// Data specific to the
/// [`Op::AgentActivity`](holochain_integrity_types::op::Op::AgentActivity)
/// operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpActivity<UnitType, LT> {
    /// This operation registers the Action for an
    /// app defined entry type to the author's chain.
    CreateEntry {
        /// The unit version of the app defined entry type. If this is [`None`] then the entry type
        /// is defined in a different zome.
        app_entry_type: Option<UnitType>,
        /// The Create action that creates the entry.
        action: TypedAction<CreateData>,
    },
    /// This operation registers the Action for an
    /// app defined private entry type to the author's chain.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type. If this is [`None`] then the entry type
        /// is defined in a different zome.
        app_entry_type: Option<UnitType>,
        /// The Create action that creates the entry.
        action: TypedAction<CreateData>,
    },
    /// This operation registers the Action for an
    /// [`AgentPubKey`] to the author's chain.
    CreateAgent {
        /// The agent key this action creates.
        agent: AgentPubKey,
        /// The Create action that creates the entry.
        action: TypedAction<CreateData>,
    },
    /// This operation registers the Action for a
    /// Capability Claim to the author's chain.
    CreateCapClaim {
        /// The Create action that creates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim).
        action: TypedAction<CreateData>,
    },
    /// This operation registers the Action for a
    /// Capability Grant to the author's chain.
    CreateCapGrant {
        /// The Create action that creates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant).
        action: TypedAction<CreateData>,
    },
    /// This operation registers the Action for an
    /// updated app defined entry type to the author's chain.
    UpdateEntry {
        /// The unit version of the app defined entry type. If this is [`None`] then the entry type
        /// is defined in a different zome.
        app_entry_type: Option<UnitType>,
        /// The Update action that updates the entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation registers the Action for an
    /// updated app defined private entry type to the author's chain.
    UpdatePrivateEntry {
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined in a different zome.
        app_entry_type: Option<UnitType>,
        /// The Update action that updates the entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation registers the Action for an
    /// updated [`AgentPubKey`] to the author's chain.
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The Update action that updates the agent's key.
        action: TypedAction<UpdateData>,
    },
    /// This operation registers the Action for an
    /// updated Capability Claim to the author's chain.
    UpdateCapClaim {
        /// The Update action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim).
        action: TypedAction<UpdateData>,
    },
    /// This operation registers the Action for an
    /// updated Capability Grant to the author's chain.
    UpdateCapGrant {
        /// The Update action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant).
        action: TypedAction<UpdateData>,
    },
    /// This operation registers the Action for a
    /// deleted app defined entry type to the author's chain.
    DeleteEntry {
        /// The action that deletes the original entry.
        action: TypedAction<DeleteData>,
    },
    /// This operation registers the Action for a
    /// new link to the author's chain.
    CreateLink {
        /// The app defined link type of this link.
        /// If this is [`None`] then the link type is defined in a different zome.
        link_type: Option<LT>,
        /// The action that creates this link.
        action: TypedAction<CreateLinkData>,
    },
    /// This operation registers the Action for a
    /// deleted link to the author's chain.
    DeleteLink {
        /// The DeleteLink action that deletes the link.
        action: TypedAction<DeleteLinkData>,
    },
    /// This operation registers the Action for an
    /// [`Action::Dna`](holochain_integrity_types::action::ActionData::Dna) to the author's chain.
    Dna {
        /// The hash of the DNA.
        dna_hash: DnaHash,
        /// The Dna action.
        action: TypedAction<DnaData>,
    },
    /// This operation registers the Action for an
    /// [`Action::OpenChain`](holochain_integrity_types::action::ActionData::OpenChain) to the author's
    /// chain and contains the previous chain's [`MigrationTarget`].
    OpenChain {
        /// Target for the previous chain that we are migrating from.
        previous_target: MigrationTarget,
        /// Hash of the corresponding CloseChain.
        close_hash: ActionHash,
        /// The OpenChain action.
        action: TypedAction<OpenChainData>,
    },
    /// This operation registers the Action for an
    /// [`Action::CloseChain`](holochain_integrity_types::action::ActionData::CloseChain) to the
    /// author's chain and contains the new chain's [`MigrationTarget`] if applicable.
    CloseChain {
        /// Target for the new chain that we are migrating to.
        new_target: Option<MigrationTarget>,
        /// The CloseChain action.
        action: TypedAction<CloseChainData>,
    },
    /// This operation registers the Action for an
    /// [`Action::AgentValidationPkg`](holochain_integrity_types::action::ActionData::AgentValidationPkg)
    /// to the author's chain and contains the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA.
        membrane_proof: Option<MembraneProof>,
        /// The AgentValidationPkg action.
        action: TypedAction<AgentValidationPkgData>,
    },
    /// This operation registers the Action for an
    /// [`Action::InitZomesComplete`](holochain_integrity_types::action::ActionData::InitZomesComplete)
    /// to the author's chain.
    InitZomesComplete {
        /// The InitZomesComplete action.
        action: TypedAction<InitZomesCompleteData>,
    },
}
