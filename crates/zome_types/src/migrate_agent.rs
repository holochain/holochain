use holochain_serialized_bytes::prelude::*;
use crate::zome_io::GuestOutput;
use crate::zome::ZomeName;

#[derive(Clone, Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgent {
    Open,
    Close,
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum MigrateAgentCallbackResult {
    Pass,
    Fail(ZomeName, String),
}

impl From<GuestOutput> for MigrateAgentCallbackResult {
    fn from(callback_guest_output: GuestOutput) -> Self {
        match callback_guest_output.try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(ZomeName::unknown(), format!("{:?}", e)),
        }
    }
}
