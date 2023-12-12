//! Types for validation receipts and signed validation receipts to be sent between peers.

use crate::prelude::{Signature, Timestamp};
use futures::{Stream, StreamExt, TryStreamExt};
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_keystore::{AgentPubKeyExt, MetaLairClient};
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;
use std::vec::IntoIter;

/// Validation receipt content - to be signed.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
pub struct ValidationReceipt {
    /// the op this validation receipt is for.
    pub dht_op_hash: DhtOpHash,

    /// the result of this validation.
    pub validation_status: ValidationStatus,

    /// the remote validator which is signing this receipt.
    pub validators: Vec<AgentPubKey>,

    /// Time when the op was integrated
    pub when_integrated: Timestamp,
}

impl ValidationReceipt {
    /// Sign this validation receipt.
    pub async fn sign(
        self,
        keystore: &MetaLairClient,
    ) -> holochain_keystore::LairResult<Option<SignedValidationReceipt>> {
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
                async move { validator.sign(&keystore, this).await }
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

/// A full, signed validation receipt.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
)]
pub struct SignedValidationReceipt {
    /// the content of the validation receipt.
    pub receipt: ValidationReceipt,

    // TODO This is just the signature and not the original message, should this be a full signature and get validated
    //      when it is received? https://github.com/holochain/holochain/pull/2848#discussion_r1346160783
    /// the signature of the remote validator.
    pub validators_signatures: Vec<Signature>,
}

/// A bundle of validation receipts to be sent together.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct ValidationReceiptBundle(Vec<SignedValidationReceipt>);

impl From<Vec<SignedValidationReceipt>> for ValidationReceiptBundle {
    fn from(value: Vec<SignedValidationReceipt>) -> Self {
        ValidationReceiptBundle(value)
    }
}

impl IntoIterator for ValidationReceiptBundle {
    type Item = SignedValidationReceipt;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::validation_receipt::try_stream_of_results;

    #[tokio::test]
    async fn test_try_stream_of_results() {
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
