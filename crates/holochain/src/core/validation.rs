use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{RibosomeT, WasmRibosome};
use holochain_types::{dna::Dna, entry::Entry, shims::*};

pub async fn run_validation(
    dna: Dna,
    zome_name: String,
    entry: &Entry,
) -> RibosomeResult<ValidationCallbackResult> {
    let ribosome = WasmRibosome::new(dna);
    ribosome.run_validation(zome_name, entry)
}
