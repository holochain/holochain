//! The `OpActivity` type; see the description in the [`crate::flat_op`] parent module.
use super::*;
use holochain_integrity_types::prelude::MigrationTarget;

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
        /// The Create action that creates the entry
        action: Action,
    },
    /// This operation registers the Action for an
    /// app defined private entry type to the author's chain.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type. If this is [`None`] then the entry type
        /// is defined in a different zome.
        app_entry_type: Option<UnitType>,
        /// The Create action that creates the entry
        action: Action,
    },
    /// This operation registers the Action for an
    /// [`AgentPubKey`] to the author's chain.
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The Create action that creates the entry
        action: Action,
    },
    /// This operation registers the Action for a
    /// Capability Claim to the author's chain.
    CreateCapClaim {
        /// The Create action that creates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        action: Action,
    },
    /// This operation registers the Action for a
    /// Capability Grant to the author's chain.
    CreateCapGrant {
        /// The Create action that creates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        action: Action,
    },
    /// This operation registers the Action for an
    /// updated app defined entry type to the author's chain.
    UpdateEntry {
        /// The hash of the Action that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type. If this is [`None`] then the entry type
        /// is defined in a different zome.
        app_entry_type: Option<UnitType>,
        /// The Update action that updates the entry
        action: Action,
    },
    /// This operation registers the Action for an
    /// updated app defined private entry type to the author's chain.
    UpdatePrivateEntry {
        /// The hash of the Action that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined in a different zome.
        app_entry_type: Option<UnitType>,
        /// The Update action that updates the entry
        action: Action,
    },
    /// This operation registers the Action for an
    /// updated [`AgentPubKey`] to the author's chain.
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the Action that created the original entry
        original_action_hash: ActionHash,
        /// The Update action that updates the agent's key
        action: Action,
    },
    /// This operation registers the Action for an
    /// updated Capability Claim to the author's chain.
    UpdateCapClaim {
        /// The hash of the Action that created the original
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        original_action_hash: ActionHash,
        /// The hash of the original
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        original_entry_hash: EntryHash,
        /// The Update action that updates the
        /// [`CapClaim`](holochain_integrity_types::action::EntryType::CapClaim)
        action: Action,
    },
    /// This operation registers the Action for an
    /// updated Capability Grant to the author's chain.
    UpdateCapGrant {
        /// The hash of the Action that created the original
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        original_action_hash: ActionHash,
        /// The hash of the original
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        original_entry_hash: EntryHash,
        /// The Update action that updates the
        /// [`CapGrant`](holochain_integrity_types::action::EntryType::CapGrant)
        action: Action,
    },
    /// This operation registers the Action for a
    /// deleted app defined entry type to the author's chain.
    DeleteEntry {
        /// The hash of the Action that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The action that deletes the original entry
        action: Action,
    },
    /// This operation registers the Action for a
    /// new link to the author's chain.
    CreateLink {
        /// The base address of the link.
        base_address: AnyLinkableHash,
        /// The target address of the link.
        target_address: AnyLinkableHash,
        /// The link's tag.
        tag: LinkTag,
        /// The app defined link type of this link.
        /// If this is [`None`] then the link type is defined in a different zome.
        link_type: Option<LT>,
        /// The action that creates this link
        action: Action,
    },
    /// This operation registers the Action for a
    /// deleted link to the author's chain and contains the original link's
    /// Action hash.
    DeleteLink {
        /// The deleted link's CreateLink Action.
        original_action_hash: ActionHash,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The DeleteLink action that deletes the link
        action: Action,
    },
    /// This operation registers the Action for an
    /// [`Action::Dna`](holochain_integrity_types::action::ActionData::Dna) to the author's chain.
    Dna {
        /// The hash of the DNA
        dna_hash: DnaHash,
        /// The Dna action
        action: Action,
    },
    /// This operation registers the Action for an
    /// [`Action::OpenChain`](holochain_integrity_types::action::ActionData::OpenChain) to the author's
    /// chain and contains the previous chain's [`MigrationTarget`].
    OpenChain {
        /// Target for the previous chain that we are migrating from
        previous_target: MigrationTarget,
        /// Hash of the corresponding CloseChain.
        close_hash: ActionHash,
        /// The OpenChain action
        action: Action,
    },
    /// This operation registers the Action for an
    /// [`Action::CloseChain`](holochain_integrity_types::action::ActionData::CloseChain) to the
    /// author's chain and contains the new chain's [`MigrationTarget`] if applicable.
    CloseChain {
        /// Target for the new chain that we are migrating to
        new_target: Option<MigrationTarget>,
        /// The CloseChain action
        action: Action,
    },
    /// This operation registers the Action for an
    /// [`Action::AgentValidationPkg`](holochain_integrity_types::action::ActionData::AgentValidationPkg)
    /// to the author's chain and contains the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA
        membrane_proof: Option<MembraneProof>,
        /// The AgentValidationPkg action
        action: Action,
    },
    /// This operation registers the Action for an
    /// [`Action::InitZomesComplete`](holochain_integrity_types::action::ActionData::InitZomesComplete)
    /// to the author's chain.
    InitZomesComplete {
        /// The InitZomesComplete action
        action: Action,
    },
}

impl<UnitType, LT> OpActivity<UnitType, LT> {
    /// DRY constructor. `action.data` must be
    /// [`ActionData::OpenChain`](holochain_integrity_types::action::ActionData::OpenChain).
    pub(crate) fn open_chain(action: Action) -> Self {
        let (previous_target, close_hash) = match &action.data {
            holochain_integrity_types::action::ActionData::OpenChain(d) => {
                (d.prev_target.clone(), d.close_hash.clone())
            }
            other => unreachable!("OpActivity::open_chain requires OpenChain data, got {other:?}"),
        };
        Self::OpenChain {
            previous_target,
            close_hash,
            action,
        }
    }

    /// DRY constructor. `action.data` must be
    /// [`ActionData::CloseChain`](holochain_integrity_types::action::ActionData::CloseChain).
    pub(crate) fn close_chain(action: Action) -> Self {
        let new_target = match &action.data {
            holochain_integrity_types::action::ActionData::CloseChain(d) => d.new_target.clone(),
            other => {
                unreachable!("OpActivity::close_chain requires CloseChain data, got {other:?}")
            }
        };
        Self::CloseChain { new_target, action }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holo_hash::{ActionHash, AgentPubKey, DnaHash};
    use holochain_integrity_types::action::{
        ActionData, ActionHeader, CloseChainData, OpenChainData,
    };

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
    fn open_chain_constructor_extracts_fields() {
        let target = MigrationTarget::Dna(DnaHash::from_raw_36(vec![5u8; 36]));
        let close = ActionHash::from_raw_36(vec![6u8; 36]);
        let action = action_from_data(ActionData::OpenChain(OpenChainData {
            prev_target: target.clone(),
            close_hash: close.clone(),
        }));
        let op = OpActivity::<(), ()>::open_chain(action);
        match op {
            OpActivity::OpenChain {
                previous_target,
                close_hash,
                ..
            } => {
                assert_eq!(previous_target, target);
                assert_eq!(close_hash, close);
            }
            _ => panic!("expected OpenChain"),
        }
    }

    #[test]
    fn close_chain_constructor_extracts_target() {
        let action = action_from_data(ActionData::CloseChain(CloseChainData { new_target: None }));
        let op = OpActivity::<(), ()>::close_chain(action);
        match op {
            OpActivity::CloseChain { new_target, .. } => assert_eq!(new_target, None),
            _ => panic!("expected CloseChain"),
        }
    }
}
