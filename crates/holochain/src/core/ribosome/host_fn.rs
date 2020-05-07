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

use crate::core::ribosome::guest_callback::CallbackInvocation;
use holochain_types::nucleus::ZomeInvocation;

pub struct HostContext {
    zome_name: String,
}

/// build the HostContext from a _reference_ to ZomeInvocation to avoid cloning potentially huge
/// serialized bytes
impl From<&ZomeInvocation> for HostContext {
    fn from(zome_invocation: &ZomeInvocation) -> HostContext {
        HostContext {
            zome_name: zome_invocation.zome_name.clone(),
        }
    }
}

impl From<&CallbackInvocation<'_>> for HostContext {
    fn from(callback_invocation: &CallbackInvocation) -> HostContext {
        HostContext {
            zome_name: callback_invocation.zome_name.clone(),
        }
    }
}
