use holochain_serialized_bytes::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgent {
    Open,
    Close,
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgentCallbackResult {
    Pass,
    Fail(String),
}
