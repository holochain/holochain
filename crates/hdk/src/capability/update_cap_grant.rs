/// Updates a CapGrant.
///
/// An update works exactly as a grant delete+create.
///
/// The hash evalutes to the HeaderHash of the deleted grant.
/// The input evalutes to the new grant.
///
/// @see create_cap_grant
/// @see delete_cap_grant
#[macro_export]
macro_rules! update_cap_grant {
    ( $hash:expr, $input:expr ) => {{
        update!(
            $hash,
            $crate::prelude::EntryDefId::CapGrant,
            $crate::prelude::Entry::CapGrant($input)
        )
    }};
}
pub fn update_cap_grant(
    hash: HeaderHash,
    input: CapGrantEntry,
) -> HdkResult<HeaderHash> {
    update!(
        hash,
        EntryDefId::CapGrant,
        Entry::CapGrant(input),
    )
}
