use crate::element::SignedHeaderHashed;
use crate::ChainTopOrdering;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;

#[derive(
    Debug,
    Copy,
    Clone,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct LinkType(pub u8);

impl LinkType {
    pub fn new(u: u8) -> Self {
        Self(u)
    }

    pub fn into_inner(self) -> u8 {
        self.0
    }
}

/// Opaque tag for the link applied at the app layer, used to differentiate
/// between different semantics and validation rules for different links
#[derive(
    Debug,
    PartialOrd,
    Ord,
    Clone,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct LinkTag(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl LinkTag {
    /// New tag from bytes
    pub fn new<T>(t: T) -> Self
    where
        T: Into<Vec<u8>>,
    {
        Self(t.into())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for LinkTag {
    fn from(b: Vec<u8>) -> Self {
        Self(b)
    }
}

impl From<()> for LinkTag {
    fn from(_: ()) -> Self {
        Self(Vec::new())
    }
}

impl AsRef<Vec<u8>> for LinkTag {
    fn as_ref(&self) -> &Vec<u8> {
        &self.0
    }
}

#[derive(
    Debug,
    PartialOrd,
    Ord,
    Clone,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
)]
pub struct Link {
    /// The [Entry] being linked to
    pub target: holo_hash::AnyLinkableHash,
    /// When the link was added
    pub timestamp: crate::Timestamp,
    /// A tag used to find this link
    pub tag: LinkTag,
    /// The hash of this link's create header
    pub create_link_hash: HeaderHash,
}

/// Zome IO inner type for link creation.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct CreateLinkInput {
    pub base_address: holo_hash::AnyLinkableHash,
    pub target_address: holo_hash::AnyLinkableHash,
    pub link_type: LinkType,
    pub tag: LinkTag,
    pub chain_top_ordering: ChainTopOrdering,
}

impl CreateLinkInput {
    pub fn new(
        base_address: holo_hash::AnyLinkableHash,
        target_address: holo_hash::AnyLinkableHash,
        link_type: LinkType,
        tag: LinkTag,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            base_address,
            target_address,
            link_type,
            tag,
            chain_top_ordering,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct DeleteLinkInput {
    /// Address of the link being deleted.
    pub address: holo_hash::HeaderHash,
    /// Chain top ordering rules for writes.
    pub chain_top_ordering: ChainTopOrdering,
}

impl DeleteLinkInput {
    pub fn new(address: holo_hash::HeaderHash, chain_top_ordering: ChainTopOrdering) -> Self {
        Self {
            address,
            chain_top_ordering,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct GetLinksInput {
    pub base_address: holo_hash::AnyLinkableHash,
    pub tag_prefix: Option<crate::link::LinkTag>,
}

impl GetLinksInput {
    pub fn new(
        base_address: holo_hash::AnyLinkableHash,
        tag_prefix: Option<crate::link::LinkTag>,
    ) -> Self {
        Self {
            base_address,
            tag_prefix,
        }
    }
}

type CreateLinkWithDeleteLinks = Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)>;
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
/// CreateLinks with and DeleteLinks on them
/// `[CreateLink, [DeleteLink]]`
pub struct LinkDetails(CreateLinkWithDeleteLinks);

impl From<CreateLinkWithDeleteLinks> for LinkDetails {
    fn from(v: CreateLinkWithDeleteLinks) -> Self {
        Self(v)
    }
}

impl From<LinkDetails> for CreateLinkWithDeleteLinks {
    fn from(link_details: LinkDetails) -> Self {
        link_details.0
    }
}

impl LinkDetails {
    pub fn into_inner(self) -> CreateLinkWithDeleteLinks {
        self.into()
    }
}

#[cfg(feature = "full")]
impl rusqlite::ToSql for LinkTag {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed((&self.0[..]).into()))
    }
}
