use std::time::Duration;

use holochain::{
    conductor::config::DpkiConfig, sweettest::*, test_utils::inline_zomes::simple_create_read_zome,
};
use holochain_conductor_services::KeyState;
use holochain_types::prelude::*;

#[tokio::test(flavor = "multi_thread")]
async fn initialize_dpki() {
    holochain_trace::test_run();

    let mut config = SweetConductorConfig::standard();
    config.dpki = DpkiConfig::new(None, "TODO".to_string());
    let mut conductor = SweetConductor::from_config(config).await;

    assert!(conductor.running_services().dpki.is_some());

    // Install app
    {
        let (app_dna_file, _, _) =
            SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

        conductor
            .setup_app("installed_app_id", &[app_dna_file])
            .await
            .unwrap();
    }
}

/// Crude check that an agent without the same DPKI instance as others can't
/// validate actions
#[tokio::test(flavor = "multi_thread")]
async fn validate_with_dpki() {
    holochain_trace::test_run();

    let rendezvous = SweetLocalRendezvous::new().await;
    let config = SweetConductorConfig::rendezvous(true).tune_conductor(|p| {
        p.sys_validation_retry_delay = Some(Duration::from_secs(1));
    });

    let mut conductors = SweetConductorBatch::new(vec![
        SweetConductor::from_config_rendezvous(config.clone(), rendezvous.clone()).await,
        SweetConductor::from_config_rendezvous(config.clone(), rendezvous.clone()).await,
        SweetConductor::from_config_rendezvous(config.clone().no_dpki(), rendezvous.clone()).await,
    ]);

    let (app_dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let ((alice,), (bob,), (carol,)) = conductors
        .setup_app("app", [&app_dna_file])
        .await
        .unwrap()
        .into_tuples();

    async fn key_state(conductor: &SweetConductor, agent: &AgentPubKey) -> KeyState {
        conductor
            .running_services()
            .dpki
            .as_ref()
            .unwrap()
            .state()
            .await
            .key_state(agent.clone(), Timestamp::now())
            .await
            .unwrap()
    }

    assert!(matches!(
        key_state(&conductors[0], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        key_state(&conductors[1], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

    println!("--------------------------------------------");
    println!("AGENTS:");
    println!("alice: {:?}", alice.agent_pubkey());
    println!("bob:   {:?}", bob.agent_pubkey());
    println!("carol: {:?}", carol.agent_pubkey());
    println!("--------------------------------------------");

    await_consistency(30, &conductors.dpki_cells()[0..=1])
        .await
        .unwrap();
    await_consistency(30, [&alice, &bob]).await.unwrap();

    // Both now see each other in DPKI
    assert!(matches!(
        key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    await_consistency(30, [&alice, &bob]).await.unwrap();

    // Carol is nowhere to be found since she never installed DPKI
    assert!(matches!(
        key_state(&conductors[0], carol.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        key_state(&conductors[1], carol.agent_pubkey()).await,
        KeyState::NotFound
    ));

    let record_bob: Option<Record> = conductors[1]
        .call(&bob.zome("simple"), "read", hash.clone())
        .await;
    let record_carol: Option<Record> = conductors[2]
        .call(&carol.zome("simple"), "read", hash.clone())
        .await;

    assert!(record_bob.is_some());

    // Carol can't get the record. This doesn't necessarily prove that DPKI
    // is working, but it at least demonstrates something basic about validation.
    // A better test would check the *reason* why the record couldn't be fetched.
    assert!(
        record_carol.is_none(),
        "Carol should not be able to communicate with the other two"
    );
}
