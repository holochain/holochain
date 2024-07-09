//! Defines the Warrant variant of DhtOp

use std::str::FromStr;

use holochain_keystore::{AgentPubKeyExt, LairResult, MetaLairClient};
use holochain_zome_types::prelude::*;

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
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
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

    /// Accessor for the warrant
    pub fn into_warrant(self) -> Warrant {
        self.0.into_data()
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

impl holochain_sqlite::rusqlite::ToSql for WarrantOpType {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        Ok(holochain_sqlite::rusqlite::types::ToSqlOutput::Owned(
            format!("{}", self).into(),
        ))
    }
}

impl holochain_sqlite::rusqlite::types::FromSql for WarrantOpType {
    fn column_result(
        value: holochain_sqlite::rusqlite::types::ValueRef<'_>,
    ) -> holochain_sqlite::rusqlite::types::FromSqlResult<Self> {
        String::column_result(value).and_then(|string| {
            WarrantOpType::from_str(&string)
                .map_err(|_| holochain_sqlite::rusqlite::types::FromSqlError::InvalidType)
        })
    }
}
