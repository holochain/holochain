//! Module for items related to aggregating validation_receipts

use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_keystore::KeystoreSender;
use holochain_keystore::{keystore_actor::KeystoreApiResult, AgentPubKeyExt};
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::Timestamp;
use holochain_zome_types::signature::Signature;
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
    pub validator: AgentPubKey,

    /// Time when the op was integrated
    pub when_integrated: Timestamp,
}

impl ValidationReceipt {
    /// Sign this validation receipt.
    pub async fn sign(
        self,
        keystore: &KeystoreSender,
    ) -> KeystoreApiResult<SignedValidationReceipt> {
        let signature = self.validator.sign(keystore, self.clone()).await?;
        Ok(SignedValidationReceipt {
            receipt: self,
            validator_signature: signature,
        })
    }
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
    pub validator_signature: Signature,
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
    use holochain_keystore::KeystoreSenderExt;
    use holochain_sqlite::db::ReadManager;
    use holochain_types::test_utils::fake_dht_op_hash;
    use holochain_types::timestamp;

    async fn fake_vr(
        dht_op_hash: &DhtOpHash,
        keystore: &KeystoreSender,
    ) -> SignedValidationReceipt {
        let agent = keystore
            .clone()
            .generate_sign_keypair_from_pure_entropy()
            .await
            .unwrap();
        let receipt = ValidationReceipt {
            dht_op_hash: dht_op_hash.clone(),
            validation_status: ValidationStatus::Valid,
            validator: agent,
            when_integrated: timestamp::now(),
        };
        receipt.sign(keystore).await.unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validation_receipts_db_populate_and_list() -> DatabaseResult<()> {
        observability::test_run().ok();

        let test_env = crate::test_utils::test_cell_env();
        let env = test_env.env();
        let keystore = crate::test_utils::test_keystore();

        let test_op_hash = fake_dht_op_hash(1);
        let vr1 = fake_vr(&test_op_hash, &keystore).await;
        let vr2 = fake_vr(&test_op_hash, &keystore).await;

        {
            let mut vr_buf1 = ValidationReceiptsBuf::new(&env)?;
            let mut vr_buf2 = ValidationReceiptsBuf::new(&env)?;

            vr_buf1.add_if_unique(vr1.clone())?;
            vr_buf1.add_if_unique(vr1.clone())?;

            vr_buf1.add_if_unique(vr2.clone())?;

            env.conn()
                .unwrap()
                .with_commit(|writer| vr_buf1.flush_to_txn(writer))?;

            vr_buf2.add_if_unique(vr1.clone())?;

            env.conn()
                .unwrap()
                .with_commit(|writer| vr_buf2.flush_to_txn(writer))?;
        }

        let mut g = env.conn().unwrap();
        g.with_reader_test(|mut reader| {
            let vr_buf = ValidationReceiptsBuf::new(&env).unwrap();

            assert_eq!(2, vr_buf.count_valid(&mut reader, &test_op_hash).unwrap());

            let mut list = vr_buf
                .list_receipts(&mut reader, &test_op_hash)
                .unwrap()
                .collect::<Vec<_>>()
                .unwrap();
            list.sort_by(|a, b| {
                a.receipt
                    .validator
                    .partial_cmp(&b.receipt.validator)
                    .unwrap()
            });

            let mut expects = vec![vr1, vr2];
            expects.sort_by(|a, b| {
                a.receipt
                    .validator
                    .partial_cmp(&b.receipt.validator)
                    .unwrap()
            });

            assert_eq!(expects, list);
        });
        Ok(())
    }
}
