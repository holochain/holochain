use holochain::{
    prelude::{AppBundleSource, Signal},
    sweettest::SweetConductor,
};
use holochain_client::{
    AdminWebsocket, AppWebsocket, AuthorizeSigningCredentialsPayload, ClientAgentSigner,
    InstallAppPayload, InstalledAppId,
};
use holochain_conductor_api::{AppInfoStatus, CellInfo, IssueAppAuthenticationTokenPayload};
use holochain_types::{
    app::{AppBundle, AppManifestV1, DisabledAppReason},
    websocket::AllowedOrigins,
};
use holochain_websocket::ConnectRequest;
use holochain_zome_types::dependencies::holochain_integrity_types::ExternIO;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};
use std::{
    collections::HashMap,
    sync::{Arc, Barrier},
};

mod fixture;

#[tokio::test(flavor = "multi_thread")]
async fn handle_signal() {
    let conductor = SweetConductor::from_standard_config().await;

    // Connect admin client
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
        .await
        .unwrap();

    // Set up the test app
    let app_id: InstalledAppId = "test-app".into();
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

    // Connect app agent client
    let app_ws_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::Any, None)
        .await
        .unwrap();
    let token_issued = admin_ws
        .issue_app_auth_token(app_id.clone().into())
        .await
        .unwrap();
    let signer = ClientAgentSigner::default();
    let app_ws = AppWebsocket::connect(
        (Ipv4Addr::LOCALHOST, app_ws_port),
        token_issued.token,
        signer.clone().into(),
    )
    .await
    .unwrap();

    let installed_app = app_ws.app_info().await.unwrap().unwrap();

    let cells = installed_app.cell_info.into_values().next().unwrap();
    let cell_id = match cells[0].clone() {
        CellInfo::Provisioned(c) => c.cell_id,
        _ => panic!("Invalid cell type"),
    };

    // ******** SIGNED ZOME CALL  ********

    const TEST_ZOME_NAME: &str = "foo";
    const TEST_FN_NAME: &str = "emitter";

    let credentials = admin_ws
        .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
            cell_id: cell_id.clone(),
            functions: None,
        })
        .await
        .unwrap();
    signer.add_credentials(cell_id.clone(), credentials);

    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();

    app_ws
        .on_signal(move |signal| match signal {
            Signal::App { signal, .. } => {
                let ts: TestString = signal.into_inner().decode().unwrap();
                assert_eq!(ts.0.as_str(), "i am a signal");
                barrier_clone.wait();
            }
            _ => panic!("Invalid signal"),
        })
        .await;

    app_ws
        .call_zome(
            cell_id.into(),
            TEST_ZOME_NAME.into(),
            TEST_FN_NAME.into(),
            ExternIO::encode(()).unwrap(),
        )
        .await
        .unwrap();

    barrier.wait();
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestString(pub String);

