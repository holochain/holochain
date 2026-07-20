//! Defines the Warrant variant of DhtOp

use holo_hash::{hash_type, HashableContent, HashableContentBytes};
use holochain_keystore::{AgentPubKeyExt, LairResult, MetaLairClient};
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;
use holochain_zome_types::prelude::{SignedWarrant, Warrant, WarrantProof};

/// A Warrant DhtOp
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializedBytes,
    Eq,
    PartialEq,
    Hash,
    derive_more::From,
    derive_more::Deref,
)]
pub struct WarrantOp(SignedWarrant);

impl WarrantOp {
    /// Get the type of warrant
    pub fn get_type(&self) -> WarrantOpType {
        match self.proof {
            WarrantProof::ChainIntegrity(_) => WarrantOpType::ChainIntegrityWarrant,
        }
    }

    /// Sign the warrant for use as an Op
    pub async fn sign(keystore: &MetaLairClient, warrant: Warrant) -> LairResult<Self> {
        let signature = warrant.author.sign(keystore, warrant.clone()).await?;
        Ok(Self::from(SignedWarrant::new(warrant, signature)))
    }

    /// Accessor for the timestamp of the warrant
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    /// Accessor for the warrant
    pub fn warrant(&self) -> &Warrant {
        self
    }
}

/// Different types of warrants
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
pub enum WarrantOpType {
    /// A chain integrity warrant
    ChainIntegrityWarrant,
}

impl HashableContent for WarrantOp {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        self.warrant().hashable_content()
    }
}
