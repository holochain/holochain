use hdi::prelude::*;

#[hdi_entry_helper]
pub struct Test(String);

#[hdi_link_types]
pub enum LinkTypes {
    SomeLinks,
    SomeOtherLinks,
}
