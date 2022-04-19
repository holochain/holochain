//! Links interrelate entries in a source chain.

use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::AnyLinkableHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;
use regex::Regex;

use crate::dht_op::error::DhtOpError;
use crate::dht_op::error::DhtOpResult;
use crate::dht_op::DhtOpType;
use crate::dht_op::RenderedOp;
use crate::dht_op::RenderedOps;

/// Links interrelate entries in a source chain.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, SerializedBytes)]
pub struct Link {
    base: EntryHash,
    target: EntryHash,
    tag: LinkTag,
}

/// Owned link key for sending across networks
#[deprecated = "This is being replaced by WireLinkKey"]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum WireLinkMetaKey {
    /// Search for all links on a base
    Base(EntryHash),
    /// Search for all links on a base, for a zome
    BaseZome(EntryHash, ZomeId),
    /// Search for all links on a base, for a zome and with a tag
    BaseZomeTag(EntryHash, ZomeId, LinkTag),
    /// This will match only the link created with a certain [CreateLink] hash
    Full(EntryHash, ZomeId, LinkTag, HeaderHash),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Link key for sending across the wire for get links requests.
pub struct WireLinkKey {
    /// Base the links are on.
    pub base: AnyLinkableHash,
    /// The zome the links are in.
    pub type_query: Option<LinkTypeQuery<ZomeId>>,
    /// Optionally specify a tag for more specific queries.
    pub tag: Option<LinkTag>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes, Default)]
/// Condensed link ops for sending across the wire in response to get links.
pub struct WireLinkOps {
    /// create links that match this query.
    pub creates: Vec<WireCreateLink>,
    /// delete links that match this query.
    pub deletes: Vec<WireDeleteLink>,
}

impl WireLinkOps {
    /// Create an empty wire response.
    pub fn new() -> Self {
        Default::default()
    }
    /// Render these ops to their full types.
    pub fn render(self, key: &WireLinkKey) -> DhtOpResult<RenderedOps> {
        let Self { creates, deletes } = self;
        let mut ops = Vec::with_capacity(creates.len() + deletes.len());
        // We silently ignore ops that fail to render as they come from the network.
        ops.extend(creates.into_iter().filter_map(|op| op.render(key).ok()));
        ops.extend(deletes.into_iter().filter_map(|op| op.render(key).ok()));
        Ok(RenderedOps {
            ops,
            ..Default::default()
        })
    }
}

#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Condensed version of a [`CreateLink`]
pub struct WireCreateLink {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub target_address: AnyLinkableHash,
    pub zome_id: ZomeId,
    pub link_type: LinkType,
    pub tag: Option<LinkTag>,
    pub signature: Signature,
    pub validation_status: ValidationStatus,
}

#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Condensed version of a [`DeleteLink`]
pub struct WireDeleteLink {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub header_seq: u32,
    pub prev_header: HeaderHash,

    pub link_add_address: HeaderHash,
    pub signature: Signature,
    pub validation_status: ValidationStatus,
}

impl WireCreateLink {
    fn new(
        h: CreateLink,
        signature: Signature,
        validation_status: ValidationStatus,
        tag: bool,
    ) -> Self {
        Self {
            author: h.author,
            timestamp: h.timestamp,
            header_seq: h.header_seq,
            prev_header: h.prev_header,
            target_address: h.target_address,
            zome_id: h.zome_id,
            link_type: h.link_type,
            tag: if tag { Some(h.tag) } else { None },
            signature,
            validation_status,
        }
    }
    /// Condense down a create link op for the wire without a tag.
    pub fn condense_base_only(
        h: CreateLink,
        signature: Signature,
        validation_status: ValidationStatus,
    ) -> Self {
        Self::new(h, signature, validation_status, false)
    }
    /// Condense down a create link op for the wire with a tag.
    pub fn condense(
        h: CreateLink,
        signature: Signature,
        validation_status: ValidationStatus,
    ) -> Self {
        Self::new(h, signature, validation_status, true)
    }
    /// Render these ops to their full types.
    pub fn render(self, key: &WireLinkKey) -> DhtOpResult<RenderedOp> {
        let tag = self
            .tag
            .or_else(|| key.tag.clone())
            .ok_or(DhtOpError::LinkKeyTagMissing)?;
        let header = Header::CreateLink(CreateLink {
            author: self.author,
            timestamp: self.timestamp,
            header_seq: self.header_seq,
            prev_header: self.prev_header,
            base_address: key.base.clone(),
            target_address: self.target_address,
            zome_id: self.zome_id,
            link_type: self.link_type,
            tag,
        });
        let signature = self.signature;
        let validation_status = Some(self.validation_status);
        RenderedOp::new(
            header,
            signature,
            validation_status,
            DhtOpType::RegisterAddLink,
        )
    }
}

