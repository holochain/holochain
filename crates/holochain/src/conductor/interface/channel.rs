use crate::conductor::{api::*, interface::interface::*};
use crate::core::signal::Signal;
//use async_trait::async_trait;
//use tracing::*;
use super::error::{InterfaceError, InterfaceResult};
use futures::select;
use holochain_serialized_bytes::SerializedBytes;
use holochain_websocket::{websocket_bind, WebsocketConfig, WebsocketMessage, WebsocketReceiver};
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use tokio::stream::StreamExt;
use tokio::sync::broadcast;
use tracing::*;
use url2::url2;

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

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Stub;
    holochain_serialized_bytes::holochain_serial!(Stub);

    let (_chan_sig_send, chan_req_recv): (
        ConductorSideSignalSender<Stub>, // stub impl signals
        ConductorSideRequestReceiver<ConductorRequest, ConductorResponse>,
    ) = create_interface_channel(send_sig, recv_req);

    let join_handle = attach_external_conductor_api(api, chan_req_recv);

    (send_req, join_handle)
}

pub async fn create_app_interface(
    port: u16,
    signal_broadcaster: broadcast::Sender<Signal>,
) -> InterfaceResult<()> {
    let mut listener = websocket_bind(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    trace!("LISTENING AT: {}", listener.local_addr());
    let mut listener_handles = Vec::new();
    while let Some(maybe_con) = listener.next().await {
        let (send_socket, recv_socket) = maybe_con.await?;
        let signal_rx = signal_broadcaster.subscribe();
        listener_handles.push(tokio::task::spawn(recv_msgs_and_signals(
            recv_socket,
            signal_rx,
        )));
    }
    for h in listener_handles {
        h.await?;
    }
    Ok(())
}

async fn recv_msgs(mut recv_socket: WebsocketReceiver) -> () {
    while let Some(msg) = recv_socket.next().await {
        if let Err(_todo) = handle_msg(msg).await {
            break;
        }
    }
}

async fn recv_msgs_and_signals(
    recv_socket: WebsocketReceiver,
    signal_rx: broadcast::Receiver<Signal>,
) -> InterfaceResult<()> {
    trace!("CONNECTION: {}", recv_socket.remote_addr());

    let mut rx = {
        signal_rx
            .map(|signal| {
                InterfaceResult::Ok(WebsocketMessage::Signal(SerializedBytes::try_from(
                    signal.map_err(InterfaceError::SignalReceive)?,
                )?))
            })
            .merge(recv_socket.map(Ok))
    };

    while let Some(msg) = rx.next().await {
        if let Ok(msg) = msg {
            if let Err(_todo) = handle_msg(msg).await {
                unimplemented!()
            }
        } else {
            unimplemented!()
        }
    }
    Ok(())
}

async fn handle_msg(msg: WebsocketMessage) -> InterfaceResult<()> {
    match msg {
        WebsocketMessage::Request(msg, response) => {
            let sb: SerializedBytes =
                SerializedBytes::try_from(ConductorResponse::AdminResponse {
                    response: Box::new(AdminResponse::DnaAdded),
                })?;
            response(sb).await?;
            Ok(())
        }
        msg => {
            debug!("Other message: {:?}", msg);
            unimplemented!()
        }
    }
}
