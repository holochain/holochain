extern crate wee_alloc;
#[macro_use]
extern crate lazy_static;

use holochain_wasmer_guest::*;
use sx_zome_types::*;
use sx_zome_types::globals::ZomeGlobals;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// define the host functions we require in order to pull/push data across the host/guest boundary
memory_externs!();

macro_rules! guest_functions {
    ( $( [ $host_fn:ident, $guest_fn:ident, $input_type:ty, $output_type:ty ] ),* ) => {
        $(
            host_externs!($host_fn);
            #[no_mangle]
            pub extern "C" fn $guest_fn(host_allocation_ptr: RemotePtr) -> RemotePtr {
                let input = {
                    let v: ZomeExternHostInput = host_args!(host_allocation_ptr);
                    let deserialized = <$input_type>::try_from(v.inner());
                    try_result!(deserialized, "failed to deserialize host inputs")
                };
                let output: $output_type = try_result!(
                    host_call!(
                        $host_fn,
                        input
                    ),
                    format!("failed to call host function {}", stringify!($host_fn))
                );
                let output_sb: SerializedBytes = try_result!(
                    output.try_into(),
                    "failed to serialize output for extern response"
                );
                ret!(ZomeExternGuestOutput::new(output_sb));
            }
        )*
    }
}

guest_functions!(
    [ __debug, debug, DebugInput, DebugOutput ],
    [ __globals, globals, GlobalsInput, GlobalsOutput ],
    [ __sys_time, sys_time, SysTimeInput, SysTimeOutput ]
);

lazy_static! {
    /// Internal global for retrieving all Zome API globals
    pub(crate) static ref GLOBALS: ZomeGlobals = {
        let output: GlobalsOutput = host_call!(__globals, GlobalsInput::new(())).unwrap();
        output.inner()
    };
}
