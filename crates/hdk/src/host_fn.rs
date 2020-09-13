pub mod agent_info;
pub mod call;
pub mod call_remote;
pub mod create;
pub mod create_link;
pub mod debug;
pub mod decrypt;
pub mod delete;
pub mod delete_link;
pub mod emit_signal;
pub mod encrypt;
pub mod entry_type_properties;
pub mod get;
pub mod get_details;
pub mod get_link_details;
pub mod get_links;
pub mod hash_entry;
pub mod keystore;
pub mod property;
pub mod query;
pub mod random_bytes;
pub mod schedule;
pub mod show_env;
pub mod sign;
pub mod sys_time;
pub mod unreachable;
pub mod update;
pub mod zome_info;

/// simple wrapper around the holochain_wasmer_guest host_call! macro
///
/// - ensures the holochain_externs!() are setup
/// - unwraps the output type into the inner result to hide the host/guest crossover
/// - needs the host_fn to call, input/output types to be provided
///
/// every ribosome function can be called and interacted with in a standard way using this fn
///
/// ```ignore
/// let foo_value = host_fn!(__foo_ribosome_fn, FooRibosomeFnInput( ... ), FooRibosomeOutput)?;
/// ```
///
/// note: every host_fn! call returns a Result that represents the possibility that the guest can
///       fail to deserialize whatever the host is injecting into it.
///       it is a Result because this is designed to be used if happ devs want to get more low
///       level and we can't assume whether this is being used in a native rust extern that returns
///       an int or a function that returns a result, and we should never unwrap such things in
///       wasm because that is hard to debug.
#[macro_export]
macro_rules! host_fn {
    ( $f:ident, $input:expr, $outputt:ty ) => {{
        $crate::prelude::holochain_externs!();

        let result: Result<$outputt, $crate::prelude::SerializedBytesError> =
            $crate::prelude::host_call!($f, $input);
        result.map(|r| r.into_inner())
    }};
}
