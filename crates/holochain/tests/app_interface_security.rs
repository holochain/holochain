use either::Either;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use holochain::sweettest::{websocket_client_by_port, SweetConductor, SweetDnaFile, WsPollRecv};
use holochain_conductor_api::{
    AdminRequest, AdminResponse, AppAuthenticationRequest, AppAuthenticationToken, AppRequest,
    AppResponse, IssueAppAuthenticationTokenPayload,
};
use holochain_types::prelude::InstalledAppId;
use holochain_types::websocket::AllowedOrigins;
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::{connect, ConnectRequest, ReceiveMessage, WebsocketConfig};

#[tokio::test(flavor = "multi_thread")]
async fn app_allowed_origins() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    let port = conductor
        .clone()
        .add_app_interface(
            either::Either::Left(0),
            "http://localhost:3000".to_string().into(),
            None,
        )
        .await
        .unwrap();

    assert!(connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap()
        )
    )
    .await
    .is_err());

    let token = create_multi_use_token(&conductor, "test-app".into()).await;

    check_app_port(port, "http://localhost:3000", token).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn app_allowed_origins_independence() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    let token = create_multi_use_token(&conductor, "test-app".into()).await;

    let port_1 = conductor
        .clone()
        .add_app_interface(
            Either::Left(0),
            "http://localhost:3001".to_string().into(),
            None,
        )
        .await
        .unwrap();

    let port_2 = conductor
        .clone()
        .add_app_interface(
            Either::Left(0),
            "http://localhost:3002".to_string().into(),
            None,
        )
        .await
        .unwrap();

    // Check that access to another port's origin is blocked

    assert!(connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port_1}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap()
        )
        .try_set_header("origin", "http://localhost:3002")
        .unwrap()
    )
    .await
    .is_err());

    assert!(connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port_2}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap()
        )
        .try_set_header("origin", "http://localhost:3001")
        .unwrap()
    )
    .await
    .is_err());

    // Check that correct access is allowed

    check_app_port(port_1, "http://localhost:3001", token.clone()).await;
    check_app_port(port_2, "http://localhost:3002", token).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn app_interface_requires_auth() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    // App interface with no restrictions, but should still require auth
    let app_port = conductor
        .clone()
        .add_app_interface(Either::Left(0), AllowedOrigins::Any, None)
        .await
        .unwrap();

    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    // Try to send a request before authenticating, results in connection closed
    let err = app_tx
        .request::<_, AppResponse>(AppRequest::AppInfo)
        .await
        .unwrap_err();
    assert_eq!("ConnectionClosed", err.to_string());

    let token = create_token(&conductor, "test-app".into()).await;

    // Try to authenticate against the connection which is supposed to be closed
    let err = app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!("WebsocketClosed", err.to_string());

    // Token didn't get used above, so create a new connection and try to use it
    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap();

    // Authentication should have worked, so now we can make requests
    let response: AppResponse = app_tx
        .request(AppRequest::ListWasmHostFunctions)
        .await
        .unwrap();
    assert!(matches!(response, AppResponse::ListWasmHostFunctions(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn app_interface_can_handle_bad_auth_payload() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    let app_port = conductor
        .clone()
        .add_app_interface(Either::Left(0), AllowedOrigins::Any, None)
        .await
        .unwrap();

    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    // Send a payload that is clearly not an authentication request, which should kill the connection
    // but NOT the interface
    app_tx.authenticate(AppRequest::AppInfo).await.unwrap();

    let token = create_token(&conductor, "test-app".into()).await;

    // Try to authenticate against the connection which should be closed
    let err = app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!("WebsocketClosed", err.to_string());

    // Open a new connection
    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    // Now authenticate properly
    app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap();

    // Authentication should have worked, so now we can make requests
    let response: AppResponse = app_tx
        .request(AppRequest::ListWasmHostFunctions)
        .await
        .unwrap();
    assert!(matches!(response, AppResponse::ListWasmHostFunctions(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn app_interfaces_can_be_bound_to_apps() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    // App interface with an app restriction
    let app_port = conductor
        .clone()
        .add_app_interface(
            Either::Left(0),
            AllowedOrigins::Any,
            Some("test-app".to_string()),
        )
        .await
        .unwrap();

    let token = create_token(&conductor, "other-app".into()).await;

    // Try to use the app interface with a token for a different app
    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    // Authentication fails and connection closes but we don't get an error here, have to try to use
    // the connection to see that it's closed.
    app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap();

    let err = app_tx
        .request::<_, AppResponse>(AppRequest::ListWasmHostFunctions)
        .await
        .unwrap_err();
    assert_eq!("ConnectionClosed", err.to_string());

    // Now create a token for the correct app and try again
    let token = create_token(&conductor, "test-app".into()).await;

    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap();

    // This authentication should have worked, so make a request to demonstrate that.
    let response: AppResponse = app_tx
        .request(AppRequest::ListWasmHostFunctions)
        .await
        .unwrap();
    assert!(matches!(response, AppResponse::ListWasmHostFunctions(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn signals_are_not_sent_until_after_auth() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal]).await;

    let app = conductor
        .setup_app("test-app", &[dna_file.clone()])
        .await
        .unwrap();

    // emit a signal
    let _: () = conductor
        .easy_call_zome(
            app.agent(),
            None,
            app.cells().first().unwrap().cell_id().clone(),
            TestWasm::EmitSignal.coordinator_zome_name(),
            "emit",
            (),
        )
        .await
        .unwrap();

    // App interface with an app restriction
    let app_port = conductor
        .clone()
        .add_app_interface(
            Either::Left(0),
            AllowedOrigins::Any,
            Some("test-app".to_string()),
        )
        .await
        .unwrap();

    let (app_tx, mut app_rx) = websocket_client_by_port(app_port).await.unwrap();

    // We should not receive any signals yet
    tokio::time::timeout(std::time::Duration::from_millis(10), async {
        let receive = app_rx.recv::<AppResponse>().await.unwrap();
        panic!("Should not have received anything but got {:?}", receive);
    })
    .await
    .unwrap_err();

    // Now create a token and authenticate
    let token = create_token(&conductor, "test-app".into()).await;

    // Only after authenticating should we be subscribed to signals
    app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap();

    // The original signal is gone, we weren't subscribed yet
    tokio::time::timeout(std::time::Duration::from_millis(10), async {
        let receive = app_rx.recv::<AppResponse>().await.unwrap();
        panic!("Should not have received anything but got {:?}", receive);
    })
    .await
    .unwrap_err();

    // emit another signal
    let _: () = conductor
        .easy_call_zome(
            app.agent(),
            None,
            app.cells().first().unwrap().cell_id().clone(),
            TestWasm::EmitSignal.coordinator_zome_name(),
            "emit",
            (),
        )
        .await
        .unwrap();

    // Now we do get the signal
    let receive = app_rx.recv::<AppResponse>().await.unwrap();
    match receive {
        ReceiveMessage::Signal(_) => (),
        _ => panic!("Expected signal but got {:?}", receive),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn signals_are_restricted_by_app() {
    holochain_trace::test_run();

    let mut conductor = SweetConductor::from_standard_config().await;

    let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal]).await;

    let app_1 = conductor
        .setup_app("test-app-1", &[dna_file.clone()])
        .await
        .unwrap();

    let _app_2 = conductor
        .setup_app("test-app-2", &[dna_file.clone()])
        .await
        .unwrap();

    // App interface with an app restriction for app 1
    let app_1_port = conductor
        .clone()
        .add_app_interface(
            Either::Left(0),
            AllowedOrigins::Any,
            Some("test-app-1".to_string()),
        )
        .await
        .unwrap();

    // App interface with an app restriction for app 2
    let app_2_port = conductor
        .clone()
        .add_app_interface(
            Either::Left(0),
            AllowedOrigins::Any,
            Some("test-app-2".to_string()),
        )
        .await
        .unwrap();

    // App interface without an app restriction
    let app_3_port = conductor
        .clone()
        .add_app_interface(Either::Left(0), AllowedOrigins::Any, None)
        .await
        .unwrap();

    // Create connections to each interface, with two connections to the interface without an app restriction
    let (app_1_tx, mut app_1_rx) = websocket_client_by_port(app_1_port).await.unwrap();

    let (app_2_tx, mut app_2_rx) = websocket_client_by_port(app_2_port).await.unwrap();

    let (app_3_tx_app_1, mut app_3_rx_app_1) = websocket_client_by_port(app_3_port).await.unwrap();

    let (app_3_tx_app_2, mut app_3_rx_app_2) = websocket_client_by_port(app_3_port).await.unwrap();

    // Authenticate each connection with the appropriate app
    let token_1 = create_token(&conductor, "test-app-1".into()).await;
    app_1_tx
        .authenticate(AppAuthenticationRequest {
            token: token_1.clone(),
        })
        .await
        .unwrap();

    let token_2 = create_token(&conductor, "test-app-2".into()).await;
    app_2_tx
        .authenticate(AppAuthenticationRequest {
            token: token_2.clone(),
        })
        .await
        .unwrap();

    let token_3 = create_token(&conductor, "test-app-1".into()).await;
    app_3_tx_app_1
        .authenticate(AppAuthenticationRequest {
            token: token_3.clone(),
        })
        .await
        .unwrap();

    let token_4 = create_token(&conductor, "test-app-2".into()).await;
    app_3_tx_app_2
        .authenticate(AppAuthenticationRequest {
            token: token_4.clone(),
        })
        .await
        .unwrap();

    // Emit a signal from app 1
    let _: () = conductor
        .easy_call_zome(
            app_1.agent(),
            None,
            app_1.cells().first().unwrap().cell_id().clone(),
            TestWasm::EmitSignal.coordinator_zome_name(),
            "emit",
            (),
        )
        .await
        .unwrap();

    // Now check that the right connections see the signal

    // app_1_rx is connected to the app_1 interface, so should see the signal
    let receive = app_1_rx.recv::<AppResponse>().await.unwrap();
    match receive {
        ReceiveMessage::Signal(_) => (),
        _ => panic!("Expected signal but got {:?}", receive),
    }

    // app_2_rx is connected to the app_2 interface, so should not see the signal
    tokio::time::timeout(std::time::Duration::from_millis(10), async {
        let receive = app_2_rx.recv::<AppResponse>().await.unwrap();
        panic!("Should not have received anything but got {:?}", receive);
    })
    .await
    .unwrap_err();

    // app_3_rx_app_1 is connected to the app_3 which has no restriction but the connection is for
    // app_1 so should see the signal
    let receive = app_3_rx_app_1.recv::<AppResponse>().await.unwrap();
    match receive {
        ReceiveMessage::Signal(_) => (),
        _ => panic!("Expected signal but got {:?}", receive),
    }

    // app_3_rx_app_2 is connected to the app_3 which has no restriction but the connection is for
    // app_2 so should not see the signal
    tokio::time::timeout(std::time::Duration::from_millis(10), async {
        let receive = app_3_rx_app_2.recv::<AppResponse>().await.unwrap();
        panic!("Should not have received anything but got {:?}", receive);
    })
    .await
    .unwrap_err();
}

#[tokio::test(flavor = "multi_thread")]
async fn revoke_app_auth_token() {
    holochain_trace::test_run();

    let conductor = SweetConductor::from_standard_config().await;

    let app_port = conductor
        .clone()
        .add_app_interface(Either::Left(0), AllowedOrigins::Any, None)
        .await
        .unwrap();

    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    // Create a multi-use token
    let token = create_multi_use_token(&conductor, "test-app".into()).await;

    // Demonstrate that the token is valid by connecting, authenticating and sending a request
    app_tx
        .authenticate(AppAuthenticationRequest {
            token: token.clone(),
        })
        .await
        .unwrap();

    let listed: AppResponse = app_tx
        .request(AppRequest::ListWasmHostFunctions)
        .await
        .unwrap();
    assert!(matches!(listed, AppResponse::ListWasmHostFunctions(_)));

    // Now revoke the token, although it would otherwise be valid for another use!
    revoke_token(&conductor, token.clone()).await;

    // Try to connect and authenticate again
    let (app_tx, app_rx) = websocket_client_by_port(app_port).await.unwrap();
    let _app_rx = WsPollRecv::new::<AppResponse>(app_rx);

    app_tx
        .authenticate(AppAuthenticationRequest { token })
        .await
        .unwrap();

    // Authentication should have failed
    let err = app_tx
        .request::<_, AppResponse>(AppRequest::ListWasmHostFunctions)
        .await
        .unwrap_err();
    assert_eq!("ConnectionClosed", err.to_string());
}

async fn check_app_port(port: u16, origin: &str, token: AppAuthenticationToken) {
    let (client, rx) = connect(
        Arc::new(WebsocketConfig::CLIENT_DEFAULT),
        ConnectRequest::new(
            format!("localhost:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
        )
        .try_set_header("origin", origin)
        .unwrap(),
    )
    .await
    .unwrap();

    let _rx = WsPollRecv::new::<AppResponse>(rx);

    client
        .authenticate(AppAuthenticationRequest { token })
        .await
        .unwrap();

    let request = AppRequest::ListWasmHostFunctions;
    let _: AppResponse = client.request(request).await.unwrap();
}

async fn create_token(
    conductor: &SweetConductor,
    for_installed_app_id: InstalledAppId,
) -> AppAuthenticationToken {
    let (admin_tx, _admin_rx) = conductor.admin_ws_client::<AdminResponse>().await;
    let issued: AdminResponse = admin_tx
        .request(AdminRequest::IssueAppAuthenticationToken(
            for_installed_app_id.into(),
        ))
        .await
        .unwrap();

    let token = match issued {
        AdminResponse::AppAuthenticationTokenIssued(issued) => issued.token,
        _ => panic!("Unexpected response"),
    };

    token
}

async fn create_multi_use_token(
    conductor: &SweetConductor,
    for_installed_app_id: InstalledAppId,
) -> AppAuthenticationToken {
    let (admin_sender, _admin_rx) = conductor.admin_ws_client::<AdminResponse>().await;

    let token_response: AdminResponse = admin_sender
        .request(AdminRequest::IssueAppAuthenticationToken(
            IssueAppAuthenticationTokenPayload::for_installed_app_id(for_installed_app_id)
                .single_use(false)
                .expiry_seconds(0),
        ))
        .await
        .unwrap();
    let token = match token_response {
        AdminResponse::AppAuthenticationTokenIssued(issued) => issued.token,
        _ => panic!("unexpected response"),
    };

    token
}

async fn revoke_token(conductor: &SweetConductor, token: AppAuthenticationToken) {
    let (admin_sender, _admin_rx) = conductor.admin_ws_client::<AdminResponse>().await;

    let token_response: AdminResponse = admin_sender
        .request(AdminRequest::RevokeAppAuthenticationToken(token))
        .await
        .unwrap();
    match token_response {
        AdminResponse::AppAuthenticationTokenRevoked => (),
        _ => panic!("unexpected response"),
    };
}
