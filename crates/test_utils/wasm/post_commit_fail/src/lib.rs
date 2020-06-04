extern crate wee_alloc;

use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use holo_hash_core::HeaderHash;
use holochain_zome_types::post_commit::PostCommitCallbackResult;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
pub extern "C" fn post_commit(_: RemotePtr) -> RemotePtr {
    ret!(
        GuestOutput::new(
            try_result!(
                PostCommitCallbackResult::Fail(
                    vec![HeaderHash::new(vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x99, 0xf6, 0x1f, 0xc2])].into(),
                    "empty header fail".into()
                ).try_into(), "failed to serialize post commit return value")));
}
