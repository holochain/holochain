//! This module contains all the types needed to implement a keystore actor.
//! We will re-export the main KeystoreSender usable by clients at the lib.

use crate::*;

ghost_actor::ghost_actor! {
    Visibility(pub),
    Name(Keystore),
    Error(KeystoreError),
    Api {
        GenerateSignKeypairFromPureEntropy(
            "generates a new pure entropy keypair in the keystore, returning the public key",
            (),
            holo_hash::AgentPubKey,
        ),
        ListSignKeys(
            "list all the signature public keys this keystore is tracking",
            (),
            Vec<holo_hash::AgentPubKey>,
        ),
        Sign(
            "generate a signature for a given blob of binary data",
            SignInput,
            Signature,
        ),
    }
}
