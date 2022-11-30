use crate::*;
use holochain_zome_types::prelude::*;
use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::LairResult;
use must_future::MustBoxFuture;
use std::sync::Arc;

/// Extend holo_hash::AgentPubKey with additional signature functionality
/// from Keystore.
pub trait AgentPubKeyExt {
    /// create a new agent keypair in given keystore, returning the AgentPubKey
    fn new_random(
        keystore: &MetaLairClient,
    ) -> MustBoxFuture<'static, LairResult<holo_hash::AgentPubKey>>
    where
        Self: Sized;

    /// sign some arbitrary raw bytes
    fn sign_raw(
        &self,
        keystore: &MetaLairClient,
        data: Arc<[u8]>,
    ) -> MustBoxFuture<'static, LairResult<Signature>>;

    /// verify a signature for given raw bytes with this agent public_key is valid
    fn verify_signature_raw(
        &self,
        signature: &Signature,
        data: Arc<[u8]>,
    ) -> MustBoxFuture<'static, bool>;

    // -- provided -- //

    /// sign some arbitrary data
    fn sign<S>(
        &self,
        keystore: &MetaLairClient,
        input: S,
    ) -> MustBoxFuture<'static, LairResult<Signature>>
    where
        S: Serialize + std::fmt::Debug,
    {
        use futures::future::FutureExt;

        let data = match holochain_serialized_bytes::encode(&input) {
            Err(e) => {
                return async move { Err(one_err::OneErr::new(e)) }.boxed().into();
            }
            Ok(data) => data,
        };

        self.sign_raw(keystore, data.into())
    }

    /// verify a signature for given data with this agent public_key is valid
    fn verify_signature<D>(&self, signature: &Signature, data: D) -> MustBoxFuture<'static, bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        use futures::future::FutureExt;

        let data = match data.try_into() {
            Err(e) => {
                tracing::error!("Serialization Error: {:?}", e);
                return async move { false }.boxed().into();
            }
            Ok(data) => data,
        };

        let data: Vec<u8> = UnsafeBytes::from(data).into();

        self.verify_signature_raw(signature, data.into())
    }
}

impl AgentPubKeyExt for holo_hash::AgentPubKey {
    fn new_random(
        keystore: &MetaLairClient,
    ) -> MustBoxFuture<'static, LairResult<holo_hash::AgentPubKey>>
    where
        Self: Sized,
    {
        let f = keystore.new_sign_keypair_random();
        MustBoxFuture::new(async move { f.await })
    }

    fn sign_raw(
        &self,
        keystore: &MetaLairClient,
        data: Arc<[u8]>,
    ) -> MustBoxFuture<'static, LairResult<Signature>> {
        let f = keystore.sign(self.clone(), data);
        MustBoxFuture::new(async move { f.await })
    }

    fn verify_signature_raw(
        &self,
        signature: &Signature,
        data: Arc<[u8]>,
    ) -> MustBoxFuture<'static, bool> {
        let mut pub_key = [0; 32];
        pub_key.copy_from_slice(self.get_raw_32());
        let pub_key = <lair_keystore_api::prelude::BinDataSized<32>>::from(pub_key);
        let sig = signature.0;

        MustBoxFuture::new(async move {
            match pub_key.verify_detached(sig.into(), data).await {
                Ok(b) => b,
                _ => false,
            }
        })
    }
}
