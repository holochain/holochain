//! Methods for awaiting consistency between cells of the same DNA

use super::*;
use crate::prelude::*;
use holochain_sqlite::error::DatabaseError;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

/// A duration expressed properly, or just as seconds
#[derive(derive_more::From, Debug)]
pub enum DurationOrSeconds {
    /// Proper duration
    Duration(Duration),
    /// Just seconds
    Seconds(u64),
}

impl DurationOrSeconds {
    /// Get the proper duration
    pub fn into_duration(self) -> Duration {
        match self {
            Self::Duration(d) => d,
            Self::Seconds(s) => Duration::from_secs(s),
        }
    }
}

/// Wait 60s for all cells to reach consistency.
///
/// This should be used as the default, unless your test case specifically requires a longer duration,
/// or requires immediate consistency
pub async fn await_consistency<'a, I: IntoIterator<Item = &'a SweetCell>>(
    cells: I,
) -> Result<(), String> {
    await_consistency_s(60, cells).await
}

/// Check cell consistency.
pub async fn check_consistency<'a, I: IntoIterator<Item = &'a SweetCell>>(
    cells: I,
) -> Result<(), String> {
    await_consistency_s(Duration::ZERO, cells).await
}

/// Wait for all cells to reach consistency
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub async fn await_consistency_s<'a, I: IntoIterator<Item = &'a SweetCell>>(
    timeout: impl Into<DurationOrSeconds>,
    cells: I,
) -> Result<(), String> {
    #[allow(clippy::type_complexity)]
    let all_cell_dbs: Vec<(AgentPubKey, DbRead<DbKindDht>)> = cells
        .into_iter()
        .map(|c| (c.agent_pubkey().clone(), c.dht_db().clone().into()))
        .collect();
    let all_cell_dbs: Vec<_> = all_cell_dbs.iter().map(|c| (&c.0, &c.1)).collect();
    await_op_integration(&all_cell_dbs[..], timeout.into().into_duration()).await
}

#[derive(Clone, Debug)]
struct DhtOpRow {
    hash: DhtOpHash,
    op_type: DhtOpType,
    action_seq: u32,
    author: AgentPubKey,
    when_integrated: Option<Timestamp>,
}

