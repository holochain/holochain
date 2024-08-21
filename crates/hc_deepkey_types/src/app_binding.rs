use crate::{
    MetaData,
};
use hdi::prelude::*;
use holo_hash::DnaHash;


#[hdk_entry_helper]
#[derive(Clone, PartialEq)]
pub struct AppBinding {
    pub app_index: u32,
    pub app_name: String,
    pub installed_app_id: String,
    pub dna_hashes: Vec<DnaHash>,
    #[serde(default)]
    pub metadata: MetaData,
}
