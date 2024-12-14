use holochain_integrity_types::MigrationTarget;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreRecord`](holochain_integrity_types::op::Op::StoreRecord)
/// operation.
pub enum OpRecord<ET: UnitEnum, LT> {
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an app
    /// defined entry type.
    CreateEntry {
        /// The app defined entry type with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data.
        app_entry: ET,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an app
    /// defined private entry type.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type. Note it is not possible to deserialize
        /// the full entry type here because we don't have the
        /// [`Entry`](holochain_integrity_types::entry::Entry) data.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`AgentPubKey`] that has been created.
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// Capability Claim that has been created.
    CreateCapClaim {
        /// The [`Create`] action that creates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        action: Create,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// Capability Grant that has been created.
    CreateCapGrant {
        /// The [`Create`] action that creates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        action: Create,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated app defined entry type.
    UpdateEntry {
        /// The hash of the [`Action`](holochain_integrity_types::action::Action) that created the
        /// original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`](holochain_integrity_types::entry::Entry) data from the new entry. Note the
        /// new entry type is always the same as the original entry type however the data may have
        /// changed.
        app_entry: ET,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated app defined private entry type.
    UpdatePrivateEntry {
        /// The hash of the [`Action`](holochain_integrity_types::action::Action) that created the
        /// original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type. Note the new entry type is always the
        /// same as the original entry type however the data may have changed.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The hash of the [`Action`](holochain_integrity_types::action::Action) that created the
        /// original key
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated Capability Claim.
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
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// updated Capability Grant.
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
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// deleted app defined entry type.
    DeleteEntry {
        /// The hash of the [`Action`](holochain_integrity_types::action::Action) that created the
        /// original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The [`Delete`] action that creates the entry
        action: Delete,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a new
    /// link.
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
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for a
    /// deleted link and contains the original link's
    /// [`Action`](holochain_integrity_types::action::Action) hash.
    DeleteLink {
        /// The deleted links [`CreateLink`] [`Action`](holochain_integrity_types::action::Action).
        original_action_hash: ActionHash,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The [`DeleteLink`] action that deletes the link
        action: DeleteLink,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::Dna`](holochain_integrity_types::action::Action::Dna).
    Dna {
        /// The hash of the DNA
        dna_hash: DnaHash,
        /// The [`Dna`] action
        action: Dna,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::OpenChain`](holochain_integrity_types::action::Action::OpenChain) and contains
    /// the previous chains's [`MigrationTarget`].
    OpenChain {
        /// Specifier for the previous chain that we are migrating from
        previous_target: MigrationTarget,
        /// The hash of the corresponding CloseChain action.
        close_hash: ActionHash,
        /// The [`OpenChain`] action
        action: OpenChain,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::CloseChain`](holochain_integrity_types::action::Action::CloseChain) and contains
    /// the new chains's [`MigrationTarget`], if applicable.
    CloseChain {
        /// Specifier for the new chain that we are migrating to
        new_target: Option<MigrationTarget>,
        /// The [`CloseChain`] action
        action: CloseChain,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::AgentValidationPkg`](holochain_integrity_types::action::Action::AgentValidationPkg)
    /// and contains the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA
        membrane_proof: Option<MembraneProof>,
        /// The [`AgentValidationPkg`] action
        action: AgentValidationPkg,
    },
    /// This operation stores the [`Record`](holochain_integrity_types::record::Record) for an
    /// [`Action::InitZomesComplete`](holochain_integrity_types::action::Action::InitZomesComplete).
    InitZomesComplete {
        /// The [`InitZomesComplete`] action
        action: InitZomesComplete,
    },
}

impl<ET: UnitEnum, LT> OpRecord<ET, LT> {
    /// DRY constructor
    pub fn open_chain(action: OpenChain) -> Self {
        Self::OpenChain {
            previous_target: action.prev_target.clone(),
            close_hash: action.close_hash.clone(),
            action,
        }
    }

    /// DRY constructor
    pub fn close_chain(action: CloseChain) -> Self {
        Self::CloseChain {
            new_target: action.new_target.clone(),
            action,
        }
    }
}
