//! Methods for awaiting consistency between cells of the same DNA

use super::*;
use crate::conductor::wire_rows_to_legacy_ops;
use crate::prelude::*;
use holochain_state::dht_store::DhtStoreRead;
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
    let all_cell_dbs: Vec<(AgentPubKey, DhtStoreRead)> = cells
        .into_iter()
        .map(|c| (c.agent_pubkey().clone(), c.dht_store().as_read()))
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
    when_integrated: i64,
}

/// Read the integrated ops a node holds, as `(hash, row)` pairs for reporting.
///
/// "Integrated" follows the new store semantics: locally-validated chain ops,
/// GET-cached copies excluded. **Warrants are deliberately excluded** — the
/// legacy consistency check inner-joined `Action` (warrants have no `Action`
/// row), so it only ever compared chain ops. Warrants are not guaranteed to
/// reach every node (zero-arc nodes, gossip timing), so requiring cross-node
/// warrant consistency here would hang; warrant propagation is asserted
/// separately by the warrant tests. Ops are reconstructed into legacy `DhtOp`s
/// so their hashes match across nodes.
async fn integrated_op_rows(dht_store: &DhtStoreRead) -> Result<Vec<DhtOpRow>, String> {
    let dump_rows = dht_store
        .integrated_chain_ops_for_dump(None)
        .await
        .map_err(|e| e.to_string())?;
    // Reconstruct each row on its own so its `when_integrated` stays paired with
    // the op: `wire_rows_to_legacy_ops` drops rows that fail to rebuild, so
    // zipping its output against the original list could misalign.
    Ok(dump_rows
        .into_iter()
        .flat_map(|row| {
            let when_integrated = row.when_integrated;
            wire_rows_to_legacy_ops(vec![row.wire], vec![])
                .into_iter()
                .map(move |op| (op, when_integrated))
        })
        .map(|(op, when_integrated)| {
            let op_type = op.get_type();
            let (action_seq, author) = match &op {
                DhtOp::ChainOp(chain_op) => (
                    chain_op.action().action_seq(),
                    chain_op.action().author().clone(),
                ),
                DhtOp::WarrantOp(warrant_op) => (0, warrant_op.author.clone()),
            };
            let hashed = DhtOpHashed::from_content_sync(op);
            DhtOpRow {
                hash: hashed.hash,
                op_type,
                action_seq,
                author,
                when_integrated,
            }
        })
        .collect())
}

