//! Module for items related to aggregating validation_receipts

use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_sqlite::prelude::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_types::prelude::{SignedValidationReceipt, ValidationReceipt};
use mutations::StateMutationResult;

use crate::mutations;
use crate::prelude::from_blob;
use crate::prelude::StateQueryResult;

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

pub fn get_pending_validation_receipts(
    txn: &Transaction,
    validators: Vec<AgentPubKey>,
) -> StateQueryResult<Vec<(ValidationReceipt, AgentPubKey)>> {
    let mut stmt = txn.prepare(
        "
            SELECT Action.author, DhtOp.hash, DhtOp.validation_status,
            DhtOp.when_integrated
            From DhtOp
            JOIN Action ON DhtOp.action_hash = Action.hash
            WHERE
            DhtOp.require_receipt = 1
            AND
            DhtOp.when_integrated IS NOT NULL
            AND
            DhtOp.validation_status IS NOT NULL
            ",
    )?;

    let ops = stmt
        .query_and_then([], |r| {
            let author: AgentPubKey = r.get("author")?;
            let dht_op_hash: DhtOpHash = r.get("hash")?;
            let validation_status = r.get("validation_status")?;
            // NB: timestamp will never be null, so this is OK
            let when_integrated = r.get("when_integrated")?;
            Ok((
                ValidationReceipt {
                    dht_op_hash,
                    validation_status,
                    validators: validators.clone(),
                    when_integrated,
                },
                author,
            ))
        })?
        .collect::<StateQueryResult<Vec<_>>>()?;

    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutations::set_when_integrated;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use holo_hash::{HasHash, HoloHashOf};
    use holochain_keystore::{test_keystore, MetaLairClient};
    use std::collections::HashSet;

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
        holochain_trace::test_run();

        let test_db = crate::test_utils::test_authored_db();
        let env = test_db.to_db();
        let keystore = test_keystore();

        let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));
        let test_op_hash = op.as_hash().clone();
        env.write_async(move |txn| mutations::insert_op(txn, &op))
            .await
            .unwrap();

        let vr1 = fake_vr(&test_op_hash, &keystore).await;
        let vr2 = fake_vr(&test_op_hash, &keystore).await;

        env.write_async({
            let put_vr1 = vr1.clone();
            let put_vr2 = vr2.clone();

            move |txn| {
                add_if_unique(txn, put_vr1.clone())?;
                add_if_unique(txn, put_vr1.clone())?;
                add_if_unique(txn, put_vr2.clone())
            }
        })
        .await?;

        env.write_async({
            let put_vr1 = vr1.clone();

            move |txn| add_if_unique(txn, put_vr1)
        })
        .await?;

        env.read_async(move |reader| -> DatabaseResult<()> {
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

            Ok(())
        })
        .await
        .unwrap();
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn no_pending_receipts() {
        holochain_trace::test_run();

        let env = crate::test_utils::test_dht_db().to_db();

        // With no validators
        let pending = env
            .read_async(|txn| get_pending_validation_receipts(&txn, vec![]))
            .await
            .unwrap();

        assert!(pending.is_empty());

        // Same result with validators
        let pending = env
            .read_async(|txn| get_pending_validation_receipts(&txn, vec![fixt!(AgentPubKey)]))
            .await
            .unwrap();

        assert!(pending.is_empty());
    }

    async fn create_modified_op(
        vault: DbWrite<DbKindDht>,
        modifier: fn(txn: &mut Transaction, op_hash: HoloHashOf<DhtOp>) -> StateMutationResult<()>,
    ) -> StateMutationResult<DhtOpHash> {
        // The actual op does not matter, just some of the status fields
        let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));

        let test_op_hash = op.as_hash().clone();
        vault
            .write_async({
                let test_op_hash = test_op_hash.clone();
                move |txn| -> StateMutationResult<()> {
                    mutations::insert_op(txn, &op)?;
                    modifier(txn, test_op_hash)?;

                    Ok(())
                }
            })
            .await
            .unwrap();

        Ok(test_op_hash)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn filter_for_pending_validation_receipts() {
        holochain_trace::test_run();

        let test_db = crate::test_utils::test_dht_db();
        let env = test_db.to_db();

        // Has not been integrated yet
        create_modified_op(env.clone(), |_txn, _hash| {
            // Do nothing
            Ok(())
        })
        .await
        .unwrap();

        // Is ready to have a receipt sent
        let valid_op_hash = create_modified_op(env.clone(), |txn, op_hash| {
            set_require_receipt(txn, &op_hash, true)?;
            set_when_integrated(txn, &op_hash, Timestamp::now())?;
            set_validation_status(txn, &op_hash, ValidationStatus::Valid)?;
            Ok(())
        })
        .await
        .unwrap();

        // Is ready to have a receipt sent, with rejected status
        let rejected_op_hash = create_modified_op(env.clone(), |txn, op_hash| {
            set_require_receipt(txn, &op_hash, true)?;
            set_when_integrated(txn, &op_hash, Timestamp::now())?;
            set_validation_status(txn, &op_hash, ValidationStatus::Rejected)?;
            Ok(())
        })
        .await
        .unwrap();

        // Is ready to have a receipt sent, with abandoned status
        let abandoned_op_hash = create_modified_op(env.clone(), |txn, op_hash| {
            set_require_receipt(txn, &op_hash, true)?;
            set_when_integrated(txn, &op_hash, Timestamp::now())?;
            set_validation_status(txn, &op_hash, ValidationStatus::Abandoned)?;
            Ok(())
        })
        .await
        .unwrap();

        // Is ready to have a receipt sent, but does not require one
        create_modified_op(env.clone(), |txn, op_hash| {
            set_require_receipt(txn, &op_hash, false)?;
            set_when_integrated(txn, &op_hash, Timestamp::now())?;
            set_validation_status(txn, &op_hash, ValidationStatus::Valid)?;
            Ok(())
        })
        .await
        .unwrap();

        // Is ready to have a receipt sent, but when_integrated was not set
        create_modified_op(env.clone(), |txn, op_hash| {
            set_require_receipt(txn, &op_hash, true)?;
            set_validation_status(txn, &op_hash, ValidationStatus::Valid)?;
            Ok(())
        })
        .await
        .unwrap();

        // Is ready to have a receipt sent, but validation_status was not set
        create_modified_op(env.clone(), |txn, op_hash| {
            set_require_receipt(txn, &op_hash, true)?;
            set_when_integrated(txn, &op_hash, Timestamp::now())?;
            Ok(())
        })
        .await
        .unwrap();

        let pending = env
            .read_async(
                move |txn| -> StateQueryResult<Vec<(ValidationReceipt, AgentPubKey)>> {
                    get_pending_validation_receipts(&txn, vec![])
                },
            )
            .await
            .unwrap();

        assert_eq!(3, pending.len());

        let pending_ops: HashSet<DhtOpHash> =
            pending.into_iter().map(|p| p.0.dht_op_hash).collect();
        assert!(pending_ops.contains(&valid_op_hash));
        assert!(pending_ops.contains(&rejected_op_hash));
        assert!(pending_ops.contains(&abandoned_op_hash));
    }
}
