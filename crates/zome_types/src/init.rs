use holochain_serialized_bytes::prelude::*;
use holo_hash_core::EntryHash;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum InitCallbackResult {
    Pass,
    Fail(String),
    UnresolvedDependencies(Vec<EntryHash>),
}
