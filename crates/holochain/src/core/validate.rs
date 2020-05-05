//! validate an entry via the ribosome
//! @see the ribosome docs for more info

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{RibosomeT, WasmRibosome};
use holochain_types::dna::Dna;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::validate::ValidateEntryResult;

/// build a ribosome from a dna and validate an entry
pub async fn run_validate(
    dna: Dna,
    zome_name: String,
    entry: &Entry,
) -> RibosomeResult<ValidateEntryResult> {
    let ribosome = WasmRibosome::new(dna);
    ribosome.run_validate(zome_name, entry)
}
