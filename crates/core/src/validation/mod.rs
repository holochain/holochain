use crate::cursor::CasCursorX;
use crate::ribosome::Ribosome;
use sx_types::shims::*;

pub async fn run_validation(dna: Dna, cursor: CasCursorX, entry: Entry) -> ValidationResult {
    let ribosome = Ribosome::new(dna);
    ribosome.run_validation(&cursor, entry)
}
