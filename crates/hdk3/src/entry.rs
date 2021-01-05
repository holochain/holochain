pub mod create_entry;
pub mod delete_entry;
pub mod hash_entry;
pub mod update_entry;

pub struct HdkEntry(pub crate::prelude::EntryDefId, pub crate::prelude::Entry);
