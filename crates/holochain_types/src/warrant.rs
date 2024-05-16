//! Defines the Warrant variant of DhtOp

use std::str::FromStr;

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
    derive_more::Constructor,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct WarrantOp {
    /// The warrant which was issued
    pub warrant: Warrant,
    /// author of the warrant
    pub author: AgentPubKey,
    /// signature of (Warrant, Timestamp) by the author
    pub signature: Signature,
    /// time when the warrant was issued
    pub timestamp: Timestamp,
}

impl WarrantOp {
    /// Get the type of warrant
    pub fn get_type(&self) -> WarrantOpType {
        match self.warrant {
            Warrant::ChainIntegrity(_) => WarrantOpType::ChainIntegrityWarrant,
        }
    }

    /// Convert the WarrantOp into a SignedWarrant
    pub fn into_signed_warrant(self) -> SignedWarrant {
        Signed::new((self.warrant, self.timestamp), self.signature)
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
