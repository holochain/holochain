//! The `OpRecord` type; see the description in the [`crate::flat_op`] parent module.
use super::*;
use holochain_integrity_types::prelude::{
    AgentValidationPkgData, CloseChainData, DnaData, InitZomesCompleteData, MigrationTarget,
    OpenChainData,
};

/// Data specific to the [`Op::CreateRecord`](holochain_integrity_types::op::Op::CreateRecord)
/// operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpRecord<ET: UnitEnum, LT> {
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an app
    /// defined entry type.
    CreateEntry {
        /// The app defined entry type with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data.
        app_entry: ET,
        /// The Create action that creates the entry.
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an app
    /// defined private entry type.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type. Note it is not possible to deserialize
        /// the full entry type here because we don't have the
        /// [`Entry`](holochain_integrity_types::entry::Entry) data.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The Create action that creates the entry.
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`AgentPubKey`] that has been created.
    CreateAgent {
        /// The Create action that creates the entry.
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// Capability Claim that has been created.
    CreateCapClaim {
        /// The Create action that creates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim).
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// Capability Grant that has been created.
    CreateCapGrant {
        /// The Create action that creates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant).
        action: TypedAction<CreateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated app defined entry type.
    UpdateEntry {
        /// The app defined entry type with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data from the new entry. Note the
        /// new entry type is always the same as the original entry type however the data may have
        /// changed.
        app_entry: ET,
        /// The Update action that updates the entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated app defined private entry type.
    UpdatePrivateEntry {
        /// The unit version of the app defined entry type. Note the new entry type is always the
        /// same as the original entry type however the data may have changed.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The Update action that updates the entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The Update action that updates the entry.
        action: TypedAction<UpdateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated Capability Claim.
    UpdateCapClaim {
        /// The Update action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim).
        action: TypedAction<UpdateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated Capability Grant.
    UpdateCapGrant {
        /// The Update action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant).
        action: TypedAction<UpdateData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// deleted app defined entry type.
    DeleteEntry {
        /// The Delete action that deletes the original entry.
        action: TypedAction<DeleteData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a new
    /// link.
    CreateLink {
        /// The app defined link type of this link.
        link_type: LT,
        /// The CreateLink action that creates this link.
        action: TypedAction<CreateLinkData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// deleted link and contains the original link's
    /// [`Action`](holochain_integrity_types::action::Action) hash.
    DeleteLink {
        /// The DeleteLink action that deletes the link.
        action: TypedAction<DeleteLinkData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::Dna`](holochain_integrity_types::action::ActionData::Dna).
    Dna {
        /// The hash of the DNA.
        dna_hash: DnaHash,
        /// The Dna action.
        action: TypedAction<DnaData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::OpenChain`](holochain_integrity_types::action::ActionData::OpenChain) and contains
    /// the previous chains's [`MigrationTarget`].
    OpenChain {
        /// Specifier for the previous chain that we are migrating from.
        previous_target: MigrationTarget,
        /// The hash of the corresponding CloseChain action.
        close_hash: ActionHash,
        /// The OpenChain action.
        action: TypedAction<OpenChainData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::CloseChain`](holochain_integrity_types::action::ActionData::CloseChain) and contains
    /// the new chains's [`MigrationTarget`], if applicable.
    CloseChain {
        /// Specifier for the new chain that we are migrating to.
        new_target: Option<MigrationTarget>,
        /// The CloseChain action.
        action: TypedAction<CloseChainData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::AgentValidationPkg`](holochain_integrity_types::action::ActionData::AgentValidationPkg)
    /// and contains the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA.
        membrane_proof: Option<MembraneProof>,
        /// The AgentValidationPkg action.
        action: TypedAction<AgentValidationPkgData>,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::InitZomesComplete`](holochain_integrity_types::action::ActionData::InitZomesComplete).
    InitZomesComplete {
        /// The InitZomesComplete action.
        action: TypedAction<InitZomesCompleteData>,
    },
}

impl<ET: UnitEnum, LT> OpRecord<ET, LT> {
    /// The agent key this action creates, for [`OpRecord::CreateAgent`].
    pub fn agent(&self) -> Option<AgentPubKey> {
        match self {
            OpRecord::CreateAgent { action } => Some(action.data.entry_hash.clone().into()),
            _ => None,
        }
    }

    /// The new agent key this action updates to, for [`OpRecord::UpdateAgent`].
    pub fn new_key(&self) -> Option<AgentPubKey> {
        match self {
            OpRecord::UpdateAgent { action } => Some(action.data.entry_hash.clone().into()),
            _ => None,
        }
    }

    /// The original agent key being updated, for [`OpRecord::UpdateAgent`].
    pub fn original_key(&self) -> Option<AgentPubKey> {
        match self {
            OpRecord::UpdateAgent { action } => {
                Some(action.data.original_entry_address.clone().into())
            }
            _ => None,
        }
    }
}
