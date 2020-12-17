use crate::prelude::*;

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
