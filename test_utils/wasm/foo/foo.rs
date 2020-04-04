extern crate wee_alloc;

use test_wasm_common::TestString;
use holochain_wasmer_guest::*;
use sx_wasm_types::WasmExternResponse;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// define the host functions we require in order to pull/push data across the host/guest boundary
memory_externs!();

#[no_mangle]
/// always returns "foo" in a TestString
pub extern "C" fn foo(_: RemotePtr) -> RemotePtr {
 // this is whatever the dev wants we don't know
 let response = TestString::from(String::from("foo"));

 // imagine this is inside the hdk
 let response_sb: SerializedBytes = try_result!(response.try_into(), "failed to serialize TestString");
 ret!(WasmExternResponse::new(response_sb));
}
