#![cfg(feature = "glacial_tests")]

use futures::sink::SinkExt;
use futures::stream::StreamExt;
use holochain::conductor::{
    api::{AdminRequest, AdminResponse},
    Conductor,
};
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::conductor::KeystoreConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::InterfaceDriver;
use holochain_websocket::WireMessage;
use tempfile::TempDir;
use tokio_tungstenite::*;
use tungstenite::Message;

use std::sync::atomic::{AtomicU64, Ordering};

static ID: AtomicU64 = AtomicU64::new(1);

use holochain_serialized_bytes as hsb;
use holochain_types::websocket::AllowedOrigins;

const MINUTES_LONG_BEHAVED_COUNT: usize = 10;
const MINUTES_LONG_BAD_COUNT: usize = 10;
const SECONDS_LONG_BEHAVED_COUNT: usize = 100;
const SECONDS_LONG_BAD_COUNT: usize = 100;

static CONS_MADE: AtomicU64 = AtomicU64::new(0);
static MSGS: AtomicU64 = AtomicU64::new(0);
static GOOD_CLOSE: AtomicU64 = AtomicU64::new(0);
static BAD_CLOSE: AtomicU64 = AtomicU64::new(0);

#[tokio::test(flavor = "multi_thread")]
pub async fn websocket_stress() {
    let tmp_dir = TempDir::new().unwrap();
    let data_root_path = tmp_dir.path().to_path_buf();
    let config = ConductorConfig {
        admin_interfaces: Some(vec![AdminInterfaceConfig {
            driver: InterfaceDriver::Websocket {
                port: 0,
                allowed_origins: AllowedOrigins::Any,
            },
        }]),
        data_root_path: Some(data_root_path.into()),
        keystore: KeystoreConfig::DangerTestKeystore,
        ..ConductorConfig::empty()
    };
    let conductor_handle = Conductor::builder().config(config).build().await.unwrap();
    let port = conductor_handle
        .get_arbitrary_admin_websocket_port()
        .expect("No admin port open on conductor");

    for _ in 0..MINUTES_LONG_BEHAVED_COUNT {
        tokio::task::spawn(run_client(port, 30, false));
    }

    for _ in 0..MINUTES_LONG_BAD_COUNT {
        tokio::task::spawn(run_client(port, 30, true));
    }

    for _ in 0..SECONDS_LONG_BEHAVED_COUNT {
        tokio::task::spawn(run_client(port, 1, false));
    }

    for _ in 0..SECONDS_LONG_BAD_COUNT {
        tokio::task::spawn(run_client(port, 1, true));
    }

    for _ in 0..(6 * 4/* 4 minutes */) {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        println!(
            "Connections: {}, Messages: {}, Graceful Closes: {}, Bad Closes: {}",
            CONS_MADE.load(Ordering::Relaxed),
            MSGS.load(Ordering::Relaxed),
            GOOD_CLOSE.load(Ordering::Relaxed),
            BAD_CLOSE.load(Ordering::Relaxed),
        );
    }
}

async fn run_client(port: u16, wait: u64, is_bad: bool) {
    let req: Vec<u8> = hsb::UnsafeBytes::from(hsb::encode(&AdminRequest::ListDnas).unwrap())
        .try_into()
        .unwrap();

    loop {
        let (mut client, _) = connect_async(format!("ws://127.0.0.1:{}", port))
            .await
            .unwrap();
        CONS_MADE.fetch_add(1, Ordering::Relaxed);

        for _ in 0..5 {
            let this_id = ID.fetch_add(1, Ordering::Relaxed);
            let msg = WireMessage::Request {
                id: this_id,
                data: req.clone(),
            };
            let msg: Vec<u8> = hsb::UnsafeBytes::from(hsb::encode(&msg).unwrap())
                .try_into()
                .unwrap();

            client.send(Message::Binary(msg)).await.unwrap();

            while let Some(msg) = client.next().await {
                let rsp: Vec<u8> = msg.unwrap().into_data();
                let rsp: WireMessage = hsb::decode(&rsp).unwrap();
                if let WireMessage::Response { id, data } = rsp {
                    if id == this_id {
                        if let Some(data) = data {
                            let rsp: AdminResponse = hsb::decode(&data).unwrap();
                            assert!(matches!(rsp, AdminResponse::DnasListed { .. }));
                            break;
                        } else {
                            panic!("no data");
                        }
                    }
                }
            }

            MSGS.fetch_add(1, Ordering::Relaxed);

            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        }

        if is_bad {
            client.send(Message::Binary(req.clone())).await.unwrap();
            BAD_CLOSE.fetch_add(1, Ordering::Relaxed);
        } else {
            client.close(None).await.unwrap();
            GOOD_CLOSE.fetch_add(1, Ordering::Relaxed);
        }

        drop(client);
    }
}
