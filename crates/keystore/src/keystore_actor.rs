//! This module contains all the types needed to implement a keystore actor.
//! We will re-export the main KeystoreSender usable by clients at the lib.

use crate::*;

ghost_actor::ghost_chan! {
    /// A "Keystore" actor keeps private keys secure while allowing them to be
    /// used for signing, encryption, etc.
    pub chan KeystoreApi<KeystoreError> {
        /// Generates a new pure entropy keypair in the keystore, returning the public key.
        fn generate_sign_keypair_from_pure_entropy() -> holo_hash::AgentPubKey;

        /// List all the signature public keys this keystore is tracking.
        fn list_sign_keys() -> Vec<holo_hash::AgentPubKey>;

        /// Generate a signature for a given blob of binary data.
        fn sign(input: SignInput) -> Signature;
    }
}

/// GhostSender type for the KeystoreApi
pub type KeystoreSender = ghost_actor::GhostSender<KeystoreApi>;
