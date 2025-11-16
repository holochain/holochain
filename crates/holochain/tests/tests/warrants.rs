use hdk::prelude::{
    ActionHashed, ActivityRequest, CellId, ChainFilter, ChainTopOrdering, CreateInput, EntryDef,
    EntryDefIndex, EntryVisibility, GetAgentActivityInput, MustGetAgentActivityInput, Op,
    SerializedBytes, ValidateCallbackResult,
};
use holo_hash::{ActionHash, DnaHash};
use holochain::{
    prelude::{DisabledAppReason, InlineZomeSet},
    sweettest::{
        await_consistency, SweetCell, SweetConductor, SweetConductorBatch, SweetConductorConfig,
        SweetDnaFile, SweetInlineZomes,
    },
    test_utils::retry_fn_until_timeout,
};
use holochain_sqlite::prelude::ReadAccess;
use holochain_state::prelude::{
    dump_db, insert_op_dht, set_validation_status, set_when_integrated,
};
use holochain_state::query::{from_blob, CascadeTxnWrapper, StateQueryResult, Store};
use holochain_timestamp::Timestamp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::prelude::WarrantOp;
use holochain_zome_types::op::ChainOpType;
use holochain_zome_types::prelude::{ChainIntegrityWarrant, ValidationStatus, Warrant};
use holochain_zome_types::record::SignedAction;
use holochain_zome_types::warrant::WarrantProof;
use holochain_zome_types::Entry;
use rusqlite::named_params;
use serde::{Deserialize, Serialize};

// Alice creates an invalid op and publishes it to Bob. Bob issues a warrant and
// blocks Alice.
#[tokio::test(flavor = "multi_thread")]
async fn warranted_agent_is_blocked() {
    holochain_trace::test_run();

    let config = SweetConductorConfig::rendezvous(true);
    let TestCase {
        mut conductors_and_cells,
        dna_hash,
    } = TestCase::create([config.clone(), config]).await;
    let (alice_conductor, alice_cell) = conductors_and_cells.remove(0);
    let (bob_conductor, bob_cell) = conductors_and_cells.remove(0);

    // Let all agents sync.
    await_consistency(10, [&alice_cell, &bob_cell])
        .await
        .unwrap();

    // Alice creates an invalid action.
    let _: ActionHash = alice_conductor
        .call(
            &alice_cell.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            "entry1".to_string(),
        )
        .await;

    await_consistency(10, [&alice_cell, &bob_cell])
        .await
        .unwrap();

    // The warrant against Alice and the warrant op should have been written to Bob's authored database.
    retry_fn_until_timeout(
        || async {
            let alice_pubkey = alice_cell.agent_pubkey().clone();
            let warrants = bob_conductor
                .get_spaces()
                .get_all_authored_dbs(&dna_hash)
                .unwrap()[0]
                .test_read(move |txn| {
                    let store = CascadeTxnWrapper::from(txn);
                    store.get_warrants_for_agent(&alice_pubkey, false).unwrap()
                });

            tracing::info!("number of warrants: {}", warrants.len());

            warrants.len() == 1 && warrants[0].warrant().warrantee == *alice_cell.agent_pubkey()
        },
        Some(5_000),
        None,
    )
    .await
    .unwrap();

    // Check that Alice is blocked by Bob.
    let target = hdk::prelude::BlockTargetId::Cell(CellId::new(
        dna_hash.clone(),
        alice_cell.agent_pubkey().clone(),
    ));
    assert!(bob_conductor
        .holochain_p2p()
        .is_blocked(target)
        .await
        .unwrap());
}

