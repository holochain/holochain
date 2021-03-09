//! Module for items related to aggregating validation_receipts

use fallible_iterator::FallibleIterator;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_keystore::KeystoreSender;
use holochain_serialized_bytes::prelude::*;
use holochain_sqlite::buffer::BufferedStore;
use holochain_sqlite::buffer::KvvBufUsed;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::prelude::Readable;
use holochain_sqlite::prelude::Writer;
use holochain_sqlite::prelude::*;
use holochain_zome_types::signature::Signature;

/// The result of a DhtOp Validation.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(tag = "type")]
pub enum ValidationResult {
    /// Successful validation.
    Valid,
    // TODO - fill out with additional options, which may (or may not) have content
    // Abandoned { .. },
    // Warrant { .. },
}

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
    pub validation_result: ValidationResult,

    /// the remote validator which is signing this receipt.
    pub validator: AgentPubKey,
}

impl ValidationReceipt {
    /// Sign this validation receipt.
    pub async fn sign(self, keystore: &KeystoreSender) -> DatabaseResult<SignedValidationReceipt> {
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

/// The database/buffer for aggregating validation_receipts sent by remote
/// nodes in charge of storage thereof.
pub struct ValidationReceiptsBuf(KvvBufUsed<DhtOpHash, SignedValidationReceipt>);

impl ValidationReceiptsBuf {
    /// Constructor given read-only transaction and db ref.
    pub fn new(dbs: &impl GetTable) -> DatabaseResult<ValidationReceiptsBuf> {
        Ok(Self(KvvBufUsed::new_opts(
            dbs.get_table_m(TableName::ValidationReceipts)?,
            true, // set to no_dup_data mode
        )))
    }

    /// List all the validation receipts for a given hash.
    pub fn list_receipts<'r, R: Readable>(
        &'r self,
        r: &'r mut R,
        dht_op_hash: &'r DhtOpHash,
    ) -> DatabaseResult<
        impl fallible_iterator::FallibleIterator<
                Item = SignedValidationReceipt,
                Error = DatabaseError,
            > + '_,
    > {
        Ok(fallible_iterator::convert(self.0.get(r, dht_op_hash)?))
    }

    /// Get the current valid receipt count for a given hash.
    pub fn count_valid<'r, R: Readable>(
        &'r self,
        r: &'r mut R,
        dht_op_hash: &DhtOpHash,
    ) -> DatabaseResult<usize> {
        let mut count = 0;

        let mut iter = self.list_receipts(r, dht_op_hash)?;
        while let Some(v) = iter.next()? {
            if v.receipt.validation_result == ValidationResult::Valid {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Add this receipt if it isn't already in the database.
    pub fn add_if_unique(&mut self, receipt: SignedValidationReceipt) -> DatabaseResult<()> {
        // The underlying KvvBufUsed manages the uniqueness
        self.0.insert(receipt.receipt.dht_op_hash.clone(), receipt);

        Ok(())
    }
}

impl BufferedStore for ValidationReceiptsBuf {
    type Error = DatabaseError;

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        // we are in no_dup_data mode
        // so even if someone else added a dup in the mean time
        // it will not get written to the DB
        self.0.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_keystore::KeystoreSenderExt;
    use holochain_sqlite::db::ReadManager;
    use holochain_types::test_utils::fake_dht_op_hash;

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
            validation_result: ValidationResult::Valid,
            validator: agent,
        };
        receipt.sign(keystore).await.unwrap()
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validation_receipts_db_populate_and_list() -> DatabaseResult<()> {
        observability::test_run().ok();

        let test_env = holochain_sqlite::test_utils::test_cell_env();
        let env = test_env.env();
        let keystore = holochain_sqlite::test_utils::test_keystore();

        let test_op_hash = fake_dht_op_hash(1);
        let vr1 = fake_vr(&test_op_hash, &keystore).await;
        let vr2 = fake_vr(&test_op_hash, &keystore).await;

        {
            let mut vr_buf1 = ValidationReceiptsBuf::new(&env)?;
            let mut vr_buf2 = ValidationReceiptsBuf::new(&env)?;

            vr_buf1.add_if_unique(vr1.clone())?;
            vr_buf1.add_if_unique(vr1.clone())?;

            vr_buf1.add_if_unique(vr2.clone())?;

            env.guard()
                .with_commit(|writer| vr_buf1.flush_to_txn(writer))?;

            vr_buf2.add_if_unique(vr1.clone())?;

            env.guard()
                .with_commit(|writer| vr_buf2.flush_to_txn(writer))?;
        }

        let mut g = env.guard();
        let mut reader = g.reader()?;
        let vr_buf = ValidationReceiptsBuf::new(&env)?;

        assert_eq!(2, vr_buf.count_valid(&mut reader, &test_op_hash)?);

        let mut list = vr_buf
            .list_receipts(&mut reader, &test_op_hash)?
            .collect::<Vec<_>>()?;
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

        Ok(())
    }
}
