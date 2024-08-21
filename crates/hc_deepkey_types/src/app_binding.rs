use crate::MetaData;
use hdi::prelude::{holo_hash::DnaHash, *};

#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct AppBinding {
    // TODO: if an app binding will not change for the series of registration updates, it doesn't
    // make sense to point to 1 key meta when there could be many in the series.
    pub app_index: u32,
    pub app_name: String,
    pub installed_app_id: String,
    pub dna_hashes: Vec<DnaHash>,
    pub key_anchor_addr: ActionHash,
    #[serde(default)]
    pub metadata: MetaData,
}
