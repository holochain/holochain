use crate::prelude::*;

/// Get all link creates and deletes that reference a base entry hash, optionally filtered by tag
///
/// Tag filtering is a simple bytes prefix.
///
/// e.g. if you had these links:
///      - a: `[ 1, 2, 3]`
///      - b: `[ 1, 2, 4]`
///      - c: `[ 1, 3, 5]`
///
/// then tag filters:
///      - `[ 1 ]` returns `[ a, b, c]`
///      - `[ 1, 2 ]` returns `[ a, b ]`
///      - `[ 1, 2, 3 ]` returns `[ a ]`
///      - `[ 5 ]` returns `[ ]` (does _not_ return c because the filter is by "prefix", not "contains")
///
/// This is mostly identical to get_links but it returns all the creates and all the deletes.
/// c.f. get_links that returns only the creates that have not been deleted.
///
/// @see get_links
pub fn get_link_details(base: EntryHash, link_tag: Option<LinkTag>) -> ExternResult<LinkDetails> {
    host_call::<GetLinksInputInner, LinkDetails>(
        __get_link_details,
        GetLinksInputInner::new(base, link_tag),
    )
}
