use crate::test_utils::inline_zomes::simple_create_read_zome;
use parking_lot::Mutex;

use super::*;

/// Instead of reading KeyState from a Deepkey DNA's chain, we store the KeyState
/// of each agent in a hashmap.
type DpkiState = Arc<Mutex<HashMap<AgentPubKey, KeyState>>>;

fn make_mock_dpi_impl(keystore: MetaLairClient, state: DpkiState) -> DpkiImpl {
    let mut dpki = MockDpkiService::new();

    dpki.expect_uuid().return_const([1; 32]);
    dpki.expect_should_run().return_const(true);

    dpki.expect_key_state().returning({
        let state = state.clone();
        move |a, _t| {
            let state = state.lock().get(&a).cloned().unwrap_or(KeyState::NotFound);
            async move { Ok(state) }.boxed()
        }
    });

    dpki.expect_derive_and_register_new_key().returning({
        move |_, _| {
            let keystore = keystore.clone();
            let state = state.clone();
            async move {
                let agent = keystore.new_sign_keypair_random().await.unwrap();
                state
                    .lock()
                    .insert(agent.clone(), KeyState::Valid(fixt!(SignedActionHashed)));

                Ok(agent)
            }
            .boxed()
        }
    });

    Arc::new(dpki)
}

fn make_dpki_conductor_builder(
    // dpki: DpkiImpl,
    config: ConductorConfig,
    // keystore: MetaLairClient,
    state: DpkiState,
) -> ConductorBuilder {
    let keystore = holochain_keystore::test_keystore();
    let dpki = make_mock_dpi_impl(keystore.clone(), state);
    let mut builder = Conductor::builder()
        .config(config)
        .with_keystore(keystore)
        .no_print_setup();
    builder.dpki = Some(dpki);
    builder
}

async fn get_key_state(conductor: &SweetConductor, agent: &AgentPubKey) -> KeyState {
    conductor
        .running_services()
        .dpki
        .as_ref()
        .unwrap()
        .key_state(agent.clone(), Timestamp::now())
        .await
        .unwrap()
}

