use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::globals::ZomeInfo;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn init(_: GuestPtr) -> GuestPtr {
    let zome_info: ZomeInfo = try_result!(host_call!(__zome_info, ()), "failed to get zome_info");
    ret!(GuestOutput::new(try_result!(InitCallbackResult::Fail(zome_info.zome_name, "because i said so".into()).try_into(), "failed to serialize init return value")));
}
