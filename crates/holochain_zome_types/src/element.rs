//! Defines a Element, the basic unit of Holochain data.

use crate::signature::Signature;
use crate::Action;
use holo_hash::hash_type;
use holo_hash::HashableContent;
use holo_hash::HashableContentBytes;
use holochain_serialized_bytes::prelude::*;

pub use holochain_integrity_types::element::*;

#[cfg(feature = "test_utils")]
pub mod facts;

/// A combination of a Action and its signature.
///
/// Has implementations From and Into its tuple form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SignedAction(pub Action, pub Signature);

impl SignedAction {
    /// Accessor for the Action
    pub fn action(&self) -> &Action {
        &self.0
    }

    /// Accessor for the Signature
    pub fn signature(&self) -> &Signature {
        &self.1
    }
}

impl HashableContent for SignedAction {
    type HashType = hash_type::Action;

    fn hash_type(&self) -> Self::HashType {
        hash_type::Action
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            (&self.0)
                .try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

impl From<(Action, Signature)> for SignedAction {
    fn from((h, s): (Action, Signature)) -> Self {
        Self(h, s)
    }
}

impl From<SignedAction> for (Action, Signature) {
    fn from(s: SignedAction) -> Self {
        (s.0, s.1)
    }
}

impl From<SignedActionHashed> for SignedAction {
    fn from(shh: SignedActionHashed) -> SignedAction {
        (shh.hashed.content, shh.signature).into()
    }
}
