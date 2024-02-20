use crate::prelude::*;
use holo_hash::{ActionHash, AgentPubKey};
use holochain_integrity_types::ZomeIndex;
use holochain_serialized_bytes::prelude::*;

pub use holochain_integrity_types::link::*;
use kitsune_p2p_timestamp::Timestamp;

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
    /// The author of this link
    pub author: holo_hash::AgentPubKey,
    /// The [AnyLinkableHash] being linked from
    pub base: holo_hash::AnyLinkableHash,
    /// The [AnyLinkableHash] being linked to
    pub target: holo_hash::AnyLinkableHash,
    /// When the link was added
    pub timestamp: Timestamp,
    /// The [`ZomeIndex`] for where this link is defined.
    pub zome_index: ZomeIndex,
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
    pub zome_index: ZomeIndex,
    pub link_type: LinkType,
    pub tag: LinkTag,
    pub chain_top_ordering: ChainTopOrdering,
}

impl CreateLinkInput {
    pub fn new(
        base_address: holo_hash::AnyLinkableHash,
        target_address: holo_hash::AnyLinkableHash,
        zome_index: ZomeIndex,
        link_type: LinkType,
        tag: LinkTag,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            base_address,
            target_address,
            zome_index,
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
    /// The base to get links from.
    pub base_address: holo_hash::AnyLinkableHash,

    /// The link types to include in this get.
    pub link_type: LinkTypeFilter,

    /// The tag prefix to filter by.
    pub tag_prefix: Option<LinkTag>,

    /// Only include links created after this time.
    pub after: Option<Timestamp>,

    /// Only include links created before this time.
    pub before: Option<Timestamp>,

    /// Only include links created by this author.
    pub author: Option<AgentPubKey>,
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
