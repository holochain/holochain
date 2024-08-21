use hdi::prelude::*;

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub enum KeyType {
//     AppUI,
//     AppSig,
//     AppEncryption,
//     TLS,
// }

#[hdk_entry_helper]
#[derive(Clone)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct KeyMeta {
    // TODO: make sure we can ensure there is only 1 key anchor creation action
    pub app_binding_addr: ActionHash,
    pub key_index: u32,
    pub key_registration_addr: ActionHash,
    pub key_anchor_addr: ActionHash,
    pub derivation_seed: Option<Vec<u8>>,
    pub derivation_bytes: Option<Vec<u8>>,
    // pub key_type: KeyType,
}
