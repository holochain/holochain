use crate::conductor::ConductorHandle;
use crate::core::workflow::incoming_dht_ops_workflow::IncomingDhtOpsWorkspace;
use crate::test_utils::host_fn_caller::*;
use crate::test_utils::setup_app;
use crate::test_utils::wait_for_integration;
use ::fixt::prelude::*;
use fallible_iterator::FallibleIterator;
use hdk::prelude::LinkTag;
use holo_hash::AnyDhtHash;
use holo_hash::DhtOpHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::SerializedBytes;
use holochain_sqlite::fresh_reader_test;
use holochain_sqlite::prelude::ReadManager;
use holochain_state::element_buf::ElementBuf;
use holochain_state::validation_db::ValidationLimboStatus;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::cell::CellId;
use holochain_zome_types::Entry;
use holochain_zome_types::ValidationStatus;
use matches::assert_matches;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::Duration;
use tracing::*;

#[tokio::test(threaded_scheduler)]
async fn sys_validation_workflow_test() {
    observability::test_run().ok();

    let dna_file = DnaFile::new(
        DnaDef {
            name: "sys_validation_workflow_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
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
    shutdown.await.unwrap();
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

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
        wait_for_integration(
            &alice_env,
            expected_count,
            num_attempts,
            delay_per_attempt.clone(),
        )
        .await;

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        let res: Vec<_> = fresh_reader_test!(alice_env, |mut r| {
            workspace
                .validation_limbo
                .iter(&mut r)
                .unwrap()
                .map(|(k, i)| Ok((k.to_vec(), i)))
                .collect()
                .unwrap()
        });
        {
            let s = debug_span!("inspect_ops");
            let _g = s.enter();
            let element_buf = ElementBuf::vault(alice_env.clone().into(), true).unwrap();
            for (k, i) in &res {
                let hash = DhtOpHash::from_raw_39(k.clone());
                let el = element_buf.get_element(&i.op.header_hash()).unwrap();
                debug!(?hash, ?i, op_in_val = ?el);
            }
        }
        assert_eq!(res.len(), 0, "{:?}", res);
        let int_limbo: Vec<_> = fresh_reader_test!(alice_env, |mut r| {
            workspace
                .integration_limbo
                .iter(&mut r)
                .unwrap()
                .map(|(k, i)| Ok((k.to_vec(), i)))
                .collect()
                .unwrap()
        });
        assert_eq!(int_limbo.len(), 0, "{:?}", int_limbo);
        let res: Vec<_> = fresh_reader_test!(alice_env, |mut r| {
            workspace
                .integrated_dht_ops
                .iter(&mut r)
                .unwrap()
                // Every op should be valid
                .inspect(|(_, i)| {
                    // let s = debug_span!("inspect_ops");
                    // let _g = s.enter();
                    debug!(?i.op);
                    assert_eq!(i.validation_status, ValidationStatus::Valid);
                    Ok(())
                })
                .map(|(_, i)| Ok(i))
                .collect()
                .unwrap()
        });
        {
            let s = debug_span!("inspect_ops");
            let _g = s.enter();
            let element_buf = ElementBuf::vault(alice_env.clone().into(), true).unwrap();
            for i in &res {
                let el = element_buf.get_element(&i.op.header_hash()).unwrap();
                debug!(?i.op, op_in_buf = ?el);
            }
        }

        assert_eq!(res.len(), expected_count, "{:?}", res);
    }

    let (bad_update_header, bad_update_entry_hash, link_add_hash) =
        bob_makes_a_large_link(&bob_cell_id, &handle, &dna_file).await;

    // Integration should have 13 ops in it
    let expected_count = 14 + expected_count;

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
        wait_for_integration(
            &alice_env,
            expected_count,
            num_attempts,
            delay_per_attempt.clone(),
        )
        .await;

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should be empty
        assert_eq!(
            fresh_reader_test!(alice_env, |mut r| workspace
                .validation_limbo
                .iter(&mut r)
                .unwrap()
                .inspect(|(_, i)| {
                    let s = debug_span!("inspect_ops");
                    let _g = s.enter();
                    debug!(?i.op);
                    assert_eq!(i.status, ValidationLimboStatus::Pending);
                    Ok(())
                })
                .count()
                .unwrap()),
            0
        );

        let bad_update_entry_hash: AnyDhtHash = bad_update_entry_hash.into();

        let int_limbo: Vec<_> = fresh_reader_test!(alice_env, |mut r| workspace
            .integration_limbo
            .iter(&mut r)
            .unwrap()
            .map(|(_, v)| Ok(v.clone()))
            .collect()
            .unwrap());

        assert_eq!(
            fresh_reader_test!(alice_env, |mut r| workspace
                .integrated_dht_ops
                .iter(&mut r)
                .unwrap()
                // Every op should be valid except register updated by
                // Store entry for the update
                .inspect(|(_, i)| {
                    let s = debug_span!("inspect_ops");
                    let _g = s.enter();
                    debug!(?i.op);
                    match &i.op {
                        DhtOpLight::StoreEntry(hh, _, eh)
                            if eh == &bad_update_entry_hash && hh == &bad_update_header =>
                        {
                            assert_eq!(i.validation_status, ValidationStatus::Rejected)
                        }
                        DhtOpLight::StoreElement(hh, _, _) if hh == &bad_update_header => {
                            assert_eq!(i.validation_status, ValidationStatus::Rejected)
                        }
                        DhtOpLight::RegisterAddLink(hh, _) if hh == &link_add_hash => {
                            assert_eq!(i.validation_status, ValidationStatus::Rejected)
                        }
                        DhtOpLight::RegisterUpdatedContent(hh, _, _)
                            if hh == &bad_update_header =>
                        {
                            assert_eq!(i.validation_status, ValidationStatus::Rejected)
                        }
                        DhtOpLight::RegisterUpdatedElement(hh, _, _)
                            if hh == &bad_update_header =>
                        {
                            assert_eq!(i.validation_status, ValidationStatus::Rejected)
                        }
                        _ => assert_eq!(i.validation_status, ValidationStatus::Valid),
                    }
                    Ok(())
                })
                .count()
                .unwrap()),
            expected_count,
            "{:?}",
            int_limbo,
        );
    }

    dodgy_bob(&bob_cell_id, &handle, &dna_file).await;

    // Integration should have new 4 ops in it
    let expected_count = 4 + expected_count;

    {
        let alice_env = handle.get_cell_env(&alice_cell_id).await.unwrap();
        wait_for_integration(
            &alice_env,
            expected_count,
            num_attempts,
            delay_per_attempt.clone(),
        )
        .await;

        let workspace = IncomingDhtOpsWorkspace::new(alice_env.clone().into()).unwrap();
        // Validation should still contain bobs link pending because the target was missing
        assert_eq!(
            {
                let mut guard = alice_env.guard();
                let mut r = guard.reader().unwrap();
                workspace
                    .validation_limbo
                    .iter(&mut r)
                    .unwrap()
                    .inspect(|(_, i)| {
                        let s = debug_span!("inspect_ops");
                        let _g = s.enter();
                        debug!(?i.op);
                        assert_matches!(
                            i.status,
                            ValidationLimboStatus::Pending
                                | ValidationLimboStatus::AwaitingAppDeps(_)
                        );
                        Ok(())
                    })
                    .count()
                    .unwrap()
            },
            2
        );
        assert_eq!(
            {
                let mut guard = alice_env.guard();
                let mut r = guard.reader().unwrap();
                workspace
                    .integrated_dht_ops
                    .iter(&mut r)
                    .unwrap()
                    .count()
                    .unwrap()
            },
            expected_count
        );
    }
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
    triggers.produce_dht_ops.trigger();
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

    let bytes = (0..401).map(|_| 0u8).into_iter().collect::<Vec<_>>();
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
    triggers.produce_dht_ops.trigger();
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
    triggers.produce_dht_ops.trigger();
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
