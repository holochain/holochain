use crate::shims::*;

pub async fn run_validation(dna: Dna, cursor: CascadingCursor, entry: Entry) -> ValidationResult {
    let ribosome = Ribosome::new(dna, cursor);
    ribosome.run_validation(entry)
}
