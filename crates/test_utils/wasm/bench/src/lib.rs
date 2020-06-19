//! externs to help bench the wasm ribosome
//! note that the majority of the benching is/should be done in the upstream holochain_wasmer
//! crates and this is a relatively lightweight suite to check that we don't have severe
//! performance regressions compared to the upstream baseline
use holochain_wasmer_guest::*;
use holochain_zome_types::HostInput;
use holochain_zome_types::GuestOutput;

holochain_wasmer_guest::holochain_externs!();

#[no_mangle]
/// round trip bytes back to the host
/// useful to see what the basic throughput of our wasm implementation is
pub extern "C" fn echo_bytes(ptr: GuestPtr) -> GuestPtr {
    let input: HostInput = host_args!(ptr);
    let sb: SerializedBytes = input.into_inner();
    ret!(GuestOutput::new(sb));
}
