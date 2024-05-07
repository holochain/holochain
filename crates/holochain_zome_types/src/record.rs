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
        &*self
    }
}

impl HashableContent for SignedAction {
    type HashType = hash_type::Action;

    fn hash_type(&self) -> Self::HashType {
        hash_type::Action
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            self.action()
                .try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

impl From<SignedActionHashed> for SignedAction {
    fn from(shh: SignedActionHashed) -> SignedAction {
        (shh.hashed.content, shh.signature).into()
    }
}
