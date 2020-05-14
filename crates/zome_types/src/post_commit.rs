use crate::header::HeaderHashes;
use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum PostCommitCallbackResult {
    Success,
    Fail(HeaderHashes, String),
}

impl From<GuestOutput> for PostCommitCallbackResult {
    fn from(guest_output: GuestOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(vec![].into(), format!("{:?}", e)),
        }
    }
}

impl CallbackResult for PostCommitCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            PostCommitCallbackResult::Fail(_, _) => true,
            _ => false,
        }
    }
}
