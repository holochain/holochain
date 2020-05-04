use holo_hash_core::EntryHash;
use holochain_serialized_bytes::prelude::*;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum InitCallbackResult {
    Pass,
    Fail(String),
    UnresolvedDependencies(Vec<EntryHash>),
}