/// Wait for all cell envs to reach consistency, meaning that every op
/// published by every cell has been integrated by every node.
async fn await_op_integration(
    cells: &[(&AgentPubKey, &DhtStoreRead)],
    timeout: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    let result = tokio::time::timeout(timeout, async {
        'compare_dbs_loop: loop {
            tokio::time::sleep(Duration::from_millis(500)).await;

            // If any node still has ops awaiting validation or integration,
            // consistency cannot have been reached; sleep and retry.
            for (_, dht_store) in cells.iter() {
                let (validation_limbo, integration_limbo, _) = dht_store
                    .limbo_state_counts()
                    .await
                    .map_err(|e| e.to_string())?;
                if validation_limbo > 0 || integration_limbo > 0 {
                    tracing::trace!("Unintegrated op found, sleeping...");
                    continue 'compare_dbs_loop;
                }
            }

            // Read the integrated ops for each node in parallel.
            let queries = cells
                .iter()
                .map(|(_, dht_store)| integrated_op_rows(dht_store));
            let rows_per_db = futures::future::join_all(queries)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, String>>()?;

            // Build a set of all op hashes and create lists of hashes for each DHT DB.
            let mut all_hashes = HashSet::new();
            let mut hash_lists = Vec::new();
            for (index, dht_op_rows) in rows_per_db.into_iter().enumerate() {
                tracing::debug!(
                    "Agent {} with key {} has {} ops in their DHT store",
                    index,
                    cells[index].0,
                    dht_op_rows.len()
                );
                let mut hash_list = Vec::new();
                for row in dht_op_rows {
                    hash_list.push(row.hash.clone());
                    all_hashes.insert(row.hash);
                }
                hash_lists.push(hash_list);
            }
            // All ops currently in the DHT stores have been integrated.
            // Check if all ops are in all DHT stores.

            // If each DHT store contains all hashes, consistency is reached.
            if hash_lists
                .iter()
                .all(|hash_list| all_hashes.iter().all(|hash| hash_list.contains(hash)))
            {
                tracing::info!("Consistency reached after {:?}", start.elapsed());
                break;
            } else {
                // Otherwise some ops haven't made it to all agents yet.
                tracing::debug!(
                    "Not all op hashes were found in all DHT stores after {:?}.",
                    start.elapsed()
                );
            }
        }
        Ok::<_, String>(())
    })
    .await;

    // A timeout (the outer `Err`) or an internal store error (the inner `Err`)
    // both mean consistency was not reached.
    let consistent = matches!(result, Ok(Ok(())));

    if !consistent {
        // Print a report now that consistency hasn't been reached.
        //
        // Re-read each node's state fresh rather than relying on whatever the
        // wait loop left behind. The loop only records integrated rows once
        // every node has integrated them. Reporting the limbo counts alongside
        // the integrated.
        println!("\nConsistency not reached.\n");
        for (index, (_, dht_store)) in cells.iter().enumerate() {
            let (validation_limbo, integration_limbo, integrated) =
                match dht_store.limbo_state_counts().await {
                    Ok(counts) => counts,
                    Err(e) => {
                        println!(
                            "Agent {} with key {}: failed to read integration state: {e}",
                            index, cells[index].0
                        );
                        continue;
                    }
                };
            println!(
                "Agent {} with key {}: {} in validation limbo, {} in integration limbo, {} integrated",
                index, cells[index].0, validation_limbo, integration_limbo, integrated
            );

            let mut rows = match integrated_op_rows(dht_store).await {
                Ok(rows) => rows,
                Err(e) => {
                    println!("  failed to read integrated ops: {e}");
                    continue;
                }
            };
            // Sort rows by author first, then action sequence.
            rows.sort_by_key(|row| (row.author.clone(), row.action_seq));
            println!("The following ops are in the DHT store:");
            println!(
                "{:53}  {:10}  {:28}  {:53}  {:20}",
                "Author", "Action seq", "Op type", "Op hash", "When integrated"
            );
            for row in rows {
                println!(
                    "{:53}  {:10}  {:28}  {:53}  {:20}",
                    row.author,
                    row.action_seq,
                    format!("{:?}", row.op_type),
                    row.hash,
                    row.when_integrated,
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
    use hdk::prelude::{LegacyActionFixturator, SignatureFixturator};
    use holo_hash::ActionHash;
    use holochain_serialized_bytes::SerializedBytes;
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
        let mut conductors = SweetConductorBatch::standard(2).await;
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
        let mut conductors = SweetConductorBatch::standard(2).await;
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
                    .await
                    .unwrap()
                    && conductors[1]
                        .all_ops_integrated(dna_file.dna_hash())
                        .await
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
        let mut conductors = SweetConductorBatch::standard(2).await;
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

        // `record_incoming_ops` is v2-native; this arbitrary op only needs to
        // exist unvalidated in limbo, so build it directly as v2 rather than
        // via the legacy `ChainOp`/`DhtOpHashed` this test module otherwise
        // uses for op reconstruction (see `wire_rows_to_legacy_ops`).
        let v2_action = holochain_zome_types::dht_v2::from_legacy_action(&fixt!(LegacyAction));
        let op = holochain_types::dht_v2::ChainOp::AgentActivity(
            holochain_zome_types::dht_v2::SignedAction::new(v2_action, fixt!(Signature)),
        );
        let unintegrated_op = holochain_types::dht_v2::DhtOpHashed::from_content_sync(
            holochain_types::dht_v2::DhtOp::from(op),
        );
        // Stage the op into the DHT store's validation limbo so it is present
        // but not integrated.
        conductors[0]
            .get_dht_store(dna_file.dna_hash())
            .unwrap()
            .record_incoming_ops(vec![(unintegrated_op, false)])
            .await
            .unwrap();

        // Unintegrated op will prevent consistency.
        check_consistency(&[alice, bob]).await.unwrap_err();
    }
}
