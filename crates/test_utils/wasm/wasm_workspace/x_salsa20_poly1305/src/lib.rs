use hdk3::prelude::*;

#[hdk_extern]
fn x_salsa20_poly1305_encrypt(input: XSalsa20Poly1305EncryptInput) -> ExternResult<XSalsa20Poly1305EncryptOutput> {
    let encrypt = input.into_inner();
    Ok(XSalsa20Poly1305EncryptOutput::new(hdk3::prelude::x_salsa20_poly1305_encrypt(encrypt.as_key_ref_ref().to_owned(), encrypt.as_data_ref().to_owned())?))
}

#[hdk_extern]
fn x_salsa20_poly1305_decrypt(input: XSalsa20Poly1305DecryptInput) -> ExternResult<XSalsa20Poly1305DecryptOutput> {
    let decrypt = input.into_inner();
    Ok(XSalsa20Poly1305DecryptOutput::new(hdk3::prelude::x_salsa20_poly1305_decrypt(decrypt.as_key_ref_ref().to_owned(), decrypt.as_encrypted_data_ref().to_owned())?))
}

#[hdk_extern]
fn create_x25519_keypair(_: ()) -> ExternResult<X25519PubKey> {
    Ok(hdk3::prelude::create_x25519_keypair()?)
}

#[hdk_extern]
fn x_25519_x_salsa20_poly1305_encrypt(input: X25519XSalsa20Poly1305EncryptInput) -> ExternResult<X25519XSalsa20Poly1305EncryptOutput> {
    let encrypt = input.into_inner();
    Ok(X25519XSalsa20Poly1305EncryptOutput::new(hdk3::prelude::x_25519_x_salsa20_poly1305_encrypt(encrypt.as_sender_ref().to_owned(), encrypt.as_recipient_ref().to_owned(), encrypt.as_data_ref().to_owned())?))
}

#[hdk_extern]
fn x_25519_x_salsa20_poly1305_decrypt(input: X25519XSalsa20Poly1305DecryptInput) -> ExternResult<X25519XSalsa20Poly1305DecryptOutput> {
    let decrypt = input.into_inner();
    Ok(X25519XSalsa20Poly1305DecryptOutput::new(hdk3::prelude::x_25519_x_salsa20_poly1305_decrypt(decrypt.as_recipient_ref().to_owned(), decrypt.as_sender_ref().to_owned(), decrypt.as_encrypted_data_ref().to_owned())?))
}
