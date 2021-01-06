use crate::prelude::*;

/// Create a link from a base entry to a target entry, with an optional tag.
///
/// Links represent the general idea of relationships between entries.
///
/// Links are different from the tree of CRUD relationships:
///
/// Links:
///
/// - reference two entries (base and target) not headers
/// - there is only one way to create a link, validation logic depends on only the base+target+tag
/// - can represent circular references because only entry hashes are needed
/// - support arbitrary bytes of data (i.e. "tag") that can be read or used to filter gets
/// - deletes always point to a _specific_ link creation event, not the link itself
/// - model dynamic sets of or relationships between things
/// - can reference any entry regardless of type (e.g. posts can link to comments)
/// - cannot reference other links or crud headers (@todo maybe we can do this in the future)
///
/// Note: There is a hard limit of 400 bytes of data for the tag as it is used as an lmdb key.
///
/// Crud:
///
/// - creates reference a single entry
/// - updates and deletes reference create/update elements by both their entry+header
/// - creates, updates and deletes all have different functions, network ops and validation logic
/// - is cryptographically guaranteed to be a DAG (not-circular) because they include headers
/// - model "mutability" for a single thing/identity in an immutable/append-only way
/// - only reference other entries of the same entry type (e.g. comments can _not_ update posts)
///
/// @see get_details and get for more information about CRUD
/// @see get_links and get_link_details for more information about filtering by tag
///
/// Generally links and CRUDs _do not interact_ beyond the fact that links need entry hashes to
/// reference for the base and target to already exist due to a prior create or update.
/// The entry value only needs to exist on the DHT for the link to validate, it doesn't need to be
/// live and can have any combination of valid/invalid crud headers.
/// i.e. if you use link_entries! to create relationships between two entries, then update_entry
/// on the base, the links will still only be visible to get_link(s_details)! against the original
/// base, there is no logic to "bring forward" links to the updated entry because:
///
/// - as per CRUD tree docs there is no "one size fits all" way to walk a tree of CRUDs
/// - links point at entries not headers so all create/update/delete information is in headers
/// - links are very generic and could even represent a comment thread against a specific revision
///   such as those found against individual updates in a wiki/CMS tool so they need to stay where
///   they were explicitly placed
/// - it would actually be pretty crazy at the network layer to be gossiping links around to chase
///   the "current revision" even if you could somehow unambiguously define "current revision"
///
/// This can be frustrating if you want "get all the links" for an entry but also be tracking your
/// revision history somehow.
/// A simple pattern to workaround this is to create an immutable (updates and deletes are invalid)
/// "identity" entry that links reference and is referenced as a field on the entry struct of each
/// create/update header.
/// If you have the hash of the identity entry you can get all the links, if you have the entry or
/// header hash for any of the creates or updates you can lookup the identity entry hash out of the
/// body of the create/update entry.
pub fn create_link<T: Into<LinkTag>>(
    base: EntryHash,
    target: EntryHash,
    tag: T,
) -> HdkResult<HeaderHash> {
    Ok(host_call::<CreateLinkInput, CreateLinkOutput>(
        __create_link,
        CreateLinkInput::new((base, target, tag.into())),
    )?
    .into_inner())
}
