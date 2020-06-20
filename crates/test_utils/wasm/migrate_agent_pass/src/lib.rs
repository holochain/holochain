use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn migrate_agent(_: GuestPtr) -> GuestPtr {
    ret!(GuestOutput::new(try_result!(MigrateAgentCallbackResult::Pass.try_into(), "failed to serialize migrate agent return value")));
}
