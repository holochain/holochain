use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{RibosomeT, WasmRibosome};
use holochain_types::{dna::Dna, entry::Entry, shims::*};

pub async fn run_init(
    dna: Dna,
    zome_name: String,
    entry: &Entry,
) -> RibosomeResult<InitCallbackResult> {
    let ribosome = WasmRibosome::new(dna);
    // at the end of all the zomes succeeding to init i want to commit an initialization complete
    // entry, this is the only way we can treat it as transactional is if all the zomes do their
    // thing and at the end of which we can say init complete
    // if one zome inits and another does not we have to retry
    // @todo if any of these fail, fail the whole thing
    for zome in dna.zomes() {
        ribosome.run_init(zome_name, entry)
    }
}
