extern crate wee_alloc;

use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::validate::ValidateCallbackResult;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
/// this only runs for agent entries
/// this passes but should not override the base validate call that has an Invalid result
pub extern "C" fn validate_agent(_: RemotePtr) -> RemotePtr {
    ret!(GuestOutput::new(try_result!(ValidateCallbackResult::Valid.try_into(), "failed to serialize valid agent callback")))
}

#[no_mangle]
/// this runs for all entries
/// this never passes and so should always invalide the result for any input entry
pub extern "C" fn validate(_: RemotePtr) -> RemotePtr {
    ret!(GuestOutput::new(try_result!(ValidateCallbackResult::Invalid("esoteric edge case".into()).try_into(), "failed to serialize validate return value")));
}
