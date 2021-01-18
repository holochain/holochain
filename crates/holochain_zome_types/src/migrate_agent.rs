use crate::zome_io::ExternIO;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Serialize, Deserialize, SerializedBytes, Debug)]
pub enum MigrateAgent {
    Open,
    Close,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug)]
pub enum MigrateAgentCallbackResult {
    Pass,
    Fail(String),
}

impl From<ExternIO> for MigrateAgentCallbackResult {
    fn from(guest_output: ExternIO) -> Self {
        match guest_output.decode() {
            Ok(v) => v,
            Err(e) => Self::Fail(format!("{:?}", e)),
        }
    }
}

impl CallbackResult for MigrateAgentCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, MigrateAgentCallbackResult::Fail(_))
    }
}
