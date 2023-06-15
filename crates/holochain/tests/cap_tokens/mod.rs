#![cfg(feature = "test_utils")]

use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetDnaFile;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

/// Alice creates a cap grant for her private function and then tries to use it to
/// remote call Bobs private function
#[tokio::test(flavor = "multi_thread")]

#[cfg(feature = "slow_tests")]
async fn alice_cant_remote_call_bobs_private_function() {
    use holochain::conductor::api::error::ConductorApiResult;

    holochain_trace::test_run().ok();
    const NUM_AGENTS: usize = 30;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::CapTokens]).await;

    // Create a Conductor
    let mut conductor = SweetConductor::from_standard_config().await;

    let agents = SweetAgents::get(conductor.keystore(), 2).await;
    let apps = conductor
        .setup_app_for_agents("app", &agents, &[dna_file])
        .await
        .unwrap();
    let cells = apps.cells_flattened();
    let alice = cells[0].clone();
    let bob = cells[1].clone();

    let alice_secret: CapSecret = conductor.call(&alice.zome(TestWasm::CapTokens), "create_cap_grant_for_private_function", ()).await;

    #[derive(Serialize, Deserialize, Debug, SerializedBytes)]
    pub struct RemoteCallPrivateInput {
        pub to_cell: CellId,
        pub cap_secret: CapSecret,
    }    

    // Making a remote call to bobs cell using a secret alice created should fail
    let remote_call_to_bob: RemoteCallPrivateInput = RemoteCallPrivateInput {
        to_cell: bob.cell_id().to_owned(),
        cap_secret: alice_secret
    };

    let attempted_call: ConductorApiResult<String> = conductor.call_fallible(&alice.zome(TestWasm::CapTokens), "remote_call_private_function", remote_call_to_bob ).await;

    match attempted_call {
        Ok(_) => {
            panic!("calling bobs cell remotely using alices secret should fail")
        },
        Err(err) => match err {
            // This is the result we expect
            holochain::conductor::api::error::ConductorApiError::Other(_boxed_call_error) => {
                // specifically boxed_call_error should be `CallError("Unauthorized call to private_function".to_string())` but 
                // I wasn't sure how to assert that
            },
            _ => panic!("Unexpected err {}", err),
        },
    };
}

