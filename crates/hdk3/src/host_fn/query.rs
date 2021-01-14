use crate::prelude::*;

/// Walks the source chain in reverse (latest to oldest) filtering by header and/or entry type
///
/// Given a header and entry type, returns an ElementVec
///
/// @todo document this better with examples
/// @todo do we want to return elements rather than hashes?
/// @todo implement cap grant/claim usage in terms of query
pub fn query(filter: ChainQueryFilter) -> ExternResult<ElementVec> {
    host_call::<QueryInput, ElementVec>(__query, QueryInput::new(filter))
}
