use crate::record::SignedActionHashed;
use crate::ChainTopOrdering;
use holo_hash::ActionHash;
use holochain_integrity_types::ZomeId;
use holochain_serialized_bytes::prelude::*;

pub use holochain_integrity_types::link::*;

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
    /// The [`Entry`](crate::entry::Entry) being linked to
    pub target: holo_hash::AnyLinkableHash,
    /// When the link was added
    pub timestamp: crate::Timestamp,
    /// The [`ZomeId`] for where this link is defined.
    pub zome_id: ZomeId,
    /// The [`LinkType`] for this link.
    pub link_type: LinkType,
    /// A tag used to find this link
    pub tag: LinkTag,
    /// The hash of this link's create action
    pub create_link_hash: ActionHash,
}

/// Zome IO inner type for link creation.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct CreateLinkInput {
    pub base_address: holo_hash::AnyLinkableHash,
    pub target_address: holo_hash::AnyLinkableHash,
    pub zome_id: ZomeId,
    pub link_type: LinkType,
    pub tag: LinkTag,
    pub chain_top_ordering: ChainTopOrdering,
}

impl CreateLinkInput {
    pub fn new(
        base_address: holo_hash::AnyLinkableHash,
        target_address: holo_hash::AnyLinkableHash,
        zome_id: ZomeId,
        link_type: LinkType,
        tag: LinkTag,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            base_address,
            target_address,
            zome_id,
            link_type,
            tag,
            chain_top_ordering,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct DeleteLinkInput {
    /// Address of the link being deleted.
    pub address: holo_hash::ActionHash,
    /// Chain top ordering rules for writes.
    pub chain_top_ordering: ChainTopOrdering,
}

impl DeleteLinkInput {
    pub fn new(address: holo_hash::ActionHash, chain_top_ordering: ChainTopOrdering) -> Self {
        Self {
            address,
            chain_top_ordering,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct GetLinksInput {
    pub base_address: holo_hash::AnyLinkableHash,
    /// The link types to include in this get.
    pub link_type: LinkTypeFilter,
    pub tag_prefix: Option<crate::link::LinkTag>,
}

impl GetLinksInput {
    pub fn new(
        base_address: holo_hash::AnyLinkableHash,
        link_type: LinkTypeFilter,
        tag_prefix: Option<crate::link::LinkTag>,
    ) -> Self {
        Self {
            base_address,
            link_type,
            tag_prefix,
        }
    }
}

type CreateLinkWithDeleteLinks = Vec<(SignedActionHashed, Vec<SignedActionHashed>)>;
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
