use crate::test_utils::inline_zomes::simple_create_read_zome;
use arbitrary::{Arbitrary, Unstructured};
use parking_lot::Mutex;

use super::*;

/// Instead of reading KeyState from a Deepkey DNA's chain, we store the KeyState
/// of each agent in a hashmap.
type DpkiKeyState = Arc<Mutex<HashMap<AgentPubKey, KeyState>>>;

fn make_mock_dpki_impl(u: &mut Unstructured<'_>, state: DpkiKeyState) -> DpkiImpl {
    let mut dpki = MockDpkiState::new();

    let fake_register_key_output = (
        ActionHash::arbitrary(u).unwrap(),
        KeyRegistration::arbitrary(u).unwrap(),
        KeyMeta::arbitrary(u).unwrap(),
    );

    dpki.expect_key_state().returning({
        let state = state.clone();
        move |a, _t| {
            let state = state.lock().get(&a).cloned().unwrap_or(KeyState::NotFound);
            async move { Ok(state) }.boxed()
        }
    });

    dpki.expect_next_derivation_details().returning(move |_| {
        let app_index = AtomicU32::new(0);
        async move {
            Ok(DerivationDetails {
                app_index: app_index.fetch_add(1, Ordering::Relaxed),
                key_index: 0,
            })
        }
        .boxed()
    });

    dpki.expect_register_key().returning({
        move |input| {
            let state = state.clone();
            let fake_register_key_output = fake_register_key_output.clone();
            async move {
                let agent = input.key_generation.new_key;
                state
                    .lock()
                    .insert(agent.clone(), KeyState::Valid(fixt!(SignedActionHashed)));
                Ok(fake_register_key_output)
            }
            .boxed()
        }
    });

    // All share same DNA, but different agent
    let cell_id = CellId::new(DnaHash::from_raw_32(vec![0; 32]), fixt!(AgentPubKey));

    Arc::new(DpkiService::new(
        cell_id,
        "MOCK_DEVICE_SEED".to_string(),
        dpki,
    ))
}

async fn make_dpki_conductor_builder(
    u: &mut Unstructured<'_>,
    // dpki: DpkiImpl,
    config: ConductorConfig,
    // keystore: MetaLairClient,
    state: DpkiKeyState,
) -> ConductorBuilder {
    let keystore = test_keystore();

    // Generate DPKI device seed
    keystore
        .lair_client()
        .new_seed("MOCK_DEVICE_SEED".to_string().into(), None, false)
        .await
        .unwrap();

    let dpki = make_mock_dpki_impl(u, state);
    let mut builder = Conductor::builder()
        .with_keystore(keystore)
        .config(config)
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
        .state()
        .await
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
    holochain_trace::test_run();

    let mut u = unstructured_noise();

    let states = std::iter::repeat_with(|| Arc::new(Mutex::new(HashMap::new())))
        .take(2)
        .collect::<Vec<_>>();

    let rendezvous = SweetLocalRendezvous::new().await;

    let config: ConductorConfig = SweetConductorConfig::rendezvous(true)
        .into_conductor_config(&*rendezvous)
        .await;

    let mut conductors = SweetConductorBatch::new(vec![
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(&mut u, config.clone(), states[0].clone()).await,
            rendezvous.clone(),
        )
        .await,
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(&mut u, config.clone(), states[1].clone()).await,
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

    // Assert that we *can't* reach consistency in 3 seconds
    await_consistency(3, [&alice_clone, &bob_clone])
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

    assert!(matches!(
        get_key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        get_key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

    await_consistency(10, [&alice, &bob]).await.unwrap();

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
    holochain_trace::test_run();

    let mut u = unstructured_noise();

    let states = std::iter::repeat_with(|| Arc::new(Mutex::new(HashMap::new())))
        .take(2)
        .collect::<Vec<_>>();

    let rendezvous = SweetLocalRendezvous::new().await;

    let config: ConductorConfig = SweetConductorConfig::rendezvous(true)
        .into_conductor_config(&*rendezvous)
        .await;

    let mut conductors = SweetConductorBatch::new(vec![
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(&mut u, config.clone(), states[0].clone()).await,
            rendezvous.clone(),
        )
        .await,
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(&mut u, config.clone(), states[1].clone()).await,
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
        s0.insert(a1, KeyState::Invalid(None));
        // But Bob thinks Alice is valid
        s1.insert(a0, KeyState::Valid(fixt!(SignedActionHashed)));
    }

    let hash: ActionHash = conductors[1].call(&bob.zome("simple"), "create", ()).await;

    let alice_clone = alice.clone();
    let bob_clone = bob.clone();

    // Assert that we *can't* reach consistency in 3 seconds
    tokio::spawn(async move {
        await_consistency(3, [&alice_clone, &bob_clone])
            .await
            .unwrap()
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
    holochain_trace::test_run();

    let mut u = unstructured_noise();

    let states = std::iter::repeat_with(|| Arc::new(Mutex::new(HashMap::new())))
        .take(2)
        .collect::<Vec<_>>();

    let rendezvous = SweetLocalRendezvous::new().await;

    let config: ConductorConfig = SweetConductorConfig::rendezvous(true)
        .into_conductor_config(&*rendezvous)
        .await;

    let mut conductors = SweetConductorBatch::new(vec![
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(&mut u, config.clone(), states[0].clone()).await,
            rendezvous.clone(),
        )
        .await,
        SweetConductor::from_builder_rendezvous(
            make_dpki_conductor_builder(&mut u, config.clone(), states[1].clone()).await,
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
            .flat_map(|h| (**h).clone().into_iter())
            .collect::<Vec<_>>();

        ks[0..=1].iter_mut().for_each(|h| {
            h.extend(pairs.clone());
        });
    }

    await_consistency(10, [&alice, &bob]).await.unwrap();

    let hash: ActionHash = conductors[0]
        .call(&alice.zome("simple"), "create", ())
        .await;

    await_consistency(60, [&alice, &bob]).await.unwrap();

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
