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
pub fn update<D: Into<EntryDefId>, E: Into<Entry>>(
    hash: HeaderHash,
    entry_def_id: D,
    entry: E,
) -> HdkResult<HeaderHash> {
    Ok(host_call::<UpdateInput, UpdateOutput>(
        __update,
        &UpdateInput::new((entry_def_id.into(), entry.into(), hash)),
    )?
    .into_inner())
}
