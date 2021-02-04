use crate::prelude::*;

/// Alias to delete
///
/// Takes the HeaderHash of the element to delete.
///
/// ```ignore
/// delete_entry(entry_hash(foo_entry)?)?;
/// ```
pub fn delete_entry(hash: HeaderHash) -> ExternResult<HeaderHash> {
    delete(hash)
}
