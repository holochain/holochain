use crate::prelude::*;

/// Updates a CapGrant.
///
/// An update works exactly as a grant delete+create.
///
/// The hash evalutes to the HeaderHash of the deleted grant.
/// The input evalutes to the new grant.
///
/// @see create_cap_grant
/// @see delete_cap_grant
pub fn update_cap_grant(hash: HeaderHash, input: CapGrantEntry) -> HdkResult<HeaderHash> {
    update(hash, EntryDefId::CapGrant, Entry::CapGrant(input))
}
