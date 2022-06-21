use holo_hash::DhtOpHash;
use holochain_sqlite::db::DbKindDht;
use holochain_state::query::prelude::*;
use holochain_types::db::DbRead;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::DhtOpType;
use holochain_zome_types::Entry;
use holochain_zome_types::SignedAction;

pub use crate::core::validation::DhtOpOrder;
use crate::core::workflow::error::WorkflowResult;

/// Get all ops that need to sys or app validated in order.
/// - Sys validated or awaiting app dependencies.
/// - Ordered by type then timestamp (See [`DhtOpOrder`])
pub async fn get_ops_to_app_validate(db: &DbRead<DbKindDht>) -> WorkflowResult<Vec<DhtOpHashed>> {
    get_ops_to_validate(db, false).await
}

/// Get all ops that need to sys or app validated in order.
/// - Pending or awaiting sys dependencies.
/// - Ordered by type then timestamp (See [`DhtOpOrder`])
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
        // TODO: bump stages to match new rate limiting workflow
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
    db.async_reader(move |txn| {
        let mut stmt = txn.prepare(&sql)?;
        let r = stmt.query_and_then([], |row| {
            let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
            let op_type: DhtOpType = row.get("dht_type")?;
            let hash: DhtOpHash = row.get("dht_hash")?;
            let entry: Option<Vec<u8>> = row.get("entry_blob")?;
            let entry = match entry {
                Some(entry) => Some(from_blob::<Entry>(entry)?),
                None => None,
            };
            WorkflowResult::Ok(DhtOpHashed::with_pre_hashed(
                DhtOp::from_type(op_type, action, entry)?,
                hash,
            ))
        })?;
        let r = r.collect();
        WorkflowResult::Ok(r)
    })
    .await?
}

#[cfg(test)]
mod tests {
    use arbitrary::Arbitrary;
    use arbitrary::Unstructured;
    use fixt::prelude::*;
    use holo_hash::HasHash;
    use holo_hash::HashableContentExtSync;
    use holochain_sqlite::db::WriteManager;
    use holochain_sqlite::prelude::DatabaseResult;
    use holochain_state::prelude::*;
    use holochain_state::validation_db::ValidationLimboStatus;
    use holochain_types::dht_op::DhtOpHashed;
    use holochain_types::dht_op::OpOrder;
    use holochain_zome_types::fixt::*;
    use holochain_zome_types::Action;
    use holochain_zome_types::Signature;
    use holochain_zome_types::ValidationStatus;
    use holochain_zome_types::NOISE;

    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct Facts {
        pending: bool,
        awaiting_sys_deps: bool,
        has_validation_status: bool,
    }

    struct Expected {
        results: Vec<DhtOpHashed>,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sys_validation_query() {
        observability::test_run().ok();
        let db = test_dht_db();
        let expected = test_data(&db.to_db().into());
        let r = get_ops_to_validate(&db.to_db().into(), true).await.unwrap();
        let mut r_sorted = r.clone();
        // Sorted by OpOrder
        r_sorted.sort_by_key(|d| {
            let op_type = d.as_content().get_type();
            let timestamp = d.as_content().action().timestamp();
            OpOrder::new(op_type, timestamp)
        });
        assert_eq!(r, r_sorted);
        for op in r {
            assert!(expected.results.iter().any(|i| *i == op));
        }
    }

    fn create_and_insert_op(db: &DbWrite<DbKindDht>, facts: Facts) -> DhtOpHashed {
        let state = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Action),
        ));

        db.conn()
            .unwrap()
            .with_commit_sync(|txn| {
                let hash = state.as_hash().clone();
                insert_op(txn, &state).unwrap();
                if facts.has_validation_status {
                    set_validation_status(txn, &hash, ValidationStatus::Valid).unwrap();
                }
                if facts.pending {
                    // No need to do anything because status and stage are null already.
                } else if facts.awaiting_sys_deps {
                    set_validation_stage(
                        txn,
                        &hash,
                        ValidationLimboStatus::AwaitingSysDeps(fixt!(AnyDhtHash)),
                    )
                    .unwrap();
                }
                txn.execute("UPDATE DhtOp SET num_validation_attempts = 0", [])?;
                DatabaseResult::Ok(())
            })
            .unwrap();
        state
    }

    fn test_data(db: &DbWrite<DbKindDht>) -> Expected {
        let mut results = Vec::new();
        // We **do** expect any of these in the results:
        let facts = Facts {
            pending: true,
            awaiting_sys_deps: false,
            has_validation_status: false,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(db, facts);
            results.push(op);
        }

        let facts = Facts {
            pending: false,
            awaiting_sys_deps: true,
            has_validation_status: false,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(db, facts);
            results.push(op);
        }

        // We **don't** expect any of these in the results:
        let facts = Facts {
            pending: false,
            awaiting_sys_deps: false,
            has_validation_status: true,
        };
        for _ in 0..20 {
            create_and_insert_op(db, facts);
        }

        Expected { results }
    }

    #[tokio::test(flavor = "multi_thread")]
    /// Make sure both workflows can't pull in the same ops.
    async fn workflows_are_exclusive() {
        observability::test_run().ok();
        let mut u = Unstructured::new(&NOISE);

        let db = test_dht_db();
        let db = db.to_db();
        let op = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            Signature::arbitrary(&mut u).unwrap(),
            Action::arbitrary(&mut u).unwrap(),
        ));

        db.async_commit(move |txn| {
            insert_op(txn, &op)?;
            StateMutationResult::Ok(())
        })
        .await
        .unwrap();

        let read: DbRead<_> = db.clone().into();
        let mut read_ops = std::collections::HashSet::new();
        let hashes: Vec<_> = get_ops_to_app_validate(&read)
            .await
            .unwrap()
            .into_iter()
            .map(|op| op.to_hash())
            .collect();
        for h in &hashes {
            read_ops.insert(h.clone());
        }
        let hashes: Vec<_> = get_ops_to_sys_validate(&read)
            .await
            .unwrap()
            .into_iter()
            .map(|op| op.to_hash())
            .collect();
        for h in &hashes {
            if !read_ops.insert(h.clone()) {
                panic!("Duplicate op");
            }
        }
    }
}
