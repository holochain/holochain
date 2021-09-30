//! Holochain integration with the new lair api

use futures::future::{BoxFuture, FutureExt};
use holochain_zome_types::prelude::*;
use kitsune_p2p_types::dependencies::{futures, new_lair_api};
use new_lair_api::prelude::*;
use std::sync::Arc;

/// Extend holo_hash::AgentPubKey with new lair keystore client api helpers.
pub trait AgentPubKeyNewLairExt {
    /// create a new agent keypair in given keystore, returning the AgentPubKey
    /// note this is a temporary awkward api to smoothe the transition to
    /// new lair
    fn new_lair_new_seed_sig_pub_key(
        keystore: &LairClient,
    ) -> BoxFuture<'static, LairResult<holo_hash::AgentPubKey>>
    where
        Self: Sized;

    /// sign some arbitrary raw bytes
    fn sign(
        &self,
        keystore: &LairClient,
        data: Arc<[u8]>,
    ) -> BoxFuture<'static, LairResult<Signature>>;

    /// verify a signature with this agent public_key is valid
    fn verify(&self, sig: &Signature, data: Arc<[u8]>) -> BoxFuture<'static, bool>;
}

impl AgentPubKeyNewLairExt for holo_hash::AgentPubKey {
    fn new_lair_new_seed_sig_pub_key(
        keystore: &LairClient,
    ) -> BoxFuture<'static, LairResult<holo_hash::AgentPubKey>>
    where
        Self: Sized,
    {
        let tag = nanoid::nanoid!();
        let fut = keystore.new_seed(tag.into(), None);
        async move {
            let info = fut.await?;
            let pub_key = holo_hash::AgentPubKey::from_raw_32(info.ed25519_pub_key.0.to_vec());
            Ok(pub_key)
        }
        .boxed()
    }

    fn sign(
        &self,
        keystore: &LairClient,
        data: Arc<[u8]>,
    ) -> BoxFuture<'static, LairResult<Signature>> {
        let mut pub_key = [0; 32];
        pub_key.copy_from_slice(self.get_raw_32());
        let fut = keystore.sign_by_pub_key(pub_key.into(), None, data);
        async move {
            let sig = fut.await?;
            Ok(Signature(*sig.0))
        }
        .boxed()
    }

    fn verify(&self, sig: &Signature, data: Arc<[u8]>) -> BoxFuture<'static, bool> {
        let mut pub_key = [0; 32];
        pub_key.copy_from_slice(self.get_raw_32());
        let pub_key = <BinDataSized<32>>::from(pub_key);
        let sig = sig.0;

        async move {
            match pub_key.verify_detached(sig.into(), data).await {
                Ok(b) => b,
                _ => false,
            }
        }
        .boxed()
    }
}
