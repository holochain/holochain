use crate::entry::Entry;
use crate::header::CreateLink;
use crate::zome_io::ExternOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct ValidateCreateLinkData {
    pub link_add: CreateLink,
    pub base: Entry,
    pub target: Entry,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateCreateLinkCallbackResult {
    Valid,
    Invalid(String),
}

impl CallbackResult for ValidateCreateLinkCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            ValidateCreateLinkCallbackResult::Invalid(_) => true,
            _ => false,
        }
    }
}

impl From<ExternOutput> for ValidateCreateLinkCallbackResult {
    fn from(guest_output: ExternOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}
