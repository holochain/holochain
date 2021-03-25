use crate::element::SignedHeaderHashed;
use crate::header::HeaderDetails;
use crate::builder::HeaderDeterminism;
use crate::timestamp::Timestamp;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;

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
pub struct LinkTag(#[serde(with = "serde_bytes")] pub Vec<u8>);

impl LinkTag {
    /// New tag from bytes
    pub fn new<T>(t: T) -> Self
    where
        T: Into<Vec<u8>>,
    {
        Self(t.into())
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
    pub target: holo_hash::EntryHash,
    /// When the link was added
    pub timestamp: crate::Timestamp,
    /// A tag used to find this link
    pub tag: LinkTag,
    /// The hash of this link's create header
    pub create_link_hash: HeaderHash,
}

/// Zome IO inner type for link creation, optionally with HeaderDetails.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct CreateLinkInput {
    pub base_address: holo_hash::EntryHash,
    pub target_address: holo_hash::EntryHash,
    pub tag: LinkTag,
    pub details: Option<HeaderDetails>,
}

impl CreateLinkInput {
    pub fn new(
        base_address: holo_hash::EntryHash,
        target_address: holo_hash::EntryHash,
        tag: LinkTag,
    ) -> Self {
        Self {
            base_address,
            target_address,
            tag,
	    details: None,
        }
    }
}

impl HeaderDeterminism for CreateLinkInput {
    /// Specify a specific HeaderDetails Timestamp for CreateLinkInput
    fn at(self, timestamp: Timestamp) -> Self {
        Self {
            details: Some(HeaderDetails {
                timestamp: Some(timestamp),
                ..self.details.unwrap_or(HeaderDetails::default())
            }),
            ..self
        }
    }
    fn follows(self, prev_header: holo_hash::HeaderHash) -> Self {
        Self {
            details: Some(HeaderDetails {
                prev_header: Some(prev_header),
                ..self.details.unwrap_or(HeaderDetails::default())
            }),
            ..self
        }
    }
    fn sequence(self, header_seq: u32) -> Self {
        Self {
            details: Some(HeaderDetails {
                header_seq: Some(header_seq),
                ..self.details.unwrap_or(HeaderDetails::default())
            }),
            ..self
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct GetLinksInput {
    pub base_address: holo_hash::EntryHash,
    pub tag_prefix: Option<crate::link::LinkTag>,
}

impl GetLinksInput {
    pub fn new(
        base_address: holo_hash::EntryHash,
        tag_prefix: Option<crate::link::LinkTag>,
    ) -> Self {
        Self {
            base_address,
            tag_prefix,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Clone, Debug)]
pub struct Links(Vec<Link>);

impl From<Vec<Link>> for Links {
    fn from(v: Vec<Link>) -> Self {
        Self(v)
    }
}

impl From<Links> for Vec<Link> {
    fn from(links: Links) -> Self {
        links.0
    }
}

impl Links {
    pub fn into_inner(self) -> Vec<Link> {
        self.into()
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
