use hdk::prelude::*;

#[hdk_extern]
fn x_salsa20_poly1305_shared_secret_create_random(input: Option<XSalsa20Poly1305KeyRef>) -> ExternResult<XSalsa20Poly1305KeyRef> {
    hdk::prelude::x_salsa20_poly1305_shared_secret_create_random(input)
}

#[hdk_extern]
fn x_salsa20_poly1305_shared_secret_export(input: XSalsa20Poly1305SharedSecretExport) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    hdk::prelude::x_salsa20_poly1305_shared_secret_export(
        input.as_sender_ref().to_owned(),
        input.as_recipient_ref().to_owned(),
        input.as_key_ref_ref().to_owned(),
    )
}

#[hdk_extern]
fn x_salsa20_poly1305_shared_secret_ingest(input: XSalsa20Poly1305SharedSecretIngest) -> ExternResult<XSalsa20Poly1305KeyRef> {
    hdk::prelude::x_salsa20_poly1305_shared_secret_ingest(
        input.as_recipient_ref().to_owned(),
        input.as_sender_ref().to_owned(),
        input.as_encrypted_data_ref().to_owned(),
        input.as_key_ref_ref().to_owned(),
    )
}

#[hdk_extern]
fn x_salsa20_poly1305_encrypt(input: XSalsa20Poly1305Encrypt) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    hdk::prelude::x_salsa20_poly1305_encrypt(
        input.as_key_ref_ref().to_owned(),
        input.as_data_ref().to_owned(),
    )
}

#[hdk_extern]
fn x_salsa20_poly1305_decrypt(input: XSalsa20Poly1305Decrypt) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    hdk::prelude::x_salsa20_poly1305_decrypt(
        input.as_key_ref_ref().to_owned(),
        input.as_encrypted_data_ref().to_owned()
    )
}

#[hdk_extern]
fn create_x25519_keypair(_: ()) -> ExternResult<X25519PubKey> {
    hdk::prelude::create_x25519_keypair()
}

#[hdk_extern]
fn x_25519_x_salsa20_poly1305_encrypt(input: X25519XSalsa20Poly1305Encrypt) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    hdk::prelude::x_25519_x_salsa20_poly1305_encrypt(
        input.as_sender_ref().to_owned(),
        input.as_recipient_ref().to_owned(),
        input.as_data_ref().to_owned()
    )
}

#[hdk_extern]
fn x_25519_x_salsa20_poly1305_decrypt(input: X25519XSalsa20Poly1305Decrypt) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    hdk::prelude::x_25519_x_salsa20_poly1305_decrypt(
        input.as_recipient_ref().to_owned(),
        input.as_sender_ref().to_owned(),
        input.as_encrypted_data_ref().to_owned()
    )
}
