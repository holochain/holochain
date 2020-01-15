

use crate::shims::*;

pub async fn run_validation(dna: Dna, cursor: CascadingCursor, xf: DhtTransform) -> ValidationResult {
    let ribosome = Ribosome::new(dna, cursor);
    ribosome.run_validation(xf)
}
