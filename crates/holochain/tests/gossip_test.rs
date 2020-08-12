use hdk3::prelude::*;
use holochain::conductor::{
    api::{AppInterfaceApi, AppRequest, AppResponse, RealAppInterfaceApi},
    config::{AdminInterfaceConfig, ConductorConfig, InterfaceDriver},
    dna_store::MockDnaStore,
    ConductorBuilder, ConductorHandle,
};
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_state::test_utils::{test_conductor_env, test_wasm_env, TestEnvironment};
use holochain_types::app::InstalledCell;
use holochain_types::cell::CellId;
use holochain_types::dna::DnaDef;
use holochain_types::dna::DnaFile;
use holochain_types::test_utils::fake_agent_pubkey_1;
use holochain_types::{observability, test_utils::fake_agent_pubkey_2};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::HostInput;
use std::sync::Arc;
use tempdir::TempDir;

use matches::assert_matches;
use test_wasm_common::{AnchorInput, TestString};

#[tokio::test(threaded_scheduler)]
async fn gossip_test() {
    observability::test_run().ok();
    const NUM: usize = 1;
    // ////////////
    // START DNA
    // ////////////

    let dna_file = DnaFile::new(
        DnaDef {
            name: "need_for_speed_test".to_string(),
            uuid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
            properties: SerializedBytes::try_from(()).unwrap(),
            zomes: vec![TestWasm::Anchor.into()].into(),
        },
        vec![TestWasm::Anchor.into()],
    )
    .await
    .unwrap();

    // //////////
    // END DNA
    // //////////

    // ///////////
    // START ALICE
    // ///////////

    let alice_agent_id = fake_agent_pubkey_1();
    let alice_cell_id = CellId::new(dna_file.dna_hash().to_owned(), alice_agent_id.clone());
    let alice_installed_cell = InstalledCell::new(alice_cell_id.clone(), "alice_handle".into());

    // /////////
    // END ALICE
    // /////////

    // /////////
    // START BOB
    // /////////

    let bob_agent_id = fake_agent_pubkey_2();
    let bob_cell_id = CellId::new(dna_file.dna_hash().to_owned(), bob_agent_id.clone());
    let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());

    // ///////
    // END BOB
    // ///////

    // ///////////////
    // START CONDUCTOR
    // ///////////////

    let mut dna_store = MockDnaStore::new();

    dna_store.expect_get().return_const(Some(dna_file.clone()));
    dna_store
        .expect_add_dnas::<Vec<_>>()
        .times(2)
        .return_const(());
    dna_store
        .expect_add_entry_defs::<Vec<_>>()
        .times(2)
        .return_const(());

    let (_tmpdir, app_api, handle) = setup_app(vec![(alice_installed_cell, None)], dna_store).await;

    // /////////////
    // END CONDUCTOR
    // /////////////

    // ALICE adding anchors

    let anchor_invocation = |anchor: &str, cell_id, i: usize| {
        let anchor = AnchorInput(anchor.into(), i.to_string());
        new_invocation(cell_id, "anchor", anchor)
    };

    for i in 0..NUM {
        let invocation = anchor_invocation("alice", alice_cell_id.clone(), i).unwrap();
        let response = call(&app_api, invocation).await;
        assert_matches!(response, AppResponse::ZomeCallInvocation(_));
    }

    // Give publish time to finish
    tokio::time::delay_for(std::time::Duration::from_secs(1)).await;

    // Bring Bob online
    let cell_data = vec![(bob_installed_cell, None)];
    install_app("bob_app", cell_data, handle.clone()).await;

    // Give gossip some time to finish
    tokio::time::delay_for(std::time::Duration::from_secs(1)).await;

    // Bob list anchors
    let invocation = new_invocation(
        bob_cell_id.clone(),
        "list_anchor_addresses",
        TestString("alice".into()),
    )
    .unwrap();
    let response = call(&app_api, invocation).await;
    match response {
        AppResponse::ZomeCallInvocation(r) => {
            let response: SerializedBytes = r.into_inner();
            let hashes: EntryHashes = response.try_into().unwrap();
            assert_eq!(hashes.0.len(), NUM);
        }
        _ => unreachable!(),
    }

    let shutdown = handle.take_shutdown_handle().await.unwrap();
    handle.shutdown().await;
    shutdown.await.unwrap();
}

async fn call(app_api: &RealAppInterfaceApi, invocation: ZomeCallInvocation) -> AppResponse {
    let request = AppRequest::ZomeCallInvocation(Box::new(invocation));
    app_api.handle_app_request(request).await
}

fn new_invocation<P>(
    cell_id: CellId,
    func: &str,
    payload: P,
) -> Result<ZomeCallInvocation, SerializedBytesError>
where
    P: TryInto<SerializedBytes, Error = SerializedBytesError>,
{
    Ok(ZomeCallInvocation {
        cell_id: cell_id.clone(),
        zome_name: TestWasm::Anchor.into(),
        cap: CapSecret::default(),
        fn_name: func.to_string(),
        payload: HostInput::new(payload.try_into()?),
        provenance: cell_id.agent_pubkey().clone(),
    })
}
pub async fn install_app(
    name: &str,
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    conductor_handle: ConductorHandle,
) {
    conductor_handle
        .clone()
        .install_app(name.to_string(), cell_data)
        .await
        .unwrap();

    conductor_handle
        .activate_app(name.to_string())
        .await
        .unwrap();

    let errors = conductor_handle.setup_cells().await.unwrap();

    assert!(errors.is_empty());
}

pub async fn setup_app(
    cell_data: Vec<(InstalledCell, Option<SerializedBytes>)>,
    dna_store: MockDnaStore,
) -> (Arc<TempDir>, RealAppInterfaceApi, ConductorHandle) {
    let test_env = test_conductor_env();
    let TestEnvironment {
        env: wasm_env,
        tmpdir: _tmpdir,
    } = test_wasm_env();
    let tmpdir = test_env.tmpdir.clone();

    let conductor_handle = ConductorBuilder::with_mock_dna_store(dna_store)
        .config(ConductorConfig {
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        })
        .test(test_env, wasm_env)
        .await
        .unwrap();

    install_app("alice_app", cell_data, conductor_handle.clone()).await;

    let handle = conductor_handle.clone();

    (tmpdir, RealAppInterfaceApi::new(conductor_handle), handle)
}
