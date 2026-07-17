//! Extension trait definition [`ValidationReceiptExt`].

use crate::{AgentPubKeyExt, LairResult, MetaLairClient};
use futures::{Stream, StreamExt, TryStreamExt};
use holochain_types::prelude::{SignedValidationReceipt, ValidationReceipt};
use must_future::MustBoxFuture;

/// Extension for keystore operations on a [`ValidationReceipt`].
pub trait ValidationReceiptExt {
    /// Sign this validation receipt.
    fn sign(
        self,
        keystore: &MetaLairClient,
    ) -> MustBoxFuture<'static, LairResult<Option<SignedValidationReceipt>>>;
}

impl ValidationReceiptExt for ValidationReceipt {
    fn sign(
        self,
        keystore: &MetaLairClient,
    ) -> MustBoxFuture<'static, LairResult<Option<SignedValidationReceipt>>> {
        let keystore = keystore.clone();
        MustBoxFuture::new(async move {
            if self.validators.is_empty() {
                return Ok(None);
            }
            let this = self.clone();
            // Try to sign with all validators but silently fail on
            // any that cannot sign.
            // If all signatures fail then return an error.
            let futures = self
                .validators
                .iter()
                .map(|validator| {
                    let this = this.clone();
                    let validator = validator.clone();
                    let keystore = keystore.clone();
                    async move { validator.sign(&keystore.clone(), this).await }
                })
                .collect::<Vec<_>>();
            let stream = futures::stream::iter(futures);
            let signatures = try_stream_of_results(stream).await?;
            if signatures.is_empty() {
                unreachable!("Signatures cannot be empty because the validators vec is not empty");
            }
            Ok(Some(SignedValidationReceipt {
                receipt: self,
                validators_signatures: signatures,
            }))
        })
    }
}

/// Try to collect a stream of futures that return results into a vec.
async fn try_stream_of_results<T, U, E>(stream: U) -> Result<Vec<T>, E>
where
    U: Stream,
    <U as Stream>::Item: futures::Future<Output = Result<T, E>>,
{
    stream.buffer_unordered(10).map(|r| r).try_collect().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn check_try_stream_of_results() {
        let iter: Vec<futures::future::Ready<Result<i32, String>>> = vec![];
        let stream = futures::stream::iter(iter);
        assert_eq!(Ok(vec![]), try_stream_of_results(stream).await);

        let iter = vec![async move { Result::<_, String>::Ok(0) }];
        let stream = futures::stream::iter(iter);
        assert_eq!(Ok(vec![0]), try_stream_of_results(stream).await);

        let iter = (0..10).map(|i| async move { Result::<_, String>::Ok(i) });
        let stream = futures::stream::iter(iter);
        assert_eq!(
            Ok((0..10).collect::<Vec<_>>()),
            try_stream_of_results(stream).await
        );

        let iter = vec![async move { Result::<i32, String>::Err("test".to_string()) }];
        let stream = futures::stream::iter(iter);
        assert_eq!(Err("test".to_string()), try_stream_of_results(stream).await);

        let iter = (0..10).map(|_| async move { Result::<i32, String>::Err("test".to_string()) });
        let stream = futures::stream::iter(iter);
        assert_eq!(Err("test".to_string()), try_stream_of_results(stream).await);
    }
}
