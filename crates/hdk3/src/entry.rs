pub mod create_entry;
pub mod delete_entry;
pub mod hash_entry;
pub mod update_entry;

/// Tuple struct to help juggle types internal to the HDK.
/// Makes it easier to avoid ambiguity in types and unneccesary clones.
/// @see create_entry()
pub struct HdkEntry(pub crate::prelude::EntryDefId, pub crate::prelude::Entry);
