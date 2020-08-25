/// create a link from a base entry to a target entry, with an optional tag
///
/// links represent the general idea of relationships between entries
///
/// links are different from the tree of CRUD relationships:
///
/// links:
///
/// - reference two entries (base and target) not headers
/// - there is only one way to add a link, validation logic depends on only the base+target+tag
/// - can represent circular references because only entry hashes are needed
/// - support arbitrary bytes of data (i.e. "tag") that can be read or used to filter gets
/// - link adds and removes are 1:1 pairs, a remove always points to a _specific_ link
/// - model dynamic sets of or relationships between things
/// - can reference any entry regardless of type (e.g. posts can link to comments)
///
/// note: there is no hard limit but store a "small" amount of data in the tag because it is
///       gossiped around the network as a header and internal optimisations may assume it is
///       "small"
///
/// crud:
///
/// - creates reference a single entry
/// - updates and deletes reference create/update elements by both their entry+header
/// - creates, updates and deletes all have different macros, network ops and validation logic
/// - are cryptographically guaranteed to be a DAG (not-circular) because they include headers
/// - model "mutability" for a single thing/identity in an immutable/append-only way
/// - only reference other entries of the same entry type (e.g. comments can _not_ update posts)
///
/// @see get_details! and get! for more information about CRUD
/// @see get_links! and get_link_details! for more information about filtering by tag
///
/// generally links and CRUDs _do not interact_ beyond the fact that links need entry hashes to
/// reference for the base and target to already exist due to a prior create or update.
/// i.e. if you use link_entries! to create relationships between two entries, then update_entry!
/// on the base, the links will still only be visible to get_link(s_details)! against the original
/// base, there is no logic to "bring forward" links to the updated entry because:
///
/// - as per CRUD tree docs there is no "one size fits all" way to walk a tree of CRUDs
/// - links point at entries not headers! all create/update/delete information is in headers
/// - links are very generic and could even represent a comment thread against a specific revision
///   such as those found against individual updates in a wiki/CMS tool so they need to stay where
///   they were explicitly placed
/// - it would actually be pretty crazy at the network layer to be gossiping links around to chase
///   the "current revision" even if you could somehow unambiguously define "current revision"
///
/// this can be frustrating if you want "get all the links" for an entry but also be tracking your
/// revision history somehow.
/// a simple pattern to workaround this is to create an immutable (updates and deletes are invalid)
/// "identity" entry that links reference and is referenced as a field on the entry struct of each
/// create/update header.
/// if you have the hash of the identity entry you can get all the links, if you have the entry or
/// header hash for any of the creates or updates you can lookup the identity entry hash out of the
/// body of the create/update entry.
#[macro_export]
macro_rules! link_entries {
    ( $base:expr, $target:expr ) => {
        $crate::link_entries!($base, $target, vec![])
    };
    ( $base:expr, $target:expr, $tag:expr ) => {{
        $crate::host_fn!(
            __link_entries,
            $crate::prelude::LinkEntriesInput::new(($base, $target, $tag.into())),
            $crate::prelude::LinkEntriesOutput
        )
    }};
}
