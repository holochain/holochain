use crate::prelude::*;

pub use hdi::link::*;

/// Create a link from a base entry to a target entry, with an optional tag.
///
/// Links represent the general idea of relationships between entries.
///
/// Links are different from the tree of CRUD relationships:
///
/// Links:
///
/// - reference two entries (base and target) not actions
/// - there is only one way to create a link, validation logic depends on only the base+target+tag
/// - can represent circular references because only entry hashes are needed
/// - support arbitrary bytes of data (i.e. "tag") that can be read or used to filter gets
/// - deletes always point to a _specific_ link creation event, not the link itself
/// - model dynamic sets of or relationships between things
/// - can reference any entry regardless of type (e.g. posts can link to comments)
/// - cannot reference other links or crud actions (@todo maybe we can do this in the future)
///
/// Note: There is a hard limit of 1kb of data for the tag.
///
/// Crud:
///
/// - creates reference a single entry
/// - updates and deletes reference create/update records by both their entry+action
/// - creates, updates and deletes all have different functions, network ops and validation logic
/// - is cryptographically guaranteed to be a DAG (not-circular) because they include actions
/// - model "mutability" for a single thing/identity in an immutable/append-only way
/// - only reference other entries of the same entry type (e.g. comments can _not_ update posts)
///
/// See [ `get_details` ] and get for more information about CRUD
/// See [ `get_links` ] and [ `get_link_details` ] for more information about filtering by tag
///
/// Generally links and CRUDs _do not interact_ beyond the fact that links need entry hashes to
/// reference for the base and target to already exist due to a prior create or update.
/// The entry value only needs to exist on the DHT for the link to validate, it doesn't need to be
/// live and can have any combination of valid/invalid crud actions.
/// i.e. if you use link_entries! to create relationships between two entries, then update_entry
/// on the base, the links will still only be visible to get_link(s_details)! against the original
/// base, there is no logic to "bring forward" links to the updated entry because:
///
/// - as per CRUD tree docs there is no "one size fits all" way to walk a tree of CRUDs
/// - links point at entries not actions so all create/update/delete information is in actions
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
/// create/update action.
/// If you have the hash of the identity entry you can get all the links, if you have the entry or
/// action hash for any of the creates or updates you can lookup the identity entry hash out of the
/// body of the create/update entry.
pub fn create_link<T, E>(
    base_address: impl Into<AnyLinkableHash>,
    target_address: impl Into<AnyLinkableHash>,
    link_type: T,
    tag: impl Into<LinkTag>,
) -> ExternResult<ActionHash>
where
    ScopedLinkType: TryFrom<T, Error = E>,
    WasmError: From<E>,
{
    let ScopedLinkType {
        zome_id,
        zome_type: link_type,
    } = link_type.try_into()?;
    HDK.with(|h| {
        h.borrow().create_link(CreateLinkInput::new(
            base_address.into(),
            target_address.into(),
            zome_id,
            link_type,
            tag.into(),
            ChainTopOrdering::default(),
        ))
    })
}

/// Delete a specific link creation record.
///
/// Links are defined by a [OR-Set CRDT](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type#OR-Set_(Observed-Remove_Set))
/// of "Creates" and "Deletes".
/// The deletes form a "tombstone set", each of which can nullify one of the creates.
/// A link only "exists" if it has one or more "creates" which have not been nullified by a "delete".
///
/// For this reason the delete references the Create Action, not the Entry.
/// Even more than that, both creates and deletes are _only_ actions, there is no separate entry
/// and the delete is not simply a renamed mirror of the create (i.e. with base and target).
///
/// Consider what would happen if the system simply had "create link" and "delete link" pointing at
/// the entry base and target without pairing:
/// - there would be no way to revert a specific link creation
/// - a delete may be intended for an create you haven't seen yet, so network unpredictability
///   would cause re-ording of any view on create/deletes which means an agent can see more deletes
///   than creates, etc.
/// - there would only be two ways to summarise the state of a relationship between two entries,
///   either "there are N more/less creates than deletes" or "there is at least one delete", the
///   former leads to flakiness as above and the latter means it would be impossible to create a
///   link after any previous delete of any link.
/// All of this is bad so link creates point to entries (See [ `create_link` ]) and deletes point to
/// creates.
pub fn delete_link(address: ActionHash) -> ExternResult<ActionHash> {
    HDK.with(|h| {
        h.borrow()
            .delete_link(DeleteLinkInput::new(address, ChainTopOrdering::default()))
    })
}

/// Returns all links that reference a base entry hash, optionally filtered by link type and tag.
///
/// Type can be filtered by providing a variant of the link types. To get links of all types, the
/// range operator can be used `get_links(base, .., None)`. Furthermore, vectors of link types can
/// be passed in to specify multiple types. Refer to the `get_links` function in
/// ]this coordinator zome](https://github.com/holochain/holochain/blob/develop/crates/test_utils/wasm/wasm_workspace/link/src/coordinator.rs)
/// for several examples.
/// 
/// Tag filtering is a simple bytes prefix.
///
/// e.g. if you had these links:
///   - a: `[ 1, 2, 3]`
///   - b: `[ 1, 2, 4]`
///   - c: `[ 1, 3, 5]`
///
/// Then tag filters:
///   - `[ 1 ]` returns `[ a, b, c]`
///   - `[ 1, 2 ]` returns `[ a, b ]`
///   - `[ 1, 2, 3 ]` returns `[ a ]`
///   - `[ 5 ]` returns `[ ]` (does _not_ return c because the filter is by "prefix", not "contains")
///
/// This is mostly identical to `get_link_details` but returns only creates that have not been
/// deleted c.f. get_link_details that returns all the creates and all the deletes together.
///
/// See [ `get_link_details` ].
pub fn get_links(
    base: impl Into<AnyLinkableHash>,
    link_type: impl LinkTypeFilterExt,
    link_tag: Option<LinkTag>,
) -> ExternResult<Vec<Link>> {
    let link_type = link_type.try_into_filter()?;
    Ok(HDK
        .with(|h| {
            h.borrow()
                .get_links(vec![GetLinksInput::new(base.into(), link_type, link_tag)])
        })?
        .into_iter()
        .next()
        .unwrap())
}

/// Get all link creates and deletes that reference a base entry hash, optionally filtered by tag
///
/// Tag filtering is a simple bytes prefix.
///
/// e.g. if you had these links:
///   - a: `[ 1, 2, 3]`
///   - b: `[ 1, 2, 4]`
///   - c: `[ 1, 3, 5]`
///
/// then tag filters:
///   - `[ 1 ]` returns `[ a, b, c]`
///   - `[ 1, 2 ]` returns `[ a, b ]`
///   - `[ 1, 2, 3 ]` returns `[ a ]`
///   - `[ 5 ]` returns `[ ]` (does _not_ return c because the filter is by "prefix", not "contains")
///
/// This is mostly identical to get_links but it returns all the creates and all the deletes.
/// c.f. get_links that returns only the creates that have not been deleted.
///
/// See [ `get_links` ].
pub fn get_link_details(
    base: impl Into<AnyLinkableHash>,
    link_type: impl LinkTypeFilterExt,
    link_tag: Option<LinkTag>,
) -> ExternResult<LinkDetails> {
    let link_type = link_type.try_into_filter()?;
    Ok(HDK
        .with(|h| {
            h.borrow()
                .get_link_details(vec![GetLinksInput::new(base.into(), link_type, link_tag)])
        })?
        .into_iter()
        .next()
        .unwrap())
}
