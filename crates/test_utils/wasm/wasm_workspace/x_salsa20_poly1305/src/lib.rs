use hdk3::prelude::*;

#[hdk_extern]
fn key(_: ()) -> ExternResult<SecretBoxKey> {
    SecretBoxKey::try_from_random()
}

#[hdk_extern]
fn nonce(_: ()) -> ExternResult<SecretBoxNonce> {
    SecretBoxNonce::try_from_random()
}

#[hdk_extern]
fn x_salsa20_poly1305_encrypt(input: XSalsa20Poly1305EncryptInput) -> ExternResult<XSalsa20Poly1305EncryptOutput> {
    let ( key, nonce, data ) = input.into_inner();
    Ok(XSalsa20Poly1305EncryptOutput::new(hdk3::prelude::x_salsa20_poly1305_encrypt(key, nonce, data)?))
}

#[hdk_extern]
fn x_salsa20_poly1305_decrypt(input: XSalsa20Poly1305DecryptInput) -> ExternResult<XSalsa20Poly1305DecryptOutput> {
    let ( key, nonce, encrypted_data ) = input.into_inner();
    Ok(XSalsa20Poly1305DecryptOutput::new(hdk3::prelude::x_salsa20_poly1305_decrypt(key, nonce, encrypted_data)?))
}
