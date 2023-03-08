use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::prelude::InlineZomeSet;
use crate::sweettest::*;
use crate::test_utils::consistency_10s;
use crate::test_utils::inline_zomes::simple_create_read_zome;
use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use rusqlite::Transaction;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky"]
async fn test_validation_receipt() {
    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let apps = conductors.setup_app("app", &[dna_file]).await.unwrap();
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    consistency_10s([&alice, &bobbo, &carol]).await;

    // Get op hashes
    let vault = alice.dht_db().clone().into();
    let record = fresh_store_test(&vault, |store| {
        store.get_record(&hash.clone().into()).unwrap().unwrap()
    });
    let ops = produce_ops_from_record(&record)
        .unwrap()
        .into_iter()
        .map(|op| DhtOpHash::with_data_sync(&op))
        .collect::<Vec<_>>();

    // Wait for receipts to be sent
    crate::assert_eq_retry_10s!(
        {
            let mut counts = Vec::new();
            for hash in &ops {
                let count = fresh_reader_test!(vault, |r| list_receipts(&r, hash).unwrap().len());
                counts.push(count);
            }
            counts
        },
        vec![2, 2, 2],
    );

    // Check alice has receipts from both bobbo and carol
    for hash in ops {
        let receipts: Vec<_> =
            fresh_reader_test!(vault, |mut r| list_receipts(&mut r, &hash).unwrap());
        assert_eq!(receipts.len(), 2);
        for receipt in receipts {
            let SignedValidationReceipt {
                receipt,
                validators_signatures: sigs,
            } = receipt;
            let validator = receipt.validators[0].clone();
            assert!(validator == *bobbo.agent_pubkey() || validator == *carol.agent_pubkey());
            assert!(validator.verify_signature(&sigs[0], receipt).await);
        }
    }

    // Check alice has 2 receipts in their authored dht ops table.
    crate::assert_eq_retry_1m!(
        {
            fresh_reader_test!(vault, |txn: Transaction| {
                let mut stmt = txn
                    .prepare("SELECT COUNT(hash) FROM ValidationReceipt GROUP BY op_hash")
                    .unwrap();
                stmt.query_map([], |row| row.get::<_, Option<u32>>(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .filter_map(|i| i)
                    .collect::<Vec<u32>>()
            })
        },
        vec![2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_block_invalid_receipt() {
    observability::test_run().ok();
    let unit_entry_def = EntryDef::from_id("unit");
    let integrity_name = "integrity";
    let coordinator_name = "coordinator";
    let integrity_uuid = "a";
    let create_coordinator_uuid = "b";
    let check_coordinator_uuid = "c";
    let network_seed = "network_seed";
    let create_function_name = "create";
    let app_prefix = "app-";

    let zomes_that_create = InlineZomeSet::new_single(
        integrity_name,
        coordinator_name,
        integrity_uuid,
        create_coordinator_uuid,
        vec![unit_entry_def.clone()],
        0,
    )
    .function(coordinator_name, create_function_name, move |api, ()| {
        let entry = Entry::app(().try_into().unwrap()).unwrap();
        let hash = api.create(CreateInput::new(
            InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
            EntryVisibility::Public,
            entry,
            ChainTopOrdering::default(),
        ))?;
        Ok(hash)
    });

    let zomes_that_check = InlineZomeSet::new_single(
        integrity_name,
        coordinator_name,
        integrity_uuid,
        check_coordinator_uuid,
        vec![unit_entry_def.clone()],
        0,
    )
    .function(integrity_name, "validate", |_api, op: Op| {
        dbg!("VALIDATE");
        match op {
            Op::StoreEntry(StoreEntry { action, .. })
                if action.hashed.content.app_entry_def().is_some() =>
            {
                dbg!("INVALID");
                Ok(ValidateResult::Invalid("Entry defs are bad".into()))
            }
            _ => Ok(ValidateResult::Valid),
        }
    });

    let config = SweetConductorConfig::standard();
    let conductors = SweetConductorBatch::from_config(2, config).await;
    conductors.exchange_peer_info().await;
    let mut conductors = conductors.into_inner().into_iter();

    let mut alice_conductor = conductors.next().unwrap();
    let mut bob_conductor = conductors.next().unwrap();
    let (alice_pubkey, bob_pubkey) = SweetAgents::alice_and_bob();

    let (dna_that_creates, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.into(), zomes_that_create).await;

    let (dna_that_checks, _, _) =
        SweetDnaFile::from_inline_zomes(network_seed.into(), zomes_that_check).await;

    let alice_apps = alice_conductor
        .setup_app_for_agents(app_prefix, &[alice_pubkey.clone()], &[dna_that_creates])
        .await
        .unwrap();

    let ((alice_cell,),) = alice_apps.into_tuples();

    let bob_apps = bob_conductor
        .setup_app_for_agents(app_prefix, &[bob_pubkey.clone()], &[dna_that_checks])
        .await
        .unwrap();

    let ((bob_cell,),) = bob_apps.into_tuples();

    let action_hash: ActionHash = alice_conductor
        .call(&alice_cell.zome(coordinator_name), create_function_name, ())
        .await;

    dbg!(action_hash);

    consistency_10s([&alice_cell, &bob_cell]).await;
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
}
