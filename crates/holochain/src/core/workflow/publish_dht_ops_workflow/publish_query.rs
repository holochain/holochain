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
            DhtOp.last_publish_time IS NULL

            ORDER BY op_order, dht_hash
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
            AND DhtOp.last_publish_time IS NULL
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
    #[cfg(feature = "unstable-warrants")]
    use holo_hash::fixt::ActionHashFixturator;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holo_hash::EntryHash;
    use holo_hash::HasHash;
    use holochain_conductor_api::conductor::ConductorTuningParams;
    use holochain_sqlite::db::DbWrite;

    #[derive(Debug, Clone, Copy)]
    struct Facts {
        private: bool,
        within_min_period: bool,
        has_required_receipts: bool,
        store_entry: bool,
        withold_publish: bool,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_with_same_agent() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let agent_clone = agent.clone();
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Initially num_to_publish and length of get_ops_to_publish should be 0.
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 0);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 0);

        // Insert a chain op into the DB.
        let facts = Facts {
            has_required_receipts: false,
            private: false,
            store_entry: false,
            within_min_period: false,
            withold_publish: false,
        };
        let _ = create_and_insert_chain_op(&db.to_db(), &agent, facts);

        // Should both be 1 now.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 1);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_with_different_agent() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a chain op into the DB.
        let facts = Facts {
            has_required_receipts: false,
            private: false,
            store_entry: false,
            within_min_period: false,
            withold_publish: false,
        };
        // Create chain op with different agent key.
        let _ = create_and_insert_chain_op(&db.to_db(), &fixt!(AgentPubKey), facts);

        // num_to_publish and length of get_ops_to_publish should be 0.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 0);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_store_entry_op_with_private_entry() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a chain op into the DB.
        let facts = Facts {
            has_required_receipts: false,
            private: true,
            store_entry: true,
            within_min_period: false,
            withold_publish: false,
        };
        let _ = create_and_insert_chain_op(&db.to_db(), &agent, facts);

        // num_to_publish and length of get_ops_to_publish should be 0.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 0);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_other_store_entry_op() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a chain op into the DB.
        let facts = Facts {
            has_required_receipts: false,
            private: false,
            store_entry: true,
            within_min_period: false,
            withold_publish: false,
        };
        let _ = create_and_insert_chain_op(&db.to_db(), &agent, facts);

        // num_to_publish and length of get_ops_to_publish should be 0.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 1);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_private_entry_op() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a chain op into the DB.
        let facts = Facts {
            has_required_receipts: false,
            private: true,
            store_entry: false,
            within_min_period: false,
            withold_publish: false,
        };
        let _ = create_and_insert_chain_op(&db.to_db(), &agent, facts);

        // num_to_publish and length of get_ops_to_publish should be 0.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 1);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_op_within_min_publish_interval() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a chain op into the DB.
        let facts = Facts {
            has_required_receipts: false,
            private: false,
            store_entry: false,
            within_min_period: true,
            withold_publish: false,
        };
        let _ = create_and_insert_chain_op(&db.to_db(), &agent, facts);

        // num_to_publish should be 1 because the query does not consider whether the op
        // has been published recently.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 1);
        // length of get_ops_to_publish should be 0.
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn query_withhold_publish() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a chain op into the DB.
        let facts = Facts {
            has_required_receipts: false,
            private: false,
            store_entry: false,
            within_min_period: false,
            withold_publish: true,
        };
        let _ = create_and_insert_chain_op(&db.to_db(), &agent, facts);

        // num_to_publish and length of get_ops_to_publish should be 0.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 0);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 0);
    }

    fn create_and_insert_chain_op(
        db: &DbWrite<DbKindAuthored>,
        agent: &AgentPubKey,
        facts: Facts,
    ) -> DhtOpHashed {
        let entry = Entry::App(fixt!(AppEntryBytes));
        let mut action = fixt!(Create);
        action.author = agent.clone();
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

        db.test_write({
            let query_state = state.clone();

            move |txn| {
                let hash = query_state.as_hash().clone();
                insert_op_authored(txn, &query_state).unwrap();
                set_last_publish_time(txn, &hash, last_publish).unwrap();
                set_receipts_complete(txn, &hash, facts.has_required_receipts).unwrap();
                if facts.withold_publish {
                    set_withhold_publish(txn, &hash).unwrap();
                }
            }
        });
        state
    }

    #[cfg(feature = "unstable-warrants")]
    #[tokio::test(flavor = "multi_thread")]
    async fn publish_query_includes_warrants() {
        holochain_trace::test_run();

        let agent = fixt!(AgentPubKey);
        let db = test_authored_db();

        // Insert one chain op and one warrant op into database.
        let chain_op = create_and_insert_chain_op(
            &db,
            &agent,
            Facts {
                private: false,
                within_min_period: false,
                has_required_receipts: false,
                store_entry: false,
                withold_publish: false,
            },
        )
        .content;
        let warrant_op = insert_invalid_chain_op_warrant_op(&db, &agent).content;

        let agent_clone = agent.clone();
        let num_to_publish = db
            .to_db()
            .test_read(move |txn| num_still_needing_publish(txn, agent_clone).unwrap());
        assert_eq!(num_to_publish, 2);

        let ops_to_publish = get_ops_to_publish(
            agent,
            &db.to_db(),
            ConductorTuningParams::default().min_publish_interval(),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|(_, _, op)| op)
        .collect::<Vec<_>>();
        assert_eq!(ops_to_publish, vec![chain_op, warrant_op]);
    }

    #[cfg(feature = "unstable-warrants")]
    #[tokio::test(flavor = "multi_thread")]
    async fn query_warrants_with_different_agent() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a warrant op into the DB.
        let _ = insert_invalid_chain_op_warrant_op(&db, &fixt!(AgentPubKey)).hash;

        // num_to_publish and length of get_ops_to_publish should be 0.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 0);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 0);
    }

    #[cfg(feature = "unstable-warrants")]
    #[tokio::test(flavor = "multi_thread")]
    async fn query_warrant_op_already_published() {
        let db = test_authored_db();
        let agent = fixt!(AgentPubKey);
        let min_publish_interval = ConductorTuningParams::default().min_publish_interval();

        // Insert a warrant op into the DB.
        let warrant_op_hash = insert_invalid_chain_op_warrant_op(&db, &agent).hash;

        // num_to_publish and length of get_ops_to_publish should be 1.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 1);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 1);

        // Set last publish time for warrant op to stop publishing.
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap() - min_publish_interval;
        db.test_write(move |txn| set_last_publish_time(txn, &warrant_op_hash, now).unwrap());

        // Should both be 0 again.
        let agent_clone = agent.clone();
        let num =
            db.test_read(move |txn| num_still_needing_publish(txn, agent_clone.clone()).unwrap());
        assert_eq!(num, 0);
        let ops_to_publish = get_ops_to_publish(agent.clone(), &db.to_db(), min_publish_interval)
            .await
            .unwrap();
        assert_eq!(ops_to_publish.len(), 0);
    }

    #[cfg(feature = "unstable-warrants")]
    fn insert_invalid_chain_op_warrant_op(
        db: &DbWrite<DbKindAuthored>,
        agent: &AgentPubKey,
    ) -> DhtOpHashed {
        let invalid_op_warrant = SignedWarrant::new(
            Warrant::new(
                WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
                    action_author: fixt!(AgentPubKey),
                    action: (fixt!(ActionHash), fixt!(Signature)),
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
        db.test_write(move |txn| insert_op_authored(txn, &invalid_op_warrant).unwrap());
        warrant_op
    }
}
