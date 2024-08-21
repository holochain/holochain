use hdi::prelude::*;

use crate::{
    AuthorizedSpecChange,
};


// The author needs to be linked from the KeysetRoot
#[hdk_entry_helper]
#[derive(Clone)]
pub struct ChangeRule {
    pub keyset_root: ActionHash,
    pub spec_change: AuthorizedSpecChange,
}

impl ChangeRule {
    pub fn new(
        keyset_root: ActionHash,
        spec_change: AuthorizedSpecChange,
    ) -> Self {
        Self {
            keyset_root,
            spec_change,
        }
    }
}
