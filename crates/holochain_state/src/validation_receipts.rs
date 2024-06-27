//! Module for items related to aggregating validation_receipts

use holo_hash::{ActionHash, AgentPubKey};
use holo_hash::{DhtOpHash, EntryHash};
use holochain_sqlite::prelude::*;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::Transaction;
use holochain_sqlite::rusqlite::{named_params, Params, Statement};
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::{SignedValidationReceipt, ValidationReceipt};
use holochain_zome_types::prelude::{ValidationReceiptInfo, ValidationReceiptSet};
use mutations::StateMutationResult;
use std::collections::HashMap;

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

/// Finds [DhtOp]s for the given [ActionHash] and returns the associated [ValidationReceiptSet]s.
///
/// Each [ValidationReceiptSet] contains the validation receipts we have received for a single [DhtOp].
/// If we have received enough validation receipts for an op, then its validation receipt set will
/// have the `receipts_complete` field set to `true`.
pub fn validation_receipts_for_action(
    txn: &Transaction,
    action_hash: ActionHash,
) -> StateQueryResult<Vec<ValidationReceiptSet>> {
    let stmt = txn.prepare(
        "
            SELECT
              ValidationReceipt.blob as receipt,
              DhtOp.hash as op_hash,
              DhtOp.type as op_type,
              DhtOp.receipts_complete as op_receipts_complete
            FROM
              Action
              INNER JOIN DhtOp ON DhtOp.action_hash = Action.hash
              INNER JOIN ValidationReceipt ON DhtOp.hash = ValidationReceipt.op_hash
            WHERE
              Action.hash = :action_hash
            ",
    )?;

    query_validation_receipts(
        stmt,
        named_params! {
            ":action_hash": action_hash
        },
    )
}

/// Convenience alternative to calling [validation_receipts_for_action].
///
/// This function looks up the actions associated with the given [EntryHash] and then finds [DhtOp]s
/// and [ValidationReceiptSet]s for those actions.
pub fn validation_receipts_for_entry(
    txn: &Transaction,
    entry_hash: EntryHash,
) -> StateQueryResult<Vec<ValidationReceiptSet>> {
    let stmt = txn.prepare(
        "
            SELECT
              ValidationReceipt.blob as receipt,
              DhtOp.hash as op_hash,
              DhtOp.type as op_type,
              DhtOp.receipts_complete as op_receipts_complete
            FROM
              Entry
              INNER JOIN Action ON Action.entry_hash = Entry.hash
              INNER JOIN DhtOp ON DhtOp.action_hash = Action.hash
              INNER JOIN ValidationReceipt ON DhtOp.hash = ValidationReceipt.op_hash
            WHERE
              Action.entry_hash = :entry_hash
            ",
    )?;

    query_validation_receipts(
        stmt,
        named_params! {
            ":entry_hash": entry_hash
        },
    )
}

