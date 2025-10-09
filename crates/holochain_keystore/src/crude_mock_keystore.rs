//! Defines a crude mock Keystore which always returns the same Error for every
//! call. This is about as close as we can get to a true mock which would allow
//! tweaking individual handlers, hence why this is a "crude" mock.

use crate::MetaLairClient;
use futures::FutureExt;
use lair_keystore::dependencies::lair_keystore_api::lair_client::client_traits::AsLairClient;
use lair_keystore::dependencies::lair_keystore_api::prelude::{LairApiEnum, LairClient};
use lair_keystore::dependencies::lair_keystore_api::types::SharedSizedLockedArray;
use lair_keystore::dependencies::lair_keystore_api::LairResult;
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
        })
        .await
        .unwrap();
    }
}
