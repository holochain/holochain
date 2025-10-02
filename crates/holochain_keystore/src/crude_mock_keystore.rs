//! Defines a crude mock Keystore which always returns the same Error for every
//! call. This is about as close as we can get to a true mock which would allow
//! tweaking individual handlers, hence why this is a "crude" mock.

use crate::spawn_test_keystore;
use crate::MetaLairClient;
use futures::FutureExt;
use lair_keystore::dependencies::lair_keystore_api::lair_client::client_traits::AsLairClient;
use lair_keystore::dependencies::lair_keystore_api::prelude::{LairApiEnum, LairClient};
use lair_keystore::dependencies::lair_keystore_api::types::SharedSizedLockedArray;
use lair_keystore::dependencies::lair_keystore_api::LairResult;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Spawn a test keystore which always returns the same LairError for every call.
pub async fn spawn_crude_mock_keystore<F>(err_fn: F) -> MetaLairClient
where
    F: Fn() -> one_err::OneErr + Send + Sync + 'static,
{
    let (s, _) = tokio::sync::mpsc::unbounded_channel();
    MetaLairClient(
        Arc::new(parking_lot::Mutex::new(LairClient(Arc::new(
            CrudeMockKeystore(Arc::new(err_fn)),
        )))),
        s,
    )
}

/// A keystore which always returns the same LairError for every call.
struct RealOrMockKeystore {
    mock: Box<dyn Fn(LairApiEnum) -> LairResult<LairApiEnum> + Send + Sync + 'static>,
    real: MetaLairClient,
    use_mock: Arc<AtomicBool>,
}

/// A keystore which always returns the same LairError for every call.
struct CrudeMockKeystore(Arc<dyn Fn() -> one_err::OneErr + Send + Sync + 'static>);

impl AsLairClient for CrudeMockKeystore {
    fn get_enc_ctx_key(&self) -> SharedSizedLockedArray<32> {
        unimplemented!()
    }

    fn get_dec_ctx_key(&self) -> SharedSizedLockedArray<32> {
        unimplemented!()
    }

    fn shutdown(&self) -> futures::future::BoxFuture<'static, LairResult<()>> {
        unimplemented!()
    }

    fn request(
        &self,
        _request: LairApiEnum,
    ) -> futures::future::BoxFuture<'static, LairResult<LairApiEnum>> {
        let err = (self.0)();
        async move { Err(err) }.boxed()
    }
}

impl AsLairClient for RealOrMockKeystore {
    fn get_enc_ctx_key(&self) -> SharedSizedLockedArray<32> {
        self.real.cli().0.get_enc_ctx_key()
    }

    fn get_dec_ctx_key(&self) -> SharedSizedLockedArray<32> {
        self.real.cli().0.get_dec_ctx_key()
    }

    fn shutdown(&self) -> futures::future::BoxFuture<'static, LairResult<()>> {
        self.real.cli().0.shutdown().boxed()
    }

    fn request(
        &self,
        request: LairApiEnum,
    ) -> futures::future::BoxFuture<'static, LairResult<LairApiEnum>> {
        if self.use_mock.load(std::sync::atomic::Ordering::SeqCst) {
            let r = (self.mock)(request);
            async move { r }.boxed()
        } else {
            AsLairClient::request(&*self.real.cli().0 .0, request)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_pubkey_ext::AgentPubKeyExt;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_crude_mock_keystore() {
        tokio::task::spawn(async move {
            let keystore = spawn_crude_mock_keystore(|| "err".into()).await;

            assert_eq!(
                holo_hash::AgentPubKey::new_random(&keystore).await,
                Err(one_err::OneErr::new("err"))
            );
            // let agent = holo_hash::AgentPubKey::new_from_pure_entropy(&keystore)
            //     .await
            //     .unwrap();

            // #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
            // struct MyData(Vec<u8>);

            // let data = MyData(b"signature test data 1".to_vec());

            // assert_eq!(
            //     agent.sign(&keystore, &data).await,
            //     Err(KeystoreError::LairError(LairError::other("err")))
            // );
        })
        .await
        .unwrap();
    }
}
