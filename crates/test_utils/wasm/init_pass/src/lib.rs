use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::init::InitCallbackResult;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn init(_: RemotePtr) -> RemotePtr {
    ret!(GuestOutput::new(try_result!(InitCallbackResult::Pass.try_into(), "failed to serialize init return value")));
}
