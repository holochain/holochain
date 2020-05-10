use holochain_serialized_bytes::prelude::*;
use crate::header::HeaderHashes;
use crate::zome_io::GuestOutput;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum PostCommitCallbackResult {
    Success,
    Fail(HeaderHashes, String),
}

impl From<GuestOutput> for PostCommitCallbackResult {
    fn from(callback_guest_output: GuestOutput) -> Self {
        match callback_guest_output.try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(vec![].into(), format!("{:?}", e)),
        }
    }
}
