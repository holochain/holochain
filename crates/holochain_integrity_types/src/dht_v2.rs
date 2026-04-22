//! Redesigned DHT state-model types (transitional module — see
//! `docs/design/state_model.md`).

use holochain_serialized_bytes::prelude::*;

/// Record-level validation outcome stored in `Action.record_validity`.
///
/// Schema column is `INTEGER`: `NULL = pending`, `1 = Accepted`, `2 = Rejected`.
/// `0` is never used.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum RecordValidity {
    /// The record was accepted.
    Accepted = 1,
    /// The record was rejected.
    Rejected = 2,
}

impl From<RecordValidity> for i64 {
    fn from(v: RecordValidity) -> Self {
        v as i64
    }
}

impl TryFrom<i64> for RecordValidity {
    type Error = i64;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(RecordValidity::Accepted),
            2 => Ok(RecordValidity::Rejected),
            other => Err(other),
        }
    }
}

/// Integer-backed action-type discriminant mapping to the schema
/// `Action.action_type` column. `0` is reserved and never written.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum ActionType {
    Dna = 1,
    AgentValidationPkg = 2,
    InitZomesComplete = 3,
    Create = 4,
    Update = 5,
    Delete = 6,
    CreateLink = 7,
    DeleteLink = 8,
}

impl From<ActionType> for i64 {
    fn from(t: ActionType) -> Self {
        t as i64
    }
}

impl TryFrom<i64> for ActionType {
    type Error = i64;
    fn try_from(v: i64) -> Result<Self, Self::Error> {
        use ActionType::*;
        match v {
            1 => Ok(Dna),
            2 => Ok(AgentValidationPkg),
            3 => Ok(InitZomesComplete),
            4 => Ok(Create),
            5 => Ok(Update),
            6 => Ok(Delete),
            7 => Ok(CreateLink),
            8 => Ok(DeleteLink),
            other => Err(other),
        }
    }
}

use crate::{EntryType, link::{LinkTag, LinkType}, MembraneProof};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash};
use holochain_timestamp::Timestamp;
use crate::action::ZomeIndex;

/// Common header fields shared by all action types.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct ActionHeader {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    /// `None` for the genesis `Dna` action only.
    pub prev_action: Option<ActionHash>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct DnaData {
    pub dna_hash: DnaHash,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct AgentValidationPkgData {
    pub membrane_proof: Option<MembraneProof>,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct InitZomesCompleteData {}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct CreateData {
    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct UpdateData {
    pub original_action_address: ActionHash,
    pub original_entry_address: EntryHash,
    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct DeleteData {
    pub deletes_address: ActionHash,
    pub deletes_entry_address: EntryHash,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct CreateLinkData {
    pub base_address: AnyLinkableHash,
    pub target_address: AnyLinkableHash,
    pub zome_index: ZomeIndex,
    pub link_type: LinkType,
    pub tag: LinkTag,
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, SerializedBytes,
)]
pub struct DeleteLinkData {
    pub base_address: AnyLinkableHash,
    pub link_add_address: ActionHash,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_validity_i64_roundtrip() {
        for v in [RecordValidity::Accepted, RecordValidity::Rejected] {
            let n: i64 = v.into();
            assert_eq!(RecordValidity::try_from(n).unwrap(), v);
        }
        assert!(RecordValidity::try_from(0).is_err());
        assert!(RecordValidity::try_from(3).is_err());
    }

    #[test]
    fn action_type_i64_roundtrip() {
        use ActionType::*;
        for v in [
            Dna,
            AgentValidationPkg,
            InitZomesComplete,
            Create,
            Update,
            Delete,
            CreateLink,
            DeleteLink,
        ] {
            let n: i64 = v.into();
            assert_eq!(ActionType::try_from(n).unwrap(), v);
        }
        assert!(ActionType::try_from(0).is_err());
        assert!(ActionType::try_from(9).is_err());
    }

    #[test]
    fn data_structs_construct() {
        // Sanity check that each struct has the expected shape by constructing one.
        let _ = DnaData { dna_hash: DnaHash::from_raw_36(vec![0u8; 36]) };
        let _ = InitZomesCompleteData {};
    }
}
