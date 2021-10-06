use crate::*;
use ghost_actor::dependencies::must_future::MustBoxFuture;
use holochain_zome_types::prelude::*;
use kitsune_p2p_types::dependencies::new_lair_api;
use new_lair_api::LairResult;
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
    fn verify_signature_raw(&self, signature: &Signature, data: &[u8]) -> KeystoreApiFuture<bool>;

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
        use ghost_actor::dependencies::futures::future::FutureExt;

        let data = match holochain_serialized_bytes::encode(&input) {
            Err(e) => {
                return async move { Err(one_err::OneErr::new(e)) }.boxed().into();
            }
            Ok(data) => data,
        };

        self.sign_raw(keystore, data.into())
    }

    /// verify a signature for given data with this agent public_key is valid
    fn verify_signature<D>(&self, signature: &Signature, data: D) -> KeystoreApiFuture<bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        use ghost_actor::dependencies::futures::future::FutureExt;

        let data = match data.try_into() {
            Err(e) => {
                return async move { Err(e.into()) }.boxed().into();
            }
            Ok(data) => data,
        };

        self.verify_signature_raw(signature, data.bytes())
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

    fn verify_signature_raw(&self, signature: &Signature, data: &[u8]) -> KeystoreApiFuture<bool> {
        use ghost_actor::dependencies::futures::future::FutureExt;

        let data = Arc::new(data.to_vec());
        let pub_key: legacy_lair_api::internal::sign_ed25519::SignEd25519PubKey =
            self.get_raw_32().to_vec().into();
        let sig: legacy_lair_api::internal::sign_ed25519::SignEd25519Signature =
            signature.0.to_vec().into();

        async move { Ok(pub_key.verify(data, sig).await?) }
            .boxed()
            .into()
    }
}