// Test that warrants are gossiped and received.
// Alice, Bob and Carol start a network and sync. Carol goes offline.
// Alice creates an invalid op, Bob receives it and issues a warrant.
// Carol has warrant issuance disabled and receives the warrant from Bob
// via gossip after she comes back online.
// Publish is disabled for this test.
#[tokio::test(flavor = "multi_thread")]
async fn warrant_is_gossiped() {
    holochain_trace::test_run();

    let config =
        SweetConductorConfig::rendezvous(true).tune_network_config(|nc| nc.disable_publish = true);
    // Disable warrants on Carol's conductor, so that she doesn't issue warrants herself
    // but receives them from Bob.
    let config_no_warranting = config
        .clone()
        .tune_conductor(|tc| tc.disable_warrant_issuance = true);

    let TestCase {
        mut conductors_and_cells,
        dna_hash,
    } = TestCase::create([config.clone(), config, config_no_warranting]).await;
    let (alice_conductor, alice_cell) = conductors_and_cells.remove(0);
    let (_bob_conductor, bob_cell) = conductors_and_cells.remove(0);
    let (carol_conductor, carol_cell) = conductors_and_cells.remove(0);

    await_consistency(10, [&alice_cell, &bob_cell, &carol_cell])
        .await
        .unwrap();

    // Disable Carol's app.
    carol_conductor
        .disable_app("test_app".into(), DisabledAppReason::User)
        .await
        .unwrap();

    // Alice creates an invalid action.
    let _: ActionHash = alice_conductor
        .call(
            &alice_cell.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            "s".to_string(),
        )
        .await;

    await_consistency(30, [&alice_cell, &bob_cell])
        .await
        .unwrap();

    // Bob should have issued a warrant against Alice.

    // Disable Alice's app and start Carol's.
    alice_conductor
        .disable_app("test_app".into(), DisabledAppReason::User)
        .await
        .unwrap();
    carol_conductor.enable_app("test_app".into()).await.unwrap();

    // Carol should receive the warrant against Alice.
    // The warrant and the warrant op should have been written to the DHT database,
    // as well as the invalid ops.
    retry_fn_until_timeout(
        || async {
            let alice_pubkey = alice_cell.agent_pubkey().clone();
            let invalid_ops = carol_conductor
                .get_invalid_integrated_ops(&carol_conductor.get_dht_db(&dna_hash).unwrap())
                .await
                .unwrap();
            invalid_ops.len() == 3 && {
                let warrants = carol_conductor
                    .get_spaces()
                    .dht_db(&dna_hash)
                    .unwrap()
                    .test_read(move |txn| {
                        let store = CascadeTxnWrapper::from(txn);
                        store.get_warrants_for_agent(&alice_pubkey, true).unwrap()
                    });
                !warrants.is_empty()
                    && warrants[0].warrant().warrantee == *alice_cell.agent_pubkey()
                    && warrants[0].warrant().author == *bob_cell.agent_pubkey() // Make sure that Bob authored the warrant and it's not been authored by Carol.
            }
        },
        Some(30_000),
        None,
    )
    .await
    .unwrap();
}

