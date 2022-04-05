use crate::prelude::*;

/// Libsodium secret-key authenticated encryption: secretbox_open
///
/// Opens encrypted data created by secretbox.
///
/// If the encrypted data fails authentication and cannot be decrypted this function returns None.
///
/// This means that if any decrypted data is returned by this function it was created by a holder
/// of the shared key and has not been tampered with.
///
/// See [aeads](https://www.imperialviolet.org/2015/05/16/aeads.html)
pub fn x_salsa20_poly1305_decrypt(
    key_ref: XSalsa20Poly1305KeyRef,
    encrypted_data: XSalsa20Poly1305EncryptedData,
) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    IDK.with(|h| {
        h.borrow()
            .x_salsa20_poly1305_decrypt(XSalsa20Poly1305Decrypt::new(key_ref, encrypted_data))
    })
}

/// Libsodium keypair based authenticated encryption: box_open
///
/// Opens encrypted data created by box.
///
/// If the encrypted data fails authentication and cannot be decrypted this function returns [ `None` ].
///
/// This means that if any decrypted data is returned by this function it was created by _either_
/// keypair and has not been tampered with.
///
/// See https://www.imperialviolet.org/2015/05/16/aeads.html
pub fn x_25519_x_salsa20_poly1305_decrypt(
    recipient: X25519PubKey,
    sender: X25519PubKey,
    encrypted_data: XSalsa20Poly1305EncryptedData,
) -> ExternResult<Option<XSalsa20Poly1305Data>> {
    IDK.with(|h| {
        h.borrow()
            .x_25519_x_salsa20_poly1305_decrypt(X25519XSalsa20Poly1305Decrypt::new(
                recipient,
                sender,
                encrypted_data,
            ))
    })
}
