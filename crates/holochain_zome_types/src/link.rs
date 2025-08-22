use crate::prelude::*;
use holo_hash::{ActionHash, AgentPubKey};
pub use holochain_integrity_types::link::*;
use holochain_integrity_types::ZomeIndex;
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;

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
    /// Whether to fetch the corresponding create link record from the network if it does not
    /// exist locally, or to only look it up locally. Defaults to fetching from the network.
    pub get_options: GetOptions,
}

impl DeleteLinkInput {
    pub fn new(
        address: holo_hash::ActionHash,
        get_options: GetOptions,
        chain_top_ordering: ChainTopOrdering,
    ) -> Self {
        Self {
            address,
            get_options,
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

    /// Whether to fetch latest link metadata from the network or return only
    /// locally available metadata. Defaults to fetching latest metadata.
    pub get_options: GetOptions,

    /// The tag prefix to filter by.
    pub tag_prefix: Option<LinkTag>,

    /// Only include links created after this time.
    pub after: Option<Timestamp>,

    /// Only include links created before this time.
    pub before: Option<Timestamp>,

    /// Only include links created by this author.
    pub author: Option<AgentPubKey>,
}

impl GetLinksInput {
    /// Get a new [`GetLinksInput`] from query parameters [`LinkQuery`] and [`GetOptions`].
    pub fn from_query(query: LinkQuery, get_options: impl Into<GetOptions>) -> Self {
        Self {
            base_address: query.base,
            link_type: query.link_type,
            get_options: get_options.into(),
            tag_prefix: query.tag_prefix,
            author: query.author,
            after: query.after,
            before: query.before,
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
