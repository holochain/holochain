use crate::hdk::HDK;
use crate::prelude::CloseChainInput;
use hdi::prelude::{ExternResult, MigrationTarget, OpenChainInput};
use holo_hash::ActionHash;
use holochain_zome_types::init::InitProperties;

/// Close your current source chain to indicate that you are planning to migrate to a new DNA.
///
/// This must be the last entry you try to make in your source chain. Holochain's system validation
/// will reject any actions that come after this one.
pub fn close_chain(new_target: Option<MigrationTarget>) -> ExternResult<ActionHash> {
    HDK.with(|h| h.borrow().close_chain(CloseChainInput { new_target }))
}

/// Indicate the DNA that you have migrated from. This should be committed to the new DNA's source
/// chain.
///
/// Holochain does not enforce an order for this action, or even that you must use it at all. It is
/// the only way that your app validation rules can know which DNA you have migrated from. So if
/// your app needs to know this to validate imported data then you will need to call this function.
pub fn open_chain(
    prev_target: MigrationTarget,
    close_hash: ActionHash,
) -> ExternResult<ActionHash> {
    HDK.with(|h| {
        h.borrow().open_chain(OpenChainInput {
            prev_target,
            close_hash,
        })
    })
}

/// Read the init properties supplied for this cell's role at install time.
///
/// Only callable from the `init` callback. The properties are opaque, app-defined bytes persisted
/// by the conductor and never written to the DHT. They are intended to seed a freshly migrated
/// chain, for example by carrying a signed summary and the action hash of the `CloseChain` action
/// from the chain being migrated from. Returns `None` when no init properties were supplied for
/// this cell's role.
pub fn get_init_properties() -> ExternResult<Option<InitProperties>> {
    HDK.with(|h| h.borrow().get_init_properties(()))
}
