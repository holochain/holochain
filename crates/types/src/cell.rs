//! A "Cell" represents a DNA/AgentId pair - a space where one dna/agent
//! can track its source chain and service network requests / responses.

use crate::{agent::AgentId, dna::DnaAddress, persistence::cas::content::Addressable};
use derive_more::{Display, From, Into};
use std::fmt;

/// The unique identifier for a Cell.
/// Cells are uniquely determined by this pair - this pair is necessary
/// and sufficient to refer to a cell in a conductor
#[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CellId(DnaAddress, AgentId);

/// A conductor-specific name for a Cell
/// (Used to be instance_id)
#[derive(
    Clone, Debug, Display, Hash, PartialEq, Eq, From, Into, serde::Serialize, serde::Deserialize,
)]
pub struct CellHandle(String);

impl From<&str> for CellHandle {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cell-{}-{}", self.0, self.1.address())
    }
}

impl CellId {
    /// The dna hash/address for this cell.
    pub fn dna_address(&self) -> &DnaAddress {
        &self.0
    }

    /// The agent id / public key for this cell.
    pub fn agent_id(&self) -> &AgentId {
        &self.1
    }
}

impl From<(DnaAddress, AgentId)> for CellId {
    fn from(pair: (DnaAddress, AgentId)) -> Self {
        Self(pair.0, pair.1)
    }
}