fn query_validation_receipts<P: Params>(
    mut stmt: Statement,
    params: P,
) -> StateQueryResult<Vec<ValidationReceiptSet>> {
    let db_result = stmt
        .query_and_then(params, |row| {
            let receipt = from_blob::<SignedValidationReceipt>(row.get("receipt")?)?;
            let op_hash: DhtOpHash = row.get("op_hash")?;
            let op_type: DhtOpType = row.get("op_type")?;
            let receipts_complete: Option<bool> = row.get("op_receipts_complete")?;

            Ok((
                receipt,
                op_hash,
                op_type,
                receipts_complete.unwrap_or(false),
            ))
        })?
        .collect::<StateQueryResult<Vec<_>>>()?;
    Ok(db_result
        .into_iter()
        .filter_map(
            |(receipt, op_hash, op_type, receipts_complete)| match op_type {
                DhtOpType::Chain(op_type) => Some((
                    op_hash,
                    op_type.to_string(),
                    receipts_complete,
                    ValidationReceiptInfo {
                        validation_status: receipt.receipt.validation_status,
                        validators: receipt.receipt.validators,
                    },
                )),
                _ => None,
            },
        )
        .fold(HashMap::new(), |mut acc, item| {
            acc.entry(item.0.clone())
                .or_insert_with(|| ValidationReceiptSet {
                    op_hash: item.0,
                    op_type: item.1.clone(),
                    receipts_complete: item.2,
                    receipts: Vec::new(),
                })
                .receipts
                .push(item.3);
            acc
        })
        .into_values()
        .collect())
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

        let op = DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(
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
        let op = DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(
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

    #[tokio::test(flavor = "multi_thread")]
    async fn validation_receipts_from_action() {
        let test_db = test_dht_db();
        let env = test_db.to_db();

        let keystore = test_keystore();

        let action = fixt!(Action);

        let action_hash = ActionHash::with_data_sync(&action);
        let op = DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(
            fixt!(Signature),
            action,
        ));
        let test_op_hash = op.as_hash().clone();
        env.write_async(move |txn| insert_op(txn, &op))
            .await
            .unwrap();

        let vr1 = fake_vr(&test_op_hash, &keystore).await;
        let vr2 = fake_vr(&test_op_hash, &keystore).await;

        env.write_async({
            let put_vr1 = vr1.clone();
            let put_vr2 = vr2.clone();

            move |txn| -> StateMutationResult<()> {
                add_if_unique(txn, put_vr1.clone())?;
                add_if_unique(txn, put_vr2.clone())?;
                Ok(())
            }
        })
        .await
        .unwrap();

        let receipt_sets = env
            .read_async(move |txn| validation_receipts_for_action(&txn, action_hash))
            .await
            .unwrap();

        check_receipt_sets(receipt_sets, test_op_hash, vr1, vr2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn validation_receipts_from_entry() {
        let test_db = test_dht_db();
        let env = test_db.to_db();

        let keystore = test_keystore();

        let entry = fixt!(Entry);

        let mut create_action = fixt!(Create);
        let entry_hash = EntryHash::with_data_sync(&entry);
        create_action.entry_hash = entry_hash.clone();
        let action = Action::Create(create_action);

        let op = DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(
            fixt!(Signature),
            action,
        ));
        let test_op_hash = op.as_hash().clone();
        env.write_async({
            let entry_hash = entry_hash.clone();
            move |txn| {
                insert_entry(txn, &entry_hash, &entry)?;
                insert_op(txn, &op)
            }
        })
        .await
        .unwrap();

        let vr1 = fake_vr(&test_op_hash, &keystore).await;
        let vr2 = fake_vr(&test_op_hash, &keystore).await;

        env.write_async({
            let put_vr1 = vr1.clone();
            let put_vr2 = vr2.clone();

            move |txn| -> StateMutationResult<()> {
                add_if_unique(txn, put_vr1.clone())?;
                add_if_unique(txn, put_vr2.clone())?;
                Ok(())
            }
        })
        .await
        .unwrap();

        let receipt_sets = env
            .read_async(move |txn| validation_receipts_for_entry(&txn, entry_hash))
            .await
            .unwrap();

        check_receipt_sets(receipt_sets, test_op_hash, vr1, vr2);
    }

    fn check_receipt_sets(
        receipt_sets: Vec<ValidationReceiptSet>,
        test_op_hash: DhtOpHash,
        vr1: SignedValidationReceipt,
        vr2: SignedValidationReceipt,
    ) {
        assert_eq!(receipt_sets.len(), 1);

        assert_eq!(test_op_hash, receipt_sets[0].op_hash);
        assert_eq!("RegisterAgentActivity", receipt_sets[0].op_type);

        let receipts_count = receipt_sets[0].receipts.len();
        assert_eq!(receipts_count, 2);

        assert_eq!(
            vr1.receipt.validation_status,
            receipt_sets[0].receipts[0].validation_status
        );
        assert_eq!(1, receipt_sets[0].receipts[0].validators.len());
        assert_eq!(
            vr2.receipt.validation_status,
            receipt_sets[0].receipts[1].validation_status
        );
        assert_eq!(1, receipt_sets[0].receipts[1].validators.len());
    }
}
