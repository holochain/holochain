use holochain_conductor_services::KeyState;
use holochain_state::source_chain::SourceChainError;
use holochain_types::dna::DnaWithRole;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::timestamp::Timestamp;
use matches::assert_matches;

use crate::{
    conductor::{
        api::error::{ConductorApiError, DpkiError},
        CellError,
    },
    sweettest::{SweetAgents, SweetConductor, SweetDnaFile, SweetInlineZomes, SweetZome},
};

#[tokio::test(flavor = "multi_thread")]
async fn delete_agent_key() {
    let mut conductor = SweetConductor::from_standard_config().await;
    // let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::AgentInfo]).await;
    let fn_name = "function";
    let zomes = SweetInlineZomes::new(vec![], 0).function(fn_name, |_, _: ()| Ok(()));
    let (dna_file, _, coordinator_zomes) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let app = conductor
        .setup_app("", [&("role".to_string(), dna_file)])
        .await
        .unwrap();
    let agent_key = app.agent().clone();
    let zome = SweetZome::new(
        app.cells()[0].cell_id().clone(),
        coordinator_zomes[0].name.clone(),
    );

    // no agent key provided, so DPKI should be installed
    // and the generated agent key should be valid
    println!(
        "running services {:?}",
        conductor.running_services().dpki.is_some()
    );
    let dpki = conductor
        .running_services()
        .dpki
        .expect("dpki must be running");
    let key_state = dpki
        .state()
        .await
        .key_state(agent_key.clone(), Timestamp::now())
        .await
        .unwrap();
    println!("state {key_state:?}");
    assert_matches!(key_state, KeyState::Valid(_));
    let initial_key_state = if let KeyState::Valid(signed_action_hash) = key_state {
        signed_action_hash
    } else {
        panic!("no valid key present")
    };

    // calling the cell should succeed
    let r: Result<(), _> = conductor.call_fallible(&zome, fn_name, ()).await;
    assert_matches!(r, Ok(()));

    // TODOs
    // - add multiple cells
    // - prevent cell cloning
    let result = conductor
        .clone()
        .delete_agent_key_for_app(app.installed_app_id())
        .await;
    println!("delete result {result:?}");

    let key_state = dpki
        .state()
        .await
        .key_state(agent_key.clone(), Timestamp::now())
        .await
        .unwrap();
    println!("state {key_state:?} initial key state {initial_key_state:?}");
    assert_matches!(key_state, KeyState::Invalid(_));
    // let r = conductor.call_fallible::<_, ()>(&zome, fn_name, ()).await;
    // assert_matches!(
    //     r,
    //     Err(ConductorApiError::SourceChainError(
    //         SourceChainError::ChainReadOnly
    //     ))
    // );
    // println!("r is {r:?}");
}
