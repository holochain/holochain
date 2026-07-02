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

/// Convert a legacy [`holochain_integrity_types::record::SignedActionHashed`]
/// (using the variant-per-type `Action` enum) into the v2
/// [`SignedActionHashed`] (which uses a flat [`ActionHeader`] +
/// [`ActionData`] envelope).
///
/// The hash is carried over from the original — the v2 hash is content-derived,
/// so re-hashing would change it; the pre-hashed constructor preserves the
/// existing hash as the canonical identity during the dual-write transition.
pub fn from_legacy_signed_action(
    shh: &holochain_integrity_types::record::SignedActionHashed,
) -> SignedActionHashed {
    // The action-only projection is shared with the `HashableContent` impls so
    // that the carried-over hash here is exactly the content-derived v2 hash.
    let v2_action = holochain_integrity_types::dht_v2::from_legacy_action(shh.action());
    let hashed = holo_hash::HoloHashed::with_pre_hashed(v2_action, shh.as_hash().clone());
    SignedHashed::with_presigned(hashed, shh.signature().clone())
}

/// Reverse of [`from_legacy_signed_action`]: build a legacy
/// [`holochain_integrity_types::record::SignedActionHashed`] from a v2
/// [`SignedActionHashed`].
///
/// **Lossy.** Legacy actions carry a `weight` field (rate-limit metadata)
/// that the v2 model deliberately discards; the reconstructed legacy
/// actions get the [`Default`] weight value. Because the legacy hash
/// includes the weight, the legacy hash of the reconstructed action will
/// generally differ from the original legacy hash. This helper preserves
/// the v2 hash on the returned legacy form so callers that need to use
/// the stored identity (e.g. local indexing) still can, but **the wire
/// hash will not match the stored hash** for ops that originally carried
/// non-default weights. Cross-version conductor compatibility is broken
/// by this; that is intentional during the storage migration.
pub fn to_legacy_signed_action(
    sah: &SignedActionHashed,
) -> holochain_integrity_types::record::SignedActionHashed {
    use holo_hash::ActionHash;
    use holochain_integrity_types::action::{
        Action as LegacyAction, AgentValidationPkg, CloseChain, Create, CreateLink, Delete,
        DeleteLink, Dna, InitZomesComplete, OpenChain, Update,
    };

    let v2_action = &sah.hashed.content;
    let header = &v2_action.header;
    let author = header.author.clone();
    let timestamp = header.timestamp;
    let action_seq = header.action_seq;
    // Legacy non-DNA actions require a `prev_action`. The DNA variant doesn't
    // have one; for every other variant the column is non-NULL in storage, so
    // unwrapping is safe in well-formed data. We fall back to a zero-hash to
    // keep this helper total.
    let prev_action = header
        .prev_action
        .clone()
        .unwrap_or_else(|| ActionHash::from_raw_36(vec![0u8; 36]));

    let legacy: LegacyAction = match &v2_action.data {
        ActionData::Dna(d) => LegacyAction::Dna(Dna {
            author,
            timestamp,
            hash: d.dna_hash.clone(),
        }),
        ActionData::AgentValidationPkg(d) => LegacyAction::AgentValidationPkg(AgentValidationPkg {
            author,
            timestamp,
            action_seq,
            prev_action,
            membrane_proof: d.membrane_proof.clone(),
        }),
        ActionData::InitZomesComplete(_) => LegacyAction::InitZomesComplete(InitZomesComplete {
            author,
            timestamp,
            action_seq,
            prev_action,
        }),
        ActionData::Create(d) => LegacyAction::Create(Create {
            author,
            timestamp,
            action_seq,
            prev_action,
            entry_type: d.entry_type.clone(),
            entry_hash: d.entry_hash.clone(),
            weight: Default::default(),
        }),
        ActionData::Update(d) => LegacyAction::Update(Update {
            author,
            timestamp,
            action_seq,
            prev_action,
            original_action_address: d.original_action_address.clone(),
            original_entry_address: d.original_entry_address.clone(),
            entry_type: d.entry_type.clone(),
            entry_hash: d.entry_hash.clone(),
            weight: Default::default(),
        }),
        ActionData::Delete(d) => LegacyAction::Delete(Delete {
            author,
            timestamp,
            action_seq,
            prev_action,
            deletes_address: d.deletes_address.clone(),
            deletes_entry_address: d.deletes_entry_address.clone(),
            weight: Default::default(),
        }),
        ActionData::CreateLink(d) => LegacyAction::CreateLink(CreateLink {
            author,
            timestamp,
            action_seq,
            prev_action,
            base_address: d.base_address.clone(),
            target_address: d.target_address.clone(),
            zome_index: d.zome_index,
            link_type: d.link_type,
            tag: d.tag.clone(),
            weight: Default::default(),
        }),
        ActionData::DeleteLink(d) => LegacyAction::DeleteLink(DeleteLink {
            author,
            timestamp,
            action_seq,
            prev_action,
            base_address: d.base_address.clone(),
            link_add_address: d.link_add_address.clone(),
        }),
        ActionData::CloseChain(d) => LegacyAction::CloseChain(CloseChain {
            author,
            timestamp,
            action_seq,
            prev_action,
            new_target: d.new_target.clone(),
        }),
        ActionData::OpenChain(d) => LegacyAction::OpenChain(OpenChain {
            author,
            timestamp,
            action_seq,
            prev_action,
            prev_target: d.prev_target.clone(),
            close_hash: d.close_hash.clone(),
        }),
    };

    let hashed = holo_hash::HoloHashed::with_pre_hashed(legacy, sah.as_hash().clone());
    SignedHashed::with_presigned(hashed, sah.signature().clone())
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
    use crate::action::{Create, CreateLink, EntryType};
    use crate::entry_def::EntryVisibility;
    use crate::link::LinkTag;
    use crate::prelude::AppEntryDef;
    use crate::signature::Signature;
    use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, EntryHash};
    use holochain_integrity_types::action::Action as LegacyAction;
    use holochain_integrity_types::record::SignedHashed;
    use holochain_timestamp::Timestamp;

    fn legacy_signed_create() -> holochain_integrity_types::record::SignedActionHashed {
        let action = LegacyAction::Create(Create {
            author: AgentPubKey::from_raw_36(vec![1u8; 36]),
            timestamp: Timestamp::from_micros(1_000),
            action_seq: 4,
            prev_action: ActionHash::from_raw_36(vec![2u8; 36]),
            entry_type: EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
            weight: Default::default(),
        });
        let hashed =
            holo_hash::HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![9u8; 36]));
        SignedHashed::with_presigned(hashed, Signature::from([7u8; 64]))
    }

    fn legacy_signed_create_link() -> holochain_integrity_types::record::SignedActionHashed {
        let action = LegacyAction::CreateLink(CreateLink {
            author: AgentPubKey::from_raw_36(vec![1u8; 36]),
            timestamp: Timestamp::from_micros(2_000),
            action_seq: 5,
            prev_action: ActionHash::from_raw_36(vec![2u8; 36]),
            base_address: AnyLinkableHash::from_raw_36_and_type(
                vec![4u8; 36],
                holo_hash::hash_type::AnyLinkable::Entry,
            ),
            target_address: AnyLinkableHash::from_raw_36_and_type(
                vec![5u8; 36],
                holo_hash::hash_type::AnyLinkable::Entry,
            ),
            zome_index: 1.into(),
            link_type: 2.into(),
            tag: LinkTag(vec![0xAA, 0xBB]),
            weight: Default::default(),
        });
        let hashed =
            holo_hash::HoloHashed::with_pre_hashed(action, ActionHash::from_raw_36(vec![11u8; 36]));
        SignedHashed::with_presigned(hashed, Signature::from([8u8; 64]))
    }

    #[test]
    fn from_legacy_signed_action_preserves_hash_and_signature() {
        let legacy = legacy_signed_create();
        let v2 = from_legacy_signed_action(&legacy);

        assert_eq!(v2.as_hash(), legacy.as_hash());
        assert_eq!(v2.signature(), legacy.signature());
    }

    #[test]
    fn from_legacy_signed_action_maps_create_fields() {
        let legacy = legacy_signed_create();
        let v2 = from_legacy_signed_action(&legacy);
        let action = &v2.hashed.content;

        assert_eq!(&action.header.author, legacy.action().author());
        assert_eq!(action.header.timestamp, legacy.action().timestamp());
        assert_eq!(action.header.action_seq, legacy.action().action_seq());
        assert_eq!(
            action.header.prev_action.as_ref(),
            legacy.action().prev_action()
        );
        match (&action.data, legacy.action()) {
            (ActionData::Create(v2_data), LegacyAction::Create(legacy_data)) => {
                assert_eq!(v2_data.entry_hash, legacy_data.entry_hash);
                assert_eq!(v2_data.entry_type, legacy_data.entry_type);
            }
            _ => panic!("unexpected variant pair"),
        }
    }

    #[test]
    fn from_legacy_signed_action_maps_create_link_fields() {
        let legacy = legacy_signed_create_link();
        let v2 = from_legacy_signed_action(&legacy);

        match (&v2.hashed.content.data, legacy.action()) {
            (ActionData::CreateLink(v2_data), LegacyAction::CreateLink(legacy_data)) => {
                assert_eq!(v2_data.base_address, legacy_data.base_address);
                assert_eq!(v2_data.target_address, legacy_data.target_address);
                assert_eq!(v2_data.zome_index, legacy_data.zome_index);
                assert_eq!(v2_data.link_type, legacy_data.link_type);
                assert_eq!(v2_data.tag, legacy_data.tag);
            }
            _ => panic!("unexpected variant pair"),
        }
    }

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
