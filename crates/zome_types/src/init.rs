use holo_hash_core::EntryHash;
use holochain_serialized_bytes::prelude::*;
use crate::zome_io::CallbackGuestOutput;
use crate::zome::ZomeName;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum InitCallbackResult {
    Pass,
    Fail(ZomeName, String),
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
}

impl From<CallbackGuestOutput> for InitCallbackResult {
    fn from(callback_guest_output: CallbackGuestOutput) -> Self {
        match callback_guest_output.try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(ZomeName::unknown(), format!("{:?}", e)),
        }
    }
}
