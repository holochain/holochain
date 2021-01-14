use crate::prelude::*;

/// Returns all links that reference a base entry hash, optionally filtered by tag.
///
/// Tag filtering is a simple bytes prefix.
///
/// e.g. if you had these links:
///      - a: `[ 1, 2, 3]`
///      - b: `[ 1, 2, 4]`
///      - c: `[ 1, 3, 5]`
///
/// Then tag filters:
///      - `[ 1 ]` returns `[ a, b, c]`
///      - `[ 1, 2 ]` returns `[ a, b ]`
///      - `[ 1, 2, 3 ]` returns `[ a ]`
///      - `[ 5 ]` returns `[ ]` (does _not_ return c because the filter is by "prefix", not "contains")
///
/// This is mostly identical to `get_link_details` but returns only creates that have not been
/// deleted c.f. get_link_details that returns all the creates and all the deletes together.
///
/// @see get_link_details
pub fn get_links(base: EntryHash, link_tag: Option<LinkTag>) -> ExternResult<Links> {
    host_call::<GetLinksInputInner, Links>(__get_links, GetLinksInputInner::new(base, link_tag))
}
