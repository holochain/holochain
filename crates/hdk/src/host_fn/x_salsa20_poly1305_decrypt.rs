use crate::prelude::*;

/// Libsodium secret-key authenticated encryption: secretbox_open
///
/// Opens encrypted data created by secretbox.
///
/// If the encrypted data fails authentication and cannot be decrypted this function returns None.
///
/// This means that if any decrypted data is returned by this function it is not only created by
/// a holder of the shared key but it also has not been tampered with.
///
/// @see https://www.imperialviolet.org/2015/05/16/aeads.html
pub fn x_salsa20_poly1305_decrypt(
    key: XSalsa20Poly1305Key,
    nonce: XSalsa20Poly1305Nonce,
    encrypted_data: XSalsa20Poly1305EncryptedData,
) -> HdkResult<Option<XSalsa20Poly1305Data>> {
    host_externs!(__x_salsa20_poly1305_decrypt);
    Ok(
        host_call::<XSalsa20Poly1305DecryptInput, XSalsa20Poly1305DecryptOutput>(
            __x_salsa20_poly1305_decrypt,
            &XSalsa20Poly1305DecryptInput::new((key, nonce, encrypted_data)),
        )?
        .into_inner(),
    )
}

pub fn secretbox_open(
    key: SecretBoxKey,
    nonce: SecretBoxNonce,
    encrypted_data: SecretBoxEncryptedData,
) -> HdkResult<Option<SecretBoxData>> {
    x_salsa20_poly1305_decrypt(key, nonce, encrypted_data)
}
