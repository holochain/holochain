//! Links interrelate entries in a source chain.

use crate::error::DhtOpResult;
use crate::wire_ops::RenderedOp;
use crate::wire_ops::RenderedOps;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyLinkableHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::prelude::*;
use holochain_zome_types::warrant::SignedWarrant;
use regex::Regex;

/// Link key for sending across the wire for get links requests.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct WireLinkKey {
    /// Base the links are on.
    pub base: AnyLinkableHash,
    /// The zome the links are in.
    pub type_query: LinkTypeFilter,
    /// Optionally specify a tag for more specific queries.
    pub tag: Option<LinkTag>,
    /// Specify a minimum action timestamp to filter results.
    pub after: Option<Timestamp>,
    /// Specify a maximum action timestamp to filter results.
    pub before: Option<Timestamp>,
    /// Only get links created by this author.
    pub author: Option<AgentPubKey>,
}

/// The record-serving response to a get-links request.
///
/// Serves the create-link and delete-link actions matching the query, each
/// carrying its record-level validation status. A `Rejected` action is always
/// accompanied by a warrant in `warrants` proving the rejection; the receiver
/// checks that invariant up front.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes, Default)]
pub struct WireLinkOps {
    /// Create-link actions that match this query, each with its status.
    pub creates: Vec<Judged<SignedAction>>,
    /// Delete-link actions that match this query, each with its status.
    pub deletes: Vec<Judged<SignedAction>>,
    /// Warrants proving any `Rejected` records served above.
    pub warrants: Vec<SignedWarrant>,
}

impl WireLinkOps {
    /// Create an empty wire response.
    pub fn new() -> Self {
        Default::default()
    }
    /// Expand the served records into the request-relevant ops for caching.
    ///
    /// Each served action becomes the single op the get-links request
    /// represents (`CreateLink` per create, `DeleteLink` per
    /// delete), tagged with the served validation status. Warrants are handled
    /// separately by the requester.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            creates,
            deletes,
            warrants: _,
        } = self;
        let mut ops = Vec::with_capacity(creates.len() + deletes.len());
        for op in creates {
            let status = op.validation_status();
            let (action, signature) = op.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::CreateLink,
            )?);
        }
        for op in deletes {
            let status = op.validation_status();
            let (action, signature) = op.data.into();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                ChainOpType::DeleteLink,
            )?);
        }
        Ok(RenderedOps {
            ops,
            ..Default::default()
        })
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

/// Query for links to be sent over the network.
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes, PartialEq, Clone, Debug)]
pub struct WireLinkQuery {
    /// The base to find links from.
    pub base: AnyLinkableHash,

    /// Filter by the link type.
    pub link_type: LinkTypeFilter,

    /// Filter by tag prefix.
    pub tag_prefix: Option<LinkTag>,

    /// Only include links created before this time.
    pub before: Option<Timestamp>,

    /// Only include links created after this time.
    pub after: Option<Timestamp>,

    /// Only include links created by this author.
    pub author: Option<AgentPubKey>,
}

/// Response type for a `WireLinkQuery`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct CountLinksResponse(Vec<ActionHash>);

impl CountLinksResponse {
    /// Create a new response from the action hashes of the matched links
    pub fn new(create_link_actions: Vec<ActionHash>) -> Self {
        CountLinksResponse(create_link_actions)
    }

    /// Get the action hashes of the matched links
    pub fn create_link_actions(&self) -> Vec<ActionHash> {
        self.0.clone()
    }
}
