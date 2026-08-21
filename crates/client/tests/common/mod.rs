// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use holochain::prelude::AppBundleSource;
use holochain::sweettest::{SweetConductor, SweetConductorConfig, SweetLocalRendezvous};
use holochain_client::{
    AdminWebsocket, AuthorizeSigningCredentialsPayload, ClientAgentSigner, DynAgentSigner,
    InstallAppPayload, InstalledAppId,
};
use holochain_conductor_api::{AdminInterfaceConfig, CellInfo, InterfaceDriver};
use holochain_types::websocket::AllowedOrigins;
use kitsune2_api::{DynLocalAgent, SpaceId};
use kitsune2_core::Ed25519LocalAgent;
use kitsune2_test_utils::agent::AgentBuilder;
use std::net::Ipv4Addr;
use std::sync::Arc;

pub fn make_agent(space: &SpaceId) -> String {
    AgentBuilder {
        space_id: Some(space.clone()),
        ..Default::default()
    }
    .build(Arc::new(Ed25519LocalAgent::default()) as DynLocalAgent)
    .encode()
    .unwrap()
}

/// Picks a port that is free at this instant.
///
/// Restart tests need the admin port to stay put, which the standard sweettest
/// config does not do because it binds port 0.
pub fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

/// Builds a conductor whose admin interface keeps the same port across a
/// shutdown and startup cycle.
pub async fn conductor_with_fixed_admin_port() -> (SweetConductor, u16) {
    let port = free_port();
    (conductor_on_admin_port(port).await, port)
}

/// Builds a conductor whose admin interface listens on `port`.
pub async fn conductor_on_admin_port(port: u16) -> SweetConductor {
    let mut config = SweetConductorConfig::rendezvous(true);
    config.admin_interfaces = Some(vec![AdminInterfaceConfig {
        driver: InterfaceDriver::Websocket {
            port,
            danger_bind_addr: None,
            allowed_origins: AllowedOrigins::Any,
        },
    }]);
    SweetConductor::from_config_rendezvous(config, SweetLocalRendezvous::new().await).await
}

/// The zome the fixture app exposes.
pub const FIXTURE_ZOME_NAME: &str = "foo";

/// The function the fixture app exposes to emit a signal.
pub const FIXTURE_EMIT_FN_NAME: &str = "emitter";

/// The payload the fixture app's emitter sends.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TestString(pub String);

/// Installs the fixture app on a conductor whose admin port survives a restart,
/// and returns a signer authorized to call its zomes.
pub async fn install_fixture_app_with_fixed_admin_port(
) -> (SweetConductor, u16, InstalledAppId, DynAgentSigner) {
    let (conductor, admin_port) = conductor_with_fixed_admin_port().await;

    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port), None)
        .await
        .unwrap();

    let app_id: InstalledAppId = "test-app".into();
    admin_ws
        .install_app(InstallAppPayload {
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            network_seed: None,
            roles_settings: None,
            source: AppBundleSource::Bytes(crate::fixture::get_fixture_app_bundle()),
            ignore_genesis_failure: false,
            restore_from_dht: false,
        })
        .await
        .unwrap();
    admin_ws.enable_app(app_id.clone()).await.unwrap();

    // Attached on port 0, so the conductor restores it on a different port
    // after a restart. That is what makes the rediscovery path load bearing.
    admin_ws
        .attach_app_interface(
            0,
            None,
            AllowedOrigins::Origins(vec!["my-service".to_string()].into_iter().collect()),
            Some(app_id.clone()),
        )
        .await
        .unwrap();

    let token = admin_ws
        .issue_app_auth_token(app_id.clone().into())
        .await
        .unwrap()
        .token;
    let port = holochain_client::discover_app_interface_port_for_test(
        &admin_ws,
        &app_id,
        Some("my-service"),
    )
    .await
    .unwrap();

    let signer = ClientAgentSigner::default();
    let app_ws = holochain_client::AppWebsocket::connect(
        (Ipv4Addr::LOCALHOST, port),
        token,
        signer.clone().into(),
        Some("my-service".to_string()),
    )
    .await
    .unwrap();

    let cell_id = provisioned_cell_id(app_ws.cached_app_info());

    let credentials = admin_ws
        .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
            cell_id: cell_id.clone(),
            functions: None,
        })
        .await
        .unwrap();
    signer.add_credentials(cell_id, credentials);

    (conductor, admin_port, app_id, signer.into())
}

/// Reads the provisioned cell id out of an app's info.
pub fn provisioned_cell_id(app_info: &holochain_client::AppInfo) -> holochain_client::CellId {
    let cells = app_info.cell_info.values().next().unwrap();
    match &cells[0] {
        CellInfo::Provisioned(c) => c.cell_id.clone(),
        _ => panic!("Expected a provisioned cell"),
    }
}
