use holo_hash::ActionHash;
use holochain::sweettest::{await_consistency, SweetConductorBatch, SweetDnaFile};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::AgentActivity;
use holochain_zome_types::query::ChainStatus;
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
