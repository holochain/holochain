//! Module for items related to aggregating validation_receipts

use fallible_iterator::FallibleIterator;
use holo_hash::{AgentPubKey, DhtOpHash};
use holochain_keystore::{AgentPubKeyExt, KeystoreSender, Signature};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{BufferedStore, KvvBufUsed},
    db::GetDb,
    env::EnvironmentReadRef,
    error::{DatabaseError, DatabaseResult},
    prelude::{Reader, Writer},
};

/// The result of a DhtOp Validation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, SerializedBytes,
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
    pub fn new(env_ref: &EnvironmentReadRef) -> DatabaseResult<ValidationReceiptsBuf> {
        Ok(Self(KvvBufUsed::new_opts(
            env_ref.get_db(&*holochain_state::db::VALIDATION_RECEIPTS)?,
            true, // set to no_dup_data mode
        )?))
    }

    /// List all the validation receipts for a given hash.
    pub fn list_receipts(
        &self,
        dht_op_hash: &DhtOpHash,
    ) -> DatabaseResult<
        impl fallible_iterator::FallibleIterator<
                Item = SignedValidationReceipt,
                Error = DatabaseError,
            > + '_,
    > {
        Ok(fallible_iterator::convert(
            self.0.get(todo!("pass in a reader"), dht_op_hash)?,
        ))
    }

    /// Get the current valid receipt count for a given hash.
    pub fn count_valid(&self, dht_op_hash: &DhtOpHash) -> DatabaseResult<usize> {
        let mut count = 0;

        let mut iter = self.list_receipts(dht_op_hash)?;
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

    fn flush_to_txn(self, writer: &mut Writer) -> DatabaseResult<()> {
        // we are in no_dup_data mode
        // so even if someone else added a dup in the mean time
        // it will not get written to the DB
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_keystore::KeystoreApiSender;
    use holochain_state::{env::ReadManager, prelude::*};
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
        holochain_types::observability::test_run().ok();

        let env = holochain_state::test_utils::test_cell_env();
        let env_ref = env.guard().await;
        let keystore = holochain_state::test_utils::test_keystore();

        let test_op_hash = fake_dht_op_hash(1);
        let vr1 = fake_vr(&test_op_hash, &keystore).await;
        let vr2 = fake_vr(&test_op_hash, &keystore).await;

        {
            // capture the readers at the same time
            // so we can test out the resolve-dups-on-write logic
            let reader1 = env_ref.reader()?;
            let mut vr_buf1 = ValidationReceiptsBuf::new(&env_ref)?;
            let reader2 = env_ref.reader()?;
            let mut vr_buf2 = ValidationReceiptsBuf::new(&env_ref)?;

            vr_buf1.add_if_unique(vr1.clone())?;
            vr_buf1.add_if_unique(vr1.clone())?;

            vr_buf1.add_if_unique(vr2.clone())?;

            env_ref.with_commit(|writer| vr_buf1.flush_to_txn(writer))?;

            vr_buf2.add_if_unique(vr1.clone())?;

            env_ref.with_commit(|writer| vr_buf2.flush_to_txn(writer))?;
        }

        let reader = env_ref.reader()?;
        let vr_buf = ValidationReceiptsBuf::new(&env_ref)?;

        assert_eq!(2, vr_buf.count_valid(&test_op_hash)?);

        let mut list = vr_buf.list_receipts(&test_op_hash)?.collect::<Vec<_>>()?;
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