// Alice publishes a valid op, then Bob issues a warrant against her for an invalid op. Alice should
// block Bob, since the warrant is invalid.
#[tokio::test(flavor = "multi_thread")]
async fn author_of_invalid_warrant_is_blocked() {
    holochain_trace::test_run();

    #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
    struct AppString(String);

    let string_entry_def = EntryDef::default_from_id("string");
    let inline_zome = SweetInlineZomes::new(vec![string_entry_def], 0)
        .function("create_string", |api, s: String| {
            let entry = Entry::app(AppString(s).try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .integrity_function("validate", move |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        });

    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(inline_zome).await;

    let config = SweetConductorConfig::standard();

    let mut conductors = SweetConductorBatch::from_configs([config.clone(), config]).await;

    let apps = conductors.setup_app("app", [&dna_file]).await.unwrap();

    let ((alice,), (bob,)) = apps.into_tuples();

    // Force full storage arcs so that Alice and Bob are valid publish targets for each other.
    conductors[0]
        .declare_full_storage_arcs(dna_file.dna_hash())
        .await;
    conductors[1]
        .declare_full_storage_arcs(dna_file.dna_hash())
        .await;
    conductors.exchange_peer_info().await;

    // Alice creates a valid action.
    let valid_action_hash: ActionHash = conductors[0]
        .call(
            &alice.zome(SweetInlineZomes::COORDINATOR),
            "create_string",
            "text".to_string(),
        )
        .await;

    // Wait for Alice and Bob to sync.
    await_consistency(10, [&alice, &bob]).await.unwrap();

    let alice_authored_db = conductors[0]
        .get_spaces()
        .get_or_create_authored_db(dna_file.dna_hash(), alice.agent_pubkey().clone())
        .unwrap();
    let action = alice_authored_db
        .read_async(move |txn| -> StateQueryResult<SignedAction> {
            let action: Vec<u8> = txn.query_row(
                "SELECT blob FROM Action WHERE hash = :hash",
                named_params! {":hash": valid_action_hash},
                |row| row.get(0),
            )?;

            from_blob(action)
        })
        .await
        .unwrap();

    // Now Bob needs to create a warrant against Alice's perfectly valid action.
    let warrant = Warrant::new(
        WarrantProof::ChainIntegrity(ChainIntegrityWarrant::InvalidChainOp {
            action_author: alice.agent_pubkey().clone(),
            action: (
                ActionHashed::from_content_sync(action.action().clone()).hash,
                action.signature().clone(),
            ),
            chain_op_type: ChainOpType::StoreRecord,
        }),
        bob.agent_pubkey().clone(),
        Timestamp::now(),
        alice.agent_pubkey().clone(),
    );
    let warrant_op = WarrantOp::sign(&conductors[1].keystore(), warrant)
        .await
        .unwrap();

    // Insert the warrant into Bob's DHT database.
    let warrant_op_hashed = DhtOpHashed::from_content_sync(warrant_op);

    conductors[1]
        .get_dht_db(dna_file.dna_hash())
        .unwrap()
        .test_write(move |txn| {
            insert_op_dht(txn, &warrant_op_hashed, 0, None).unwrap();
            set_validation_status(txn, &warrant_op_hashed.hash, ValidationStatus::Valid).unwrap();
            set_when_integrated(txn, &warrant_op_hashed.hash, Timestamp::now()).unwrap();
        });

    // Wait for Alice and Bob to sync so that Alice receives the warrant.
    await_consistency(10, [&alice, &bob]).await.unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            tracing::error!("Looping to check for warrant and block...");

            let alice_pubkey = alice.agent_pubkey().clone();
            let warrants = conductors[0]
                .get_dht_db(dna_file.dna_hash())
                .unwrap()
                .test_read(move |txn| {
                    dump_db(txn);

                    txn.query_row("select count(*) from Warrant", [], |r| r.get::<_, i32>(0))
                        .map(|c| tracing::warn!("Warrant count: {}", c))
                        .unwrap();

                    let store = CascadeTxnWrapper::from(txn);
                    store.get_warrants_for_agent(&alice_pubkey, false).unwrap()
                });

            tracing::warn!("Warrants: {:#?}", warrants);

            // Alice should have stored the warrant against herself.
            if !warrants.is_empty() {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }

        loop {
            // Check that Bob gets blocked by Alice.
            let target = hdk::prelude::BlockTargetId::Cell(CellId::new(
                dna_file.dna_hash().clone(),
                bob.agent_pubkey().clone(),
            ));

            if conductors[0]
                .holochain_p2p()
                .is_blocked(target)
                .await
                .unwrap()
            {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
}

mod zero_arc {
    use super::*;
    use hdk::prelude::{AgentActivity, BlockTargetId, RegisterAgentActivity};
    use holochain::prelude::DisabledAppReason;

    // Alice creates an invalid op, Bob receives it and issues a warrant.
    // Carol is a zero arc node and makes a get_agent_activity request to Bob.
    // Bob serves the warrant.
    #[tokio::test(flavor = "multi_thread")]
    async fn zero_arc_node_is_served_warrant() {
        holochain_trace::test_run();

        let config = SweetConductorConfig::rendezvous(true);
        // Carol's conductor is a zero arc conductor.
        let config_zero_arc = config
            .clone()
            .tune_network_config(|nc| nc.target_arc_factor = 0);

        let TestCase {
            mut conductors_and_cells,
            dna_hash,
        } = TestCase::create([config.clone(), config, config_zero_arc]).await;
        let (alice_conductor, alice_cell) = conductors_and_cells.remove(0);
        let (bob_conductor, bob_cell) = conductors_and_cells.remove(0);
        let (carol_conductor, carol_cell) = conductors_and_cells.remove(0);

        await_consistency(10, [&alice_cell, &bob_cell])
            .await
            .unwrap();
        bob_conductor
            .holochain_p2p()
            .test_set_full_arcs(dna_hash.to_k2_space())
            .await;
        // Update Bob's peer info in Carol's peer store after the full storage arc declaration.
        SweetConductor::exchange_peer_info([&bob_conductor, &carol_conductor]).await;

        // Alice creates an invalid action.
        let _: ActionHash = alice_conductor
            .call(
                &alice_cell.zome(SweetInlineZomes::COORDINATOR),
                "create_string",
                "s".to_string(),
            )
            .await;

        await_consistency(10, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Bob should have issued a warrant against Alice.

        // Disabling Alice's app to ensure that Carol's request goes to Bob.
        alice_conductor
            .disable_app("test_app".to_string(), DisabledAppReason::User)
            .await
            .unwrap();

        // Carol queries Alice's agent activity and should get the warrant.
        let alice_activity: AgentActivity = carol_conductor
            .call(
                &carol_cell.zome(SweetInlineZomes::COORDINATOR),
                "get_agent_activity",
                alice_cell.agent_pubkey().clone(),
            )
            .await;
        assert!(!alice_activity.warrants.is_empty());
        alice_activity.warrants.into_iter().for_each(|warrant| {
            assert_eq!(warrant.warrantee, *alice_cell.agent_pubkey());
        });
    }

    // Alice creates an invalid op, Bob receives it and issues a warrant.
    // Carol is a zero arc node and makes a get_agent_activity request to Bob.
    // Bob serves the warrant.
    #[tokio::test(flavor = "multi_thread")]
    async fn warrantees_returned_from_get_agent_activity_are_blocked() {
        holochain_trace::test_run();

        let config = SweetConductorConfig::rendezvous(true);
        // Carol's conductor is a zero arc conductor.
        let config_zero_arc = SweetConductorConfig::rendezvous(true)
            .tune_network_config(|nc| nc.target_arc_factor = 0);

        let TestCase {
            mut conductors_and_cells,
            dna_hash,
        } = TestCase::create([config.clone(), config, config_zero_arc]).await;
        let (alice_conductor, alice_cell) = conductors_and_cells.remove(0);
        let (bob_conductor, bob_cell) = conductors_and_cells.remove(0);
        let (carol_conductor, carol_cell) = conductors_and_cells.remove(0);

        await_consistency(10, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Ensure that Carol knows about Bob's full arc.
        bob_conductor
            .holochain_p2p()
            .test_set_full_arcs(dna_hash.to_k2_space())
            .await;
        // Update Bob's peer info in Carol's peer store after the full storage arc declaration.
        SweetConductor::exchange_peer_info([&bob_conductor, &carol_conductor]).await;

        // Alice creates an invalid action.
        let _: ActionHash = alice_conductor
            .call(
                &alice_cell.zome(SweetInlineZomes::COORDINATOR),
                "create_string",
                "entry1".to_string(),
            )
            .await;

        await_consistency(10, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Bob should have issued a warrant against Alice.

        // Disable Alice's app so that Carol's request will go to Bob.
        alice_conductor
            .disable_app("test_app".to_string(), DisabledAppReason::User)
            .await
            .unwrap();

        // Carol calls get_agent_activity on Alice and blocks the warrant authors.
        let _: AgentActivity = carol_conductor
            .call(
                &carol_cell.zome(SweetInlineZomes::COORDINATOR),
                "get_agent_activity",
                alice_cell.agent_pubkey().clone(),
            )
            .await;

        // Check that Carol has blocked Alice.
        retry_fn_until_timeout(
            || async {
                carol_conductor
                    .holochain_p2p()
                    .is_blocked(BlockTargetId::Cell(CellId::new(
                        dna_hash.clone(),
                        alice_cell.agent_pubkey().clone(),
                    )))
                    .await
                    .unwrap()
            },
            Some(10_000),
            None,
        )
        .await
        .unwrap();
    }

    // Alice creates an invalid op, Bob receives it and issues a warrant.
    // Carol is a zero arc node and makes a must_get_agent_activity request to Bob.
    // Bob serves the warrant. Carol validates the warrant and blocks Alice.
    // This test is contrived, because must_get_agent_activity is more commonly
    // called in validation callbacks. But it serves the purpose of demonstrating
    // that discovered warrants lead to blocks, in a simple way.
    #[tokio::test(flavor = "multi_thread")]
    async fn warrantee_returned_from_must_get_agent_activity_is_blocked() {
        holochain_trace::test_run();

        let config = SweetConductorConfig::rendezvous(true);
        // Carol's conductor is a zero arc conductor.
        let config_zero_arc = SweetConductorConfig::rendezvous(true)
            .tune_network_config(|nc| nc.target_arc_factor = 0);

        let TestCase {
            mut conductors_and_cells,
            dna_hash,
        } = TestCase::create([config.clone(), config, config_zero_arc]).await;
        let (alice_conductor, alice_cell) = conductors_and_cells.remove(0);
        let (bob_conductor, bob_cell) = conductors_and_cells.remove(0);
        let (carol_conductor, carol_cell) = conductors_and_cells.remove(0);

        await_consistency(10, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Ensure that Carol knows about Bob's full arc.
        bob_conductor
            .holochain_p2p()
            .test_set_full_arcs(dna_hash.to_k2_space())
            .await;
        // Update Bob's peer info in Carol's peer store after the full storage arc declaration.
        SweetConductor::exchange_peer_info([&bob_conductor, &carol_conductor]).await;

        // Alice creates an invalid action.
        let action_hash: ActionHash = alice_conductor
            .call(
                &alice_cell.zome(SweetInlineZomes::COORDINATOR),
                "create_string",
                "entry1".to_string(),
            )
            .await;

        await_consistency(10, [&alice_cell, &bob_cell])
            .await
            .unwrap();

        // Bob should have issued a warrant against Alice.

        // Disable Alice's app so that Carol's request will go only to Bob.
        alice_conductor
            .disable_app("test_app".to_string(), DisabledAppReason::User)
            .await
            .unwrap();

        // Carol calls must_get_agent_activity on Alice and blocks the warrant authors.
        let _: Vec<RegisterAgentActivity> = carol_conductor
            .call(
                &carol_cell.zome(SweetInlineZomes::COORDINATOR),
                "must_get_agent_activity",
                MustGetAgentActivityInput {
                    author: alice_cell.agent_pubkey().clone(),
                    chain_filter: ChainFilter::new(action_hash),
                },
            )
            .await;

        // Check that Carol has blocked Alice.
        retry_fn_until_timeout(
            || async {
                carol_conductor
                    .holochain_p2p()
                    .is_blocked(BlockTargetId::Cell(CellId::new(
                        dna_hash.clone(),
                        alice_cell.agent_pubkey().clone(),
                    )))
                    .await
                    .unwrap()
            },
            Some(10_000),
            None,
        )
        .await
        .unwrap();
    }
}

// The first conductor and cell use the zome without validation, in other words the bad actor.
// All subsequent conductors/cells are honest actors and use the zome with validation.
struct TestCase {
    conductors_and_cells: Vec<(SweetConductor, SweetCell)>,
    dna_hash: DnaHash,
}

impl TestCase {
    async fn create<T: IntoIterator<Item = SweetConductorConfig>>(configs: T) -> Self {
        #[derive(Serialize, Deserialize, SerializedBytes, Debug)]
        struct AppString(String);

        let string_entry_def = EntryDef::default_from_id("string");
        let zome_common = SweetInlineZomes::new(vec![string_entry_def], 0)
            .function("create_string", |api, s: String| {
                let entry = Entry::app(AppString(s).try_into().unwrap()).unwrap();
                let hash = api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                    EntryVisibility::Public,
                    entry,
                    ChainTopOrdering::default(),
                ))?;
                Ok(hash)
            })
            .function("get_agent_activity", |api, agent_pubkey| {
                Ok(api.get_agent_activity(GetAgentActivityInput {
                    agent_pubkey,
                    chain_query_filter: Default::default(),
                    activity_request: ActivityRequest::Full,
                })?)
            })
            .function(
                "must_get_agent_activity",
                |api, input: MustGetAgentActivityInput| Ok(api.must_get_agent_activity(input)?),
            );

        let zome_without_validation = zome_common
            .clone()
            .integrity_function("validate", move |_api, _op: Op| {
                Ok(ValidateCallbackResult::Valid)
            });
        // Any action after the genesis actions is invalid.
        let zome_with_validation =
            zome_common
                .clone()
                .integrity_function("validate", move |_api, op: Op| {
                    if op.action_seq() > 3 {
                        Ok(ValidateCallbackResult::Invalid("nope".to_string()))
                    } else {
                        Ok(ValidateCallbackResult::Valid)
                    }
                });

        let network_seed = "seed".to_string();

        let (dna_without_validation, _, _) =
            SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_without_validation).await;
        let (dna_with_validation, _, _) =
            SweetDnaFile::from_inline_zomes(network_seed.clone(), zome_with_validation).await;
        assert_eq!(
            dna_without_validation.dna_hash(),
            dna_with_validation.dna_hash()
        );
        let dna_hash = dna_without_validation.dna_hash().clone();

        let conductors = SweetConductorBatch::from_configs_rendezvous(configs).await;
        let mut conductors = conductors.into_inner();
        let mut conductors_and_cells = Vec::new();

        // Set up bad actor conductor and cell.
        let mut bad_conductor = conductors.remove(0);
        let (bad_cell,) = bad_conductor
            .setup_app("test_app", [&dna_without_validation])
            .await
            .unwrap()
            .into_tuple();
        conductors_and_cells.push((bad_conductor, bad_cell));

        // Set up honest conductors and cells.
        for mut conductor in conductors.into_iter() {
            let (cell,) = conductor
                .setup_app("test_app", [&dna_with_validation])
                .await
                .unwrap()
                .into_tuple();
            conductors_and_cells.push((conductor, cell));
        }
        Self {
            conductors_and_cells,
            dna_hash,
        }
    }
}
