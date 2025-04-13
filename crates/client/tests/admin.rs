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
use std::net::{Ipv4Addr, SocketAddr};
use std::{collections::HashMap, path::PathBuf};

const ROLE_NAME: &str = "foo";

#[tokio::test(flavor = "multi_thread")]
async fn app_interfaces() {
    let conductor = SweetConductor::from_standard_config().await;

    // Connect admin client
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
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
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
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
            source: AppBundleSource::Path(PathBuf::from("./fixture/test.happ")),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    // Connect app agent client
    let app_ws_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::Any, None)
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
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
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
            source: AppBundleSource::Path(PathBuf::from("./fixture/test.happ")),
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
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
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
            source: AppBundleSource::Path(PathBuf::from("./fixture/test.happ")),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let network_stats = admin_ws.dump_network_stats().await.unwrap();

    assert_eq!("kitsune2-core-mem", network_stats.backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn revoke_agent_key() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
        .await
        .unwrap();

    let app_info = admin_ws
        .install_app(InstallAppPayload {
            agent_key: None,
            installed_app_id: None,
            network_seed: None,
            roles_settings: None,
            source: AppBundleSource::Path(PathBuf::from("./fixture/test.happ")),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    let app_id = app_info.installed_app_id.clone();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    let agent_key = app_info.agent_pub_key.clone();
    let response = admin_ws
        .revoke_agent_key(app_id.clone(), agent_key.clone())
        .await
        .unwrap();
    assert_eq!(response, vec![]);
    let response = admin_ws
        .revoke_agent_key(app_id, agent_key.clone())
        .await
        .unwrap();
    let cell_id = if let CellInfo::Provisioned(provisioned_cell) =
        &app_info.cell_info.get(ROLE_NAME).unwrap()[0]
    {
        provisioned_cell.cell_id.clone()
    } else {
        panic!("expected provisioned cell")
    };
    assert!(matches!(&response[0], (cell, error) if *cell == cell_id && error.contains("invalid")));
}

fn make_agent(space: kitsune2_api::SpaceId) -> String {
    let local = kitsune2_core::Ed25519LocalAgent::default();
    let created_at = kitsune2_api::Timestamp::now();
    let expires_at = created_at + std::time::Duration::from_secs(60 * 20);
    let info = kitsune2_api::AgentInfo {
        agent: kitsune2_api::LocalAgent::agent(&local).clone(),
        space,
        created_at,
        expires_at,
        is_tombstone: false,
        url: None,
        storage_arc: kitsune2_api::DhtArc::FULL,
    };
    let info =
        futures::executor::block_on(kitsune2_api::AgentInfoSigned::sign(&local, info)).unwrap();
    info.encode().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_info() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
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
            source: AppBundleSource::Path(PathBuf::from("./fixture/test.happ")),
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

    let other_agent = make_agent(space);

    admin_ws
        .add_agent_info(vec![other_agent.clone()])
        .await
        .unwrap();

    let agent_infos = admin_ws.agent_info(None).await.unwrap();
    assert_eq!(agent_infos.len(), 2);
    assert!(agent_infos.contains(&other_agent));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_cell_ids() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
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
            source: AppBundleSource::Path(PathBuf::from("./fixture/test.happ")),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id).await.unwrap();
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
}

#[tokio::test(flavor = "multi_thread")]
async fn install_app_with_roles_settings() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
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
            source: AppBundleSource::Path(PathBuf::from("./fixture/test.happ")),
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
    )
    .await
    .unwrap();

    // Just to check we are connected and can get a response.
    let apps = admin_ws.list_apps(None).await.unwrap();
    assert!(apps.is_empty());
}
