/// removes a specific link by its header
///
/// links are always added and removed in pairs
/// the remove is a "tombstone" for the set of the add and the remove
///
/// for this reason the remove references the header of the add
/// well, even more than that, both adds and removes are _only_ headers, there is no separate entry
/// and the remove is not simply a renamed mirror of the add (i.e. with base and target)
///
/// consider what would happen if the system simply had "add link" and "remove link" pointing at
/// the entry base and target without pairing:
/// - there would be no way to do the inverse of a specific link add
/// - a remove may be intended for an add you haven't seen yet, so network unpredictability would
///   cause re-ording of add/removes which means an agent can see more removes than adds, etc.
/// - there would only be two ways to summarise the state of a relationship between two entries,
///   either "there are N more/less adds than removes" or "there is at least one remove", the
///   former leads to flakiness as above and the latter means it would be impossible to add a link
///   after any previous removal of any link.
/// all of this is bad so link adds point to entries (@see link_entries!) and removes point to adds
#[macro_export]
macro_rules! delete_link {
    ( $add_link_header:expr ) => {{
        $crate::prelude::host_externs!(__delete_link);

        $crate::host_fn!(
            __delete_link,
            $crate::prelude::DeleteLinkInput::new($add_link_header),
            $crate::prelude::DeleteLinkOutput
        )
    }};
}
