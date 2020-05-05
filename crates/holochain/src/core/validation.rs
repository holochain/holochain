use crate::core::ribosome::{RibosomeT, WasmRibosome};
use holochain_types::{dna::DnaFile, entry::Entry, shims::*};

pub async fn run_validation(dna_file: DnaFile, entry: Entry) -> ValidationResult {
    let ribosome = WasmRibosome::new(dna_file);
    ribosome.run_validation(entry)
}
