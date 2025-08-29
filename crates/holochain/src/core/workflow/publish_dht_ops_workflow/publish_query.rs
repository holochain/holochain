use crate::core::workflow::WorkflowResult;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_sqlite::db::DbKindAuthored;
use holochain_sqlite::prelude::ReadAccess;
use holochain_state::prelude::*;
use holochain_state::query::map_sql_dht_op;
use rusqlite::named_params;
use rusqlite::Transaction;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

/// Get all dht ops on an agents chain that need to be published.
/// - Don't publish private entries.
/// - Only get ops that haven't been published within the minimum publish interval
/// - Only get ops that have less than the RECEIPT_BUNDLE_SIZE
pub async fn get_ops_to_publish<AuthorDb>(
    agent: AgentPubKey,
    db: &AuthorDb,
    min_publish_interval: Duration,
) -> WorkflowResult<Vec<(OpBasis, DhtOpHash, DhtOp)>>
where
    AuthorDb: ReadAccess<DbKindAuthored>,
{
    let recency_threshold = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|epoch| epoch.checked_sub(min_publish_interval))
        .map(|t| t.as_secs())
        .unwrap_or(0);

    db.read_async(move |txn| {
        let mut stmt = txn.prepare(
            "
            SELECT
            Action.blob as action_blob,
            Action.author as author,
            LENGTH(Action.blob) AS action_size,
            CASE
              WHEN DhtOp.type IN ('StoreEntry', 'StoreRecord') THEN LENGTH(Entry.blob)
              ELSE 0
            END AS entry_size,
            Entry.blob as entry_blob,
            DhtOp.type as dht_type,
            DhtOp.hash as dht_hash,
            DhtOp.op_order as op_order
            FROM Action
            JOIN
            DhtOp ON DhtOp.action_hash = Action.hash
            LEFT JOIN
            Entry ON Action.entry_hash = Entry.hash
            WHERE
            Action.author = :author
            AND
            (DhtOp.type != :store_entry OR Action.private_entry = 0)
            AND
            DhtOp.withhold_publish IS NULL
            AND
            (DhtOp.last_publish_time IS NULL OR DhtOp.last_publish_time <= :recency_threshold)
            AND
            DhtOp.receipts_complete IS NULL

            UNION
            ALL

            SELECT
            Warrant.blob as action_blob,
            Warrant.author as author,
            LENGTH(Warrant.blob) AS action_size,
            0 AS entry_size,
            NULL as entry_blob,
            DhtOp.type as dht_type,
            DhtOp.hash as dht_hash,
            DhtOp.op_order as op_order
            FROM Warrant
            JOIN
            DhtOp ON DhtOp.action_hash = Warrant.hash
            WHERE
            Warrant.author = :author
            AND
            DhtOp.withhold_publish IS NULL
            AND
            (DhtOp.last_publish_time IS NULL OR DhtOp.last_publish_time <= :recency_threshold)
            AND
            DhtOp.receipts_complete IS NULL

            ORDER BY op_order
            ",
        )?;
        let r = stmt.query_and_then(
            named_params! {
                ":author": agent,
                ":recency_threshold": recency_threshold,
                ":store_entry": ChainOpType::StoreEntry,
            },
            |row| {
                let op = map_sql_dht_op(false, "dht_type", row)?;
                let op_hash: DhtOpHash = row.get("dht_hash")?;
                let basis = op.dht_basis();
                WorkflowResult::Ok((basis, op_hash, op))
            },
        )?;
        WorkflowResult::Ok(r.collect::<Result<Vec<_>, _>>())
    })
    .await?
}

