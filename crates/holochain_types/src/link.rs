//! Links interrelate entries in a source chain.

use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;
use regex::Regex;

/// Links interrelate entries in a source chain.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, SerializedBytes)]
pub struct Link {
    base: EntryHash,
    target: EntryHash,
    tag: LinkTag,
}

/// Owned link key for sending across networks
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
