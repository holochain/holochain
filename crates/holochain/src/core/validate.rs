//! validate an entry via the ribosome
//! @see the ribosome docs for more info

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::{wasm_ribosome::WasmRibosome, RibosomeT};
use holochain_types::dna::Dna;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::validate::ValidateEntryResult;
use holochain_types::nucleus::ZomeName;

/// build a ribosome from a dna and validate an entry
pub async fn run_validate(
    dna: Dna,
    zome_name: ZomeName,
    entry: &Entry,
) -> RibosomeResult<ValidateEntryResult> {
    let ribosome = WasmRibosome::new(dna);
    ribosome.run_validate(ValidateInvocation { zome_name, entry })
}
