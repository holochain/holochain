use crate::holochain_wasmer_host::prelude::*;
use crate::sweettest::SweetConductorBatch;
use crate::sweettest::SweetDnaFile;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::wait_for_integration;
use crate::{conductor::ConductorHandle, core::MAX_TAG_SIZE};
use ::fixt::prelude::*;
use hdk::prelude::LinkTag;
use holo_hash::ActionHash;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_state::prelude::fresh_reader_test;
use holochain_state::prelude::from_blob;
use holochain_state::prelude::StateQueryResult;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::cell::CellId;
use holochain_zome_types::Entry;
use holochain_zome_types::ValidationStatus;
use rusqlite::named_params;
use rusqlite::Transaction;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn sys_validation_workflow_test() {
    observability::test_run().ok();

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let mut conductors = SweetConductorBatch::from_standard_config(2).await;
    let apps = conductors
        .setup_app(&"test_app", &[dna_file.clone()])
        .await
        .unwrap();
    let ((alice,), (bob,)) = apps.into_tuples();
    let alice_cell_id = alice.cell_id().clone();
    let bob_cell_id = bob.cell_id().clone();

    conductors.exchange_peer_info().await;

    run_test(alice_cell_id, bob_cell_id, conductors, dna_file).await;
}

async fn run_test(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    conductors: SweetConductorBatch,
    dna_file: DnaFile,
) {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    bob_links_in_a_legit_way(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // Integration should have 9 ops in it.
    // Plus another 14 for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 9 + 14;

    let alice_dht_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(
        &alice_dht_db,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    let limbo_is_empty = |txn: &Transaction| {
        let not_empty: bool = txn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM DhtOP WHERE when_integrated IS NULL)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        !not_empty
    };

    // holochain_state::prelude::dump_tmp(&alice_dht_db);
    // Validation should be empty
    fresh_reader_test(alice_dht_db, |txn| {
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        let num_valid_ops: usize = txn
                .query_row("SELECT COUNT(hash) FROM DhtOP WHERE when_integrated IS NOT NULL AND validation_status = :status",
                named_params!{
                    ":status": ValidationStatus::Valid,
                },
                |row| row.get(0))
                .unwrap();
        assert_eq!(num_valid_ops, expected_count);
    });

    let (bad_update_action, bad_update_entry_hash, link_add_hash) =
        bob_makes_a_large_link(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // Integration should have 14 ops in it + the running tally
    let expected_count = 14 + expected_count;

    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(
        &alice_db,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    let bad_update_entry_hash: AnyDhtHash = bad_update_entry_hash.into();
    let num_valid_ops = |txn: &Transaction| {
        let valid_ops: usize = txn
                .query_row(
                    "
                    SELECT COUNT(hash) FROM DhtOP
                    WHERE
                    when_integrated IS NOT NULL
                    AND
                    (validation_status = :valid
                        OR (validation_status = :rejected
                            AND (
                                (type = :store_entry AND basis_hash = :bad_update_entry_hash AND action_hash = :bad_update_action)
                                OR
                                (type = :store_record AND action_hash = :bad_update_action)
                                OR
                                (type = :add_link AND action_hash = :link_add_hash)
                                OR
                                (type = :update_content AND action_hash = :bad_update_action)
                                OR
                                (type = :update_record AND action_hash = :bad_update_action)
                            )
                        )
                    )
                    ",
                named_params!{
                    ":valid": ValidationStatus::Valid,
                    ":rejected": ValidationStatus::Rejected,
                    ":store_entry": DhtOpType::StoreEntry,
                    ":store_record": DhtOpType::StoreRecord,
                    ":add_link": DhtOpType::RegisterAddLink,
                    ":update_content": DhtOpType::RegisterUpdatedContent,
                    ":update_record": DhtOpType::RegisterUpdatedRecord,
                    ":bad_update_entry_hash": bad_update_entry_hash,
                    ":bad_update_action": bad_update_action,
                    ":link_add_hash": link_add_hash,
                },
                |row| row.get(0))
                .unwrap();
        valid_ops
    };

    fresh_reader_test(alice_db, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        let valid_ops = num_valid_ops(&txn);
        assert_eq!(valid_ops, expected_count);
    });

    dodgy_bob(&bob_cell_id, &conductors[1].raw_handle(), &dna_file).await;

    // Integration should have new 5 ops in it
    let expected_count = 5 + expected_count;

    let alice_db = conductors[0].get_dht_db(alice_cell_id.dna_hash()).unwrap();
    wait_for_integration(
        &alice_db,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    // Validation should still contain bobs link delete because it points at
    // garbage hashes as a dependency.
    fresh_reader_test(alice_db.clone(), |txn| {
        let valid_ops = num_valid_ops(&txn);
        assert_eq!(valid_ops, expected_count);
    });
    crate::assert_eq_retry_1m!(
        {
            fresh_reader_test(alice_db.clone(), |txn| {
                let num_limbo_ops: usize = txn
                    .query_row(
                        "
                        SELECT COUNT(hash) FROM DhtOP
                        WHERE
                        when_integrated IS NULL
                        AND (validation_stage IS NULL OR validation_stage = 0)
                        ",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                num_limbo_ops
            })
        },
        1
    );
}

async fn bob_links_in_a_legit_way(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> ActionHash {
    let base = Post("Bananas are good for you".into());
    let target = Post("Potassium is radioactive".into());
    let base_entry_hash = Entry::try_from(base.clone()).unwrap().to_hash();
    let target_entry_hash = Entry::try_from(target.clone()).unwrap().to_hash();
    let link_tag = fixt!(LinkTag);
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;
    // 3
    call_data
        .commit_entry(
            base.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 4
    call_data
        .commit_entry(
            target.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 5
    // Link the entries
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone().into(),
            target_entry_hash.clone().into(),
            zome_index,
            LinkType(0),
            link_tag.clone(),
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(&bob_cell_id).unwrap();
    triggers
        .publish_dht_ops
        .trigger(&"bob_links_in_a_legit_way");
    link_add_address
}

async fn bob_makes_a_large_link(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (ActionHash, EntryHash, ActionHash) {
    let base = Post("Small time base".into());
    let target = Post("Spam it big time".into());
    let bad_update = Msg("This is not the msg you were looking for".into());
    let base_entry_hash = Entry::try_from(base.clone()).unwrap().to_hash();
    let target_entry_hash = Entry::try_from(target.clone()).unwrap().to_hash();
    let bad_update_entry_hash = Entry::try_from(bad_update.clone()).unwrap().to_hash();

    let bytes = (0..MAX_TAG_SIZE + 1)
        .map(|_| 0u8)
        .into_iter()
        .collect::<Vec<_>>();
    let link_tag = LinkTag(bytes);

    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;

    // 6
    let original_action_address = call_data
        .commit_entry(
            base.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 7
    call_data
        .commit_entry(
            target.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // 8
    // Commit a large action
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone().into(),
            target_entry_hash.clone().into(),
            zome_index,
            LinkType(0),
            link_tag.clone(),
        )
        .await;

    // 9
    // Commit a bad update entry
    let bad_update_action = call_data
        .update_entry(
            bad_update.clone().try_into().unwrap(),
            original_action_address,
        )
        .await;

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(&bob_cell_id).unwrap();
    triggers.publish_dht_ops.trigger(&"bob_makes_a_large_link");
    (bad_update_action, bad_update_entry_hash, link_add_address)
}

async fn dodgy_bob(bob_cell_id: &CellId, handle: &ConductorHandle, dna_file: &DnaFile) {
    let legit_entry = Post("Bob is the best and I'll link to proof so you can check".into());
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    let zome_index = call_data
        .get_entry_type(TestWasm::Create, POST_INDEX)
        .zome_index;

    // 11
    call_data
        .commit_entry(
            legit_entry.clone().try_into().unwrap(),
            EntryDefLocation::app(zome_index, POST_INDEX),
            EntryVisibility::Public,
        )
        .await;

    // Delete a link that doesn't exist buy pushing garbage addresses straight
    // on to the source chain and flush the workspace.
    let (_ribosome, call_context, workspace_lock) = call_data.unpack().await;
    // garbage addresses.
    let base_address: AnyLinkableHash = EntryHash::from_raw_32([1_u8; 32].to_vec()).into();
    let link_add_address = ActionHash::from_raw_32([2_u8; 32].to_vec());

    let source_chain = call_context
        .host_context
        .workspace_write()
        .source_chain()
        .as_ref()
        .expect("Must have source chain if write_workspace access is given");

    let action_builder = builder::DeleteLink {
        link_add_address,
        base_address,
    };
    let _action_hash = source_chain
        .put(action_builder, None, ChainTopOrdering::default())
        .await
        .map_err(|source_chain_error| {
            wasm_error!(WasmErrorInner::Host(source_chain_error.to_string()))
        })
        .unwrap();
    workspace_lock.flush(&call_data.network).await.unwrap();

    // Produce and publish these commits
    let triggers = handle.get_cell_triggers(&bob_cell_id).unwrap();
    triggers.publish_dht_ops.trigger(&"dodgy_bob");
}

//////////////////////
//// Test Ideas
//////////////////////
// These are tests that I think might break
// validation but are too hard to write currently

// 1. Delete points to an action that isn't a NewEntryType.
// ## Comments
// I think this will fail RegisterDeleteBy but pass as StoreRecord
// which is wrong.
// ## Scenario
// 1. Commit a Delete Action that points to a valid EntryHash and
// a ActionHash that exists but is not a NewEntryAction (use CreateLink).
// 2. The Create link is integrated and valid.
// ## Expected
// The Delete action should be invalid for all authorities.

fn show_limbo(txn: &Transaction) -> Vec<DhtOpLight> {
    txn.prepare(
        "
        SELECT DhtOp.type, Action.hash, Action.blob
        FROM DhtOp
        JOIN Action ON DhtOp.action_hash = Action.hash
        WHERE
        when_integrated IS NULL
    ",
    )
    .unwrap()
    .query_and_then([], |row| {
        let op_type: DhtOpType = row.get("type")?;
        let hash: ActionHash = row.get("hash")?;
        let action: SignedAction = from_blob(row.get("blob")?)?;
        Ok(DhtOpLight::from_type(op_type, hash, &action.0)?)
    })
    .unwrap()
    .collect::<StateQueryResult<Vec<DhtOpLight>>>()
    .unwrap()
}
