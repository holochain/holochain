extern crate wee_alloc;

use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn migrate_agent(_: RemotePtr) -> RemotePtr {
    ret!(GuestOutput::new(try_result!(MigrateAgentCallbackResult::Pass.try_into(), "failed to serialize migrate agent return value")));
}
