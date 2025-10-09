//! Query functions for warrants

use crate::prelude::named_params;
use crate::query::map_sql_dht_op;
use holo_hash::ActionHash;
use holochain_sqlite::db::{DbKindDht, DbRead};
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::sql::sql_cell::warrant::SELECT_BY_TYPE_AND_WARRANTEE;
use holochain_types::dht_op::DhtOp;
use holochain_types::prelude::WarrantOpType;
use holochain_zome_types::prelude::{ValidationStatus, WarrantProof};
use holochain_zome_types::warrant::ChainIntegrityWarrant;

/// Check whether a given action has any invalid data chain integrity warrants issued against it.
///
/// This looks for all warrants issued against the identified author, with a `ChainIntegrity` proof.
/// The list of warrants is then checked to see if any of them warrant the given action as invalid.
///
/// Warrants are considered whether they are pending validation or valid, but rejected or abandoned
/// warrants are ignored. This is because a warrant pending validation may be in the process of
/// being validated when this function is used.
pub async fn is_action_warranted_as_invalid(
    dht_db: &DbRead<DbKindDht>,
    action_hash: ActionHash,
    action_author: holo_hash::AgentPubKey,
) -> DatabaseResult<bool> {
    dht_db
        .read_async(move |txn| -> DatabaseResult<bool> {
            // Select all warrants issued against the action author
            // that are of type ChainIntegrityWarrant
            let mut stmt = txn.prepare_cached(
                SELECT_BY_TYPE_AND_WARRANTEE,
            )?;

            // Query and map each result to a DhtOp
            let result = stmt.query_and_then(
                named_params! {
                    ":author": action_author,
                    ":warrant_type": WarrantOpType::ChainIntegrityWarrant,
                    ":status_valid": ValidationStatus::Valid,
                },
                |row| map_sql_dht_op(false, "dht_type", row),
            )?;

            // Check if any of the warrants are for invalid data, specifically for the given action_hash
            let matched_warrant = result.into_iter().any(|result| match result {
                Ok(DhtOp::WarrantOp(warrant_op)) => match &warrant_op.proof {
                    WarrantProof::ChainIntegrity(chain_integrity) => matches!(chain_integrity, ChainIntegrityWarrant::InvalidChainOp { action, .. } if action.0 == action_hash),
                },
                Ok(_) => false,
                Err(e) => {
                    tracing::error!("Error reading warrant op: {:?}", e);
                    false
                }
            });

            Ok(matched_warrant)
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutations::insert_op_dht;
    use crate::prelude::{set_validation_status, SignatureFixturator};
    use crate::prelude::{test_dht_db, Action, RecordEntry};
    use fixt::fixt;
    use holo_hash::fixt::ActionHashFixturator;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holo_hash::fixt::DnaHashFixturator;
    use holo_hash::{AgentPubKey, HashableContentExtSync};
    use holochain_timestamp::Timestamp;
    use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
    use holochain_types::warrant::WarrantOp;
    use holochain_zome_types::action::Dna;
    use holochain_zome_types::op::ChainOpType;
    use holochain_zome_types::prelude::{
        ChainIntegrityWarrant, SignedWarrant, Warrant, WarrantProof,
    };

    #[tokio::test]
    async fn no_such_op() {
        let db = test_dht_db();

        assert!(
            !is_action_warranted_as_invalid(&db, fixt!(ActionHash), fixt!(AgentPubKey))
                .await
                .unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn no_warrants_for_op() {
        let db = test_dht_db();

        let action_author = fixt!(AgentPubKey);
        let chain_op = DhtOp::ChainOp(Box::new(ChainOp::StoreRecord(
            fixt!(Signature),
            Action::Dna(Dna {
                author: action_author.clone(),
                timestamp: Timestamp::now(),
                hash: fixt!(DnaHash),
            }),
            RecordEntry::NA,
        )));
        let chain_op_hashed = DhtOpHashed::from_content_sync(chain_op);
        let action_hash = chain_op_hashed.as_chain_op().unwrap().action().to_hash();

        db.test_write(move |txn| {
            insert_op_dht(txn, &chain_op_hashed, 0, None).unwrap();
        });

        assert!(
            !is_action_warranted_as_invalid(&db, action_hash, action_author,)
                .await
                .unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn op_warranted() {
        let db = test_dht_db();

        let warranted_action_hash = fixt!(ActionHash);
        let warranted_action_author = fixt!(AgentPubKey);
        let warrant_op_hashed = create_test_warrant_op_hashed(
            warranted_action_hash.clone(),
            warranted_action_author.clone(),
        );
        db.test_write(move |txn| {
            insert_op_dht(txn, &warrant_op_hashed, 0, None).unwrap();
        });

        assert!(is_action_warranted_as_invalid(
            &db,
            warranted_action_hash,
            warranted_action_author
        )
        .await
        .unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ignore_rejected_warrant() {
        let db = test_dht_db();

        let warranted_action_hash = fixt!(ActionHash);
        let warranted_action_author = fixt!(AgentPubKey);
        let warrant_op_hashed = create_test_warrant_op_hashed(
            warranted_action_hash.clone(),
            warranted_action_author.clone(),
        );
        db.test_write(move |txn| {
            insert_op_dht(txn, &warrant_op_hashed, 0, None).unwrap();
            set_validation_status(txn, &warrant_op_hashed.hash, ValidationStatus::Rejected)
                .unwrap();
        });

        assert!(!is_action_warranted_as_invalid(
            &db,
            warranted_action_hash,
            warranted_action_author
        )
        .await
        .unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ignore_abandoned_warrant() {
        let db = test_dht_db();

        let warranted_action_hash = fixt!(ActionHash);
        let warranted_action_author = fixt!(AgentPubKey);
        let warrant_op_hashed = create_test_warrant_op_hashed(
            warranted_action_hash.clone(),
            warranted_action_author.clone(),
        );
        db.test_write(move |txn| {
            insert_op_dht(txn, &warrant_op_hashed, 0, None).unwrap();
            set_validation_status(txn, &warrant_op_hashed.hash, ValidationStatus::Abandoned)
                .unwrap();
        });

        assert!(!is_action_warranted_as_invalid(
            &db,
            warranted_action_hash,
            warranted_action_author
        )
        .await
        .unwrap());
    }

    fn create_test_warrant_op_hashed(
        warranted_action_hash: ActionHash,
        warranted_action_author: AgentPubKey,
    ) -> DhtOpHashed {
        let warrant = SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author: warranted_action_author.clone(),
                    action: (warranted_action_hash.clone(), fixt!(Signature)),
                    chain_op_type: ChainOpType::RegisterAddLink,
                }),
                fixt!(AgentPubKey),
                Timestamp::now(),
                warranted_action_author.clone(),
            ),
            fixt!(Signature),
        );
        let warrant_op = DhtOp::WarrantOp(Box::new(WarrantOp::from(warrant)));
        DhtOpHashed::from_content_sync(warrant_op)
    }
}
