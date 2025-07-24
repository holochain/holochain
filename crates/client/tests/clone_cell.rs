use holochain::{
    prelude::{DeleteCloneCellPayload, DisableCloneCellPayload, EnableCloneCellPayload},
    sweettest::SweetConductor,
};
use holochain_client::{
    AdminWebsocket, AppWebsocket, AuthorizeSigningCredentialsPayload, ClientAgentSigner,
    ConductorApiError, InstallAppPayload,
};
use holochain_types::prelude::{
    AppBundleSource, CloneDnaId, CloneId, CreateCloneCellPayload, DnaModifiersOpt, InstalledAppId,
};
use holochain_types::websocket::AllowedOrigins;
use holochain_zome_types::{dependencies::holochain_integrity_types::ExternIO, prelude::RoleName};
use std::net::Ipv4Addr;

mod fixture;

#[tokio::test(flavor = "multi_thread")]
async fn clone_cell_management() {
    let conductor = SweetConductor::from_standard_config().await;

    // Connect admin client
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port), None)
        .await
        .unwrap();

    // Set up the test app
    let app_id: InstalledAppId = "test-app".into();
    let role_name: RoleName = "foo".into();
    let app_info = admin_ws
        .install_app(InstallAppPayload {
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            network_seed: None,
            roles_settings: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();
    let app_api_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::Any, None)
        .await
        .unwrap();

    // Connect an app agent client
    let issued_token = admin_ws
        .issue_app_auth_token(app_id.clone().into())
        .await
        .unwrap();
    let signer = ClientAgentSigner::default();
    let app_ws = AppWebsocket::connect(
        format!("127.0.0.1:{}", app_api_port),
        issued_token.token,
        signer.clone().into(),
        None,
    )
    .await
    .unwrap();

    let clone_cell = {
        let clone_cell = app_ws
            .create_clone_cell(CreateCloneCellPayload {
                role_name: role_name.clone(),
                modifiers: DnaModifiersOpt::none().with_network_seed("seed".into()),
                membrane_proof: None,
                name: None,
            })
            .await
            .unwrap();
        assert_eq!(*clone_cell.dna_id.agent_pubkey(), app_info.agent_pub_key);
        assert_eq!(clone_cell.clone_id, CloneId::new(&role_name, 0));
        clone_cell
    };
    let dna_id = clone_cell.dna_id.clone();

    let credentials = admin_ws
        .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
            dna_id: dna_id.clone(),
            functions: None,
        })
        .await
        .unwrap();
    signer.add_credentials(dna_id.clone(), credentials);

    const TEST_ZOME_NAME: &str = "foo";
    const TEST_FN_NAME: &str = "foo";

    // call clone cell should succeed
    let response = app_ws
        .call_zome(
            dna_id.clone().into(),
            TEST_ZOME_NAME.into(),
            TEST_FN_NAME.into(),
            ExternIO::encode(()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.decode::<String>().unwrap(), "foo");

    // disable clone cell
    app_ws
        .disable_clone_cell(DisableCloneCellPayload {
            clone_dna_id: CloneDnaId::CloneId(clone_cell.clone().clone_id),
        })
        .await
        .unwrap();

    // call disabled clone cell should fail
    let response = app_ws
        .call_zome(
            dna_id.clone().into(),
            TEST_ZOME_NAME.into(),
            TEST_FN_NAME.into(),
            ExternIO::encode(()).unwrap(),
        )
        .await;
    assert!(response.is_err());

    // enable clone cell
    let enabled_cell = app_ws
        .enable_clone_cell(EnableCloneCellPayload {
            clone_dna_id: CloneDnaId::CloneId(clone_cell.clone().clone_id),
        })
        .await
        .unwrap();
    assert_eq!(enabled_cell, clone_cell);

    // call enabled clone cell should succeed
    let response = app_ws
        .call_zome(
            dna_id.clone().into(),
            TEST_ZOME_NAME.into(),
            TEST_FN_NAME.into(),
            ExternIO::encode(()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.decode::<String>().unwrap(), "foo");

    // disable clone cell again
    app_ws
        .disable_clone_cell(DisableCloneCellPayload {
            clone_dna_id: CloneDnaId::CloneId(clone_cell.clone().clone_id),
        })
        .await
        .unwrap();

    // delete disabled clone cell
    admin_ws
        .delete_clone_cell(DeleteCloneCellPayload {
            app_id: app_id.clone(),
            clone_dna_id: CloneDnaId::DnaHash(clone_cell.dna_id.dna_hash().clone()),
        })
        .await
        .unwrap();
    // restore deleted clone cell should fail
    let enable_clone_cell_response = app_ws
        .enable_clone_cell(EnableCloneCellPayload {
            clone_dna_id: CloneDnaId::CloneId(clone_cell.clone_id),
        })
        .await;
    assert!(enable_clone_cell_response.is_err());
}

// Check that app info can be refreshed to allow zome calls to a clone cell identified by its clone dna id
#[tokio::test(flavor = "multi_thread")]
pub async fn app_info_refresh() {
    let conductor = SweetConductor::from_standard_config().await;

    // Connect admin client
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port), None)
        .await
        .unwrap();

    let app_id: InstalledAppId = "test-app".into();
    let role_name: RoleName = "foo".into();

    // Install and enable an app
    admin_ws
        .install_app(InstallAppPayload {
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            network_seed: None,
            roles_settings: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let signer = ClientAgentSigner::default();

    // Create an app interface and connect an app agent to it
    let app_api_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::Any, None)
        .await
        .unwrap();

    let token_issued = admin_ws
        .issue_app_auth_token(app_id.clone().into())
        .await
        .unwrap();
    let mut app_agent_ws = AppWebsocket::connect(
        (Ipv4Addr::LOCALHOST, app_api_port),
        token_issued.token,
        signer.clone().into(),
        None,
    )
    .await
    .unwrap();

    // Create a clone cell, AFTER the app agent has been created
    let cloned_cell = app_agent_ws
        .create_clone_cell(CreateCloneCellPayload {
            role_name: role_name.clone(),
            modifiers: DnaModifiersOpt::none().with_network_seed("test seed".into()),
            membrane_proof: None,
            name: None,
        })
        .await
        .unwrap();

    // Authorise signing credentials for the cloned cell
    let credentials = admin_ws
        .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
            dna_id: cloned_cell.dna_id.clone(),
            functions: None,
        })
        .await
        .unwrap();
    signer.add_credentials(cloned_cell.dna_id.clone(), credentials);

    // Call the zome function on the clone cell, expecting a failure
    let err = app_agent_ws
        .call_zome(
            cloned_cell.clone_id.clone().into(),
            "foo".into(),
            "foo".into(),
            ExternIO::encode(()).unwrap(),
        )
        .await
        .expect_err("Should fail because the client doesn't know the clone cell exists");
    match err {
        ConductorApiError::CellNotFound => (),
        _ => panic!("Unexpected error: {:?}", err),
    }

    // Refresh the app info, which means the app agent will now know about the clone cell
    app_agent_ws.refresh_app_info().await.unwrap();

    // Call the zome function on the clone cell again, expecting success
    app_agent_ws
        .call_zome(
            cloned_cell.clone_id.clone().into(),
            "foo".into(),
            "foo".into(),
            ExternIO::encode(()).unwrap(),
        )
        .await
        .unwrap();
}
