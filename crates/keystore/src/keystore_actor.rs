//! This module contains all the types needed to implement a keystore actor.
//! We will re-export the main KeystoreSender usable by clients at the lib.

use crate::*;
use ghost_actor::dependencies::futures::future::FutureExt;
use holo_hash::{HOLO_HASH_CORE_LEN, HOLO_HASH_PREFIX_LEN};
use holochain_zome_types::signature::{Sign, Signature};
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
        let fut = self.sign_ed25519_sign_by_pub_key(
            input.key.as_ref()[HOLO_HASH_PREFIX_LEN..HOLO_HASH_PREFIX_LEN + HOLO_HASH_CORE_LEN]
                .to_vec()
                .into(),
            <Vec<u8>>::from(UnsafeBytes::from(input.data)).into(),
        );
        async move {
            let res = fut.await?;
            Ok(Signature(res.to_vec()))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_keystore::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_tls_cert_get_or_create() {
        let keystore = spawn_test_keystore().await.unwrap();
        let (dig1, cert1, priv1) = keystore.get_or_create_first_tls_cert().await.unwrap();
        let (dig2, cert2, priv2) = keystore.get_or_create_first_tls_cert().await.unwrap();
        assert_eq!(dig1, dig2);
        assert_eq!(cert1, cert2);
        assert_eq!(priv1, priv2);
    }
}
