//! This module contains all the types needed to implement a keystore actor.
//! We will re-export the main KeystoreSender usable by clients at the lib.

use crate::*;
use ghost_actor::dependencies::futures::future::FutureExt;
use holo_hash::{HOLO_HASH_CORE_LEN, HOLO_HASH_PREFIX_LEN};
use holochain_zome_types::signature::{Sign, Signature};
use holochain_zome_types::x_salsa20_poly1305::{
    X25519XSalsa20Poly1305Decrypt, X25519XSalsa20Poly1305Encrypt,
};
use lair_keystore_api::actor::{
    Cert, CertDigest, CertPrivKey, LairClientApiSender, LairEntryType, TlsCertOptions,
};

/// GhostSender type for the KeystoreApi
pub type KeystoreSender = ghost_actor::GhostSender<lair_keystore_api::actor::LairClientApi>;

/// Result type for legacy API calls.
pub type KeystoreApiResult<T> = Result<T, KeystoreError>;

/// Future type for legacy API calls.
pub type KeystoreApiFuture<T> =
    ghost_actor::dependencies::must_future::MustBoxFuture<'static, KeystoreApiResult<T>>;

/// Some legacy APIs to make refactor easier.
pub trait KeystoreSenderExt {
    /// Generates a new pure entropy keypair in the keystore, returning the public key.
    fn generate_sign_keypair_from_pure_entropy(&self) -> KeystoreApiFuture<holo_hash::AgentPubKey>;

    /// Generate a signature for a given blob of binary data.
    fn sign(&self, input: Sign) -> KeystoreApiFuture<Signature>;

    /// If we have a TLS cert in lair - return the first one
    /// Errors if no certs in lair
    fn get_first_tls_cert(&self) -> KeystoreApiFuture<(CertDigest, Cert, CertPrivKey)>;

    /// If we have a TLS cert in lair - return the first one
    /// otherwise, generate a TLS cert and return it
    fn get_or_create_first_tls_cert(&self) -> KeystoreApiFuture<(CertDigest, Cert, CertPrivKey)>;

    /// Generate a new x25519 keypair in lair and get the pubkey back for general usage.
    fn create_x25519_keypair(
        &self,
    ) -> KeystoreApiFuture<holochain_zome_types::x_salsa20_poly1305::x25519::X25519PubKey>;

    /// If we have an X25519 pub key in lair use it to ECDH negotiate a shared key and then
    /// Salsa20Poly1305 encrypt the data with that and a random nonce.
    /// a.k.a. libsodium crypto_box()
    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        input: X25519XSalsa20Poly1305Encrypt,
    ) -> KeystoreApiFuture<
        holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData,
    >;
    /// The inverse of x_25519_x_salsa20_poly1305_encrypt.
    /// Returns None if decryption fails.
    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        input: X25519XSalsa20Poly1305Decrypt,
    ) -> KeystoreApiFuture<
        Option<holochain_zome_types::x_salsa20_poly1305::data::XSalsa20Poly1305Data>,
    >;
}

impl KeystoreSenderExt for KeystoreSender {
    fn generate_sign_keypair_from_pure_entropy(&self) -> KeystoreApiFuture<holo_hash::AgentPubKey> {
        let fut = self.sign_ed25519_new_from_entropy();
        async move {
            let (_, pk) = fut.await?;
            Ok(holo_hash::AgentPubKey::from_raw_32(pk.to_vec()))
        }
        .boxed()
        .into()
    }

    fn sign(&self, input: Sign) -> KeystoreApiFuture<Signature> {
        let span = tracing::debug_span!("signing request", input);
        let span_enter = span.enter();
        let fut = self.sign_ed25519_sign_by_pub_key(
            input.key.as_ref()[HOLO_HASH_PREFIX_LEN..HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN]
                .to_vec()
                .into(),
            <Vec<u8>>::from(UnsafeBytes::from(input.data.to_vec())).into(),
        );
        async move {
            let res = fut.await?;
            drop(span_enter);
            Ok(Signature::try_from(res.to_vec().as_ref())?)
        }
        .boxed()
        .into()
    }

