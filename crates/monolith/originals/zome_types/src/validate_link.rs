use crate::holochain_zome_types::entry::Entry;
use crate::holochain_zome_types::header::CreateLink;
use crate::holochain_zome_types::header::DeleteLink;
use crate::holochain_zome_types::zome_io::ExternOutput;
use crate::holochain_zome_types::CallbackResult;
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
        match self {
            ValidateLinkCallbackResult::Invalid(_) => true,
            _ => false,
        }
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
