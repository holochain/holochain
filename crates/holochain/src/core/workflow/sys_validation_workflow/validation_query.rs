use holo_hash::DhtOpHash;
use holochain_sqlite::db::DbKindDht;
use holochain_state::query::prelude::*;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::DhtOpType;
use holochain_types::env::DbReadOnly;
use holochain_zome_types::Entry;
use holochain_zome_types::SignedHeader;

use crate::core::workflow::error::WorkflowResult;

/// Get all ops that need to sys or app validated in order.
/// - Sys validated or awaiting app dependencies.
/// - Ordered by type then timestamp (See [`DhtOpOrder`])
pub async fn get_ops_to_app_validate(
    env: &DbReadOnly<DbKindDht>,
) -> WorkflowResult<Vec<DhtOpHashed>> {
    get_ops_to_validate(env, false).await
}

/// Get all ops that need to sys or app validated in order.
/// - Pending or awaiting sys dependencies.
/// - Ordered by type then timestamp (See [`DhtOpOrder`])
pub async fn get_ops_to_sys_validate(
    env: &DbReadOnly<DbKindDht>,
) -> WorkflowResult<Vec<DhtOpHashed>> {
    get_ops_to_validate(env, true).await
}

async fn get_ops_to_validate(
    env: &DbReadOnly<DbKindDht>,
    system: bool,
) -> WorkflowResult<Vec<DhtOpHashed>> {
    let mut sql = "
        SELECT 
        Header.blob as header_blob,
        Entry.blob as entry_blob,
        DhtOp.type as dht_type,
        DhtOp.hash as dht_hash
        FROM Header
        JOIN
        DhtOp ON DhtOp.header_hash = Header.hash
        LEFT JOIN
        Entry ON Header.entry_hash = Entry.hash
        "
    .to_string();
    if system {
        sql.push_str(
            "
            WHERE
            (DhtOp.validation_status IS NULL OR DhtOp.validation_stage = 0)
            ",
        );
    } else {
        sql.push_str(
            "
            WHERE
            (DhtOp.validation_stage = 1 OR DhtOp.validation_stage = 2)
            ",
        );
    }
    sql.push_str(
        "
        ORDER BY 
        DhtOp.op_order ASC
        ",
    );
    env.async_reader(move |txn| {
        let mut stmt = txn.prepare(&sql)?;
        let r = stmt.query_and_then([], |row| {
            let header = from_blob::<SignedHeader>(row.get("header_blob")?)?;
            let op_type: DhtOpType = row.get("dht_type")?;
            let hash: DhtOpHash = row.get("dht_hash")?;
            let entry: Option<Vec<u8>> = row.get("entry_blob")?;
            let entry = match entry {
                Some(entry) => Some(from_blob::<Entry>(entry)?),
                None => None,
            };
            WorkflowResult::Ok(DhtOpHashed::with_pre_hashed(
                DhtOp::from_type(op_type, header, entry)?,
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
    use fixt::prelude::*;
    use holo_hash::HasHash;
    use holochain_sqlite::db::WriteManager;
    use holochain_sqlite::prelude::DatabaseResult;
    use holochain_state::prelude::*;
    use holochain_state::validation_db::ValidationLimboStatus;
    use holochain_types::dht_op::DhtOpHashed;
    use holochain_types::dht_op::OpOrder;
    use holochain_zome_types::fixt::*;
    use holochain_zome_types::ValidationStatus;

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
        let env = test_dht_env();
        let expected = test_data(&env.env().into());
        let r = get_ops_to_validate(&env.env().into(), true).await.unwrap();
        let mut r_sorted = r.clone();
        // Sorted by OpOrder
        r_sorted.sort_by_key(|d| {
            let op_type = d.as_content().get_type();
            let timestamp = d.as_content().header().timestamp();
            OpOrder::new(op_type, timestamp)
        });
        assert_eq!(r, r_sorted);
        for op in r {
            assert!(expected.results.iter().any(|i| *i == op));
        }
    }

    fn create_and_insert_op(env: &DbWrite<DbKindDht>, facts: Facts) -> DhtOpHashed {
        let state = DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(
            fixt!(Signature),
            fixt!(Header),
        ));

        env.conn()
            .unwrap()
            .with_commit_sync(|txn| {
                let hash = state.as_hash().clone();
                insert_op(txn, state.clone()).unwrap();
                if facts.has_validation_status {
                    set_validation_status(txn, hash.clone(), ValidationStatus::Valid).unwrap();
                }
                if facts.pending {
                    // No need to do anything because status and stage are null already.
                } else if facts.awaiting_sys_deps {
                    set_validation_stage(
                        txn,
                        hash,
                        ValidationLimboStatus::AwaitingSysDeps(fixt!(AnyDhtHash)),
                    )
                    .unwrap();
                }
                DatabaseResult::Ok(())
            })
            .unwrap();
        state
    }

    fn test_data(env: &DbWrite<DbKindDht>) -> Expected {
        let mut results = Vec::new();
        // We **do** expect any of these in the results:
        let facts = Facts {
            pending: true,
            awaiting_sys_deps: false,
            has_validation_status: false,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(env, facts);
            results.push(op);
        }

        let facts = Facts {
            pending: false,
            awaiting_sys_deps: true,
            has_validation_status: false,
        };
        for _ in 0..20 {
            let op = create_and_insert_op(env, facts);
            results.push(op);
        }

        // We **don't** expect any of these in the results:
        let facts = Facts {
            pending: false,
            awaiting_sys_deps: false,
            has_validation_status: true,
        };
        for _ in 0..20 {
            create_and_insert_op(env, facts);
        }

        Expected { results }
    }
}
