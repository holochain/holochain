use crate::types::cursor::CasCursorX;
use crate::shims::*;

pub async fn run_validation(dna: Dna, cursor: CasCursorX, entry: Entry) -> ValidationResult {
    let ribosome = Ribosome::new(dna);
    ribosome.run_validation(&cursor, entry)
}
