#![cfg(feature = "test_utils")]

use holochain::sweettest::SweetAgents;
use holochain::sweettest::SweetConductor;
use holochain::sweettest::SweetDnaFile;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;

/// A single link with a Path for the base and target is committed by one
/// agent, and after a delay, all agents can get the link
#[tokio::test(flavor = "multi_thread")]

#[cfg(feature = "slow_tests")]
async fn alice_cant_remote_call_bobs_private_function() {
    use holochain::conductor::api::error::ConductorApiResult;

    holochain_trace::test_run().ok();

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

    #[derive(Serialize, Deserialize, Debug, SerializedBytes)]
    pub struct RemoteCallPrivateInput {
        pub to_cell: CellId,
        pub maybe_cap_secret: Option<CapSecret>,
    }    

    // Making a remote call to bobs cell without a secret should fail
    let remote_call_to_bob: RemoteCallPrivateInput = RemoteCallPrivateInput {
        to_cell: bob.cell_id().to_owned(),
        maybe_cap_secret: None
    };

    let attempted_call_no_secret: ConductorApiResult<String> = conductor.call_fallible(&alice.zome(TestWasm::CapTokens), "remote_call_private_function", remote_call_to_bob ).await;

    match attempted_call_no_secret {
        Ok(_) => {
            panic!("calling bobs cell remotely with no secret should fail")
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

    let alice_secret: CapSecret = conductor.call(&alice.zome(TestWasm::CapTokens), "create_cap_grant_for_private_function", ()).await;

    // Making a remote call to bobs cell using a secret alice created should fail
    let remote_call_to_bob: RemoteCallPrivateInput = RemoteCallPrivateInput {
        to_cell: bob.cell_id().to_owned(),
        maybe_cap_secret: Some(alice_secret)
    };

    let attempted_call_wrong_secret: ConductorApiResult<String> = conductor.call_fallible(&alice.zome(TestWasm::CapTokens), "remote_call_private_function", remote_call_to_bob ).await;

    match attempted_call_wrong_secret {
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

