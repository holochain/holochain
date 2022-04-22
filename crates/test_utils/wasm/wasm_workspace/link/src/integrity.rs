use holochain_deterministic_integrity::prelude::*;

#[hdk_link_types]
pub enum LinkTypes {
    Any = HdkLinkType::Any as u8,
}
