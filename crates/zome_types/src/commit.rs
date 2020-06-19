//! Types related to committing an entry

use holo_hash_core::HeaderHash;
use holochain_serialized_bytes::prelude::*;

/// The result of calling the `commit_entry` host function
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, SerializedBytes)]
pub enum CommitEntryResult {
    /// Denotes a successful commit
    Success(HeaderHash),
    /// Denotes a failure to commit
    Fail(String),
}
