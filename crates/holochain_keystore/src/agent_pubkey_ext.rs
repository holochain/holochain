use crate::*;
use holochain_zome_types::prelude::*;
use std::sync::Arc;

/// Extend holo_hash::AgentPubKey with additional signature functionality
/// from Keystore.
pub trait AgentPubKeyExt {
    /// create a new agent keypair in given keystore, returning the AgentPubKey
    fn new_from_pure_entropy(
        keystore: &KeystoreSender,
    ) -> KeystoreApiFuture<holo_hash::AgentPubKey>
    where
        Self: Sized;

    /// sign some arbitrary data
    fn sign<S>(&self, keystore: &KeystoreSender, data: S) -> KeystoreApiFuture<Signature>
    where
        S: Serialize + std::fmt::Debug;

    /// sign some arbitrary raw bytes
    fn sign_raw(&self, keystore: &KeystoreSender, data: &[u8]) -> KeystoreApiFuture<Signature>;

    /// verify a signature for given data with this agent public_key is valid
    fn verify_signature<D>(&self, signature: &Signature, data: D) -> KeystoreApiFuture<bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>;

    /// verify a signature for given raw bytes with this agent public_key is valid
    fn verify_signature_raw(&self, signature: &Signature, data: &[u8]) -> KeystoreApiFuture<bool>;
}

impl AgentPubKeyExt for holo_hash::AgentPubKey {
    fn new_from_pure_entropy(keystore: &KeystoreSender) -> KeystoreApiFuture<holo_hash::AgentPubKey>
    where
        Self: Sized,
    {
        let f = keystore.generate_sign_keypair_from_pure_entropy();
        ghost_actor::dependencies::must_future::MustBoxFuture::new(async move { f.await })
    }

    fn sign<S>(&self, keystore: &KeystoreSender, input: S) -> KeystoreApiFuture<Signature>
    where
        S: Serialize + std::fmt::Debug,
    {
        use ghost_actor::dependencies::futures::future::FutureExt;
        let keystore = keystore.clone();
        let maybe_data: Result<Vec<u8>, SerializedBytesError> =
            holochain_serialized_bytes::encode(&input);
        let key = self.clone();
        async move {
            let data = maybe_data?;
            let f = keystore.sign(Sign {
                key,
                data: serde_bytes::ByteBuf::from(data),
            });
            match tokio::time::timeout(std::time::Duration::from_secs(30), f).await {
                Ok(r) => r,
                Err(_) => Err(KeystoreError::Other(
                    "Keystore timeout while signing agent key".to_string(),
                )),
            }
        }
        .boxed()
        .into()
    }

    fn sign_raw(&self, keystore: &KeystoreSender, data: &[u8]) -> KeystoreApiFuture<Signature> {
        use ghost_actor::dependencies::futures::future::FutureExt;
        let keystore = keystore.clone();
        let input = Sign::new_raw(self.clone(), data.to_vec());
        async move { keystore.sign(input).await }.boxed().into()
    }

    fn verify_signature<D>(&self, signature: &Signature, data: D) -> KeystoreApiFuture<bool>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        use ghost_actor::dependencies::futures::future::FutureExt;

        let pub_key: lair_keystore_api::internal::sign_ed25519::SignEd25519PubKey =
            self.get_raw_32().to_vec().into();
        let sig: lair_keystore_api::internal::sign_ed25519::SignEd25519Signature =
            signature.0.to_vec().into();

        let data: Result<SerializedBytes, SerializedBytesError> = data.try_into();

        async move {
            let data = Arc::new(data?.bytes().to_vec());
            Ok(pub_key.verify(data, sig).await?)
        }
        .boxed()
        .into()
    }

    fn verify_signature_raw(&self, signature: &Signature, data: &[u8]) -> KeystoreApiFuture<bool> {
        use ghost_actor::dependencies::futures::future::FutureExt;

        let data = Arc::new(data.to_vec());
        let pub_key: lair_keystore_api::internal::sign_ed25519::SignEd25519PubKey =
            self.get_raw_32().to_vec().into();
        let sig: lair_keystore_api::internal::sign_ed25519::SignEd25519Signature =
            signature.0.to_vec().into();

        async move { Ok(pub_key.verify(data, sig).await?) }
            .boxed()
            .into()
    }
}
