use holochain_sqlite::db::DbKindDht;
use holochain_state::prelude::*;

use crate::core::workflow::WorkflowResult;

/// Get all ops that need to sys or app validated in order.
/// - Sys validated or awaiting app dependencies.
/// - Ordered by type then timestamp (See [`OpOrder`])
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn get_ops_to_app_validate(db: &DbRead<DbKindDht>) -> WorkflowResult<Vec<DhtOpHashed>> {
    get_ops_to_validate(db, false).await
}

/// Get all ops that need to sys or app validated in order.
/// - Pending or awaiting sys dependencies.
/// - Ordered by type then timestamp (See [`OpOrder`])
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn get_ops_to_sys_validate(db: &DbRead<DbKindDht>) -> WorkflowResult<Vec<DhtOpHashed>> {
    get_ops_to_validate(db, true).await
}

async fn get_ops_to_validate(
    db: &DbRead<DbKindDht>,
    system: bool,
) -> WorkflowResult<Vec<DhtOpHashed>> {
    let mut sql = "
        SELECT
        Action.blob as action_blob,
        Action.author as author,
        Entry.blob as entry_blob,
        DhtOp.type as dht_type,
        DhtOp.hash as dht_hash
        FROM DhtOp
        JOIN
        Action ON DhtOp.action_hash = Action.hash
        LEFT JOIN
        Entry ON Action.entry_hash = Entry.hash
        "
    .to_string();
    if system {
        sql.push_str(
            "
            WHERE
            DhtOp.when_integrated IS NULL
            AND DhtOp.validation_status IS NULL
            AND (
                DhtOp.validation_stage IS NULL
                OR DhtOp.validation_stage = 0
            )
            ",
        );
    } else {
        sql.push_str(
            "
            WHERE
            DhtOp.when_integrated IS NULL
            AND DhtOp.validation_status IS NULL
            AND (
                DhtOp.validation_stage = 1
                OR DhtOp.validation_stage = 2
            )
            ",
        );
    }
    // TODO: There is a very unlikely chance that 10000 ops
    // could all fail to validate and prevent validation from
    // moving on but this is not easy to overcome.
    // Once we impl abandoned this won't happen anyway.
    sql.push_str(
        "
        ORDER BY
        DhtOp.num_validation_attempts ASC,
        DhtOp.op_order ASC
        LIMIT 10000
        ",
    );
    db.read_async(move |txn| {
        let mut stmt = txn.prepare(&sql)?;
        let r = stmt.query_and_then([], |row| {
            let op = WorkflowResult::Ok(holochain_state::query::map_sql_dht_op(
                true, "dht_type", row,
            )?)?;
            let hash = row.get("dht_hash")?;
            Ok(DhtOpHashed::with_pre_hashed(op, hash))
        })?;
        let r = r.collect();
        WorkflowResult::Ok(r)
    })
    .await?
}

#[cfg(test)]
mod tests {
    use ::fixt::prelude::*;
    use holo_hash::HasHash;
    use holochain_sqlite::prelude::DatabaseResult;
    use holochain_state::prelude::*;
    use holochain_state::validation_db::ValidationStage;
    use std::collections::HashSet;

    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct Facts {
        pending: bool,
        awaiting_sys_deps: bool,
        sys_validated: bool,
        awaiting_app_deps: bool,
        awaiting_integration: bool,
        has_validation_status: bool,
        num_attempts: usize,
    }

