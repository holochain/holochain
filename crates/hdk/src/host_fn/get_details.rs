/// Get an element from the hash AND the details for the entry or header hash passed in.
/// Returns None if the entry/header does not exist.
///
/// Note: The return details will be inferred by the hash type passed in, be careful to pass in the
///       correct hash type for the details you want.
///
/// Note: The element returned is the same that would be returned by `get!`, i.e. the "oldest live"
///       entry if an entry hash is passed in, or the specified element if a header hash is passed
///       @see get! for more information about what "oldest live" means.
///
/// The details returned include relevant creates, updates and deletes for the hash passed in.
///
/// Creates are initial header/entry combinations (elements) produced by commit_entry! and cannot
/// reference other headers.
/// Updates and deletes both reference a specific header+entry combination.
/// Updates must reference another create or update header+entry.
/// Deletes must reference a create or update header+entry (nothing can reference a delete).
///
/// Details for a header hash return all updates and deletes that reference that specific header.
/// Details for an entry hash return all creates, updates and deletes that reference that entry.
///
/// Note: Entries are just values, so can be referenced by many CRUD headers by many authors.
///       e.g. the number 1 or string "foo" can be referenced by anyone publishing CRUD headers at
///       any time they need to represent 1 or "foo" for a create, update or delete.
///       If you need to disambiguate entry values, provide uniqueness in the entry value such as
///       a unique hash (e.g. current chain head), timestamp (careful about collisions!), or random
///       bytes/uuid (see random_bytes!() and the uuid rust crate that supports uuids from bytes).
///
/// Note: There are multiple header types that exist and operate entirely outside of CRUD elements
///       so they cannot reference or be referenced by CRUD, so are immutable or have their own
///       mutation logic (e.g. link create/delete) and will not be included in get_details! results
///       e.g. the DNA itself, links, migrations, etc.
///       However the element will still be returned by get_details! if a header hash is passed,
///       these header-only elements will have None as the entry value.
#[macro_export]
macro_rules! get_details {
    ( $hash:expr, $options:expr ) => {{
        $crate::host_fn!(
            __get_details,
            $crate::prelude::GetDetailsInput::new(($hash.into(), $options)),
            $crate::prelude::GetDetailsOutput
        )
    }};
    ( $hash:expr ) => {
        get_details!($hash, $crate::prelude::GetOptions)
    };
}
