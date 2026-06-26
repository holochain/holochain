//! Defines a Record, the basic unit of Holochain data.

use crate::signature::Signed;
use crate::Action;
use holo_hash::hash_type;
use holo_hash::HashableContent;
use holo_hash::HashableContentBytes;
pub use holochain_integrity_types::record::*;

/// A combination of an action and its signature.
///
/// Has implementations From and Into its tuple form.
pub type SignedAction = Signed<Action>;

impl SignedAction {
    /// Accessor for the Action
    pub fn action(&self) -> &Action {
        self
    }
}

impl HashableContent for SignedAction {
    type HashType = hash_type::Action;

    fn hash_type(&self) -> Self::HashType {
        hash_type::Action
    }

    fn hashable_content(&self) -> HashableContentBytes {
        // A `SignedAction` must hash to the same canonical `ActionHash` as its
        // inner `Action`. #5822 flipped `Action`'s hash to the content-derived
        // v2 projection but missed this impl, which used to serialize the legacy
        // action bytes — so hashing a bare `SignedAction` produced a hash that
        // disagreed with `Action`, `SignedActionHashed`, and every stored
        // `action_hash`. Delegate to the inner `Action` to keep the identity.
        self.action().hashable_content()
    }
}

impl From<SignedActionHashed> for SignedAction {
    fn from(shh: SignedActionHashed) -> SignedAction {
        (shh.hashed.content, shh.signature).into()
    }
}
