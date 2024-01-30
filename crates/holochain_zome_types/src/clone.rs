//! Cells can be cloned to create new cells with the different properties.

use derive_more::Display;
use holo_hash::DnaHash;
use holochain_integrity_types::DnaModifiers;
use crate::cell::{CellId, CloneId};

/// The arguments to create a clone of an existing cell.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CreateCloneCellInput {
    /// The app id that the DNA to clone belongs to
    pub app_id: String,
    /// The DNA's role name to clone
    pub role_name: crate::call::RoleName,
    /// Modifiers to set for the new cell.
    /// At least one of the modifiers must be set to obtain a distinct hash for
    /// the clone cell's DNA.
    #[cfg(feature = "properties")]
    pub modifiers: holochain_integrity_types::DnaModifiersOpt<crate::properties::YamlProperties>,
    /// Optionally set a proof of membership for the clone cell
    pub membrane_proof: Option<holochain_integrity_types::MembraneProof>,
    /// Optionally a name for the DNA clone
    pub name: Option<String>,
}

/// Cloned cell that was created from a provisioned cell at runtime.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClonedCell {
    /// The cell's identifying data
    pub cell_id: CellId,
    /// A conductor-local clone identifier
    pub clone_id: CloneId,
    /// The hash of the DNA that this cell was instantiated from
    pub original_dna_hash: DnaHash,
    /// The DNA modifiers that were used to instantiate this clone cell
    pub dna_modifiers: DnaModifiers,
    /// The name the cell was instantiated with
    pub name: String,
    /// Whether or not the cell is running
    pub enabled: bool,
}

/// Ways of specifying a clone cell.
#[derive(Clone, Debug, Display, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum CloneCellId {
    /// Clone id consisting of role name and clone index.
    CloneId(CloneId),
    /// Cell id consisting of DNA hash and agent pub key.
    CellId(CellId),
}

/// Arguments to specify the clone cell to be disabled.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisableCloneCellInput {
    /// The app id that the clone cell belongs to
    pub app_id: String,
    /// The clone id or cell id of the clone cell
    pub clone_cell_id: CloneCellId,
}
