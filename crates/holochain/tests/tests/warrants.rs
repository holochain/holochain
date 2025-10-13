use hdk::prelude::{
    ActivityRequest, CellId, ChainTopOrdering, CreateInput, EntryDef, EntryDefIndex,
    EntryVisibility, GetAgentActivityInput, Op, SerializedBytes, ValidateCallbackResult,
};
use holo_hash::{ActionHash, DnaHash};
use holochain::{
    prelude::InlineZomeSet,
    sweettest::{
        await_consistency, SweetCell, SweetConductor, SweetConductorBatch, SweetConductorConfig,
        SweetDnaFile, SweetInlineZomes,
    },
    test_utils::retry_fn_until_timeout,
};
use holochain_state::query::{CascadeTxnWrapper, Store};
use holochain_zome_types::Entry;
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

            warrants.len() == 3 && warrants[0].warrant().warrantee == *alice_cell.agent_pubkey()
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
    let config_no_warranting = SweetConductorConfig::rendezvous(true)
        .tune_conductor(|tc| tc.disable_warrant_issuance = true)
        .tune_network_config(|nc| nc.disable_publish = true);

    let TestCase {
        mut conductors_and_cells,
        dna_hash,
    } = TestCase::create([config.clone(), config, config_no_warranting]).await;
    let (mut alice_conductor, alice_cell) = conductors_and_cells.remove(0);
    let (_bob_conductor, bob_cell) = conductors_and_cells.remove(0);
    let (mut carol_conductor, carol_cell) = conductors_and_cells.remove(0);

    await_consistency(10, [&alice_cell, &bob_cell, &carol_cell])
        .await
        .unwrap();

    // Shutdown Carol's conductor.
    carol_conductor.shutdown().await;

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

    // Shutdown Alice and startup Carol.
    alice_conductor.shutdown().await;
    carol_conductor.startup(false).await;

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
                        // TODO: check_valid here should be removed once warrants are validated.
                        store.get_warrants_for_agent(&alice_pubkey, false).unwrap()
                    });
                warrants.len() == 3
                    && warrants[0].warrant().warrantee == *alice_cell.agent_pubkey()
                    && warrants[0].warrant().author == *bob_cell.agent_pubkey() // Make sure that Bob authored the warrant and it's not been authored by Carol.
            }
        },
        Some(10_000),
        None,
    )
    .await
    .unwrap();
}

mod zero_arc {
    use super::*;
    use hdk::prelude::AgentActivity;

    // Alice creates an invalid op, Bob receives it and issues a warrant.
    // Carol is a zero arc node and makes a get_agent_activity request to Bob.
    // Bob serves the warrant.
    #[tokio::test(flavor = "multi_thread")]
    async fn zero_arc_node_is_served_warrant() {
        holochain_trace::test_run();

        let config = SweetConductorConfig::rendezvous(true)
            .tune_network_config(|nc| nc.disable_publish = true);
        // Carol's conductor is a zero arc conductor.
        let config_zero_arc = SweetConductorConfig::rendezvous(true)
            .tune_network_config(|nc| nc.target_arc_factor = 0);

        let TestCase {
            mut conductors_and_cells,
            dna_hash,
        } = TestCase::create([config.clone(), config, config_zero_arc]).await;
        let (mut alice_conductor, alice_cell) = conductors_and_cells.remove(0);
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

        // Shutdown Alice so that Carol's request will go to Bob.
        alice_conductor.shutdown().await;

        // Carol queries Alice's agent activity and should get the warrant.
        let alice_activity: AgentActivity = carol_conductor
            .call(
                &carol_cell.zome(SweetInlineZomes::COORDINATOR),
                "get_agent_activity",
                alice_cell.agent_pubkey().clone(),
            )
            .await;
        assert_eq!(alice_activity.warrants.len(), 3);
        assert_eq!(
            alice_activity.warrants[0].warrantee,
            *alice_cell.agent_pubkey()
        );
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
            });

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
