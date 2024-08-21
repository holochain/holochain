use crate::KeyBytes;
use hdi::prelude::*;

// Represents an M:N multisignature spec.
// The trivial case 1:1 represents a single agent to sign.
// We need an entry to define the rules of authority
// (for authorizing or revoking) keys in the space under a KeysetRoot.
// This is only committed by the first Deepkey agent.
#[hdk_entry_helper]
#[derive(Clone)]
pub struct AuthoritySpec {
    // set to 1 for a single signer scenario
    pub sigs_required: u8,
    // These signers may not exist on the DHT.
    // E.g. a revocation key used to create the first change rule.
    pub authorized_signers: Vec<KeyBytes>,
}

impl AuthoritySpec {
    pub fn new(sigs_required: u8, authorized_signers: Vec<KeyBytes>) -> Self {
        Self {
            sigs_required,
            authorized_signers,
        }
    }
}
