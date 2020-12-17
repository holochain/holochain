use hdk3::prelude::*;

#[hdk_extern]
fn key(_: ()) -> ExternResult<SecretBoxKeyRef> {
    SecretBoxKeyRef::try_from_random()
}

#[hdk_extern]
fn nonce(_: ()) -> ExternResult<SecretBoxNonce> {
    SecretBoxNonce::try_from_random()
}

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
