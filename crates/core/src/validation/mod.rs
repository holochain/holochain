use crate::ribosome::Ribosome;
use sx_types::{dna::Dna, shims::*};

/// TODO Determine cursor type
pub type Cursor = crate::txn::source_chain::Cursor;

pub async fn run_validation(dna: Dna, cursor: Cursor, entry: Entry) -> ValidationResult {
    let ribosome = Ribosome::new(dna);
    ribosome.run_validation(&cursor, entry)
}
