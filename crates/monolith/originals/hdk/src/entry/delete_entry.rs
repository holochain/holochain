use crate::hdk3::prelude::*;

/// Alias to delete
///
/// Takes any expression that evaluates to the HeaderHash of the deleted element.
///
/// ```ignore
/// delete_entry(entry_hash(foo_entry)?)?;
/// ```
pub fn delete_entry(hash: HeaderHash) -> HdkResult<HeaderHash> {
    delete(hash)
}
