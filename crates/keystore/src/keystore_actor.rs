//! This module contains all the types needed to implement a keystore actor.
//! We will re-export the main KeystoreSender usable by clients at the lib.

use crate::*;
use ghost_actor::dependencies::futures::future::FutureExt;
use holo_hash::{HOLO_HASH_CORE_LEN, HOLO_HASH_PREFIX_LEN};
use holochain_zome_types::signature::SignInput;
use holochain_zome_types::signature::Signature;

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
    fn sign(&self, input: SignInput) -> KeystoreApiFuture<Signature>;
}

impl KeystoreSenderExt for KeystoreSender {
    fn generate_sign_keypair_from_pure_entropy(&self) -> KeystoreApiFuture<holo_hash::AgentPubKey> {
        use lair_keystore_api::actor::LairClientApiSender;
        let fut = self.sign_ed25519_new_from_entropy();
        async move {
            let (_, pk) = fut.await?;
            Ok(holo_hash::AgentPubKey::from_raw_32(pk.to_vec()))
        }
        .boxed()
        .into()
    }

    fn sign(&self, input: SignInput) -> KeystoreApiFuture<Signature> {
        use lair_keystore_api::actor::LairClientApiSender;
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
}
