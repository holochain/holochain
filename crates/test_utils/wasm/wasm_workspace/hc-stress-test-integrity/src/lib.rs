pub mod file;
pub use file::*;
use hdi::prelude::*;
#[hdi_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    File(File),
}
#[hdi_link_types]
pub enum LinkTypes {
    FileUpdates,
    AllFiles,
    AllImages,
}
