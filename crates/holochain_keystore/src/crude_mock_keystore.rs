//! Defines a crude mock Keystore which always returns the same Error for every
//! call. This is about as close as we can get to a true mock which would allow
//! tweaking individual handlers, hence why this is a "crude" mock.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use ghost_actor::dependencies::futures::FutureExt;
use kitsune_p2p_types::dependencies::lair_keystore_api::lair_client::traits::AsLairClient;
use kitsune_p2p_types::dependencies::lair_keystore_api::prelude::{LairApiEnum, LairClient};
use kitsune_p2p_types::dependencies::lair_keystore_api::LairResult;

use crate::test_keystore::spawn_test_keystore;
use crate::MetaLairClient;

/// Spawn a test keystore which always returns the same LairError for every call.
pub async fn spawn_crude_mock_keystore<F>(err_fn: F) -> MetaLairClient
where
    F: Fn() -> one_err::OneErr + Send + Sync + 'static,
{
    MetaLairClient::Lair(LairClient(Arc::new(CrudeMockKeystore(Arc::new(err_fn)))))
}

/// Spawn a test keystore that can switch between mocked and real.
/// It starts off as real and can be switched to the given callback mock
/// using the [`MockLairControl`].
pub async fn spawn_real_or_mock_keystore<F>(
    func: F,
) -> LairResult<(MetaLairClient, MockLairControl)>
where
    F: Fn(LairApiEnum) -> LairResult<LairApiEnum> + Send + Sync + 'static,
{
    let real = spawn_test_keystore().await?;
    let use_mock = Arc::new(AtomicBool::new(false));
    let mock = RealOrMockKeystore {
        mock: Box::new(func),
        real,
        use_mock: use_mock.clone(),
    };

    let control = MockLairControl(use_mock);

    Ok((MetaLairClient::Lair(LairClient(Arc::new(mock))), control))
}
/// A keystore which always returns the same LairError for every call.
struct RealOrMockKeystore {
    mock: Box<dyn Fn(LairApiEnum) -> LairResult<LairApiEnum> + Send + Sync + 'static>,
    real: MetaLairClient,
    use_mock: Arc<AtomicBool>,
}

/// Control if a mocked lair keystore is using
/// the real keystore or the mock callback.
pub struct MockLairControl(Arc<AtomicBool>);

impl MockLairControl {
    /// Use the mock callback.
    pub fn use_mock(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Use the real test keystore.
    pub fn use_real(&self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
/// A keystore which always returns the same LairError for every call.
struct CrudeMockKeystore(Arc<dyn Fn() -> one_err::OneErr + Send + Sync + 'static>);

impl AsLairClient for CrudeMockKeystore {
    fn get_enc_ctx_key(&self) -> sodoken::BufReadSized<32> {
        unimplemented!()
    }

    fn get_dec_ctx_key(&self) -> sodoken::BufReadSized<32> {
        unimplemented!()
    }

    fn shutdown(
        &self,
    ) -> ghost_actor::dependencies::futures::future::BoxFuture<'static, LairResult<()>> {
        unimplemented!()
    }

    fn request(
        &self,
        _request: LairApiEnum,
    ) -> ghost_actor::dependencies::futures::future::BoxFuture<'static, LairResult<LairApiEnum>>
    {
        let err = (self.0)();
        async move { Err(err) }.boxed()
    }
}

impl AsLairClient for RealOrMockKeystore {
    fn get_enc_ctx_key(&self) -> sodoken::BufReadSized<32> {
        match &self.real {
            MetaLairClient::Lair(client) => client.get_enc_ctx_key(),
        }
    }

    fn get_dec_ctx_key(&self) -> sodoken::BufReadSized<32> {
        match &self.real {
            MetaLairClient::Lair(client) => client.get_dec_ctx_key(),
        }
    }

    fn shutdown(
        &self,
    ) -> ghost_actor::dependencies::futures::future::BoxFuture<'static, LairResult<()>> {
        match &self.real {
            MetaLairClient::Lair(client) => client.shutdown().boxed(),
        }
    }

    fn request(
        &self,
        request: LairApiEnum,
    ) -> ghost_actor::dependencies::futures::future::BoxFuture<'static, LairResult<LairApiEnum>>
    {
        if self.use_mock.load(std::sync::atomic::Ordering::SeqCst) {
            let r = (self.mock)(request);
            async move { r }.boxed()
        } else {
            match &self.real {
                MetaLairClient::Lair(client) => client.0.request(request),
            }
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
