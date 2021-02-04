use hdk3::prelude::*;

#[hdk_extern]
fn x_salsa20_poly1305_encrypt(input: XSalsa20Poly1305Encrypt) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    hdk3::prelude::x_salsa20_poly1305_encrypt(
        input.as_key_ref_ref().to_owned(),
        input.as_data_ref().to_owned(),
    )
}

#[hdk_extern]
fn x_salsa20_poly1305_decrypt(input: XSalsa20Poly1305Decrypt) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    hdk3::prelude::x_salsa20_poly1305_decrypt(
        input.as_key_ref_ref().to_owned(),
        input.as_encrypted_data_ref().to_owned()
    )
}

#[hdk_extern]
fn create_x25519_keypair(_: ()) -> ExternResult<X25519PubKey> {
    hdk3::prelude::create_x25519_keypair()
}

#[hdk_extern]
fn x_25519_x_salsa20_poly1305_encrypt(input: X25519XSalsa20Poly1305Encrypt) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    hdk3::prelude::x_25519_x_salsa20_poly1305_encrypt(
        input.as_sender_ref().to_owned(),
        input.as_recipient_ref().to_owned(),
        input.as_data_ref().to_owned()
    )
}

#[hdk_extern]
fn x_25519_x_salsa20_poly1305_decrypt(input: X25519XSalsa20Poly1305Decrypt) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    hdk3::prelude::x_25519_x_salsa20_poly1305_decrypt(
        input.as_recipient_ref().to_owned(),
        input.as_sender_ref().to_owned(),
        input.as_encrypted_data_ref().to_owned()
    )
}
