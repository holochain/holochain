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
pub fn create(entry_def_with_id: EntryWithDefId) -> ExternResult<HeaderHash> {
    host_call::<EntryWithDefId, HeaderHash>(__create, entry_def_with_id)
}
