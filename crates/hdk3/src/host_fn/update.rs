use crate::prelude::*;

/// Update any entry type.
///
/// This is used under the hood by `update_entry`, `update_cap_grant!` and `update_cap_claim!`.
/// @todo implement update_cap_claim
///
/// The host builds an `Update` header for the passed entry value and commits a new update to the
/// chain.
///
/// Usually you don't need to use this function directly; it is the most general way to update an
/// entry and standardises the internals of higher level create functions.
///
/// @see update_entry
/// @see update_cap_grant!
/// @see update_cap_claim!
pub fn update(hash: HeaderHash, entry_with_def_id: EntryWithDefId) -> ExternResult<HeaderHash> {
    host_call::<UpdateInput, HeaderHash>(__update, UpdateInput::new(hash, entry_with_def_id))
}
