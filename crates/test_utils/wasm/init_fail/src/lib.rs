use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::globals::ZomeGlobals;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn init(_: GuestPtr) -> GuestPtr {
    let globals: ZomeGlobals = try_result!(host_call!(__globals, ()), "failed to get globals");
    ret!(GuestOutput::new(try_result!(InitCallbackResult::Fail(globals.zome_name, "because i said so".into()).try_into(), "failed to serialize init return value")));
}
