use crate::*;
use ::lair_keystore::dependencies::lair_keystore_api;
use holochain_zome_types::prelude::*;
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
    ) -> MustBoxFuture<'static, KeystoreResult<bool>>;

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
    fn verify_signature<D>(
        &self,
        signature: &Signature,
        data: D,
    ) -> MustBoxFuture<'static, KeystoreResult<bool>>
    where
        D: TryInto<SerializedBytes, Error = SerializedBytesError>,
    {
        use futures::future::FutureExt;

        let data = match data.try_into() {
            Err(e) => {
                tracing::error!("Serialization Error: {:?}", e);
                return async move { Err(e.into()) }.boxed().into();
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
        MustBoxFuture::new(f)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(keystore, data)))]
    fn sign_raw(
        &self,
        keystore: &MetaLairClient,
        data: Arc<[u8]>,
    ) -> MustBoxFuture<'static, LairResult<Signature>> {
        let f = keystore.sign(self.clone(), data);
        MustBoxFuture::new(f)
    }

    fn verify_signature_raw(
        &self,
        signature: &Signature,
        data: Arc<[u8]>,
    ) -> MustBoxFuture<'static, KeystoreResult<bool>> {
        let mut pub_key = [0; 32];
        pub_key.copy_from_slice(self.get_raw_32());
        let pub_key = <lair_keystore_api::prelude::BinDataSized<32>>::from(pub_key);
        let sig = signature.0;

        MustBoxFuture::new(async move { Ok(pub_key.verify_detached(sig.into(), data).await) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_keystore::test_keystore;
    use holo_hash::DnaHash;
    use holochain_zome_types::dependencies::holochain_integrity_types::action::{
        Action as LegacyAction, Dna,
    };
    use holochain_zome_types::dht_v2::from_legacy_action;

    /// Mirrors the two production call sites this crate's `sign`/
    /// `verify_signature` feed: `holochain_state`'s source chain signs over
    /// the v2 action content, and
    /// `holochain::core::sys_validate::verify_action_signature` verifies
    /// against the v2 action content. A signature produced this way must
    /// verify against the v2 action and must NOT verify against the legacy
    /// bytes directly — the two representations serialize differently, which
    /// is exactly the invariant this test pins.
    // `test_keystore()` spawns lair onto a blocking task, which requires a
    // multi-threaded runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn v2_projected_signature_round_trips_and_rejects_legacy_bytes() {
        let keystore = test_keystore();
        let author = holo_hash::AgentPubKey::new_random(&keystore).await.unwrap();

        let legacy = LegacyAction::Dna(Dna {
            author: author.clone(),
            timestamp: holochain_zome_types::timestamp::Timestamp::now(),
            hash: DnaHash::from_raw_36(vec![7u8; 36]),
        });

        // Sign over the v2 action, as the source chain does.
        let v2 = from_legacy_action(&legacy);
        let signature = legacy.signer().sign(&keystore, &v2).await.unwrap();

        // Verify, exactly as `verify_action_signature` does: project again
        // and check over the same v2 bytes.
        let v2_check = from_legacy_action(&legacy);
        assert!(
            v2_check
                .signer()
                .verify_signature(&signature, &v2_check)
                .await
                .unwrap(),
            "a signature computed over the v2 projection must verify against it"
        );

        // The legacy and v2 serializations differ, so the same signature
        // must not verify against the legacy bytes.
        assert!(
            !legacy
                .signer()
                .verify_signature(&signature, &legacy)
                .await
                .unwrap(),
            "a v2-projected signature must not verify against the legacy bytes"
        );
    }
}
