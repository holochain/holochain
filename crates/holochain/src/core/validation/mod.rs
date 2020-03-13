use crate::core::ribosome::{WasmRibosome, RibosomeT};
use sx_types::{dna::Dna, entry::Entry, shims::*};

pub async fn run_validation(dna: Dna, entry: Entry) -> ValidationResult {
    let ribosome = WasmRibosome::new(dna);
    ribosome.run_validation(entry)
}
