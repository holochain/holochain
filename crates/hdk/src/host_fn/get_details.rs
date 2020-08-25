/// get an element from the hash AND the details for the entry or header hash passed in
/// returns None if the entry/header does not exist.
///
/// note: the return details will be inferred by the hash type passed in, be careful to pass in the
///       correct hash type for the details you want.
///
/// note: the element returned is the same that would be returned by `get!`, i.e. the "oldest live"
///       entry if an entry hash is passed in, or the specified element if a header hash is passed
///       @see get! for more information about what "oldest live" means
///
/// the details returned include relevant creates, updates and deletes for the hash passed in
///
/// creates are initial header/entry combinations (elements) produced by commit_entry! and cannot
/// reference other headers
/// updates and deletes both reference a specific header+entry combination when they are committed
/// updates must reference another create or update header+entry
/// deletes must reference a create or update header+entry (nothing can reference a delete)
///
/// details for a header hash return all updates and deletes that reference that specific header
/// details for an entry hash return all creates, updates and deletes that reference that entry
///
/// note: entries are just values, so can be referenced by many CRUD headers by many authors.
///       e.g. the number 1 or string "foo" can be referenced by anyone publishing CRUD headers at
///       any time they need to represent 1 or "foo" for a create, update or delete.
///       if you need to disambiguate entry values, provide uniqueness in the entry value such as
///       a unique hash (e.g. current chain head), timestamp (careful about collisions!), or random
///       bytes/uuid (see random_bytes!() and the uuid rust crate that supports uuids from bytes).
///
/// note: there are multiple header types that exist and operate entirely outside of CRUD elements
///       so they cannot reference or be referenced by CRUD, so are immutable or have their own
///       mutation logic (e.g. link add/remove), and will not be included in get_details! results
///       e.g. the DNA itself, links, migrations, etc.
///       however the header will still be returned by get_details! if a header hash is passed,
///       these header-only elements will have None as the entry value
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
