use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::prelude::InlineZomeSet;
use crate::sweettest::*;
use crate::test_utils::inline_zomes::simple_create_read_zome;
use hdk::prelude::*;
use holo_hash::DhtOpHash;
use holochain_keystore::AgentPubKeyExt;
use holochain_state::prelude::*;
use rusqlite::Transaction;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "flaky, doesn't take into account timing or retries"]
async fn test_validation_receipt() {
    holochain_trace::test_run();
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

    await_consistency(10, [&alice, &bobbo, &carol])
        .await
        .unwrap();

    // Get op hashes
    let vault = alice.dht_db();
    let record = vault
        .read_async(move |txn| -> StateQueryResult<Record> {
            Ok(Txn::from(&txn).get_record(&hash.clone().into())?.unwrap())
        })
        .await
        .unwrap();
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
                let count = vault
                    .read_async({
                        let query_hash = hash.clone();
                        move |r| -> StateQueryResult<usize> {
                            Ok(list_receipts(&r, &query_hash)?.len())
                        }
                    })
                    .await
                    .unwrap();
                counts.push(count);
            }
            counts
        },
        vec![2, 2, 2],
    );

    // Check alice has receipts from both bobbo and carol
    for hash in &ops {
        let receipts: Vec<_> = vault
            .read_async({
                let query_hash = hash.clone();
                move |r| list_receipts(&r, &query_hash)
            })
            .await
            .unwrap();
        assert_eq!(receipts.len(), 2);
        for receipt in receipts {
            let SignedValidationReceipt {
                receipt,
                validators_signatures: sigs,
            } = receipt;
            let validator = receipt.validators[0].clone();
            assert!(validator == *bobbo.agent_pubkey() || validator == *carol.agent_pubkey());
            assert!(validator.verify_signature(&sigs[0], receipt).await.unwrap());
        }
    }

    // Check alice has 2 receipts in their authored dht ops table.
    crate::assert_eq_retry_1m!(
        {
            vault
                .read_async(move |txn: Transaction| -> DatabaseResult<Vec<u32>> {
                    let mut stmt = txn
                        .prepare("SELECT COUNT(hash) FROM ValidationReceipt GROUP BY op_hash")
                        .unwrap();
                    Ok(stmt
                        .query_map([], |row| row.get::<_, Option<u32>>(0))
                        .unwrap()
                        .map(Result::unwrap)
                        .flatten()
                        .collect::<Vec<u32>>())
                })
                .await
                .unwrap()
        },
        vec![2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]
    );
}

macro_rules! wait_until {
    ($expression:expr; $interval_ms:literal; $timeout_ms:literal; $wait_msg:literal; $timeout_msg:literal;) => {
        let timeout = (Timestamp::now() + std::time::Duration::from_millis($timeout_ms)).unwrap();
        let interval_duration = std::time::Duration::from_millis($interval_ms);
        tokio::time::sleep(interval_duration).await;
        while !$expression {
            if Timestamp::now() > timeout {
                panic!($timeout_msg);
            }
            dbg!($wait_msg);
            tokio::time::sleep(interval_duration).await;
        }
    };
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn test_block_invalid_receipt() {
    holochain_trace::test_run();
    let unit_entry_def = EntryDef::default_from_id("unit");
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
    // .function(
    //     coordinator_name,
    //     get_function_name,
    //     move |api, hash: AnyDhtHash| {
    //         let records = api.get(vec![GetInput::new(hash, Default::default())])?;
    //         Ok(records[0])
    //     },
    // );

    let zomes_that_check = InlineZomeSet::new_single(
        integrity_name,
        coordinator_name,
        integrity_uuid,
        check_coordinator_uuid,
        vec![unit_entry_def.clone()],
        0,
    )
    .function(integrity_name, "validate", |_api, op: Op| match op {
        Op::StoreEntry(StoreEntry { action, .. })
            if action.hashed.content.app_entry_def().is_some() =>
        {
            dbg!("entry defs ARE bad!");
            Ok(ValidateResult::Invalid("Entry defs are bad".into()))
        }
        _ => Ok(ValidateResult::Valid),
    });

    let config = SweetConductorConfig::rendezvous(true);
    let conductors = SweetConductorBatch::from_config_rendezvous(2, config).await;

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

    let _action_hash: ActionHash = alice_conductor
        .call(&alice_cell.zome(coordinator_name), create_function_name, ())
        .await;

    await_consistency(10, [&alice_cell, &bob_cell])
        .await
        .unwrap();

    let alice_block_target = BlockTargetId::Cell(alice_cell.cell_id().to_owned());
    let bob_block_target = BlockTargetId::Cell(bob_cell.cell_id().to_owned());

    for now in [Timestamp::now(), Timestamp::MIN, Timestamp::MAX] {
        assert!(!alice_conductor
            .spaces
            .is_blocked(alice_block_target.clone(), now)
            .await
            .unwrap());
        assert!(!alice_conductor
            .spaces
            .is_blocked(bob_block_target.clone(), now)
            .await
            .unwrap());
        assert!(!bob_conductor
            .spaces
            .is_blocked(bob_block_target.clone(), now)
            .await
            .unwrap());

        // It can take a little more than consistency to have the receipts
        // processed.
        wait_until!(
            bob_conductor.spaces.is_blocked(alice_block_target.clone(), now).await.unwrap();
            1000;
            10000;
            "waiting for block due to warrant";
            "warrant block never happened";
        );
    }
}
