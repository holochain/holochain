//! For more details see [`holochain_integrity_types::op`].

use crate::prelude::{Deserialize, Serialize};

#[doc(no_inline)]
pub use holochain_integrity_types::op;
#[doc(inline)]
pub use holochain_integrity_types::op::*;

/// This enum is used to encode just the enum variant of ChainOp
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    derive_more::Display,
    strum_macros::EnumString,
)]
pub enum ChainOpType {
    #[display("StoreRecord")]
    StoreRecord,
    #[display("StoreEntry")]
    StoreEntry,
    #[display("RegisterAgentActivity")]
    RegisterAgentActivity,
    #[display("RegisterUpdatedContent")]
    RegisterUpdatedContent,
    #[display("RegisterUpdatedRecord")]
    RegisterUpdatedRecord,
    #[display("RegisterDeletedBy")]
    RegisterDeletedBy,
    #[display("RegisterDeletedEntryAction")]
    RegisterDeletedEntryAction,
    #[display("RegisterAddLink")]
    RegisterAddLink,
    #[display("RegisterRemoveLink")]
    RegisterRemoveLink,
}

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