#[tokio::test(flavor = "multi_thread")]
async fn close_on_drop_is_clone_safe() {
    let conductor = SweetConductor::from_standard_config().await;

    // Connect admin client
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
        .await
        .unwrap();

    // Set up the test app
    let app_id: InstalledAppId = "test-app".into();
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

    // Connect app client
    let app_ws_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::Any, None)
        .await
        .unwrap();
    let token_issued = admin_ws
        .issue_app_auth_token(app_id.clone().into())
        .await
        .unwrap();
    let signer = ClientAgentSigner::default().into();
    let app_ws = AppWebsocket::connect(
        (Ipv4Addr::LOCALHOST, app_ws_port),
        token_issued.token,
        signer,
    )
    .await
    .unwrap();

    {
        let app_ws_2 = app_ws.clone();
        let app_info_2 = app_ws_2.app_info().await.unwrap().unwrap();
        assert_eq!(app_info.installed_app_id, app_info_2.installed_app_id);
    }

    // Should still work after dropping the second app_ws
    let app_info_3 = app_ws.app_info().await.unwrap().unwrap();
    assert_eq!(app_info.installed_app_id, app_info_3.installed_app_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn deferred_memproof_installation() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
        .await
        .unwrap();
    let app_id: InstalledAppId = "test-app".into();

    // Modify app bundle to enable deferred membrane proofs.
    let app_bundle_source = AppBundleSource::Bytes(fixture::get_fixture_app_bundle());
    let original_bundle = app_bundle_source.resolve().await.unwrap();
    let manifest = AppManifestV1 {
        allow_deferred_memproofs: true,
        description: None,
        name: "".to_string(),
        roles: original_bundle.manifest().app_roles(),
    };
    let app_bundle_deferred_memproofs = AppBundle::from(
        original_bundle
            .into_inner()
            .update_manifest(manifest.into())
            .unwrap(),
    );
    let app_bundle_bytes = app_bundle_deferred_memproofs.pack().unwrap();

    admin_ws
        .install_app(InstallAppPayload {
            agent_key: None,
            installed_app_id: Some(app_id.clone()),
            network_seed: None,
            roles_settings: None,
            source: AppBundleSource::Bytes(app_bundle_bytes),
            ignore_genesis_failure: false,
        })
        .await
        .unwrap();

    // Connect app client
    let app_ws_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::Any, None)
        .await
        .unwrap();
    let token_issued = admin_ws
        .issue_app_auth_token(app_id.clone().into())
        .await
        .unwrap();
    let signer = ClientAgentSigner::default();
    let app_ws = AppWebsocket::connect(
        (Ipv4Addr::LOCALHOST, app_ws_port),
        token_issued.token,
        signer.clone().into(),
    )
    .await
    .unwrap();

    // App status should be `AwaitingMemproofs`.
    let app_info = app_ws
        .app_info()
        .await
        .unwrap()
        .expect("app info must exist");
    assert_eq!(app_info.status, AppInfoStatus::AwaitingMemproofs);

    app_ws.enable_app().await.unwrap_err();

    app_ws.provide_memproofs(HashMap::new()).await.unwrap();

    let app_info = app_ws
        .app_info()
        .await
        .unwrap()
        .expect("app info must exist");
    assert_eq!(
        app_info.status,
        AppInfoStatus::Disabled {
            reason: DisabledAppReason::NotStartedAfterProvidingMemproofs
        },
        "app status should be NotStartedAfterProvidingMemproofs"
    );

    app_ws.enable_app().await.unwrap();

    // App status should be `Running` now.
    let app_info = app_ws
        .app_info()
        .await
        .unwrap()
        .expect("app info must exist");
    assert_eq!(app_info.status, AppInfoStatus::Running);
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_multiple_addresses() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();

    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
        .await
        .unwrap();
    let app_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::Any, None)
        .await
        .unwrap();

    // Set up the test app
    let app_id: InstalledAppId = "test-app".into();
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

    let issued = admin_ws
        .issue_app_auth_token(IssueAppAuthenticationTokenPayload::for_installed_app_id(
            app_id.clone(),
        ))
        .await
        .unwrap();

    let signer = ClientAgentSigner::default().into();

    let app_ws = AppWebsocket::connect(
        &[
            // Shouldn't be able to connect on this port
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 5000),
            // Should then move on and try this one
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), app_port),
        ][..],
        issued.token,
        signer,
    )
    .await
    .unwrap();

    // Just to check we are connected and can get a response.
    let app = app_ws.app_info().await.unwrap().expect("app should exist");
    assert_eq!(app_id, app.installed_app_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_with_custom_origin() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();

    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
        .await
        .unwrap();
    let app_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::from("my_cli_app".to_string()), None)
        .await
        .unwrap();

    // Set up the test app
    let app_id: InstalledAppId = "test-app".into();
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

    let issued = admin_ws
        .issue_app_auth_token(IssueAppAuthenticationTokenPayload::for_installed_app_id(
            app_id.clone(),
        ))
        .await
        .unwrap();

    let signer = ClientAgentSigner::default().into();

    let request: ConnectRequest = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), app_port).into();
    let request = request.try_set_header("Origin", "my_cli_app").unwrap();

    let app_ws = AppWebsocket::connect_with_request_and_config(
        request,
        Arc::new(holochain_websocket::WebsocketConfig::CLIENT_DEFAULT),
        issued.token,
        signer,
    )
    .await
    .unwrap();

    // Just to check we are connected and can get a response.
    let app = app_ws.app_info().await.unwrap().expect("app should exist");
    assert_eq!(app_id, app.installed_app_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_network_stats() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
        .await
        .unwrap();
    let app_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::from("my_cli_app".to_string()), None)
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

    let issued = admin_ws
        .issue_app_auth_token(IssueAppAuthenticationTokenPayload::for_installed_app_id(
            app_id.clone(),
        ))
        .await
        .unwrap();

    let signer = ClientAgentSigner::default().into();

    let request: ConnectRequest = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), app_port).into();
    let request = request.try_set_header("Origin", "my_cli_app").unwrap();

    let app_ws = AppWebsocket::connect_with_request_and_config(
        request,
        Arc::new(holochain_websocket::WebsocketConfig::CLIENT_DEFAULT),
        issued.token,
        signer,
    )
    .await
    .unwrap();

    let network_stats = app_ws.dump_network_stats().await.unwrap();

    assert_eq!("kitsune2-core-mem", network_stats.backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn dump_network_metrics() {
    let conductor = SweetConductor::from_standard_config().await;
    let admin_port = conductor.get_arbitrary_admin_websocket_port().unwrap();
    let admin_ws = AdminWebsocket::connect(format!("127.0.0.1:{}", admin_port))
        .await
        .unwrap();
    let app_port = admin_ws
        .attach_app_interface(0, AllowedOrigins::from("my_cli_app".to_string()), None)
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

    let issued = admin_ws
        .issue_app_auth_token(IssueAppAuthenticationTokenPayload::for_installed_app_id(
            app_id.clone(),
        ))
        .await
        .unwrap();

    let signer = ClientAgentSigner::default().into();

    let request: ConnectRequest = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), app_port).into();
    let request = request.try_set_header("Origin", "my_cli_app").unwrap();

    let app_ws = AppWebsocket::connect_with_request_and_config(
        request,
        Arc::new(holochain_websocket::WebsocketConfig::CLIENT_DEFAULT),
        issued.token,
        signer,
    )
    .await
    .unwrap();

    let metrics = app_ws.dump_network_metrics(None, true).await.unwrap();

    assert_eq!(1, metrics.len());
}
