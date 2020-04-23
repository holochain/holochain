//! sx_types::dna::zome is a set of structs for working with holochain dna.

use super::wasm::DnaWasm;
use holochain_serialized_bytes::prelude::*;

/// Represents an individual "zome".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, SerializedBytes)]
pub struct Zome {
    /// The Wasm code for this Zome.
    pub code: DnaWasm,
}

impl Eq for Zome {}
