//! A "Cell" represents a DNA/AgentId pair - a space where one dna/agent
//! can track its source chain and service network requests / responses.

use crate::prelude::*;
use holo_hash::*;
use holochain_serialized_bytes::prelude::*;
use std::fmt;

/// The unique identifier for a Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
#[derive(
    Clone, Debug, Hash, PartialEq, Eq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct CellId<H: HashSerializer = holo_hash::ByteArraySerializer>(
    #[serde(bound(deserialize = "H: serde::de::DeserializeOwned"))] DnaHash<H>,
    #[serde(bound(deserialize = "H: serde::de::DeserializeOwned"))] AgentPubKey<H>,
);

/// Delimiter in a clone id that separates the base cell's role name from the
/// clone index.
pub const CLONE_ID_DELIMITER: &str = ".";

/// Identifier of a clone cell, composed of the DNA's role name and the index
/// of the clone, starting at 0.
///
/// Example: `profiles.0`
#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CloneId(pub RoleName);

impl CloneId {
    /// Construct a clone id from role name and clone index.
    pub fn new(role_name: &RoleName, clone_index: u32) -> Self {
        CloneId(format!(
            "{}{}{}",
            role_name, CLONE_ID_DELIMITER, clone_index
        ))
    }

    /// Get the clone's base cell's role name.
    pub fn as_base_role_name(&self) -> RoleName {
        let (role_name, _) = self.0.split_once(CLONE_ID_DELIMITER).unwrap();
        role_name.into()
    }

    /// Get the index of the clone cell.
    pub fn as_clone_index(&self) -> u32 {
        let (_, clone_index) = self.0.split_once(CLONE_ID_DELIMITER).unwrap();
        clone_index.parse::<u32>().unwrap()
    }

    /// Get an app role name representation of the clone id.
    pub fn as_app_role_name(&self) -> &RoleName {
        &self.0
    }
}

impl fmt::Display for CloneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.as_base_role_name(),
            CLONE_ID_DELIMITER,
            self.as_clone_index()
        )
    }
}

/// Errors during conversion from [`RoleName`] to [`CloneId`].
#[derive(Debug, thiserror::Error)]
pub enum CloneIdError {
    /// Multiple clone id delimiters found in app role name. There must only be one delimiter.
    #[error("Multiple occurrences of reserved character '{CLONE_ID_DELIMITER}' found in app role name: {0}")]
    MultipleDelimiters(RoleName),
    /// The clone index could not be parsed into a u32.
    #[error("Malformed clone index in app role name: {0}")]
    MalformedCloneIndex(RoleName),
    /// The role name is not composed of two parts separated by the clone id delimiter.
    #[error("The role name is not composed of two parts separated by the clone id delimiter: {0}")]
    MalformedCloneId(RoleName),
}

impl TryFrom<RoleName> for CloneId {
    type Error = CloneIdError;
    fn try_from(value: RoleName) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = value.split(CLONE_ID_DELIMITER).collect();
        if parts.len() > 2 {
            return Err(CloneIdError::MultipleDelimiters(value));
        }
        if parts.len() < 2 {
            return Err(CloneIdError::MalformedCloneId(value));
        }
        let role_name = parts[0];
        let clone_index = parts[1]
            .parse::<u32>()
            .map_err(|_| CloneIdError::MalformedCloneIndex(value.clone()))?;
        Ok(Self::new(&role_name.into(), clone_index))
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cell({}, {})", self.dna_hash(), self.agent_pubkey())
    }
}

impl<H: HashSerializer> CellId<H> {
    /// Create a CellId from its components
    pub fn new(dna_hash: DnaHash<H>, agent_pubkey: AgentPubKey<H>) -> Self {
        CellId(dna_hash, agent_pubkey)
    }

    /// The dna hash/address for this cell.
    pub fn dna_hash(&self) -> &DnaHash<H> {
        &self.0
    }

    /// The agent id / public key for this cell.
    pub fn agent_pubkey(&self) -> &AgentPubKey<H> {
        &self.1
    }

    /// Into [DnaHash<H>] and [AgentPubKey<H>]
    pub fn into_dna_and_agent(self) -> (DnaHash<H>, AgentPubKey<H>) {
        (self.0, self.1)
    }
}

impl<H: HashSerializer> From<(DnaHash<H>, AgentPubKey<H>)> for CellId<H> {
    fn from(pair: (DnaHash<H>, AgentPubKey<H>)) -> Self {
        Self(pair.0, pair.1)
    }
}
