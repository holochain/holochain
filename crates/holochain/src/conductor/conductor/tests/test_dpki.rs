use crate::test_utils::inline_zomes::simple_create_read_zome;
use parking_lot::Mutex;

use super::*;

/// Crude check that an agent without the same DPKI instance as others can't
/// validate actions
#[tokio::test(flavor = "multi_thread")]
async fn validate_with_mock_dpki() {
    holochain_trace::test_run().ok();

    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    // The DPKI state of each conductor about all other agents.
    type KeyStates = Arc<Mutex<[HashMap<AgentPubKey, KeyState>; NUM_CONDUCTORS]>>;

    let key_states = [HashMap::new(), HashMap::new(), HashMap::new()];
    let key_states: KeyStates = Arc::new(Mutex::new(key_states));

    fn setup_mock_dpki(conductors: &SweetConductorBatch, index: usize, key_states: KeyStates) {
        let mut dpki = MockDpkiService::new();
        let keystore = conductors[index].keystore().clone();

        dpki.expect_uuid().return_const([1; 32]);
        dpki.expect_should_run().return_const(true);

        dpki.expect_key_state().returning({
            let key_states = key_states.clone();
            move |a, _t| {
                let state = key_states.lock()[index]
                    .get(&a)
                    .cloned()
                    .unwrap_or(KeyState::NotFound);
                async move { Ok(state) }.boxed()
            }
        });

        dpki.expect_derive_and_register_new_key().returning({
            move |_, _| {
                let keystore = keystore.clone();
                let key_states = key_states.clone();
                async move {
                    let agent = keystore.new_sign_keypair_random().await.unwrap();
                    key_states.lock()[index]
                        .insert(agent.clone(), KeyState::Valid(fixt!(SignedActionHashed)));

                    Ok(agent)
                }
                .boxed()
            }
        });

        conductors[index].running_services.share_mut(|s| {
            s.dpki = Some(Arc::new(dpki));
        });
    }

    setup_mock_dpki(&conductors, 0, key_states.clone());
    setup_mock_dpki(&conductors, 1, key_states.clone());

    let (app_dna_file, _, _) =
        SweetDnaFile::unique_from_inline_zomes(("simple", simple_create_read_zome())).await;
    dbg!(&app_dna_file.dna().integrity_zomes);
    let ((alice,), (bob,), (carol,)) = conductors
        .setup_app("app", [&("role".to_string(), app_dna_file)])
        .await
        .unwrap()
        .into_tuples();

    dbg!(alice.dna_hash(), bob.dna_hash(), carol.dna_hash());
    assert_eq!(alice.dna_hash(), bob.dna_hash());
    // Because of carol's lack of DPKI, the DnaCompatParams are different and so is the DNA hash.
    assert_ne!(alice.dna_hash(), carol.dna_hash());

    async fn key_state(conductor: &SweetConductor, agent: &AgentPubKey) -> KeyState {
        conductor
            .running_services()
            .dpki
            .as_ref()
            .unwrap()
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

    conductors.exchange_peer_info().await;

    {
        // exchange all
        let mut ks = key_states.lock();

        let pairs = ks
            .iter()
            .flat_map(|h| h.clone().into_iter())
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
        key_state(&conductors[0], bob.agent_pubkey()).await,
        KeyState::Valid(_)
    ));
    assert!(matches!(
        key_state(&conductors[1], alice.agent_pubkey()).await,
        KeyState::Valid(_)
    ));

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
