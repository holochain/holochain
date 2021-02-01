use crate::header::HeaderHashes;
use crate::zome_io::ExternOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum PostCommitCallbackResult {
    Success,
    Fail(HeaderHashes, String),
}

impl From<ExternOutput> for PostCommitCallbackResult {
    fn from(guest_output: ExternOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Fail(vec![].into(), format!("{:?}", e)),
        }
    }
}

impl CallbackResult for PostCommitCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, PostCommitCallbackResult::Fail(_, _))
    }
}
