//! Redesigned DHT state-model types (transitional — see `docs/design/state_model.md`).

pub use holochain_integrity_types::dht_v2::*;

use crate::op::ChainOpType;
use crate::signature::Signed;
use holochain_integrity_types::record::SignedHashed;

/// A v2 `Action` with its signature.
pub type SignedAction = Signed<Action>;

/// A v2 `Action` that is both hashed and signed.
pub type SignedActionHashed = SignedHashed<Action>;

/// A `Warrant` with its signature. Re-uses the existing `Warrant` type
/// from `holochain_zome_types::warrant` — unchanged by the v2 redesign.
pub use crate::warrant::SignedWarrant;

/// Map the existing `ChainOpType` enum onto the schema `op_type` INTEGER
/// column (1..=9). `0` is reserved.
pub fn chain_op_type_to_i64(t: ChainOpType) -> i64 {
    match t {
        ChainOpType::StoreRecord => 1,
        ChainOpType::StoreEntry => 2,
        ChainOpType::RegisterAgentActivity => 3,
        ChainOpType::RegisterUpdatedContent => 4,
        ChainOpType::RegisterUpdatedRecord => 5,
        ChainOpType::RegisterDeletedBy => 6,
        ChainOpType::RegisterDeletedEntryAction => 7,
        ChainOpType::RegisterAddLink => 8,
        ChainOpType::RegisterRemoveLink => 9,
    }
}

pub fn chain_op_type_from_i64(n: i64) -> Option<ChainOpType> {
    Some(match n {
        1 => ChainOpType::StoreRecord,
        2 => ChainOpType::StoreEntry,
        3 => ChainOpType::RegisterAgentActivity,
        4 => ChainOpType::RegisterUpdatedContent,
        5 => ChainOpType::RegisterUpdatedRecord,
        6 => ChainOpType::RegisterDeletedBy,
        7 => ChainOpType::RegisterDeletedEntryAction,
        8 => ChainOpType::RegisterAddLink,
        9 => ChainOpType::RegisterRemoveLink,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_op_type_i64_roundtrip() {
        for t in [
            ChainOpType::StoreRecord,
            ChainOpType::StoreEntry,
            ChainOpType::RegisterAgentActivity,
            ChainOpType::RegisterUpdatedContent,
            ChainOpType::RegisterUpdatedRecord,
            ChainOpType::RegisterDeletedBy,
            ChainOpType::RegisterDeletedEntryAction,
            ChainOpType::RegisterAddLink,
            ChainOpType::RegisterRemoveLink,
        ] {
            let n = chain_op_type_to_i64(t);
            assert_eq!(chain_op_type_from_i64(n).unwrap(), t);
        }
        assert!(chain_op_type_from_i64(0).is_none());
        assert!(chain_op_type_from_i64(10).is_none());
    }
}
