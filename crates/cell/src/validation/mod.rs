use crate::ribosome::{Ribosome, RibosomeT};
use sx_types::{dna::Dna, entry::Entry, shims::*};

/// TODO Determine cursor type

pub async fn run_validation(dna: Dna, entry: Entry) -> ValidationResult {
    let ribosome = Ribosome::new(dna);
    ribosome.run_validation(entry)
}
