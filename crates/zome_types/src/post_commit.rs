use holochain_serialized_bytes::prelude::*;
use holo_hash_core::HeaderHash;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum PostCommitCallbackResult {
    Success(HeaderHash),
    Fail(HeaderHash, String),
}