/// Get the number of ops that might need to publish again in the future.
pub fn num_still_needing_publish(txn: &Transaction, agent: AgentPubKey) -> WorkflowResult<usize> {
    let count = txn.query_row(
        "
        SELECT
        (
          SELECT COUNT(DhtOp.rowid)
          FROM Action
          JOIN DhtOp ON DhtOp.action_hash = Action.hash
          WHERE
            Action.author = :author
            AND DhtOp.withhold_publish IS NULL
            AND (DhtOp.type != :store_entry OR Action.private_entry = 0)
            AND DhtOp.receipts_complete IS NULL
        )
        +
        (
          SELECT COUNT(DhtOp.rowid)
          FROM Warrant
          JOIN DhtOp ON DhtOp.action_hash = Warrant.hash
          WHERE
            Warrant.author = :author
            AND DhtOp.withhold_publish IS NULL
            AND DhtOp.receipts_complete IS NULL
        )
        AS num_ops
        ",
        named_params! {
            ":author": agent,
            ":store_entry": ChainOpType::StoreEntry,
        },
        |row| row.get("num_ops"),
    )?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::fixt::prelude::*;
    use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator};
    use holo_hash::EntryHash;
    use holo_hash::HasHash;
    use holochain_conductor_api::conductor::ConductorTuningParams;
    use holochain_sqlite::db::DbWrite;
    use holochain_sqlite::prelude::DatabaseResult;

    #[derive(Debug, Clone, Copy)]
    struct Facts {
        private: bool,
        within_min_period: bool,
        has_required_receipts: bool,
        is_this_agent: bool,
        store_entry: bool,
        withold_publish: bool,
    }

    struct Consistent {
        this_agent: AgentPubKey,
    }

    struct Expected {
        agent: AgentPubKey,
        results: Vec<DhtOpHashed>,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn publish_query() {
        holochain_trace::test_run();

        let agent = fixt!(AgentPubKey);
        let db = test_authored_db();
        let expected = test_data(&db.to_db(), agent.clone()).await;
        let r = get_ops_to_publish(
            expected.agent.clone(),
            &db.to_db(),
            ConductorTuningParams::default().min_publish_interval(),
        )
        .await
        .unwrap();
        assert_eq!(
            r.into_iter().map(|t| t.1).collect::<Vec<_>>(),
            expected
                .results
                .iter()
                .cloned()
                .map(|op| op.into_inner().1)
                .collect::<Vec<_>>(),
        );

        let num_to_publish = db
            .to_db()
            .read_async(|txn| num_still_needing_publish(txn, agent))
            .await
            .unwrap();

        // +1 because `get_ops_to_publish` will filter on `last_publish_time` where `num_still_needing_publish` should
        // not because those ops may need publishing again in the future if we don't get enough validation receipts.
        assert_eq!(expected.results.len() + 1, num_to_publish);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn publish_query_includes_warrants() {
        holochain_trace::test_run();

        let agent = fixt!(AgentPubKey);
        let db = test_authored_db();

        // Insert one chain op and one warrant op into database.
        let chain_op = create_and_insert_op(
            &db,
            Facts {
                private: false,
                within_min_period: false,
                has_required_receipts: false,
                is_this_agent: true,
                store_entry: false,
                withold_publish: false,
            },
            &Consistent {
                this_agent: agent.clone(),
            },
        )
        .await
        .content;
        let warrant_op = insert_invalid_op_warrant_op(&db, &agent).content;

        let ops_to_publish = get_ops_to_publish(
            agent.clone(),
            &db.to_db(),
            ConductorTuningParams::default().min_publish_interval(),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|(_, _, op)| op)
        .collect::<Vec<_>>();
        assert_eq!(ops_to_publish, vec![chain_op, warrant_op]);

        let num_to_publish = db
            .to_db()
            .test_read(|txn| num_still_needing_publish(txn, agent).unwrap());
        assert_eq!(num_to_publish, 2);
    }

    fn insert_invalid_chain_op_warrant_op(
        db: &DbWrite<DbKindAuthored>,
        agent: &AgentPubKey,
    ) -> DhtOpHashed {
        let invalid_op_warrant = SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author: fixt!(AgentPubKey),
                    action: (fixt!(ActionHash), fixt!(Signature)),
                    validation_type: ValidationType::App,
                    chain_op_type: ChainOpType::RegisterAddLink,
                }),
                agent.clone(),
                Timestamp::now(),
                fixt!(AgentPubKey),
            ),
            fixt!(Signature),
        );
        let invalid_op_warrant = DhtOpHashed::from_content_sync(DhtOp::WarrantOp(Box::new(
            WarrantOp::from(invalid_op_warrant),
        )));
        let warrant_op = invalid_op_warrant.clone();
        db.test_write({
            move |txn| {
                insert_op_authored(txn, &invalid_op_warrant).unwrap();
                set_last_publish_time(
                    txn,
                    &invalid_op_warrant.hash,
                    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
                        - ConductorTuningParams::default().min_publish_interval(),
                )
                .unwrap();
                set_receipts_complete(txn, &invalid_op_warrant.hash, false).unwrap();
            }
        });
        warrant_op
    }

    async fn create_and_insert_op(
        db: &DbWrite<DbKindAuthored>,
        facts: Facts,
        consistent_data: &Consistent,
    ) -> DhtOpHashed {
        let this_agent = consistent_data.this_agent.clone();
        let entry = Entry::App(fixt!(AppEntryBytes));
        let mut action = fixt!(Create);
        action.author = this_agent.clone();
        action.entry_hash = EntryHash::with_data_sync(&entry);
        if facts.private {
            // - Private: true
            action.entry_type = AppEntryDefFixturator::new(EntryVisibility::Private)
                .map(EntryType::App)
                .next()
                .unwrap();
        } else {
            // - Private: false
            action.entry_type = AppEntryDefFixturator::new(EntryVisibility::Public)
                .map(EntryType::App)
                .next()
                .unwrap();
        }

        // - IsThisAgent: false.
        if !facts.is_this_agent {
            action.author = fixt!(AgentPubKey);
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let last_publish = if facts.within_min_period {
            // - WithinMinPeriod: true.
            now
        } else {
            // - WithinMinPeriod: false.
            now - ConductorTuningParams::default().min_publish_interval()
        };

        let state = if facts.store_entry {
            DhtOpHashed::from_content_sync(ChainOp::StoreEntry(
                fixt!(Signature),
                NewEntryAction::Create(action.clone()),
                entry.clone(),
            ))
        } else {
            DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
                fixt!(Signature),
                Action::Create(action.clone()),
                entry.clone().into(),
            ))
        };

        db.write_async({
            let query_state = state.clone();

            move |txn| -> DatabaseResult<()> {
                let hash = query_state.as_hash().clone();
                insert_op_authored(txn, &query_state).unwrap();
                set_last_publish_time(txn, &hash, last_publish).unwrap();
                set_receipts_complete(txn, &hash, facts.has_required_receipts).unwrap();
                if facts.withold_publish {
                    set_withhold_publish(txn, &hash).unwrap();
                }
                Ok(())
            }
        })
        .await
        .unwrap();
        state
    }

    async fn test_data(db: &DbWrite<DbKindAuthored>, agent: AgentPubKey) -> Expected {
        let mut results = Vec::new();
        let cd = Consistent { this_agent: agent };
        // We **do** expect any of these in the results:
        // - Private: false.
        // - WithinMinPeriod: false.
        // - HasRequireReceipts: false.
        // - IsThisAgent: true.
        // - StoreEntry: true.
        // - WitholdPublish: false.
        let facts = Facts {
            private: false,
            within_min_period: false,
            has_required_receipts: false,
            is_this_agent: true,
            store_entry: true,
            withold_publish: false,
        };
        let op = create_and_insert_op(db, facts, &cd).await;
        results.push(op);

        // All facts are the same unless stated:

        // - Private: true.
        // - StoreEntry: false.
        let mut f = facts;
        f.private = true;
        f.store_entry = false;
        let op = create_and_insert_op(db, f, &cd).await;
        results.push(op);

        // We **don't** expect any of these in the results:
        // - Private: true.
        let mut f = facts;
        f.private = true;
        create_and_insert_op(db, f, &cd).await;

        // - WithinMinPeriod: true.
        let mut f = facts;
        f.within_min_period = true;
        create_and_insert_op(db, f, &cd).await;

        // - HasRequireReceipts: true.
        let mut f = facts;
        f.has_required_receipts = true;
        create_and_insert_op(db, f, &cd).await;

        // - IsThisAgent: false.
        let mut f = facts;
        f.is_this_agent = false;
        create_and_insert_op(db, f, &cd).await;

        // - WitholdPublish: true.
        let mut f = facts;
        f.withold_publish = true;
        create_and_insert_op(db, f, &cd).await;

        Expected {
            agent: cd.this_agent.clone(),
            results,
        }
    }
}