/// Check that if a node can't validate another agent's DPKI KeyState due to the state
/// not being present yet, it will retry and eventually successfully validate.
///
/// We actually do a consistency check which is expected to panic here, so there is
/// a bunch of panic output from this test.
#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn mock_dpki_validation_limbo() {
    holochain_trace::test_run().ok();

    let states = std::iter::repeat_with(|| Arc::new(Mutex::new(HashMap::new())))
        .take(2)
        .collect::<Vec<_>>();

    let rendezvous = SweetLocalRendezvous::new().await;

    let config: ConductorConfig = SweetConductorConfig::rendezvous(true)
        .into_conductor_config(&*rendezvous)
        .await;

    let mut conductors = SweetConductorBatch::new(vec![
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(config.clone(), states[0].clone()),
            rendezvous.clone(),
        )
        .await,
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(config.clone(), states[1].clone()),
            rendezvous.clone(),
        )
        .await,
    ]);

    let (app_dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let ((alice,), (bob,)) = conductors
        .setup_app("app", [&("role".to_string(), app_dna_file)])
        .await
        .unwrap()
        .into_tuples();

    assert!(matches!(
        get_key_state(&conductors[0], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        get_key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        get_key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        get_key_state(&conductors[1], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    let alice_clone = alice.clone();
    let bob_clone = bob.clone();
    tokio::spawn(async move {
        consistency(
            [&alice_clone, &bob_clone],
            3,
            tokio::time::Duration::from_secs(1),
        )
        .await
    })
    .await
    .unwrap_err();

    let record_bob: Option<Record> = conductors[1]
        .call(&bob.zome("simple"), "read", hash.clone())
        .await;

    assert!(record_bob.is_none());

    {
        // lock all state mutexes
        let mut ks: Vec<_> = states.iter().map(|s| s.lock()).collect();

        // exchange all key states
        let pairs = ks
            .iter()
            .flat_map(|h| (*h).clone().into_iter())
            .collect::<Vec<_>>();

        ks[0..=1].iter_mut().for_each(|h| {
            h.extend(pairs.clone());
        });
    }

    consistency_10s([&alice, &bob]).await;

    assert!(matches!(
        get_key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        get_key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

    let record_alice: Option<Record> = conductors[0]
        .call(&alice.zome("simple"), "read", hash.clone())
        .await;

    let record_bob: Option<Record> = conductors[1]
        .call(&bob.zome("simple"), "read", hash.clone())
        .await;

    assert!(record_alice.is_some());
    assert!(record_bob.is_some());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
async fn mock_dpki_invalid_key_state() {
    holochain_trace::test_run().ok();

    let states = std::iter::repeat_with(|| Arc::new(Mutex::new(HashMap::new())))
        .take(2)
        .collect::<Vec<_>>();

    let rendezvous = SweetLocalRendezvous::new().await;

    let config: ConductorConfig = SweetConductorConfig::rendezvous(true)
        .into_conductor_config(&*rendezvous)
        .await;

    let mut conductors = SweetConductorBatch::new(vec![
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(config.clone(), states[0].clone()),
            rendezvous.clone(),
        )
        .await,
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(config.clone(), states[1].clone()),
            rendezvous.clone(),
        )
        .await,
    ]);

    let (app_dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let ((alice,), (bob,)) = conductors
        .setup_app("app", [&("role".to_string(), app_dna_file)])
        .await
        .unwrap()
        .into_tuples();

    {
        let mut s0 = states[0].lock();
        let mut s1 = states[1].lock();

        let a0 = s0.keys().next().unwrap().clone();
        let a1 = s1.keys().next().unwrap().clone();

        // Alice thinks Bob's DPKI key is invalid
        s0.insert(a1, KeyState::Invalidated(fixt!(SignedActionHashed)));
        // But Bob thinks Alice is valid
        s1.insert(a0, KeyState::Valid(fixt!(SignedActionHashed)));
    }

    let hash: ActionHash = conductors[1].call(&bob.zome("simple"), "create", ()).await;

    // tokio::time::sleep(tokio::time::Duration::from_secs(3))
    let alice_clone = alice.clone();
    let bob_clone = bob.clone();
    tokio::spawn(async move {
        consistency(
            [&alice_clone, &bob_clone],
            3,
            tokio::time::Duration::from_secs(1),
        )
        .await
    })
    .await
    .unwrap_err();

    let record_alice: Option<Details> = conductors[0]
        .call(&alice.zome("simple"), "read_details", hash.clone())
        .await;

    assert_matches!(
        record_alice.unwrap(),
        Details::Record(RecordDetails {
            validation_status: ValidationStatus::Rejected,
            ..
        })
    );
}

/// Crude check that an agent without the same DPKI instance as others can't
/// validate actions, due to preflight check failure.
#[tokio::test(flavor = "multi_thread")]
async fn mock_dpki_preflight_check() {
    holochain_trace::test_run().ok();

    let states = std::iter::repeat_with(|| Arc::new(Mutex::new(HashMap::new())))
        .take(2)
        .collect::<Vec<_>>();

    let rendezvous = SweetLocalRendezvous::new().await;

    let config: ConductorConfig = SweetConductorConfig::rendezvous(true)
        .into_conductor_config(&*rendezvous)
        .await;

    let mut conductors = SweetConductorBatch::new(vec![
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(config.clone(), states[0].clone()),
            rendezvous.clone(),
        )
        .await,
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(config.clone(), states[1].clone()),
            rendezvous.clone(),
        )
        .await,
        SweetConductor::from_config_rendezvous(config, rendezvous.clone()).await,
    ]);

    let (app_dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;

    let ((alice,), (bob,), (carol,)) = conductors
        .setup_app("app", [&("role".to_string(), app_dna_file)])
        .await
        .unwrap()
        .into_tuples();

    assert!(matches!(
        get_key_state(&conductors[0], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        get_key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        get_key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        get_key_state(&conductors[1], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

    {
        // lock all state mutexes
        let mut ks: Vec<_> = states.iter().map(|s| s.lock()).collect();

        // exchange all key states
        let pairs = ks
            .iter()
            .flat_map(|h| (*h).clone().into_iter())
            .collect::<Vec<_>>();

        ks[0..=1].iter_mut().for_each(|h| {
            h.extend(pairs.clone());
        });
    }

    consistency_10s([&alice, &bob]).await;

    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    consistency_60s([&alice, &bob]).await;

    assert!(matches!(
        get_key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        get_key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

    // Carol is nowhere to be found since she never installed DPKI
    assert!(matches!(
        get_key_state(&conductors[0], carol.agent_pubkey()).await,
        KeyState::NotFound
    ));
    assert!(matches!(
        get_key_state(&conductors[1], carol.agent_pubkey()).await,
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
