use crate::prelude::*;

/// Libsodium keypair based authenticated encryption: box.
///
/// Libsodium asymmetric encryption (two keypairs to encrypt/decrypt) is called box.
/// Box can be used directly to hide data and is part of cryptographic systems such as
/// [saltpack](https://saltpack.org/).
///
/// Important information about box:
///  - The secret half of the keypair is generated in and remains in lair.
///  - The nonce is randomly generated in lair for every call to encrypt.
///  - The nonce is PUBLIC and UNIQUE, it must NEVER be re-used (currently can't be set directly).
///  - Box is the same encryption as secretbox using ECDH off the keypairs for the shared key.
///  - Box is repudible. Either keypair can create any message to be read by the other party. Each
///    party knows they did not create a certain message so they know it came from the counterpary
///    but neither can prove to a third party that any message wasn't forged. Note that if you want
///    the opposite it is not enough to simply layer signatures and encryption.
///    @see https://theworld.com/~dtd/sign_encrypt/sign_encrypt7.html
///  - To encrypt something potentially large for potentially many recipients efficiently it may be
///    worth chunking the large data, secret boxing it with a unique key for each chunk, then
///    boxing the _keys_ for each recipient alongside the chunks, to avoid encrypting the large
///    data repeatedly for every recipient.
///  - Box is NOT quantum resistant.
///
/// If you want to hide data:
///  - Consider using capability tokens and/or dedicated DHT networks to control access.
///  - Consider how the keypairs are being generated and pubkeys distributed.
///  - Consider that a hybrid approach between network access + encryption might be best.
///  - Consider that encrypted data cannot be validated effectively by the public DHT.
///
/// The main use-case is to control access to data that may be broadcast across a semi-trusted or
/// untrusted context, where the intended recipients have all negotiated or shared a key outside
/// that context.
///
/// If you want to encrypt content so that _any_ recipient with a shared secret can decrypt it
/// then see the libsodium `secretbox` algorithm or similar.
///
/// @see https://doc.libsodium.org/public-key_cryptography/authenticated_encryption
/// @see https://nacl.cr.yp.to/box.html
pub fn x_25519_x_salsa20_poly1305_encrypt(
    sender: X25519PubKey,
    recipient: X25519PubKey,
    data: XSalsa20Poly1305Data,
) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    host_call::<
        holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Encrypt,
        XSalsa20Poly1305EncryptedData,
    >(
        __x_25519_x_salsa20_poly1305_encrypt,
        holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Encrypt::new(
            sender, recipient, data,
        ),
    )
}
