use crate::prelude::*;

/// Generate a new x25519 keypair in lair from entropy.
/// Only the pubkey is returned from lair because the secret key never leaves lair.
/// @todo ability to export secrets from lair in encrypted format to send to other agents.
pub fn create_x25519_keypair() -> HdkResult<X25519PubKey> {
    host_externs!(__create_x25519_keypair);
    Ok(
        host_call::<CreateX25519KeypairInput, CreateX25519KeypairOutput>(
            __create_x25519_keypair,
            &().into(),
        )?
        .into_inner(),
    )
}
