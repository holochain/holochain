extern crate wee_alloc;

use holochain_wasmer_guest::*;
use test_wasm_common::TestString;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// define the host functions we require in order to pull/push data across the host/guest boundary
memory_externs!();

#[no_mangle]
/// always returns "foo" in a TestString
pub extern "C" fn foo(_: RemotePtr) -> RemotePtr {
 ret!(TestString::from(String::from("foo")));
}