/// Wait for all cell envs to reach consistency, meaning that every op
/// published by every cell has been integrated by every node.
async fn await_op_integration(
    cells: &[(&AgentPubKey, &impl ReadAccess<DbKindDht>)],
    timeout: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    // Declare op hash lists here so they can be accessed for reporting after timeout.
    let mut rows_per_db = Vec::new();
    let result = tokio::time::timeout(timeout, async {
        'compare_dbs_loop: loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            // Create query for each DHT DB.
            let queries = cells.iter().map(|(_, dht_db)| {
                dht_db.read_async(|txn| {
                    let mut stmt = txn
                        .prepare(
                            "\
                            SELECT DhtOp.hash, DhtOp.type, DhtOp.when_integrated, Action.seq, Action.author
                            FROM DhtOp
                            JOIN Action ON DhtOp.action_hash = Action.hash",
                        )
                        .unwrap();
                    let mut rows = stmt.query([]).unwrap();
                    let mut values = Vec::new();
                    while let Some(row) = rows.next().unwrap() {
                        let hash = row.get_unwrap::<_, DhtOpHash>(0);
                        let op_type = row.get_unwrap::<_, DhtOpType>(1);
                        let when_integrated = row.get_unwrap::<_, Option<Timestamp>>(2);
                        let action_seq = row.get_unwrap::<_, u32>(3);
                        let author = row.get_unwrap::<_, AgentPubKey>(4);
                        values.push(DhtOpRow {
                            hash,
                            op_type,
                            action_seq,
                            author,
                            when_integrated,
                        });
                    }
                    Ok::<_, DatabaseError>(values)
                })
            });
            // Execute queries in parallel.
            rows_per_db = futures::future::join_all(queries)
                .await
                .into_iter()
                .collect::<Result<_, _>>()
                .unwrap();
            // Build a set of all op hashes and create lists of hashes for each DHT DB.
            let mut all_hashes = HashSet::new();
            let mut hash_lists = Vec::new();
            for (index, dht_op_rows) in rows_per_db
                .clone()
                .into_iter()
                .enumerate() {
                    tracing::debug!(
                        "Agent {} with key {} has {} ops in their DHT DB",
                        index,
                        cells[index].0,
                        dht_op_rows.len()
                    );
                    let mut hash_list = Vec::new();
                    for row in dht_op_rows {
                        // If any op is not yet integrated, continue to the next loop iteration.
                        if row.when_integrated.is_none() {
                            tracing::trace!("Unintegrated op found, sleeping...");
                            continue 'compare_dbs_loop;
                        }
                        hash_list.push(row.hash.clone());
                        all_hashes.insert(row.hash);
                    }
                    hash_lists.push(hash_list);
                }
            // All ops currently in the DHT DBs have been integrated.
            // Check if all ops are in all DHT DBs.

            // If each DHT DB contains all hashes, consistency is reached.
            if hash_lists
                .iter()
                .all(|hash_list| all_hashes.iter().all(|hash| hash_list.contains(hash)))
            {
                tracing::info!("Consistency reached after {:?}", start.elapsed());
                break;
            } else {
                // Otherwise some ops haven't made it to all agents yet.
                tracing::debug!("Not all op hashes were found in all DHT DBs after {:?}.", start.elapsed());
            }
        }
    })
    .await;

    if result.is_err() {
        // Print a report now that consistency hasn't been reached.
        println!("\nConsistency not reached.\n");
        for (index, mut rows) in rows_per_db.into_iter().enumerate() {
            // Sort rows by author first, then action sequence.
            rows.sort_by_key(|row| (row.author.clone(), row.action_seq));
            println!(
                "Agent {} with key {} has the following ops in the DHT DB:",
                index, cells[index].0
            );
            println!(
                "{:53}  {:10}  {:21}  {:53}  {:10}",
                "Author", "Action seq", "Op type", "Op hash", "Integrated"
            );
            for row in rows {
                let chain_op_type = match row.op_type {
                    DhtOpType::Chain(chain_op_type) => chain_op_type,
                    _ => panic!("Warrant ops must not be in the DHT database"),
                };
                println!(
                    "{:53}  {:10}  {:21}  {:53}  {:10}",
                    row.author,
                    row.action_seq,
                    chain_op_type.to_string(),
                    row.hash,
                    row.when_integrated.is_some()
                );
            }
            println!();
        }
        return Err("Consistency not reached".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::sweettest::{await_consistency_s, SweetConductorConfig};
    use crate::{
        prelude::holochain_serial,
        sweettest::{await_consistency, check_consistency, SweetConductorBatch, SweetDnaFile},
        test_utils::retry_fn_until_timeout,
    };
    use ::fixt::fixt;
    use hdk::prelude::{ActionFixturator, SignatureFixturator};
    use holo_hash::ActionHash;
    use holochain_serialized_bytes::SerializedBytes;
    use holochain_state::prelude::insert_op_dht;
    use holochain_types::dht_op::{ChainOp, DhtOpHashed};
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::{
        action::ChainTopOrdering,
        entry::{AppEntryBytes, AppEntryDefLocation, CreateInput, EntryDefLocation},
        entry_def::{EntryDef, EntryVisibility},
        zome::inline_zome::InlineIntegrityZome,
        Entry,
    };
    use serde::{Deserialize, Serialize};

    #[tokio::test(flavor = "multi_thread")]
    #[cfg_attr(
        not(feature = "transport-iroh"),
        ignore = "requires Iroh transport for stability"
    )]
    async fn consistency_reached() {
        holochain_trace::test_run();
        let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;
        #[derive(Debug, Deserialize, Serialize)]
        struct E;
        holochain_serial!(E);
        let entry_def = EntryDef::default_from_id("entry");
        let dna_file = SweetDnaFile::unique_from_inline_zomes((
            "integrity",
            InlineIntegrityZome::new_unique(vec![entry_def], 0).function(
                "make_some_noise",
                |api, ()| {
                    api.create(CreateInput::new(
                        EntryDefLocation::App(AppEntryDefLocation {
                            zome_index: 0.into(),
                            entry_def_index: 0.into(),
                        }),
                        EntryVisibility::Public,
                        Entry::App(AppEntryBytes(SerializedBytes::try_from(E).unwrap())),
                        ChainTopOrdering::Relaxed,
                    ))
                    .unwrap();
                    Ok(())
                },
            ),
        ))
        .await
        .0;
        let ((alice,), (bob,)) = conductors
            .setup_app("", &[dna_file])
            .await
            .unwrap()
            .into_tuples();

        await_consistency(&[alice.clone(), bob.clone()])
            .await
            .unwrap();

        // Both peers create an entry.
        conductors[0]
            .call::<_, ()>(&alice.zome("integrity"), "make_some_noise", ())
            .await;
        conductors[1]
            .call::<_, ()>(&bob.zome("integrity"), "make_some_noise", ())
            .await;

        await_consistency(&[alice, bob]).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg_attr(
        not(feature = "transport-iroh"),
        ignore = "requires Iroh transport for stability"
    )]
    async fn consistency_reached_with_private_entry() {
        holochain_trace::test_run();
        let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;
        let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
            .await
            .0;
        let ((alice,), (bob,)) = conductors
            .setup_app("", &[dna_file])
            .await
            .unwrap()
            .into_tuples();

        await_consistency(&[alice.clone(), bob.clone()])
            .await
            .unwrap();

        // Both peers create a private entry.
        conductors[0]
            .call::<_, ActionHash>(
                &alice.zome(TestWasm::Create.coordinator_zome()),
                "create_priv_msg",
                (),
            )
            .await;
        conductors[1]
            .call::<_, ActionHash>(
                &bob.zome(TestWasm::Create.coordinator_zome()),
                "create_priv_msg",
                (),
            )
            .await;

        await_consistency(&[alice, bob]).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg_attr(
        not(feature = "transport-iroh"),
        ignore = "requires Iroh transport for stability"
    )]
    async fn consistency_not_reached_when_ops_not_synced() {
        holochain_trace::test_run();
        // No bootstrap service.
        let mut config = SweetConductorConfig::rendezvous(false);
        config.network.disable_bootstrap = true;
        let mut conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;
        let dna_file = SweetDnaFile::unique_from_inline_zomes((
            "integrity",
            InlineIntegrityZome::new_unique(vec![], 0),
        ))
        .await
        .0;
        let ((alice,), (bob,)) = conductors
            .setup_app("", std::slice::from_ref(&dna_file))
            .await
            .unwrap()
            .into_tuples();

        // Await genesis actions to be integrated for both peers.
        retry_fn_until_timeout(
            || async {
                conductors[0]
                    .all_ops_integrated(dna_file.dna_hash())
                    .unwrap()
                    && conductors[1]
                        .all_ops_integrated(dna_file.dna_hash())
                        .unwrap()
            },
            Some(5000),
            Some(100),
        )
        .await
        .unwrap();

        // Genesis actions will be integrated but not gossiped. Consistency cannot be reached.
        await_consistency_s(10, &[alice, bob]).await.unwrap_err();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn consistency_not_reached_when_ops_not_integrated() {
        holochain_trace::test_run();
        let mut conductors = SweetConductorBatch::from_standard_config_rendezvous(2).await;
        let dna_file = SweetDnaFile::unique_from_inline_zomes((
            "integrity",
            InlineIntegrityZome::new_unique(vec![], 0),
        ))
        .await
        .0;
        let ((alice,), (bob,)) = conductors
            .setup_app("", std::slice::from_ref(&dna_file))
            .await
            .unwrap()
            .into_tuples();

        await_consistency(&[alice.clone(), bob.clone()])
            .await
            .unwrap();

        let op = ChainOp::RegisterAgentActivity(fixt!(Signature), fixt!(Action));
        let unintegrated_op = DhtOpHashed::from_content_sync(op);
        conductors[0]
            .get_dht_db(dna_file.dna_hash())
            .unwrap()
            .test_write(move |txn| insert_op_dht(txn, &unintegrated_op, 0, None))
            .unwrap();

        // Unintegrated op will prevent consistency.
        check_consistency(&[alice, bob]).await.unwrap_err();
    }
}
