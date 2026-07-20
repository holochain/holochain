//! For more details see [`holochain_integrity_types::op`].

use serde::{Deserialize, Serialize};

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
    #[display("CreateRecord")]
    CreateRecord,
    #[display("CreateEntry")]
    CreateEntry,
    #[display("AgentActivity")]
    AgentActivity,
    #[display("UpdateEntry")]
    UpdateEntry,
    #[display("UpdateRecord")]
    UpdateRecord,
    #[display("DeleteRecord")]
    DeleteRecord,
    #[display("DeleteEntry")]
    DeleteEntry,
    #[display("CreateLink")]
    CreateLink,
    #[display("DeleteLink")]
    DeleteLink,
}

/// Maps [`ChainOpType`] onto the schema `op_type` INTEGER column (`1..=9`).
/// `0` is reserved and never written.
///
/// Variant ordering is pinned to `docs/design/state_model.md`:
///
/// | `op_type` | [`ChainOpType`] variant | Authority       |
/// |-----------|-------------------------|-----------------|
/// | 1         | `CreateRecord`          | action          |
/// | 2         | `CreateEntry`           | entry           |
/// | 3         | `AgentActivity`         | agent           |
/// | 4         | `UpdateEntry`           | entry           |
/// | 5         | `UpdateRecord`          | action          |
/// | 6         | `DeleteEntry`           | entry           |
/// | 7         | `DeleteRecord`          | action          |
/// | 8         | `CreateLink`            | link base       |
/// | 9         | `DeleteLink`            | link base       |
impl From<ChainOpType> for i64 {
    fn from(t: ChainOpType) -> Self {
        match t {
            ChainOpType::CreateRecord => 1,
            ChainOpType::CreateEntry => 2,
            ChainOpType::AgentActivity => 3,
            ChainOpType::UpdateEntry => 4,
            ChainOpType::UpdateRecord => 5,
            ChainOpType::DeleteEntry => 6,
            ChainOpType::DeleteRecord => 7,
            ChainOpType::CreateLink => 8,
            ChainOpType::DeleteLink => 9,
        }
    }
}

/// Inverse of [`From<ChainOpType> for i64`]. Returns `Err(v)` for `0` and any
/// value outside `1..=9`.
impl TryFrom<i64> for ChainOpType {
    type Error = i64;

    fn try_from(n: i64) -> Result<Self, Self::Error> {
        Ok(match n {
            1 => ChainOpType::CreateRecord,
            2 => ChainOpType::CreateEntry,
            3 => ChainOpType::AgentActivity,
            4 => ChainOpType::UpdateEntry,
            5 => ChainOpType::UpdateRecord,
            6 => ChainOpType::DeleteEntry,
            7 => ChainOpType::DeleteRecord,
            8 => ChainOpType::CreateLink,
            9 => ChainOpType::DeleteLink,
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
            (ChainOpType::CreateRecord, 1_i64),
            (ChainOpType::CreateEntry, 2),
            (ChainOpType::AgentActivity, 3),
            (ChainOpType::UpdateEntry, 4),
            (ChainOpType::UpdateRecord, 5),
            (ChainOpType::DeleteEntry, 6),
            (ChainOpType::DeleteRecord, 7),
            (ChainOpType::CreateLink, 8),
            (ChainOpType::DeleteLink, 9),
        ];
        for (variant, n) in expected {
            assert_eq!(i64::from(variant), n);
            assert_eq!(ChainOpType::try_from(n).unwrap(), variant);
        }
        assert!(ChainOpType::try_from(0).is_err());
        assert!(ChainOpType::try_from(10).is_err());
    }
}
