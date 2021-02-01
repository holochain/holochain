use crate::entry::Entry;
use crate::header::CreateLink;
use crate::header::DeleteLink;
use crate::zome_io::ExternOutput;
use crate::CallbackResult;
use holo_hash::AnyDhtHash;
use holochain_serialized_bytes::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct ValidateCreateLinkData {
    pub link_add: CreateLink,
    pub base: Entry,
    pub target: Entry,
}

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct ValidateDeleteLinkData {
    pub delete_link: DeleteLink,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateLinkCallbackResult {
    Valid,
    Invalid(String),
    UnresolvedDependencies(Vec<AnyDhtHash>),
}

impl CallbackResult for ValidateLinkCallbackResult {
    fn is_definitive(&self) -> bool {
        matches!(self, ValidateLinkCallbackResult::Invalid(_))
    }
}

impl From<ExternOutput> for ValidateLinkCallbackResult {
    fn from(guest_output: ExternOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}
