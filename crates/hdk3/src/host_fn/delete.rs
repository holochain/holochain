use crate::prelude::*;

/// General that can delete any entry type.
///
/// This is used under the hood by `delete_entry`, `delete_cap_grant` and `delete_cap_claim!`.
/// @todo implement delete_cap_claim
///
/// The host builds a `Delete` header for the passed entry and commits a new element to the chain.
///
/// Usually you don't need to use this function directly; it is the most general way to update an
/// entry and standardises the internals of higher level create functions.
pub fn delete(hash: HeaderHash) -> ExternResult<HeaderHash> {
    host_call::<HeaderHash, HeaderHash>(__delete, hash)
}
