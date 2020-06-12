use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::validate::ValidateCallbackResult;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn validate(_: RemotePtr) -> RemotePtr {
    ret!(GuestOutput::new(try_result!(ValidateCallbackResult::Valid.try_into(), "failed to serialize validate return value")));
}
