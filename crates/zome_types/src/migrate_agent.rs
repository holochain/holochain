use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgent {
    Open,
    Close,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgentCallbackResult {
    Pass,
    Fail(String),
}

impl From<GuestOutput> for MigrateAgentCallbackResult {
    fn from(guest_output: GuestOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(format!("{:?}", e)),
        }
    }
}

impl CallbackResult for MigrateAgentCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            MigrateAgentCallbackResult::Fail(_) => true,
            _ => false,
        }
    }
}
