pub mod file;
pub use file::*;
use hdi::prelude::*;
#[hdk_entry_defs]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    File(File),
}
#[hdk_link_types]
pub enum LinkTypes {
    FileUpdates,
    AllFiles,
    AllImages,
}
