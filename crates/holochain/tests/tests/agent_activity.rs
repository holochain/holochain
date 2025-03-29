use holo_hash::ActionHash;
use holochain::retry_until_timeout;
use holochain::sweettest::{await_consistency, SweetConductorBatch, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::AgentActivity;
use holochain_zome_types::query::ChainStatus;
use kitsune2_api::DhtArc;
use matches::assert_matches;

#[tokio::test(flavor = "multi_thread")]
async fn get_agent_activity() {
    holochain_trace::test_run();

    let mut conductor_batch = SweetConductorBatch::from_standard_config_rendezvous(2).await;

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Crd]).await;

    let cells = conductor_batch
        .setup_app("alice", [&dna])
        .await
        .unwrap()
        .cells_flattened();

    let alice_cell = cells.first().unwrap();
    let bob_cell = cells.last().unwrap();

    conductor_batch[0]
        .holochain_p2p()
        .test_set_full_arcs(dna.dna_hash().to_k2_space())
        .await;
    retry_until_timeout!(5_000, 1_000, {
        if conductor_batch[0]
            .holochain_p2p()
            .peer_store(dna.dna_hash().clone())
            .await
            .unwrap()
            .get(alice_cell.agent_pubkey().to_k2_agent())
            .await
            .unwrap()
            .unwrap()
            .storage_arc
            == DhtArc::FULL
        {
            break;
        }
    });

    let mut created_hashes = Vec::new();
    for _ in 0..5 {
        let created: ActionHash = conductor_batch[0]
            .call(
                &alice_cell.zome(TestWasm::Crd.coordinator_zome_name()),
                "create",
                (),
            )
            .await;

        created_hashes.push(created);
    }

    // Wait for gossip to have started, so we know that Bob will be able to connect to Alice
    conductor_batch[1]
        .require_initial_gossip_activity_for_cell(bob_cell, 1, std::time::Duration::from_secs(30))
        .await
        .unwrap();

    // TODO No way to force a network call to get the agent activity, so we have to wait for a sync
    //      first and then check the agent activity
    await_consistency(std::time::Duration::from_secs(60), [alice_cell, bob_cell])
        .await
        .unwrap();

    let agent_activity: AgentActivity = conductor_batch[1]
        .call(
            &bob_cell.zome(TestWasm::Crd.coordinator_zome_name()),
            "get_agent_activity",
            alice_cell.agent_pubkey().clone(),
        )
        .await;

    assert_matches!(agent_activity.status, ChainStatus::Valid(_));
    assert_eq!(4 + 5, agent_activity.valid_activity.len()); // 4 initial + 5 creates
    assert_eq!(
        created_hashes,
        agent_activity
            .valid_activity
            .iter()
            .skip(4)
            .map(|a| a.1.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(0, agent_activity.rejected_activity.len());
    assert_eq!(
        8,
        agent_activity.highest_observed.clone().unwrap().action_seq
    );
    assert_eq!(
        1,
        agent_activity.highest_observed.clone().unwrap().hash.len()
    );
    assert_eq!(
        created_hashes.last().unwrap(),
        agent_activity
            .highest_observed
            .unwrap()
            .hash
            .first()
            .unwrap()
    );
}
