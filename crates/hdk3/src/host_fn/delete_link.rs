use crate::prelude::*;

/// Delete a specific link creation element by its header.
///
/// Links are always created and deleted in pairs.
/// The delete is a "tombstone" for one pair of the create and the delete elements.
///
/// For this reason the delete references the header of the create.
/// Even more than that, both creates and deletes are _only_ headers, there is no separate entry
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
/// All of this is bad so link creates point to entries (@see link_entries!) and deletes point to
/// creates.
pub fn delete_link(add_link_header: HeaderHash) -> HdkResult<HeaderHash> {
    Ok(host_call::<DeleteLinkInput, DeleteLinkOutput>(
        __delete_link,
        DeleteLinkInput::new(add_link_header),
    )?
    .into_inner())
}
