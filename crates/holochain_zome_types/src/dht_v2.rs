//! Redesigned DHT state-model types (transitional — see `docs/design/state_model.md`).
//!
//! Re-exports the integrity-layer v2 types and adds the zome-layer aliases
//! [`SignedAction`] (data + signature) and [`SignedActionHashed`]
//! (content-addressed + signed). Also exposes the `op_type` INTEGER mapping
//! used by the DHT schema.

pub use holochain_integrity_types::dht_v2::*;

use crate::op::ChainOpType;
use crate::signature::Signed;
use holochain_integrity_types::record::SignedHashed;

/// Convert a legacy [`crate::record::SignedActionHashed`] (using the
/// variant-per-type `Action` enum) into the v2 [`SignedActionHashed`] (which
/// uses a flat [`ActionHeader`] + [`ActionData`] envelope).
///
/// The hash is carried over from the original — the v2 hash is content-derived,
/// so re-hashing would change it; the pre-hashed constructor preserves the
/// existing hash as the canonical identity during the dual-write transition.
pub fn from_legacy_signed_action(shh: &crate::record::SignedActionHashed) -> SignedActionHashed {
    use crate::action::Action as LegacyAction;
    let legacy_action = shh.action();
    let header = ActionHeader {
        author: legacy_action.author().clone(),
        timestamp: legacy_action.timestamp(),
        action_seq: legacy_action.action_seq(),
        prev_action: legacy_action.prev_action().cloned(),
    };

    let data = match legacy_action {
        LegacyAction::Dna(d) => ActionData::Dna(DnaData {
            dna_hash: d.hash.clone(),
        }),
        LegacyAction::AgentValidationPkg(d) => {
            ActionData::AgentValidationPkg(AgentValidationPkgData {
                membrane_proof: d.membrane_proof.clone(),
            })
        }
        LegacyAction::InitZomesComplete(_) => {
            ActionData::InitZomesComplete(InitZomesCompleteData {})
        }
        LegacyAction::Create(d) => ActionData::Create(CreateData {
            entry_type: d.entry_type.clone(),
            entry_hash: d.entry_hash.clone(),
        }),
        LegacyAction::Update(d) => ActionData::Update(UpdateData {
            original_action_address: d.original_action_address.clone(),
            original_entry_address: d.original_entry_address.clone(),
            entry_type: d.entry_type.clone(),
            entry_hash: d.entry_hash.clone(),
        }),
        LegacyAction::Delete(d) => ActionData::Delete(DeleteData {
            deletes_address: d.deletes_address.clone(),
            deletes_entry_address: d.deletes_entry_address.clone(),
        }),
        LegacyAction::CreateLink(d) => ActionData::CreateLink(CreateLinkData {
            base_address: d.base_address.clone(),
            target_address: d.target_address.clone(),
            zome_index: d.zome_index,
            link_type: d.link_type,
            tag: d.tag.clone(),
        }),
        LegacyAction::DeleteLink(d) => ActionData::DeleteLink(DeleteLinkData {
            base_address: d.base_address.clone(),
            link_add_address: d.link_add_address.clone(),
        }),
        LegacyAction::CloseChain(d) => ActionData::CloseChain(CloseChainData {
            new_target: d.new_target.clone(),
        }),
        LegacyAction::OpenChain(d) => ActionData::OpenChain(OpenChainData {
            prev_target: d.prev_target.clone(),
            close_hash: d.close_hash.clone(),
        }),
    };

    let v2_action = Action { header, data };
    let hashed = holo_hash::HoloHashed::with_pre_hashed(v2_action, shh.as_hash().clone());
    SignedHashed::with_presigned(hashed, shh.signature().clone())
}

/// A v2 [`Action`] with its [`crate::signature::Signature`] (no hash).
pub type SignedAction = Signed<Action>;

/// A v2 [`Action`] that is both hashed and signed.
pub type SignedActionHashed = SignedHashed<Action>;

