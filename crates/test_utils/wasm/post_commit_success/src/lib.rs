use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holochain_zome_types::post_commit::PostCommitCallbackResult;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn post_commit(_: GuestPtr) -> GuestPtr {
    ret!(GuestOutput::new(try_result!(PostCommitCallbackResult::Success.try_into(), "failed to serialize post commit return value")));
}
