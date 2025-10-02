use common::make_agent;
use holochain::prelude::{DnaModifiersOpt, RoleSettings, YamlProperties};
use holochain::test_utils::itertools::Itertools;
use holochain::{prelude::AppBundleSource, sweettest::SweetConductor};
use holochain_client::{
    AdminWebsocket, AppWebsocket, AuthorizeSigningCredentialsPayload, ClientAgentSigner,
    InstallAppPayload, InstalledAppId,
};
use holochain_conductor_api::{CellInfo, StorageBlob};
use holochain_types::websocket::AllowedOrigins;
use holochain_zome_types::prelude::ExternIO;
use kitsune2_api::Url;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

mod common;
mod fixture;

const ROLE_NAME: &str = "foo";

#[tokio::test(flavor = "multi_thread")]
async fn app_interfaces() {
    let conductor = SweetConductor::from_standard_config().await;

    // Connect admin client
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port), None)
        .await
        .unwrap();

    let app_interfaces = admin_ws.list_app_interfaces().await.unwrap();

    assert_eq!(app_interfaces.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn signed_zome_call() {
    let conductor = SweetConductor::from_standard_config().await;

    // Connect admin client
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port), None)
        .await
        .unwrap();

    // Set up the test app
    let app_id: InstalledAppId = "test-app".into();
    let installed_app = admin_ws
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

    // Connect app agent client
    let app_ws_port = admin_ws
        .attach_app_interface(0, None, AllowedOrigins::Any, None)
        .await
        .unwrap();
    let issued_token = admin_ws
        .issue_app_auth_token(app_id.clone().into())
        .await
        .unwrap();
    let signer = ClientAgentSigner::default();
    let app_ws = AppWebsocket::connect(
        (Ipv4Addr::LOCALHOST, app_ws_port),
        issued_token.token,
        signer.clone().into(),
        None,
    )
    .await
    .unwrap();

    let cells = installed_app.cell_info.into_values().next().unwrap();
    let cell_id = match cells[0].clone() {
        CellInfo::Provisioned(c) => c.cell_id,
        _ => panic!("Invalid cell type"),
    };

    // ******** SIGNED ZOME CALL  ********

    const TEST_ZOME_NAME: &str = "foo";
    const TEST_FN_NAME: &str = "foo";

    let credentials = admin_ws
        .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
            cell_id: cell_id.clone(),
            functions: None,
        })
        .await
        .unwrap();
    signer.add_credentials(cell_id.clone(), credentials);

    let response = app_ws
        .call_zome(
            cell_id.into(),
            TEST_ZOME_NAME.into(),
            TEST_FN_NAME.into(),
            ExternIO::encode(()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        ExternIO::decode::<String>(&response).unwrap(),
        TEST_FN_NAME.to_string()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_info() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None)
        .await
        .unwrap();
    let app_id: InstalledAppId = "test-app".into();
    let agent_key = admin_ws.generate_agent_pub_key().await.unwrap();
    admin_ws
        .install_app(InstallAppPayload {
            agent_key: Some(agent_key.clone()),
            installed_app_id: Some(app_id.clone()),
            network_seed: None,
            roles_settings: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let storage_info = admin_ws.storage_info().await.unwrap();

    let matched_storage_info = storage_info
        .blobs
        .iter()
        .filter(|b| match b {
            StorageBlob::Dna(dna_storage_info) => dna_storage_info.used_by.contains(&app_id),
        })
        .collect_vec();
    assert_eq!(1, matched_storage_info.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_network_stats() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None)
        .await
        .unwrap();
    let app_id: InstalledAppId = "test-app".into();
    let agent_key = admin_ws.generate_agent_pub_key().await.unwrap();
    admin_ws
        .install_app(InstallAppPayload {
            agent_key: Some(agent_key.clone()),
            installed_app_id: Some(app_id.clone()),
            network_seed: None,
            roles_settings: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let network_stats = admin_ws.dump_network_stats().await.unwrap();

    assert_eq!("kitsune2-core-mem", network_stats.backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_info() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None)
        .await
        .unwrap();
    let app_id: InstalledAppId = "test-app".into();
    let agent_key = admin_ws.generate_agent_pub_key().await.unwrap();
    admin_ws
        .install_app(InstallAppPayload {
            agent_key: Some(agent_key.clone()),
            installed_app_id: Some(app_id.clone()),
            roles_settings: None,
            network_seed: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let agent_infos = admin_ws.agent_info(None).await.unwrap();
    assert_eq!(agent_infos.len(), 1);

    let space = kitsune2_api::AgentInfoSigned::decode(
        &kitsune2_core::Ed25519Verifier,
        agent_infos[0].as_bytes(),
    )
    .unwrap()
    .space
    .clone();

    let other_agent = make_agent(&space);

    admin_ws
        .add_agent_info(vec![other_agent.clone()])
        .await
        .unwrap();

    let agent_infos = admin_ws.agent_info(None).await.unwrap();
    assert_eq!(agent_infos.len(), 2);
    assert!(agent_infos.contains(&other_agent));
}

#[tokio::test(flavor = "multi_thread")]
async fn peer_meta_info() {
    // This is just a rudimentary test. More detailed functionality is tested in
    // conductor tests in the holochain crate where the peer meta store is
    // accessible on the SweetConductor.

    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None)
        .await
        .unwrap();
    let app_id: InstalledAppId = "test-app".into();
    let agent_key = admin_ws.generate_agent_pub_key().await.unwrap();
    admin_ws
        .install_app(InstallAppPayload {
            agent_key: Some(agent_key.clone()),
            installed_app_id: Some(app_id.clone()),
            roles_settings: None,
            network_seed: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let app_info = admin_ws
        .list_apps(None)
        .await
        .unwrap()
        .first()
        .unwrap()
        .clone();

    let dna_hash = match app_info.cell_info.first().unwrap().1.first().unwrap() {
        CellInfo::Provisioned(c) => c.cell_id.dna_hash().clone(),
        _ => panic!("Wrong CellInfo type."),
    };

    let url = Url::from_str("ws://test.com:80/test-url").unwrap();

    // Get the agent meta info for all spaces
    let response = admin_ws.peer_meta_info(url.clone(), None).await.unwrap();
    assert_eq!(response.len(), 1);

    let meta_infos = response
        .get(&dna_hash)
        .expect("No meta infos found for dna hash.");
    assert_eq!(meta_infos.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn install_app_then_list_apps_and_list_cell_ids() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None)
        .await
        .unwrap();
    let app_id: InstalledAppId = "test-app".into();
    let agent_key = admin_ws.generate_agent_pub_key().await.unwrap();
    let app_info = admin_ws
        .install_app(InstallAppPayload {
            agent_key: Some(agent_key),
            installed_app_id: Some(app_id.clone()),
            roles_settings: None,
            network_seed: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();
    let cell_id =
        if let CellInfo::Provisioned(cell) = &app_info.cell_info.get(ROLE_NAME).unwrap()[0] {
            cell.cell_id.clone()
        } else {
            panic!("expected provisioned cell");
        };

    let cell_ids = admin_ws.list_cell_ids().await.unwrap();
    // Check if list includes cell id.
    assert_eq!(cell_ids.len(), 1);
    assert!(cell_ids.contains(&cell_id));

    let app_infos = admin_ws.list_apps(None).await.unwrap();
    // Check if list includes AppInfo with the correct installed_app_id.
    assert_eq!(app_infos.len(), 1);
    assert!(app_infos.iter().any(|a| a.installed_app_id == app_id));
}

#[tokio::test(flavor = "multi_thread")]
async fn install_app_with_roles_settings() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{admin_port}"), None)
        .await
        .unwrap();
    let app_id: InstalledAppId = "test-app".into();
    let agent_key = admin_ws.generate_agent_pub_key().await.unwrap();

    let custom_network_seed = String::from("modified seed");
    let custom_properties = YamlProperties::new(serde_yaml::Value::String(String::from(
        "some properties provided at install time",
    )));

    let custom_modifiers = DnaModifiersOpt::default()
        .with_network_seed(custom_network_seed.clone())
        .with_properties(custom_properties.clone());

    let role_settings = (
        String::from("foo"),
        RoleSettings::Provisioned {
            membrane_proof: Default::default(),
            modifiers: Some(custom_modifiers),
        },
    );

    admin_ws
        .install_app(InstallAppPayload {
            agent_key: Some(agent_key.clone()),
            installed_app_id: Some(app_id.clone()),
            roles_settings: Some(HashMap::from([role_settings])),
            network_seed: None,
            source: AppBundleSource::Bytes(fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let app_info = admin_ws
        .list_apps(None)
        .await
        .unwrap()
        .first()
        .unwrap()
        .clone();

    let manifest = app_info.manifest;

    let app_role = manifest
        .app_roles()
        .into_iter()
        .find(|r| r.name == "foo")
        .unwrap();

    assert_eq!(
        app_role.dna.modifiers.network_seed,
        Some(custom_network_seed)
    );
    assert_eq!(app_role.dna.modifiers.properties, Some(custom_properties));
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_multiple_addresses() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();

    let admin_ws = AdminWebsocket::connect(
        &[
            // Shouldn't be able to connect on this port
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 5000),
            // Should then move on and try this one
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), admin_port),
        ][..],
        None,
    )
    .await
    .unwrap();

    // Just to check we are connected and can get a response.
    let apps = admin_ws.list_apps(None).await.unwrap();
    assert!(apps.is_empty());
}
