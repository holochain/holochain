use crate::entry::Entry;
use crate::header::LinkAdd;
use crate::zome_io::GuestOutput;
use crate::CallbackResult;
use holochain_serialized_bytes::prelude::*;

#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct ValidateLinkAddData {
    pub link_add: LinkAdd,
    pub base: Entry,
    pub target: Entry,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateLinkAddCallbackResult {
    Valid,
    Invalid(String),
}

impl CallbackResult for ValidateLinkAddCallbackResult {
    fn is_definitive(&self) -> bool {
        match self {
            ValidateLinkAddCallbackResult::Invalid(_) => true,
            _ => false,
        }
    }
}

impl From<GuestOutput> for ValidateLinkAddCallbackResult {
    fn from(guest_output: GuestOutput) -> Self {
        match guest_output.into_inner().try_into() {
            Ok(v) => v,
            Err(e) => Self::Invalid(format!("{:?}", e)),
        }
    }
}
