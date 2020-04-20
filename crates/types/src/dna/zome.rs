//! sx_types::dna::zome is a set of structs for working with holochain dna.

use holochain_serialized_bytes::prelude::*;
use std::collections::BTreeMap;

/// Represents an individual "zome".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, SerializedBytes)]
pub struct Zome {}

impl Eq for Zome {}

impl Zome {
    /// Allow sane defaults for `Zome::new()`.
    pub fn new() -> Zome {
        Zome {}
    }
}
