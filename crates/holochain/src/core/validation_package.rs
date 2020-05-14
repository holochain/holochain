//! validate an entry via the ribosome
//! @see the ribosome docs for more info

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::{wasm_ribosome::WasmRibosome, RibosomeT};
use holochain_types::dna::DnaFile;

/// build a ribosome from a dna and build a validation package
pub async fn run_validation_package(
    dna_file: DnaFile,
    invocation: ValidationPackageInvocation,
) -> RibosomeResult<ValidationPackageResult> {
    let ribosome = WasmRibosome::new(dna_file);
    ribosome.run_validation_package(invocation)
}
