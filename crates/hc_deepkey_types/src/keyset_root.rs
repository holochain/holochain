use crate::KeyBytes;
use hdi::prelude::*;

pub const KEYSET_ROOT_INDEX: u32 = POST_GENESIS_SEQ_THRESHOLD + 1;


#[hdk_entry_helper]
#[derive(Clone)]
pub struct KeysetRoot {
    pub first_deepkey_agent: AgentPubKey,
    /// The private key is thrown away.
    pub root_pub_key: KeyBytes,
    pub signed_fda: Signature,
}

impl KeysetRoot {
    pub fn new(
        first_deepkey_agent: AgentPubKey,
        root_pub_key: KeyBytes,
        signed_fda: Signature,
    ) -> Self {
        Self {
            first_deepkey_agent,
            root_pub_key,
            signed_fda,
        }
    }

    pub fn root_pub_key_as_agent(&self) -> AgentPubKey {
        holo_hash::AgentPubKey::from_raw_32( self.root_pub_key.to_vec() )
    }
}
