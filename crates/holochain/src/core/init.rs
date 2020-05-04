//! init a dna via the ribosome
//! @see the ribosome docs for more info

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::{RibosomeT, WasmRibosome};
use holochain_types::dna::Dna;
use holochain_types::init::InitDnaResult;

/// init a dna
pub async fn init_dna(dna: Dna) -> RibosomeResult<InitDnaResult> {
    let ribosome = WasmRibosome::new(dna);
    // at the end of all the zomes succeeding to init i want to commit an initialization complete
    // entry, this is the only way we can treat it as transactional is if all the zomes do their
    // thing and at the end of which we can say init complete
    // if one zome inits and another does not we have to retry
    // @todo if any of these fail, fail the whole thing
    // NOTE: the InitDnaResult already aggregates InitCallbackResult values in the ribosome
    // any fail already = total fail
    ribosome.run_init()
}