    struct Expected {
        to_sys_validate: Vec<DhtOpHashed>,
        to_app_validate: Vec<DhtOpHashed>,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sys_validation_query() {
        holochain_trace::test_run();
        let db = test_dht_db();
        let expected = create_test_data(&db.to_db()).await;
        let ops = get_ops_to_sys_validate(&db.to_db().into()).await.unwrap();

        assert_sorted_by_op_order(&ops).await;
        assert_sorted_by_validation_attempts(&db.to_db(), &ops).await;

        // Check all the expected ops were returned
        for op in ops {
            assert!(expected.to_sys_validate.iter().any(|i| *i == op));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn app_validation_query() {
        holochain_trace::test_run();
        let db = test_dht_db();
        let expected = create_test_data(&db.to_db()).await;
        let ops = get_ops_to_app_validate(&db.to_db().into()).await.unwrap();

        assert_sorted_by_op_order(&ops).await;
        assert_sorted_by_validation_attempts(&db.to_db(), &ops).await;

        // Check all the expected ops were returned
        for op in ops {
            assert!(expected.to_app_validate.iter().any(|i| *i == op));
        }
    }

    /// Make sure both workflows can't pull in the same ops.
    #[tokio::test(flavor = "multi_thread")]
    async fn workflows_are_exclusive() {
        holochain_trace::test_run();
        let db = test_dht_db();
        create_test_data(&db.to_db()).await;
        let app_validation_ops = get_ops_to_app_validate(&db.to_db().into()).await.unwrap();
        let sys_validation_ops = get_ops_to_sys_validate(&db.to_db().into()).await.unwrap();

        let app_hashes = app_validation_ops
            .into_iter()
            .map(|o| o.hash)
            .collect::<HashSet<_>>();
        let sys_hashes = sys_validation_ops
            .into_iter()
            .map(|o| o.hash)
            .collect::<HashSet<_>>();

        let overlap = app_hashes.intersection(&sys_hashes).collect::<HashSet<_>>();
        assert!(overlap.is_empty());
    }

    async fn create_test_data(db: &DbWrite<DbKindDht>) -> Expected {
        let mut to_sys_validate = Vec::with_capacity(40);
        let mut to_app_validate = Vec::with_capacity(40);

        // We **do** expect any of these in the sys validation results but **do not** expect them in the app validation results:
        let facts = Facts {
            pending: true, // Should appear in sys validation if no validation stage is set
            awaiting_sys_deps: false,
            sys_validated: false,
            awaiting_app_deps: false,
            awaiting_integration: false,
            has_validation_status: false,
            num_attempts: 1,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(db, facts).await;
            to_sys_validate.push(op);
        }

        let facts = Facts {
            pending: false,
            awaiting_sys_deps: true, // Should appear in sys validation and be retried if awaiting deps
            sys_validated: false,
            awaiting_app_deps: false,
            awaiting_integration: false,
            has_validation_status: false,
            num_attempts: 0,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(db, facts).await;
            to_sys_validate.push(op);
        }

        // We **don't** expect any of these in the sys validation results but **do** expect them in app the validation results:
        let facts = Facts {
            pending: false,
            awaiting_sys_deps: false,
            sys_validated: true, // Should appear in app validation if sys validation has already been done
            awaiting_app_deps: false,
            awaiting_integration: false,
            has_validation_status: true,
            num_attempts: 6,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(db, facts).await;
            to_app_validate.push(op);
        }

        let facts = Facts {
            pending: false,
            awaiting_sys_deps: false,
            sys_validated: false,
            awaiting_app_deps: true, // Should appear in app validation and be retried if awaiting deps
            awaiting_integration: false,
            has_validation_status: true,
            num_attempts: 2,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(db, facts).await;
            to_app_validate.push(op);
        }

        // We **don't** expect any of these to appear in either sys validation or app validation
        let facts = Facts {
            pending: false,
            awaiting_sys_deps: false,
            sys_validated: false,
            awaiting_app_deps: true,
            awaiting_integration: true, // Should not appear once sys and app validation has finished and waiting for integration
            has_validation_status: true,
            num_attempts: 5,
        };
        for _ in 0..20 {
            create_and_insert_op(db, facts).await;
        }

        let facts = Facts {
            pending: false,
            awaiting_sys_deps: false,
            sys_validated: false,
            awaiting_app_deps: false,
            awaiting_integration: false,
            has_validation_status: true, // Should not appear if there is already a validation outcome
            num_attempts: 10,
        };
        for _ in 0..20 {
            create_and_insert_op(db, facts).await;
        }

        Expected {
            to_sys_validate,
            to_app_validate,
        }
    }

    async fn create_and_insert_op(db: &DbWrite<DbKindDht>, facts: Facts) -> DhtOpHashed {
        let state = DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));

        db.write_async({
            let query_state = state.clone();

            move |txn| -> DatabaseResult<()> {
                let hash = query_state.as_hash().clone();
                // XXX: This is inserted into the DHT DB, so `transfer_data` here should be Some
                insert_op(txn, &query_state, None).unwrap();
                if facts.has_validation_status {
                    set_validation_status(txn, &hash, ValidationStatus::Valid).unwrap();
                }
                if facts.pending {
                    // No need to do anything because status and stage are null already.
                } else if facts.awaiting_sys_deps {
                    set_validation_stage(txn, &hash, ValidationStage::AwaitingSysDeps).unwrap();
                } else if facts.sys_validated {
                    set_validation_stage(txn, &hash, ValidationStage::SysValidated).unwrap();
                } else if facts.awaiting_app_deps {
                    set_validation_stage(txn, &hash, ValidationStage::AwaitingAppDeps).unwrap();
                } else if facts.awaiting_integration {
                    set_validation_stage(txn, &hash, ValidationStage::AwaitingIntegration).unwrap();
                }
                txn.execute(
                    "UPDATE DhtOp SET num_validation_attempts = :num_attempts",
                    named_params! {
                        ":num_attempts": facts.num_attempts,
                    },
                )?;
                Ok(())
            }
        })
        .await
        .unwrap();
        state
    }

    async fn assert_sorted_by_op_order(ops: &Vec<DhtOpHashed>) {
        let mut ops_sorted = ops.clone();
        ops_sorted.sort_by_key(|d| {
            let op_type = d.get_type();
            let timestamp = d.timestamp();
            OpOrder::new(op_type, timestamp)
        });
        assert_eq!(ops, &ops_sorted);
    }

    async fn assert_sorted_by_validation_attempts(db: &DbWrite<DbKindDht>, ops: &Vec<DhtOpHashed>) {
        assert!(
            get_num_validation_attempts(db, ops.iter().map(|op| op.hash.clone()).collect())
                .await
                .windows(2)
                .all(|w| { w[0] <= w[1] })
        );
    }

    async fn get_num_validation_attempts(
        db: &DbWrite<DbKindDht>,
        hashes: Vec<DhtOpHash>,
    ) -> Vec<usize> {
        db.read_async(|txn| -> DatabaseResult<Vec<usize>> {
            hashes
                .into_iter()
                .map(|h| -> DatabaseResult<usize> {
                    Ok(txn.query_row(
                        "SELECT num_validation_attempts FROM DhtOp WHERE hash = :op_hash",
                        named_params! {
                            ":op_hash": h
                        },
                        |r| r.get(0),
                    )?)
                })
                .collect()
        })
        .await
        .unwrap()
    }
}
