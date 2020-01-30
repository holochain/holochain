use crate::cursor::ChainCursorX;
use crate::ribosome::Ribosome;
use sx_types::dna::Dna;
use sx_types::shims::*;

pub async fn run_validation(dna: Dna, cursor: ChainCursorX, entry: Entry) -> ValidationResult {
    let ribosome = Ribosome::new(dna);
    ribosome.run_validation(&cursor, entry)
}
