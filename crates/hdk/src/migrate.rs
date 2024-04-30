use hdi::prelude::{ExternResult, OpenChainInput};
use holo_hash::{ActionHash, DnaHash};
use crate::hdk::HDK;
use crate::prelude::CloseChainInput;

/// Close your current source chain to indicate that you are planning to migrate to a new DNA.
///
/// This must be the last entry you try to make in your source chain. Holochain's sytem validation
/// will reject any actions that come after this one.
pub fn close_chain(new_dna_hash: DnaHash) -> ExternResult<ActionHash> {
    HDK.with(|h| {
        h.borrow()
            .close_chain(CloseChainInput {
                new_dna_hash,
            })
    })
}

/// Indicate the DNA that you have migrated from. This should be committed to the new DNA's source
/// chain.
///
/// Holochain does not enforce an order for this action, or even that you must use it at all. It is
/// the only way that your app validation rules can know which DNA you have migrated from. So if
/// your app needs to know this to validate imported data then you will need to call this function.
pub fn open_chain(prev_dna_hash: DnaHash) -> ExternResult<ActionHash> {
    HDK.with(|h| h.borrow().open_chain(OpenChainInput {
        prev_dna_hash,
    }))
}
