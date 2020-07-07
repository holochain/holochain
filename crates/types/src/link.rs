//! Links interrelate entries in a source chain.

use crate::composite_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use regex::Regex;
use shrinkwraprs::Shrinkwrap;

/// Opaque tag for the link applied at the app layer, used to differentiate
/// between different semantics and validation rules for different links
#[derive(
    Shrinkwrap,
    Debug,
    Clone,
    Hash,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
    derive_more::From,
)]
#[shrinkwrap(mutable)]
pub struct LinkTag(pub Vec<u8>);

/// Links interrelate entries in a source chain.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, SerializedBytes)]
pub struct Link {
    base: EntryHash,
    target: EntryHash,
    tag: LinkTag,
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

impl LinkTag {
    /// New tag from bytes
    pub fn new<T>(t: T) -> Self
    where
        T: Into<Vec<u8>>,
    {
        Self(t.into())
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