impl WireDeleteLink {
    /// Condense down a delete link op for the wire.
    pub fn condense(
        h: DeleteLink,
        signature: Signature,
        validation_status: ValidationStatus,
    ) -> Self {
        Self {
            author: h.author,
            timestamp: h.timestamp,
            header_seq: h.header_seq,
            prev_header: h.prev_header,
            signature,
            validation_status,
            link_add_address: h.link_add_address,
        }
    }
    /// Render these ops to their full types.
    pub fn render(self, key: &WireLinkKey) -> DhtOpResult<RenderedOp> {
        let header = Header::DeleteLink(DeleteLink {
            author: self.author,
            timestamp: self.timestamp,
            header_seq: self.header_seq,
            prev_header: self.prev_header,
            base_address: key.base.clone(),
            link_add_address: self.link_add_address,
        });
        let signature = self.signature;
        let validation_status = Some(self.validation_status);
        RenderedOp::new(
            header,
            signature,
            validation_status,
            DhtOpType::RegisterRemoveLink,
        )
    }
}
// TODO: Probably don't want to send the whole headers.
// We could probably come up with a more compact
// network Wire type in the future
/// Link response to get links
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct GetLinksResponse {
    /// All the link adds on the key you searched for
    pub link_adds: Vec<(CreateLink, Signature)>,
    /// All the link removes on the key you searched for
    pub link_removes: Vec<(DeleteLink, Signature)>,
}

impl WireLinkMetaKey {
    /// Get the basis of this key
    pub fn basis(&self) -> AnyDhtHash {
        use WireLinkMetaKey::*;
        match self {
            Base(b) | BaseZome(b, _) | BaseZomeTag(b, _, _) | Full(b, _, _, _) => b.clone().into(),
        }
    }
}

impl Link {
    /// Construct a new link.
    pub fn new(base: &EntryHash, target: &EntryHash, tag: &LinkTag) -> Self {
        Link {
            base: base.to_owned(),
            target: target.to_owned(),
            tag: tag.to_owned(),
        }
    }

    /// Get the base address of this link.
    pub fn base(&self) -> &EntryHash {
        &self.base
    }

    /// Get the target address of this link.
    pub fn target(&self) -> &EntryHash {
        &self.target
    }

    /// Get the tag of this link.
    pub fn tag(&self) -> &LinkTag {
        &self.tag
    }
}

/// How do we match this link in queries?
pub enum LinkMatch<S: Into<String>> {
    /// Match all/any links.
    Any,

    /// Match exactly by string.
    Exactly(S),

    /// Match by regular expression.
    Regex(S),
}

impl<S: Into<String>> LinkMatch<S> {
    /// Build a regular expression string for this link match.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_regex_string(self) -> Result<String, String> {
        let re_string: String = match self {
            LinkMatch::Any => ".*".into(),
            LinkMatch::Exactly(s) => "^".to_owned() + &regex::escape(&s.into()) + "$",
            LinkMatch::Regex(s) => s.into(),
        };
        // check that it is a valid regex
        match Regex::new(&re_string) {
            Ok(_) => Ok(re_string),
            Err(_) => Err("Invalid regex passed to get_links".into()),
        }
    }
}
