use test_wasm_common::TestString;
use holochain_wasmer_guest::*;
use holochain_zome_types::*;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
/// always returns "foo" in a TestString
pub extern "C" fn foo(_: RemotePtr) -> RemotePtr {
 // this is whatever the dev wants we don't know
 let response = TestString::from(String::from("foo"));

 // imagine this is inside the hdk
 let response_sb: SerializedBytes = try_result!(response.try_into(), "failed to serialize TestString");
 ret!(GuestOutput::new(response_sb));
}
