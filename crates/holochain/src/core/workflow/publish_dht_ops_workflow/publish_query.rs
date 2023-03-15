use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holochain_p2p::DhtOpHashExt;
use holochain_sqlite::db::DbKindAuthored;
use holochain_state::query::prelude::*;
use holochain_types::db::DbRead;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpType;
use holochain_types::prelude::OpBasis;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryVisibility;
use holochain_zome_types::SignedAction;
use kitsune_p2p::dependencies::kitsune_p2p_fetch::OpHashSized;
use rusqlite::named_params;
use rusqlite::Transaction;

use crate::core::workflow::error::WorkflowResult;

use super::MIN_PUBLISH_INTERVAL;

/// Get all dht ops on an agents chain that need to be published.
/// - Don't publish private entries.
/// - Only get ops that haven't been published within the minimum publish interval
/// - Only get ops that have less then the RECEIPT_BUNDLE_SIZE
pub async fn get_ops_to_publish(
    agent: AgentPubKey,
    db: &DbRead<DbKindAuthored>,
) -> WorkflowResult<Vec<(OpBasis, OpHashSized, DhtOp)>> {
    let recency_threshold = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|epoch| epoch.checked_sub(MIN_PUBLISH_INTERVAL))
        .map(|t| t.as_secs())
        .unwrap_or(0);

    let results = db
        .async_reader(move |txn| {
            let mut stmt = txn.prepare(
                "
            SELECT
            Action.blob as action_blob,
            LENGTH(Action.blob) AS action_size,
            CASE
              WHEN DhtOp.type IN ('StoreEntry', 'StoreRecord') THEN LENGTH(Entry.blob)
              ELSE 0
            END AS entry_size,
            Entry.blob as entry_blob,
            DhtOp.type as dht_type,
            DhtOp.hash as dht_hash
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
            ",
            )?;
            let r = stmt.query_and_then(
                named_params! {
                    ":author": agent,
                    ":recency_threshold": recency_threshold,
                    ":store_entry": DhtOpType::StoreEntry,
                },
                |row| {
                    let action_size: usize = row.get("action_size")?;
                    // will be NULL if the op has no associated entry
                    let entry_size: Option<usize> = row.get("entry_size")?;
                    let op_size = (action_size + entry_size.unwrap_or(0)).into();
                    let action = from_blob::<SignedAction>(row.get("action_blob")?)?;
                    let op_type: DhtOpType = row.get("dht_type")?;
                    let hash: DhtOpHash = row.get("dht_hash")?;
                    let op_hash_sized = OpHashSized::new(hash.to_kitsune(), Some(op_size));
                    let entry = match action.0.entry_type().map(|et| et.visibility()) {
                        Some(EntryVisibility::Public) => {
                            let entry: Option<Vec<u8>> = row.get("entry_blob")?;
                            match entry {
                                Some(entry) => Some(from_blob::<Entry>(entry)?),
                                None => None,
                            }
                        }
                        _ => None,
                    };
                    let op = DhtOp::from_type(op_type, action, entry)?;
                    let basis = op.dht_basis();
                    WorkflowResult::Ok((basis, op_hash_sized, op))
                },
            )?;
            WorkflowResult::Ok(r.collect())
        })
        .await?;
    tracing::debug!(?results);
    results
}

