extern crate wee_alloc;

use holochain_wasmer_guest::*;
use holochain_zome_types::*;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// define the host functions we require in order to pull/push data across the host/guest boundary
memory_externs!();
host_externs!(__debug);

#[no_mangle]
pub extern "C" fn debug(_: RemotePtr) -> RemotePtr {
    let output: DebugOutput = try_result!(
        host_call!(
            __debug,
            DebugInput::new(debug_msg!("debug line numbers {}", "work"))
        ),
        "failed to call debug"
    );
    let output_sb: SerializedBytes = try_result!(
        output.try_into(),
        "failed to serialize output for extern response"
    );
    ret!(ZomeExternGuestOutput::new(output_sb));
}
