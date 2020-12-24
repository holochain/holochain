use crate::prelude::*;

/// Libsodium keypair based authenticated encryption: box_open
///
/// Opens encrypted data created by box.
///
/// If the encrypted data fails authentication and cannot be decrypted this function returns None.
///
/// This means that if any decrypted data is returned by this function it was created by _either_
/// keypair and has not been tampered with.
///
/// @see https://www.imperialviolet.org/2015/05/16/aeads.html
pub fn x_25519_x_salsa20_poly1305_decrypt(
    recipient: X25519PubKey,
    sender: X25519PubKey,
    encrypted_data: XSalsa20Poly1305EncryptedData,
) -> HdkResult<Option<XSalsa20Poly1305Data>> {
    host_externs!(__x_25519_x_salsa20_poly1305_decrypt);
    Ok(
        host_call::<X25519XSalsa20Poly1305DecryptInput, X25519XSalsa20Poly1305DecryptOutput>(
            __x_25519_x_salsa20_poly1305_decrypt,
            &X25519XSalsa20Poly1305DecryptInput::new(
                holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt::new(
                    recipient,
                    sender,
                    encrypted_data,
                ),
            ),
        )?
        .into_inner(),
    )
}
