//! Information about the current zome and dna.
use crate::header::ZomeId;
use crate::zome::ZomeName;
use crate::AppEntryType;
use crate::EntryDefId;
use crate::EntryDefs;
use crate::FunctionName;
use holo_hash::DnaHash;
use holochain_serialized_bytes::prelude::*;

/// The properties of the current dna/zome being called.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq)]
pub struct ZomeInfo {
    pub name: ZomeName,
    /// The position of this zome in the `dna.json`
    pub id: ZomeId,
    pub properties: SerializedBytes,
    pub entry_defs: EntryDefs,
    // @todo make this include function signatures when they exist.
    pub extern_fns: Vec<FunctionName>,
}

impl ZomeInfo {
    /// Create a new ZomeInfo.
    pub fn new(
        name: ZomeName,
        id: ZomeId,
        properties: SerializedBytes,
        entry_defs: EntryDefs,
        extern_fns: Vec<FunctionName>,
    ) -> Self {
        Self {
            name,
            id,
            properties,
            entry_defs,
            extern_fns,
        }
    }

    /// Check if an [`AppEntryType`] matches the [`EntryDefId`] provided for this zome.
    pub fn matches_entry_def_id(&self, entry_type: &AppEntryType, id: EntryDefId) -> bool {
        self.entry_defs
            .0
            .get(entry_type.id.index())
            .map_or(false, |stored_id| stored_id.id == id)
            && self.id == entry_type.zome_id
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Information about the current DNA.
pub struct DnaInfo {
    /// The name of this DNA.
    pub name: String,
    /// The hash of this DNA.
    pub hash: DnaHash,
    /// The properties of this DNA.
    pub properties: SerializedBytes,
    // In ZomeId order as to match corresponding `ZomeInfo` for each.
    /// The zomes in this DNA.
    pub zome_names: Vec<ZomeName>,
}
