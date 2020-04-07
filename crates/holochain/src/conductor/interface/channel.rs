use crate::conductor::{api::*, interface::interface::*};
use holochain_websocket::{websocket_bind, WebsocketConfig, WebsocketMessage};
use sx_types::prelude::*;
use tokio::stream::StreamExt;
use tracing::*;
use url2::url2;

//use async_trait::async_trait;
//use tracing::*;

use holochain_serialized_bytes::SerializedBytes;

/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a ExternalConductorApi
pub fn create_demo_channel_interface<A: ExternalConductorApi>(
    api: A,
) -> (
    futures::channel::mpsc::Sender<(SerializedBytes, ExternalSideResponder)>,
    tokio::task::JoinHandle<()>,
) {
    let (send_sig, _recv_sig) = futures::channel::mpsc::channel(1);
    let (send_req, recv_req) = futures::channel::mpsc::channel(1);

    let (_chan_sig_send, chan_req_recv): (
        ConductorSideSignalSender<SerializedBytes>, // stub impl signals
        ConductorSideRequestReceiver<InterfaceMsgIncoming, InterfaceMsgOutgoing>,
    ) = create_interface_channel(send_sig, recv_req);

    let join_handle = attach_external_conductor_api(api, chan_req_recv);

    (send_req, join_handle)
}

/*
/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a ExternalConductorApi
pub async fn create_websocket_interface<A: ExternalConductorApi>(
    api: A,
    port: u16,
) -> (
    futures::channel::mpsc::Sender<(SerializedBytes, ExternalSideResponder)>,
    tokio::task::JoinHandle<()>,
) {
    let listener = websocket_bind(
        url2!("ws://127.0.0.1:{}", port),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    let (send_sig, _recv_sig) = futures::channel::mpsc::channel(1);
    let (send_req, recv_req) = futures::channel::mpsc::channel(1);

    let (_chan_sig_send, chan_req_recv): (
        ConductorSideSignalSender<SerializedBytes>, // stub impl signals
        ConductorSideRequestReceiver<InterfaceMsgIncoming, InterfaceMsgOutgoing>,
    ) = create_interface_channel(send_sig, recv_req);

    let (send_b, _) = tokio::sync::broadcast::channel(10);

    while let Some(maybe_con) = listener.next().await {
        let loc_send_b = send_b.clone();
        let mut loc_recv_b = send_b.subscribe();

        let _ = tokio::task::spawn(async move {
            let (mut send_socket, mut recv_socket) = maybe_con.await.unwrap();

            trace!("websocket connection: {}", recv_socket.remote_addr());

            let _ = tokio::task::spawn(async move {
                while let Some(msg) = recv_socket.next().await {
                    match msg {
                        WebsocketMessage::Signal(msg) => {
                            warn!("Not expecting signals from client, but got: {:?}", msg);
                        }
                        WebsocketMessage::Request(msg, response) => {
                            let msg: InterfaceMsgIncoming = msg.try_into().expect("TODO");
                            trace!("RESPONDING to: {:?}", msg);
                            let response_msg: InterfaceMsgOutgoing = match msg {
                                InterfaceMsgIncoming::AdminRequest(request) => {
                                    InterfaceMsgOutgoing::AdminResponse(Box::new(
                                        AdminResponse::Stub,
                                    ))
                                }
                                InterfaceMsgIncoming::CryptoRequest(request) => {
                                    InterfaceMsgOutgoing::CryptoResponse(Box::new(
                                        CryptoResponse::Stub,
                                    ))
                                }
                                InterfaceMsgIncoming::ZomeInvocationRequest(request) => {
                                    unimplemented!()
                                }
                            };
                            response(response_msg.try_into().unwrap()).await.unwrap();
                        }
                        msg => {
                            eprintln!("ERROR: {:?}", msg);
                            break;
                        }
                    }
                }
            });

            let _ = tokio::task::spawn(async move {
                while let Some(Ok(msg)) = loc_recv_b.next().await {
                    send_socket.signal(msg).await.unwrap();
                }
            });
        });
    }
    let join_handle = attach_external_conductor_api(api, chan_req_recv);

    (send_req, join_handle)
}
*/