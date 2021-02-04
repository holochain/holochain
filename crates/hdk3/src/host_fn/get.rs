use crate::prelude::*;

/// Gets an element for a given entry or header hash.
///
/// The behaviour of get changes subtly per the _type of the passed hash_.
/// A header hash returns the element for that header, i.e. header+entry or header+None.
/// An entry hash returns the "oldest live" element, i.e. header+entry.
///
/// An element is no longer live once it is referenced by a valid delete element.
/// An update to an element does not change its liveness.
/// @see get_details for more information about how CRUD elements reference each other.
///
/// Note: `get` __always triggers and blocks on a network call__.
///       @todo implement a 'get optimistic' that returns based on the current opinion of the world
///       and performs network calls in the background so they are available 'next time'.
///
/// Note: Deletes are considered in the liveness but Updates are not currently followed
///       automatically due to the need for the happ to disambiguate update logic.
///       @todo implement 'redirect' logic so that updates are followed by `get`.
///
/// Note: Updates typically point to a different entry hash than what they are updating but not
///       always, e.g. consider changing `foo` to `bar` back to `foo`. The entry hashes in a crud
///       tree can be circular but the header hashes are never circular.
///       In this case, deleting the create for foo would make the second update pointing to foo
///       the "oldest live" element.
///
/// Note: "oldest live" only relates to disambiguating many creates and updates from many authors
///       pointing to a single entry, it is not the "current value" of an entry in a CRUD sense.
///       e.g. If "foo" is created then updated to "bar", a `get` on the hash of "foo" will return
///            "foo" as part of an element with the "oldest live" header.
///            To discover "bar" the agent needs to call `get_details` and decide how it wants to
///            collapse many potential creates, updates and deletes down into a single or filtered
///            set of updates, to "walk the tree".
///       e.g. Updates could include a proof of work and a tree would collapse to a simple
///            blockchain if the agent follows the "heaviest chain".
///       e.g. Updates could represent turns in a 2-player game and the update with the newest
///            timestamp countersigned by both players represents an opt-in chain of updates with
///            support for casual "undo" with player's consent.
///       e.g. Domain/user names could be claimed on a "first come, first serve" basis with only
///            creates and deletes allowed by validation rules, the "oldest live" element _does_
///            represent the element pointing at the first agent to claim a name, but it could also
///            be checked manually by the app with `get_details`.
///
/// Note: "oldest live" is only as good as the information available to the authorities the agent
///       contacts on their current network partition, there could always be an older live entry
///       on another partition, and of course the oldest live entry could be deleted and no longer
///       be live.
pub fn get<H>(hash: H, options: GetOptions) -> ExternResult<Option<Element>>
where
    AnyDhtHash: From<H>,
{
    host_call::<GetInput, Option<Element>>(__get, GetInput::new(AnyDhtHash::from(hash), options))
}
