use crate::test_utils::host_fn_caller::*;
use crate::test_utils::setup_app;
use crate::test_utils::wait_for_integration;
use crate::{conductor::ConductorHandle, core::MAX_TAG_SIZE};
use ::fixt::prelude::*;
use hdk::prelude::LinkTag;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::SerializedBytes;
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

    let dna_file = DnaFile::new(
        DnaDef {
            name: "sys_validation_workflow_test".to_string(),
            uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Create.into()].into(),
        },
        vec![TestWasm::Create.into()],
    )
    .await
    .unwrap();

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    let bob_agent_id = fake_agent_pubkey_2();
    let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
    let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

    let (_tmpdir, _app_api, handle) = setup_app(
        vec![(
            "test_app",
            vec![(alice_installed_cell, None), (bob_installed_cell, None)],
        )],
        vec![dna_file.clone()],
    )
    .await;

    run_test(alice_cell_id, bob_cell_id, handle.clone(), dna_file).await;

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap().unwrap();
}

async fn run_test(
    alice_cell_id: CellId,
    bob_cell_id: CellId,
    handle: ConductorHandle,
    dna_file: DnaFile,
) {
    // Check if the correct number of ops are integrated
    // every 100 ms for a maximum of 10 seconds but early exit
    // if they are there.
    let num_attempts = 100;
    let delay_per_attempt = Duration::from_millis(100);

    bob_links_in_a_legit_way(&bob_cell_id, &handle, &dna_file).await;

    // Integration should have 9 ops in it.
    // Plus another 14 for genesis.
    // Init is not run because we aren't calling the zome.
    let expected_count = 9 + 14;

    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(
        &alice_env,
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
    let show_limbo = |txn: &Transaction| {
        txn.prepare("SELECT blob FROM DhtOp WHERE when_integrated IS NULL")
            .unwrap()
            .query_and_then([], |row| from_blob(row.get("blob")?))
            .unwrap()
            .collect::<StateQueryResult<Vec<DhtOpLight>>>()
            .unwrap()
    };

    // Validation should be empty
    fresh_reader_test(alice_env, |txn| {
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

    let (bad_update_header, bad_update_entry_hash, link_add_hash) =
        bob_makes_a_large_link(&bob_cell_id, &handle, &dna_file).await;

    // Integration should have 13 ops in it
    let expected_count = 14 + expected_count;

    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(
        &alice_env,
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
                                (type = :store_entry AND basis_hash = :bad_update_entry_hash AND header_hash = :bad_update_header)
                                OR
                                (type = :store_element AND header_hash = :bad_update_header)
                                OR
                                (type = :add_link AND header_hash = :link_add_hash)
                                OR
                                (type = :update_content AND header_hash = :bad_update_header)
                                OR
                                (type = :update_element AND header_hash = :bad_update_header)
                            )
                        )
                    )
                    ", 
                named_params!{
                    ":valid": ValidationStatus::Valid,
                    ":rejected": ValidationStatus::Rejected,
                    ":store_entry": DhtOpType::StoreEntry,
                    ":store_element": DhtOpType::StoreElement,
                    ":add_link": DhtOpType::RegisterAddLink,
                    ":update_content": DhtOpType::RegisterUpdatedContent,
                    ":update_element": DhtOpType::RegisterUpdatedElement,
                    ":bad_update_entry_hash": bad_update_entry_hash,
                    ":bad_update_header": bad_update_header,
                    ":link_add_hash": link_add_hash,
                },
                |row| row.get(0))
                .unwrap();
        valid_ops
    };

    fresh_reader_test(alice_env, |txn| {
        // Validation should be empty
        let limbo = show_limbo(&txn);
        assert!(limbo_is_empty(&txn), "{:?}", limbo);

        let valid_ops = num_valid_ops(&txn);
        assert_eq!(valid_ops, expected_count);
    });

    dodgy_bob(&bob_cell_id, &handle, &dna_file).await;

    // Integration should have new 4 ops in it
    let expected_count = 4 + expected_count;

    let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
    wait_for_integration(
        &alice_env,
        expected_count,
        num_attempts,
        delay_per_attempt.clone(),
    )
    .await;

    // Validation should still contain bobs link pending because the target was missing
    // holochain_state::prelude::dump_tmp(&alice_env);
    fresh_reader_test(alice_env.clone(), |txn| {
        let valid_ops = num_valid_ops(&txn);
        assert_eq!(valid_ops, expected_count);
    });
    crate::assert_eq_retry_1m!(
        {
            fresh_reader_test(alice_env.clone(), |txn| {
                let num_limbo_ops: usize = txn
                    .query_row(
                        "
                        SELECT COUNT(hash) FROM DhtOP 
                        WHERE 
                        when_integrated IS NULL
                        AND (validation_stage IS NULL OR validation_stage = 2)
                        ",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                num_limbo_ops
            })
        },
        2
    );
}

