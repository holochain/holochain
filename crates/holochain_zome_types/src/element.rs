//! Defines a Element, the basic unit of Holochain data.

use crate::signature::Signature;
use crate::Header;
use holo_hash::hash_type;
use holo_hash::HashableContent;
use holo_hash::HashableContentBytes;
use holochain_serialized_bytes::prelude::*;

pub use holochain_integrity_types::element::*;

#[cfg(feature = "test_utils")]
pub mod facts;

/// A combination of a Header and its signature.
///
/// Has implementations From and Into its tuple form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SignedHeader(pub Header, pub Signature);

impl SignedHeader {
    /// Accessor for the Header
    pub fn header(&self) -> &Header {
        &self.0
    }

    /// Accessor for the Signature
    pub fn signature(&self) -> &Signature {
        &self.1
    }
}

impl HashableContent for SignedHeader {
    type HashType = hash_type::Header;

    fn hash_type(&self) -> Self::HashType {
        hash_type::Header
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            (&self.0)
                .try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

impl From<(Header, Signature)> for SignedHeader {
    fn from((h, s): (Header, Signature)) -> Self {
        Self(h, s)
    }
}

impl From<SignedHeader> for (Header, Signature) {
    fn from(s: SignedHeader) -> Self {
        (s.0, s.1)
    }
}

impl From<SignedHeaderHashed> for SignedHeader {
    fn from(shh: SignedHeaderHashed) -> SignedHeader {
        (shh.hashed.content, shh.signature).into()
    }
}