/// A `Warrant` with its signature. Re-uses the existing `Warrant` type
/// from `holochain_zome_types::warrant` — unchanged by the v2 redesign.
pub use crate::warrant::SignedWarrant;

/// Maps [`ChainOpType`] onto the schema `op_type` INTEGER column (`1..=9`).
/// `0` is reserved and never written.
///
/// Variant ordering is pinned to `docs/design/state_model.md`:
///
/// | `op_type` | [`ChainOpType`] variant         | Semantic name  | Authority       |
/// |-----------|---------------------------------|----------------|-----------------|
/// | 1         | `StoreRecord`                   | CreateRecord   | action          |
/// | 2         | `StoreEntry`                    | CreateEntry    | entry           |
/// | 3         | `RegisterAgentActivity`         | AgentActivity  | agent           |
/// | 4         | `RegisterUpdatedContent`        | UpdateEntry    | entry           |
/// | 5         | `RegisterUpdatedRecord`         | UpdateRecord   | action          |
/// | 6         | `RegisterDeletedEntryAction`    | DeleteEntry    | entry           |
/// | 7         | `RegisterDeletedBy`             | DeleteRecord   | action          |
/// | 8         | `RegisterAddLink`               | CreateLink     | link base       |
/// | 9         | `RegisterRemoveLink`            | DeleteLink     | link base       |
impl From<ChainOpType> for i64 {
    fn from(t: ChainOpType) -> Self {
        match t {
            ChainOpType::StoreRecord => 1,
            ChainOpType::StoreEntry => 2,
            ChainOpType::RegisterAgentActivity => 3,
            ChainOpType::RegisterUpdatedContent => 4,
            ChainOpType::RegisterUpdatedRecord => 5,
            ChainOpType::RegisterDeletedEntryAction => 6,
            ChainOpType::RegisterDeletedBy => 7,
            ChainOpType::RegisterAddLink => 8,
            ChainOpType::RegisterRemoveLink => 9,
        }
    }
}

/// Inverse of [`From<ChainOpType> for i64`]. Returns `Err(v)` for `0` and any
/// value outside `1..=9`.
impl TryFrom<i64> for ChainOpType {
    type Error = i64;

    fn try_from(n: i64) -> Result<Self, Self::Error> {
        Ok(match n {
            1 => ChainOpType::StoreRecord,
            2 => ChainOpType::StoreEntry,
            3 => ChainOpType::RegisterAgentActivity,
            4 => ChainOpType::RegisterUpdatedContent,
            5 => ChainOpType::RegisterUpdatedRecord,
            6 => ChainOpType::RegisterDeletedEntryAction,
            7 => ChainOpType::RegisterDeletedBy,
            8 => ChainOpType::RegisterAddLink,
            9 => ChainOpType::RegisterRemoveLink,
            other => return Err(other),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_op_type_i64_roundtrip() {
        // Pinned forward-direction mapping. If a future change reorders
        // variants (e.g. a 6/7 swap) this will fail compilation or assertion.
        let expected = [
            (ChainOpType::StoreRecord, 1_i64),
            (ChainOpType::StoreEntry, 2),
            (ChainOpType::RegisterAgentActivity, 3),
            (ChainOpType::RegisterUpdatedContent, 4),
            (ChainOpType::RegisterUpdatedRecord, 5),
            (ChainOpType::RegisterDeletedEntryAction, 6),
            (ChainOpType::RegisterDeletedBy, 7),
            (ChainOpType::RegisterAddLink, 8),
            (ChainOpType::RegisterRemoveLink, 9),
        ];
        for (variant, n) in expected {
            assert_eq!(i64::from(variant), n);
            assert_eq!(ChainOpType::try_from(n).unwrap(), variant);
        }
        assert!(ChainOpType::try_from(0).is_err());
        assert!(ChainOpType::try_from(10).is_err());
    }
}
