use holochain_serialized_bytes::prelude::*;
use holo_hash_core::DnaHash;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgent {
    Open(DnaHash),
    Close(DnaHash),
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgentCallbackResult {
    Pass,
    Fail(String),
}
