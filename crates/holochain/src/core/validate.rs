//! validate an entry via the ribosome
//! @see the ribosome docs for more info

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::{wasm_ribosome::WasmRibosome, RibosomeT};
use holochain_types::dna::Dna;
use crate::core::ribosome::guest_callback::validate::ValidateResult;

/// build a ribosome from a dna and validate an entry
pub async fn run_validate(
    dna: Dna,
    invocation: ValidateInvocation,
) -> RibosomeResult<ValidateResult> {
    let ribosome = WasmRibosome::new(dna);
    ribosome.run_validate(invocation)
}
