use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holo_hash_core::EntryContentHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum InitCallbackResult {
    Pass,
    Fail(String),
    UnresolvedDependencies(Vec<EntryContentHash>),
}

impl From<GuestOutput> for InitCallbackResult {
    fn from(callback_guest_output: GuestOutput) -> Self {
        match callback_guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(format!("{:?}", e)),
        }
    }
}

impl CallbackResult for InitCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            InitCallbackResult::Fail(_) => true,
            _ => false,
        }
    }
}
