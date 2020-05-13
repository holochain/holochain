use holo_hash_core::HeaderHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct HeaderHashes(Vec<HeaderHash>);

impl From<Vec<HeaderHash>> for HeaderHashes {
    fn from(vs: Vec<HeaderHash>) -> Self {
        Self(vs)
    }
}