/// Get the number of ops that might need to publish again in the future.
pub fn num_still_needing_publish(txn: &Transaction) -> WorkflowResult<usize> {
    let count = txn.query_row(
        "
        SELECT
        COUNT(DhtOp.rowid) as num_ops
        FROM Action
        JOIN
        DhtOp ON DhtOp.action_hash = Action.hash
        WHERE
        DhtOp.receipts_complete IS NULL
        AND
        (DhtOp.type != :store_entry OR Action.private_entry = 0)
        ",
        named_params! {
            ":store_entry": DhtOpType::StoreEntry,
        },
        |row| row.get("num_ops"),
    )?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use fixt::prelude::*;
    use holo_hash::EntryHash;
    use holo_hash::HasHash;
    use holochain_sqlite::db::DbWrite;
    use holochain_sqlite::db::WriteManager;
    use holochain_sqlite::prelude::DatabaseResult;
    use holochain_state::prelude::insert_op;
    use holochain_state::prelude::set_last_publish_time;
    use holochain_state::prelude::set_receipts_complete;
    use holochain_state::prelude::test_authored_db;
    use holochain_types::action::NewEntryAction;
    use holochain_types::dht_op::DhtOpHashed;
    use holochain_zome_types::fixt::*;
    use holochain_zome_types::Action;
    use holochain_zome_types::EntryType;
    use holochain_zome_types::EntryVisibility;

    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct Facts {
        private: bool,
        within_min_period: bool,
        has_required_receipts: bool,
        is_this_agent: bool,
        store_entry: bool,
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
        holochain_trace::test_run().ok();
        let db = test_authored_db();
        let expected = test_data(&db.to_db().into());
        let r = get_ops_to_publish(expected.agent.clone(), &db.to_db().into())
            .await
            .unwrap();
        assert_eq!(
            r.into_iter()
                .map(|t| t.1.into_inner().0)
                .collect::<Vec<_>>(),
            expected
                .results
                .into_iter()
                .map(|op| op.into_inner().1.to_kitsune())
                .collect::<Vec<_>>(),
        );
    }

    fn create_and_insert_op(
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
            now - MIN_PUBLISH_INTERVAL
        };

        let state = if facts.store_entry {
            DhtOpHashed::from_content_sync(DhtOp::StoreEntry(
                fixt!(Signature),
                NewEntryAction::Create(action.clone()),
                Box::new(entry.clone()),
            ))
        } else {
            DhtOpHashed::from_content_sync(DhtOp::StoreRecord(
                fixt!(Signature),
                Action::Create(action.clone()),
                Some(Box::new(entry.clone())),
            ))
        };

        db.conn()
            .unwrap()
            .with_commit_sync(|txn| {
                let hash = state.as_hash().clone();
                insert_op(txn, &state).unwrap();
                set_last_publish_time(txn, &hash, last_publish).unwrap();
                set_receipts_complete(txn, &hash, facts.has_required_receipts).unwrap();
                DatabaseResult::Ok(())
            })
            .unwrap();
        state
    }

    fn test_data(db: &DbWrite<DbKindAuthored>) -> Expected {
        let mut results = Vec::new();
        let cd = Consistent {
            this_agent: fixt!(AgentPubKey),
        };
        // We **do** expect any of these in the results:
        // - Private: false.
        // - WithinMinPeriod: false.
        // - HasRequireReceipts: false.
        // - IsThisAgent: true.
        // - StoreEntry: true
        let facts = Facts {
            private: false,
            within_min_period: false,
            has_required_receipts: false,
            is_this_agent: true,
            store_entry: true,
        };
        let op = create_and_insert_op(db, facts, &cd);
        results.push(op);

        // All facts are the same unless stated:

        // - Private: true.
        // - StoreEntry: false.
        let mut f = facts;
        f.private = true;
        f.store_entry = false;
        let op = create_and_insert_op(db, f, &cd);
        results.push(op);

        // We **don't** expect any of these in the results:
        // - Private: true.
        let mut f = facts;
        f.private = true;
        create_and_insert_op(db, f, &cd);

        // - WithinMinPeriod: true.
        let mut f = facts;
        f.within_min_period = true;
        create_and_insert_op(db, f, &cd);

        // - HasRequireReceipts: true.
        let mut f = facts;
        f.has_required_receipts = true;
        create_and_insert_op(db, f, &cd);

        // - IsThisAgent: false.
        let mut f = facts;
        f.is_this_agent = false;
        create_and_insert_op(db, f, &cd);

        Expected {
            agent: cd.this_agent.clone(),
            results,
        }
    }
}
