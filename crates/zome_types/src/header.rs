use holochain_serialized_bytes::prelude::*;
use holo_hash_core::HeaderHash;

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct HeaderHashes(Vec<HeaderHash>);

impl From<Vec<HeaderHash>> for HeaderHashes {
    fn from(vs: Vec<HeaderHash>) -> Self {
        Self(vs)
    }
}
