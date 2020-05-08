pub mod call;
pub mod capability;
pub mod commit_entry;
pub mod debug;
pub mod decrypt;
pub mod emit_signal;
pub mod encrypt;
pub mod entry_address;
pub mod entry_type_properties;
pub mod get_entry;
pub mod get_links;
pub mod globals;
pub mod keystore;
pub mod link_entries;
pub mod property;
pub mod query;
pub mod remove_entry;
pub mod remove_link;
pub mod schedule;
pub mod send;
pub mod show_env;
pub mod sign;
pub mod sys_time;
pub mod update_entry;
use crate::core::ribosome::guest_callback::Invocation as CallbackInvocation;
use holochain_types::nucleus::ZomeInvocation;
use holochain_types::nucleus::ZomeName;
use holochain_serialized_bytes::prelude::*;


pub struct HostContext<'a> {
    zome_name: &'a ZomeName,
}

/// build the HostContext from a _reference_ to ZomeInvocation to avoid cloning potentially huge
/// serialized bytes
impl From<&ZomeInvocation> for HostContext<'_> {
    fn from(zome_invocation: &ZomeInvocation) -> Self {
        Self {
            zome_name: (&zome_invocation).into(),
        }
    }
}

impl <I: CallbackInvocation<Error = SerializedBytesError>>From<&I> for HostContext<'_> {
    fn from(callback_invocation: &I) -> Self {
        Self {
            zome_name: (&callback_invocation).into(),
        }
    }
}
