use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::migrate_agent::MigrateAgentCallbackResult;
use holochain_zome_types::globals::ZomeInfo;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn migrate_agent(_: GuestPtr) -> GuestPtr {
    let zome_info: ZomeInfo = try_result!(host_call!(__zome_info, ()), "failed to get zome_info");
    ret!(GuestOutput::new(try_result!(MigrateAgentCallbackResult::Fail(zome_info.zome_name, "no migrate".into()).try_into(), "failed to serialize migrate agent return value")));
}
