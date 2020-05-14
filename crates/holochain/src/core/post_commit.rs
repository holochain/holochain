//! validate an entry via the ribosome
//! @see the ribosome docs for more info

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::post_commit::PostCommitInvocation;
use crate::core::ribosome::guest_callback::post_commit::PostCommitResult;
use crate::core::ribosome::{wasm_ribosome::WasmRibosome, RibosomeT};
use holochain_types::dna::DnaFile;

/// build a ribosome from a dna and run the post commit callback
pub async fn run_post_commit(
    dna_file: DnaFile,
    invocation: PostCommitInvocation,
) -> RibosomeResult<PostCommitResult> {
    let ribosome = WasmRibosome::new(dna_file);
    ribosome.run_post_commit(invocation)
}