async fn bob_links_in_a_legit_way(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> HeaderHash {
    let base = Post("Bananas are good for you".into());
    let target = Post("Potassium is radioactive".into());
    let base_entry_hash = EntryHash::with_data_sync(&Entry::try_from(base.clone()).unwrap());
    let target_entry_hash = EntryHash::with_data_sync(&Entry::try_from(target.clone()).unwrap());
    let link_tag = fixt!(LinkTag);
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;
    // 3
    call_data
        .commit_entry(base.clone().try_into().unwrap(), POST_ID)
        .await;

    // 4
    call_data
        .commit_entry(target.clone().try_into().unwrap(), POST_ID)
        .await;

    // 5
    // Link the entries
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone(),
            target_entry_hash.clone(),
            link_tag.clone(),
        )
        .await;

    // Produce and publish these commits
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger();
    link_add_address
}

async fn bob_makes_a_large_link(
    bob_cell_id: &CellId,
    handle: &ConductorHandle,
    dna_file: &DnaFile,
) -> (HeaderHash, EntryHash, HeaderHash) {
    let base = Post("Small time base".into());
    let target = Post("Spam it big time".into());
    let bad_update = Msg("This is not the msg you were looking for".into());
    let base_entry_hash = EntryHash::with_data_sync(&Entry::try_from(base.clone()).unwrap());
    let target_entry_hash = EntryHash::with_data_sync(&Entry::try_from(target.clone()).unwrap());
    let bad_update_entry_hash =
        EntryHash::with_data_sync(&Entry::try_from(bad_update.clone()).unwrap());

    let bytes = (0..MAX_TAG_SIZE + 1)
        .map(|_| 0u8)
        .into_iter()
        .collect::<Vec<_>>();
    let link_tag = LinkTag(bytes);

    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;

    // 6
    let original_header_address = call_data
        .commit_entry(base.clone().try_into().unwrap(), POST_ID)
        .await;

    // 7
    call_data
        .commit_entry(target.clone().try_into().unwrap(), POST_ID)
        .await;

    // 8
    // Commit a large header
    let link_add_address = call_data
        .create_link(
            base_entry_hash.clone(),
            target_entry_hash.clone(),
            link_tag.clone(),
        )
        .await;

    // 9
    // Commit a bad update entry
    let bad_update_header = call_data
        .update_entry(
            bad_update.clone().try_into().unwrap(),
            MSG_ID,
            original_header_address,
        )
        .await;

    // Produce and publish these commits
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger();
    (bad_update_header, bad_update_entry_hash, link_add_address)
}

async fn dodgy_bob(bob_cell_id: &CellId, handle: &ConductorHandle, dna_file: &DnaFile) {
    let base = Post("Bob is the best and I'll link to proof so you can check".into());
    let target = Post("Dodgy proof Bob is the best".into());
    let base_entry_hash = EntryHash::with_data_sync(&Entry::try_from(base.clone()).unwrap());
    let target_entry_hash = EntryHash::with_data_sync(&Entry::try_from(target.clone()).unwrap());
    let link_tag = fixt!(LinkTag);
    let call_data = HostFnCaller::create(bob_cell_id, handle, dna_file).await;

    // 11
    call_data
        .commit_entry(base.clone().try_into().unwrap(), POST_ID)
        .await;

    // Whoops forgot to commit that proof

    // Link the entries
    call_data
        .create_link(
            base_entry_hash.clone(),
            target_entry_hash.clone(),
            link_tag.clone(),
        )
        .await;

    // Produce and publish these commits
    let mut triggers = handle.get_cell_triggers(&bob_cell_id).await.unwrap();
    triggers.publish_dht_ops.trigger();
}

//////////////////////
//// Test Ideas
//////////////////////
// These are tests that I think might break
// validation but are too hard to write currently

// 1. Delete points to a header that isn't a NewEntryType.
// ## Comments
// I think this will fail RegisterDeleteBy but pass as StoreElement
// which is wrong.
// ## Scenario
// 1. Commit a Delete Header that points to a valid EntryHash and
// a HeaderHash that exists but is not a NewEntryHeader (use CreateLink).
// 2. The Create link is integrated and valid.
// ## Expected
// The Delete header should be invalid for all authorities.
