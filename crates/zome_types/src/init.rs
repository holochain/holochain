use crate::zome::ZomeName;
use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holo_hash_core::EntryHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum InitCallbackResult {
    Pass,
    Fail(ZomeName, String),
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
}

impl From<GuestOutput> for InitCallbackResult {
    fn from(callback_guest_output: GuestOutput) -> Self {
        match callback_guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(ZomeName::unknown(), format!("{:?}", e)),
        }
    }
}

impl CallbackResult for InitCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            InitCallbackResult::Fail(_, _) => true,
            _ => false,
        }
    }
}
