use crate::prelude::*;
pub use hdi::x_salsa20_poly1305::*;

// -- secretbox encryption -- //

/// Generate a new secure random shared secret suitable for encrypting and
/// decrypting using x_salsa20_poly1305_{en,de}crypt.
/// If key_ref is `None` an opaque reference will be auto-generated.
/// If key_ref is `Some` and that key already exists in the store,
/// this function will return an error.
/// If `Ok`, this function will return the KeyRef by which the shared
/// secret may be accessed.
pub fn x_salsa20_poly1305_shared_secret_create_random(
    key_ref: Option<XSalsa20Poly1305KeyRef>,
) -> ExternResult<XSalsa20Poly1305KeyRef> {
    HDK.with(|h| {
        h.borrow()
            .x_salsa20_poly1305_shared_secret_create_random(key_ref)
    })
}

/// Using the Libsodium box algorithm, encrypt a shared secret so that it
/// may be forwarded to another specific peer.
pub fn x_salsa20_poly1305_shared_secret_export(
    sender: X25519PubKey,
    recipient: X25519PubKey,
    key_ref: XSalsa20Poly1305KeyRef,
) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    HDK.with(|h| {
        h.borrow()
            .x_salsa20_poly1305_shared_secret_export(XSalsa20Poly1305SharedSecretExport::new(
                sender, recipient, key_ref,
            ))
    })
}

/// Using the Libsodium box algorithm, decrypt a shared secret, storing it
/// in the keystore so that it may be used in `x_salsa20_poly1305_decrypt`.
/// This method may be co-opted to ingest shared secrets generated by other
/// custom means. Just be careful, as WASM memory is not a very secure
/// environment for cryptographic secrets.
/// If key_ref is `None` an opaque reference string will be auto-generated.
/// If key_ref is `Some` and that key already exists in the store,
/// this function will return an error.
/// If `Ok`, this function will return the KeyRef by which the shared
/// secret may be accessed.
pub fn x_salsa20_poly1305_shared_secret_ingest(
    recipient: X25519PubKey,
    sender: X25519PubKey,
    encrypted_data: XSalsa20Poly1305EncryptedData,
    key_ref: Option<XSalsa20Poly1305KeyRef>,
) -> ExternResult<XSalsa20Poly1305KeyRef> {
    HDK.with(|h| {
        h.borrow()
            .x_salsa20_poly1305_shared_secret_ingest(XSalsa20Poly1305SharedSecretIngest::new(
                recipient,
                sender,
                encrypted_data,
                key_ref,
            ))
    })
}

/// Libsodium secret-key authenticated encryption: secretbox.
///
/// Libsodium symmetric encryption (a shared key to encrypt/decrypt) is called secretbox.
/// Secretbox can be used directly to hide data and is part of cryptographic systems such as
/// [saltpack](https://saltpack.org/).
///
/// Important information about secretbox:
///  - Wasm memory is NOT secure, a compromised host can steal the key.
///  - The key is SECRET, anyone with the key and nonce can read the encrypted message.
///  - The nonce is PUBLIC and UNIQUE, it must NEVER be re-used (so we don't allow it to be set).
///  - Secretbox is designed for 'small' data, break large data into chunks with unique nonces.
///  - Secretbox is NOT quantum resistant.
///
/// @todo shift all the secret handling into lair so that we only work with opaque key references.
///
/// If you want to hide data:
///  - Consider using capability tokens and/or dedicated DHT networks to control access.
///  - Consider how the shared key is being distributed, e.g. maybe use a key exchange protocol.
///  - Consider that a hybrid approach between network access + encryption might be best.
///  - Consider that encrypted data cannot be validated effectively by the public DHT.
///
/// The main use-case is to control access to data that may be broadcast across a semi-trusted or
/// untrusted context, where the intended recipients have all negotiated or shared a key outside
/// that context.
///
/// If you want to encrypt content so that a _specific_ recipient (i.e. public key) can decrypt it
/// then see the libsodium `box` algorithm or similar.
///
/// See <https://doc.libsodium.org/secret-key_cryptography/secretbox>
/// See <https://nacl.cr.yp.to/secretbox.html>
pub fn x_salsa20_poly1305_encrypt(
    key_ref: XSalsa20Poly1305KeyRef,
    data: XSalsa20Poly1305Data,
) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    HDK.with(|h| {
        h.borrow()
            .x_salsa20_poly1305_encrypt(XSalsa20Poly1305Encrypt::new(key_ref, data))
    })
}

// -- curve25519 box encryption -- //

/// Generate a new x25519 keypair in lair from entropy.
/// Only the pubkey is returned from lair because the secret key never leaves lair.
pub fn create_x25519_keypair() -> ExternResult<X25519PubKey> {
    HDK.with(|h| h.borrow().create_x25519_keypair(()))
}

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
///    See <https://theworld.com/~dtd/sign_encrypt/sign_encrypt7.html>
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
/// See <https://doc.libsodium.org/public-key_cryptography/authenticated_encryption>
/// See <https://nacl.cr.yp.to/box.html>
pub fn x_25519_x_salsa20_poly1305_encrypt(
    sender: X25519PubKey,
    recipient: X25519PubKey,
    data: XSalsa20Poly1305Data,
) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    HDK.with(|h| {
        h.borrow()
            .x_25519_x_salsa20_poly1305_encrypt(X25519XSalsa20Poly1305Encrypt::new(
                sender, recipient, data,
            ))
    })
}

/// Libsodium crypto_box encryption, but converts ed25519 *signing*
/// keys into x25519 encryption keys.
/// WARNING: Please first understand the downsides of using this function:
/// <https://doc.libsodium.org/advanced/ed25519-curve25519>
pub fn ed_25519_x_salsa20_poly1305_encrypt(
    sender: AgentPubKey,
    recipient: AgentPubKey,
    data: XSalsa20Poly1305Data,
) -> ExternResult<XSalsa20Poly1305EncryptedData> {
    HDK.with(|h| {
        h.borrow()
            .ed_25519_x_salsa20_poly1305_encrypt(Ed25519XSalsa20Poly1305Encrypt::new(
                sender, recipient, data,
            ))
    })
}
