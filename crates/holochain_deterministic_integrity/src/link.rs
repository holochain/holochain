use holochain_integrity_types::LinkType;

#[repr(u8)]
pub enum HdkLinkType {
    Paths = u8::MAX - 1,
    Any = u8::MAX,
}

impl From<HdkLinkType> for LinkType {
    fn from(hdk_link_type: HdkLinkType) -> Self {
        Self(hdk_link_type as u8)
    }
}
