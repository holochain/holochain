extern crate wee_alloc;

use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::validate::ValidationPackageCallbackResult;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn validation_package(_: RemotePtr) -> RemotePtr {

    ret!(GuestOutput::new(try_result!(ValidationPackageCallbackResult::Fail("bad package".into()).try_into(), "failed to serialize validation package return value")));
}
