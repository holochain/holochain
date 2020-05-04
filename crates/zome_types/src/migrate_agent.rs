use holochain_serialized_bytes::prelude::*;

pub enum MigrateAgentDirection {
    Open,
    Close,
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgentCallbackResult {
    Pass,
    Fail(String),
}