    fn get_first_tls_cert(&self) -> KeystoreApiFuture<(CertDigest, Cert, CertPrivKey)> {
        let this = self.clone();
        async move {
            let last_index = this.lair_get_last_entry_index().await?;
            for i in 1..=*last_index {
                if let Ok(LairEntryType::TlsCert) = this.lair_get_entry_type(i.into()).await {
                    let (_, digest) = this.tls_cert_get(i.into()).await?;
                    let cert = this.tls_cert_get_cert_by_index(i.into()).await?;
                    let cert_priv = this.tls_cert_get_priv_key_by_index(i.into()).await?;
                    return Ok((digest, cert, cert_priv));
                }
            }
            Err("no tls cert registered".into())
        }
        .boxed()
        .into()
    }

    fn get_or_create_first_tls_cert(&self) -> KeystoreApiFuture<(CertDigest, Cert, CertPrivKey)> {
        let this = self.clone();
        async move {
            if let Ok(r) = this.get_first_tls_cert().await {
                return Ok(r);
            }

            let mut tls_opt = TlsCertOptions::default();
            tls_opt.alg = lair_keystore_api::actor::TlsCertAlg::PkcsEcdsaP256Sha256;
            let _ = this.tls_cert_new_self_signed_from_entropy(tls_opt).await?;

            this.get_first_tls_cert().await
        }
        .boxed()
        .into()
    }

    fn create_x25519_keypair(
        &self,
    ) -> KeystoreApiFuture<holochain_zome_types::x_salsa20_poly1305::x25519::X25519PubKey> {
        let this = self.clone();
        async move {
            let fut = this.x25519_new_from_entropy();
            let (_, pubkey) = fut.await?;
            Ok(AsRef::<[u8]>::as_ref(&pubkey).try_into()?)
        }
        .boxed()
        .into()
    }

    fn x_25519_x_salsa20_poly1305_encrypt(
        &self,
        input: X25519XSalsa20Poly1305Encrypt,
    ) -> KeystoreApiFuture<
        holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData,
    > {
        let this = self.clone();
        async move {
            let fut = this.crypto_box_by_pub_key(
                input.as_sender_ref().as_ref()
                    .try_into()?,
                input.as_recipient_ref().as_ref()
                    .try_into()?,
                std::sync::Arc::new(lair_keystore_api::internal::crypto_box::CryptoBoxData {
                    data: std::sync::Arc::new(input.as_data_ref().as_ref().to_owned())
                })
            );
            let res = fut.await?;
            Ok(holochain_zome_types::x_salsa20_poly1305::encrypted_data::XSalsa20Poly1305EncryptedData::new(
                AsRef::<[u8]>::as_ref(&res.nonce).try_into()?,
                res.encrypted_data.to_vec(),
            ))
        }
        .boxed()
        .into()
    }

    fn x_25519_x_salsa20_poly1305_decrypt(
        &self,
        input: X25519XSalsa20Poly1305Decrypt,
    ) -> KeystoreApiFuture<
        Option<holochain_zome_types::x_salsa20_poly1305::data::XSalsa20Poly1305Data>,
    > {
        let this = self.clone();
        async move {
            let fut = this.crypto_box_open_by_pub_key(
                input.as_recipient_ref().as_ref().try_into()?,
                input.as_sender_ref().as_ref().try_into()?,
                std::sync::Arc::new(
                    lair_keystore_api::internal::crypto_box::CryptoBoxEncryptedData {
                        nonce: AsRef::<[u8]>::as_ref(&input.as_encrypted_data_ref().as_nonce_ref())
                            .try_into()?,
                        encrypted_data: std::sync::Arc::new(
                            input
                                .as_encrypted_data_ref()
                                .as_encrypted_data_ref()
                                .to_vec(),
                        ),
                    },
                ),
            );
            let res = fut.await?;
            Ok(res.map(|data| {
                holochain_zome_types::x_salsa20_poly1305::data::XSalsa20Poly1305Data::from(
                    data.data.to_vec(),
                )
            }))
        }
        .boxed()
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_keystore::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tls_cert_get_or_create() {
        let keystore = spawn_test_keystore().await.unwrap();
        let (dig1, cert1, priv1) = keystore.get_or_create_first_tls_cert().await.unwrap();
        let (dig2, cert2, priv2) = keystore.get_or_create_first_tls_cert().await.unwrap();
        assert_eq!(dig1, dig2);
        assert_eq!(cert1, cert2);
        assert_eq!(priv1, priv2);
    }
}
