//! Module for items related to aggregating validation_receipts

use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_keystore::MetaLairClient;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_zome_types::signature::Signature;
use holochain_zome_types::Timestamp;
use holochain_zome_types::ValidationStatus;
use mutations::StateMutationResult;

use crate::mutations;
use crate::prelude::from_blob;
use crate::prelude::StateQueryResult;

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

    /// the signature of the remote validator.
    pub validators_signatures: Vec<Signature>,
}

pub fn list_receipts(
    txn: &Transaction,
    op_hash: &DhtOpHash,
) -> StateQueryResult<Vec<SignedValidationReceipt>> {
    let mut stmt = txn.prepare(
        "
        SELECT blob FROM ValidationReceipt WHERE op_hash = :op_hash
        ",
    )?;
    let iter = stmt.query_and_then(
        named_params! {
            ":op_hash": op_hash
        },
        |row| from_blob::<SignedValidationReceipt>(row.get("blob")?),
    )?;
    iter.collect()
}

pub fn count_valid(txn: &Transaction, op_hash: &DhtOpHash) -> DatabaseResult<usize> {
    let count: usize = txn
        .query_row(
            "SELECT COUNT(hash) FROM ValidationReceipt WHERE op_hash = :op_hash",
            named_params! {
                ":op_hash": op_hash
            },
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok(count)
}

pub fn add_if_unique(
    txn: &mut Transaction,
    receipt: SignedValidationReceipt,
) -> StateMutationResult<()> {
    mutations::insert_validation_receipt(txn, receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixt::prelude::*;
    use holo_hash::HasHash;
    use holochain_types::dht_op::DhtOp;
    use holochain_types::dht_op::DhtOpHashed;
    use holochain_zome_types::fixt::*;

    async fn fake_vr(
        dht_op_hash: &DhtOpHash,
        keystore: &MetaLairClient,
    ) -> SignedValidationReceipt {
        let agent = keystore.new_sign_keypair_random().await.unwrap();
        let receipt = ValidationReceipt {
            dht_op_hash: dht_op_hash.clone(),
            validation_status: ValidationStatus::Valid,
            validators: vec![agent],
            when_integrated: Timestamp::now(),
        };
        receipt.sign(keystore).await.unwrap().unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validation_receipts_db_populate_and_list() -> StateMutationResult<()> {
        observability::test_run().ok();

        let test_env = crate::test_utils::test_authored_env();
        let env = test_env.env();
        let keystore = crate::test_utils::test_keystore();

        let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Header),
        ));
        let test_op_hash = op.as_hash().clone();
        env.conn()
            .unwrap()
            .with_commit_sync(|txn| mutations::insert_op(txn, &op))
            .unwrap();

        let vr1 = fake_vr(&test_op_hash, &keystore).await;
        let vr2 = fake_vr(&test_op_hash, &keystore).await;

        {
            env.conn().unwrap().with_commit_sync(|txn| {
                add_if_unique(txn, vr1.clone())?;
                add_if_unique(txn, vr1.clone())?;
                add_if_unique(txn, vr2.clone())
            })?;

            env.conn()
                .unwrap()
                .with_commit_sync(|txn| add_if_unique(txn, vr1.clone()))?;
        }

        let mut g = env.conn().unwrap();
        g.with_reader_test(|reader| {
            assert_eq!(2, count_valid(&reader, &test_op_hash).unwrap());

            let mut list = list_receipts(&reader, &test_op_hash).unwrap();
            list.sort_by(|a, b| {
                a.receipt.validators[0]
                    .partial_cmp(&b.receipt.validators[0])
                    .unwrap()
            });

            let mut expects = vec![vr1, vr2];
            expects.sort_by(|a, b| {
                a.receipt.validators[0]
                    .partial_cmp(&b.receipt.validators[0])
                    .unwrap()
            });

            assert_eq!(expects, list);
        });
        Ok(())
    }

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
