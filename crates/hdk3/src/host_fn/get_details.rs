use crate::prelude::*;

/// Get an element from the hash AND the details for the entry or header hash passed in.
/// Returns None if the entry/header does not exist.
/// The details returned are a contextual mix of elements and header hashes, see below.
///
/// Note: The return details will be inferred by the hash type passed in, be careful to pass in the
///       correct hash type for the details you want.
///
/// Note: If a header hash is passed in the element returned is the specified element.
///       If an entry hash is passed in all the headers (so implicitly all the elements) are
///       returned for the entry that matches that hash.
///       @see get for more information about what "oldest live" means.
///
/// The details returned include relevant creates, updates and deletes for the hash passed in.
///
/// Creates are initial header/entry combinations (elements) produced by commit_entry! and cannot
/// reference other headers.
/// Updates and deletes both reference a specific header+entry combination.
/// Updates must reference another create or update header+entry.
/// Deletes must reference a create or update header+entry (nothing can reference a delete).
///
/// Full elements are returned for direct references to the passed hash.
/// Header hashes are returned for references to references to the passed hash.
///
/// Details for a header hash return:
/// - the element for this header hash if it exists
/// - all update and delete _elements_ that reference that specified header
///
/// Details for an entry hash return:
/// - all creates, updates and delete _elements_ that reference that entry hash
/// - all update and delete _elements_ that reference the elements that reference the entry hash
///
/// Note: Entries are just values, so can be referenced by many CRUD headers by many authors.
///       e.g. the number 1 or string "foo" can be referenced by anyone publishing CRUD headers at
///       any time they need to represent 1 or "foo" for a create, update or delete.
///       If you need to disambiguate entry values, provide uniqueness in the entry value such as
///       a unique hash (e.g. current chain head), timestamp (careful about collisions!), or random
///       bytes/uuid (see random_bytes() and the uuid rust crate that supports uuids from bytes).
///
/// Note: There are multiple header types that exist and operate entirely outside of CRUD elements
///       so they cannot reference or be referenced by CRUD, so are immutable or have their own
///       mutation logic (e.g. link create/delete) and will not be included in get_details results
///       e.g. the DNA itself, links, migrations, etc.
///       However the element will still be returned by get_details if a header hash is passed,
///       these header-only elements will have None as the entry value.
pub fn get_details<H: Into<AnyDhtHash>>(
    hash: H,
    options: GetOptions,
) -> HdkResult<Option<Details>> {
    Ok(host_call::<GetDetailsInput, GetDetailsOutput>(
        __get_details,
        GetDetailsInput::new((hash.into(), options)),
    )?
    .into_inner())
}
