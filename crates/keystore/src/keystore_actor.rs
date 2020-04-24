//! This module contains all the types needed to implement a keystore actor.
//! We will re-export the main KeystoreSender usable by clients at the lib.

use crate::*;

ghost_actor::ghost_actor! {
    name: pub Keystore,
    error: KeystoreError,
    api: {
        GenerateSignKeypairFromPureEntropy::generate_sign_keypair_from_pure_entropy (
            "generates a new pure entropy keypair in the keystore, returning the public key",
            (),
            holo_hash::AgentHash
        ),
        ListSignKeys::list_sign_keys (
            "list all the signature public keys this keystore is tracking",
            (),
            Vec<holo_hash::AgentHash>
        ),
        Sign::sign (
            "generate a signature for a given blob of binary data",
            SignInput,
            Signature
        ),
    }
}
