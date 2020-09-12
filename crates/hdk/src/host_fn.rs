pub mod agent_info;
pub mod call;
pub mod call_remote;
pub mod create_entry;
pub mod debug;
pub mod decrypt;
pub mod delete_entry;
pub mod emit_signal;
pub mod encrypt;
pub mod entry_hash;
pub mod entry_type_properties;
pub mod get;
pub mod get_details;
pub mod get_link_details;
pub mod get_links;
pub mod keystore;
pub mod link_entries;
pub mod property;
pub mod query;
pub mod random_bytes;
pub mod remove_link;
pub mod schedule;
pub mod show_env;
pub mod sign;
pub mod sys_time;
pub mod unreachable;
pub mod update_entry;
pub mod zome_info;

#[macro_export]
macro_rules! host_fn {
    ( $f:ident, $input:expr, $outputt:ty ) => {{
        $crate::prelude::holochain_externs!();

        let result: Result<$outputt, $crate::prelude::SerializedBytesError> =
            $crate::prelude::host_call!($f, $input);
        result.map(|r| r.into_inner())
    }};
}
