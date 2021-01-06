use crate::prelude::*;

/// General function that can create any entry type.
///
/// This is used under the hood by `create_entry`, `create_cap_grant` and `create_cap_claim`.
///
/// The host builds a `Create` header for the passed entry value and commits a new element to the
/// chain.
///
/// Usually you don't need to use this function directly; it is the most general way to create an
/// entry and standardises the internals of higher level create functions.
///
/// @see create_entry
/// @see create_cap_grant
/// @see create_cap_claim
pub fn create<D: Into<EntryDefId>, E: Into<Entry>>(
    entry_def_id: D,
    entry: E,
) -> HdkResult<HeaderHash> {
    Ok(host_call::<CreateInput, CreateOutput>(
        __create,
        CreateInput::new((entry_def_id.into(), entry.into())),
    )?
    .into_inner())
}
