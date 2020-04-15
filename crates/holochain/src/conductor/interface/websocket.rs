use crate::conductor::{
    conductor::StopReceiver,
    interface::*,
    manager::{ManagedTaskHandle, ManagedTaskResult},
};
use crate::core::signal::Signal;
//use async_trait::async_trait;
//use tracing::*;
use super::error::{InterfaceError, InterfaceResult};
use holochain_serialized_bytes::SerializedBytes;
use holochain_wasmer_host::TryInto;
use holochain_websocket::{
    websocket_bind, WebsocketConfig, WebsocketListener, WebsocketMessage, WebsocketReceiver,
    WebsocketSender,
};
use std::convert::TryFrom;

use std::sync::Arc;
use tokio::stream::StreamExt;
use tokio::sync::broadcast;
use tracing::*;
use url2::url2;

/// Create an Admin Interface, which only receives AdminRequest messages
/// from the external client
pub async fn spawn_websocket_listener(port: u16) -> InterfaceResult<WebsocketListener> {
    trace!("Initializing Admin interface");
    let listener = websocket_bind(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    trace!("LISTENING AT: {}", listener.local_addr());
    Ok(listener)
}

pub fn spawn_admin_interface_task<A: InterfaceApi>(
    mut listener: WebsocketListener,
    api: A,
    mut stop_rx: StopReceiver,
) -> InterfaceResult<ManagedTaskHandle> {
    Ok(tokio::task::spawn(async move {
        let mut listener_handles = Vec::new();
        let mut send_sockets = Vec::new();
        loop {
            tokio::select! {
                // break if we receive on the stop channel
                _ = stop_rx.recv() => { break; },

                // establish a new connection to a client
                maybe_con = listener.next() => if let Some(conn) = maybe_con {
                    // TODO this could take some time and should be spawned
                    // This will be fixed by TK-01260
                    if let Ok((send_socket, recv_socket)) = conn.await {
                        send_sockets.push(send_socket);
                        listener_handles.push(tokio::task::spawn(recv_incoming_admin_msgs(
                            api.clone(),
                            recv_socket,
                        )));
                    }
                } else {
                    // This shouldn't actually ever happen, but if it did,
                    // we would just stop the listener task
                    break;
                }
            }
        }
        // TODO: TEST: drop listener, make sure all these tasks finish!
        drop(listener);

        // TODO Make send_socket close tell the recv socket to close locally in the websocket code
        for mut send_socket in send_sockets {
            // TODO change from u16 code to enum
            send_socket.close(1000, "Shutting down".into()).await?;
        }

        // these SHOULD end soon after we get here, or by the time we get here,
        // if not this will hang. Maybe that's OK, in which case we don't await
        for h in listener_handles {
            h.await?;
        }
        ManagedTaskResult::Ok(())
    }))
}

/// Create an App Interface, which includes the ability to receive signals
/// from Cells via a broadcast channel
// TODO: hook up a kill channel similar to `spawn_admin_interface_task` above
pub async fn spawn_app_interface_task<A: InterfaceApi>(
    port: u16,
    api: A,
    signal_broadcaster: broadcast::Sender<Signal>,
) -> InterfaceResult<()> {
    trace!("Initializing App interface");
    let mut listener = websocket_bind(
        url2!("ws://127.0.0.1:{}", port),
        Arc::new(WebsocketConfig::default()),
    )
    .await?;
    trace!("LISTENING AT: {}", listener.local_addr());
    let mut listener_handles = Vec::new();
    // TODO there is no way to exit this listner
    // If we remove the interface then we want to kill this lister
    while let Some(maybe_con) = listener.next().await {
        let (send_socket, recv_socket) = maybe_con.await?;
        let signal_rx = signal_broadcaster.subscribe();
        listener_handles.push(tokio::task::spawn(recv_incoming_msgs_and_outgoing_signals(
            api.clone(),
            recv_socket,
            signal_rx,
            send_socket,
        )));
    }
    for h in listener_handles {
        h.await??;
    }
    Ok(())
}

/// A trivial Interface, used for proof of concept only,
/// which is driven externally by a channel in order to
/// interact with a AppInterfaceApi
pub fn create_demo_channel_interface<A: AppInterfaceApi>(
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
        ConductorSideRequestReceiver<AppRequest, AppResponse>,
    ) = create_interface_channel(send_sig, recv_req);

    let join_handle = attach_external_conductor_api(api, chan_req_recv);

    (send_req, join_handle)
}

/// Polls for messages coming in from the external client.
/// Used by Admin interface.
async fn recv_incoming_admin_msgs<A: InterfaceApi>(api: A, mut recv_socket: WebsocketReceiver) {
    while let Some(msg) = recv_socket.next().await {
        if let Err(_todo) = handle_incoming_message(msg, api.clone()).await {
            break;
        }
    }
}

/// Polls for messages coming in from the external client while simultaneously
/// polling for signals being broadcast from the Cells associated with this
/// App interface.
async fn recv_incoming_msgs_and_outgoing_signals<A: InterfaceApi>(
    api: A,
    mut recv_socket: WebsocketReceiver,
    mut signal_rx: broadcast::Receiver<Signal>,
    mut signal_tx: WebsocketSender,
) -> InterfaceResult<()> {
    trace!("CONNECTION: {}", recv_socket.remote_addr());

    loop {
        // T: FIXME this will return on whoever is first and cancel
        // all remaining tasks. Is that what we want?
        // M: This is straight from a tokio example for listening on two
        // streams simultaneously. The task that's canceled is the other
        // `next()`, which allows us to go back to the top of the loop to
        // listen on both channels yet again.
        tokio::select! {
            // If we receive a Signal broadcasted from a Cell, push it out
            // across the interface
            signal = signal_rx.next() => {
                if let Some(signal) = signal {
                    let bytes = SerializedBytes::try_from(
                        signal.map_err(InterfaceError::SignalReceive)?,
                    )?;
                    signal_tx.signal(bytes).await?;
                } else {
                    debug!("Closing interface: signal stream empty");
                    break;
                }
            },

            // If we receive a message from outside, handle it
            msg = recv_socket.next() => {
                if let Some(msg) = msg {
                    // FIXME I'm not sure if cloning is the right thing to do here
                    handle_incoming_message(msg, api.clone()).await?
                } else {
                    debug!("Closing interface: message stream empty");
                    break;
                }
            },
        }
    }

    Ok(())
}

async fn handle_incoming_message<A>(ws_msg: WebsocketMessage, api: A) -> InterfaceResult<()>
where
    A: InterfaceApi,
{
    match ws_msg {
        WebsocketMessage::Request(bytes, respond) => {
            Ok(respond(api.handle_request(bytes.try_into()).await?.try_into()?).await?)
        }
        // FIXME this will kill this interface, is that what we want?
        WebsocketMessage::Signal(_) => Err(InterfaceError::UnexpectedMessage(
            "Got an unexpected Signal while handing incoming message".to_string(),
        )),
        WebsocketMessage::Close(_) => unimplemented!(),
    }
}

/* I don't think we need this?
async fn handle_incoming_admin_request(request: AdminRequest) -> InterfaceResult<AdminResponse> {
    Ok(match request {
        _ => AdminResponse::DnaAdded,
    })
}
*/

// TODO: rename AppRequest to AppRequest or something
async fn handle_incoming_app_request(request: AppRequest) -> InterfaceResult<AppResponse> {
    Ok(match request {
        _ => AppResponse::Error {
            debug: "TODO".into(),
        },
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::conductor::interface::error::AdminInterfaceError;
    use crate::conductor::{
        api::{AdminRequest, AdminResponse, StdAdminInterfaceApi},
        Conductor,
    };
    use futures::future::FutureExt;
    use holochain_serialized_bytes::prelude::*;
    use holochain_websocket::WebsocketMessage;
    use matches::assert_matches;
    use std::{collections::HashSet, convert::TryInto};
    use sx_types::{
        observability,
        prelude::*,
        test_utils::{fake_dna, fake_dna_file},
    };
    use uuid::Uuid;

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    #[serde(rename = "snake-case", tag = "type", content = "data")]
    enum AdmonRequest {
        InstallsDna(String),
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes, Clone)]
    #[serde(rename = "snake-case")]
    struct CustomProperties {
        the_answer: u32,
        secret: String,
    }

    async fn setup() -> StdAdminInterfaceApi {
        let conductor = Conductor::build().test().await.unwrap();
        StdAdminInterfaceApi::new(conductor)
    }

    #[tokio::test]
    async fn serialization_failure() {
        let admin_api = setup().await;
        let msg = AdmonRequest::InstallsDna("".into());
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::Error{ error_type: AdminInterfaceError::Serialization, ..});
            async { Ok(()) }.boxed()
        };
        let respond = Box::new(respond);
        let msg = WebsocketMessage::Request(msg, respond);
        handle_incoming_message(msg, admin_api).await.unwrap();
    }

    #[tokio::test]
    async fn invalid_request() {
        observability::test_run().ok();
        let admin_api = setup().await;
        let msg = AdminRequest::InstallDna("some$\\//weird00=-+[] \\Path".into(), None);
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::Error{ error_type: AdminInterfaceError::Io, ..});
            async { Ok(()) }.boxed()
        };
        let respond = Box::new(respond);
        let msg = WebsocketMessage::Request(msg, respond);
        handle_incoming_message(msg, admin_api).await.unwrap()
    }

    #[ignore]
    #[tokio::test]
    async fn cache_failure() {
        // TODO this can't be done easily yet
        // because we can't cause the cache to fail from an input
    }

    #[ignore]
    #[tokio::test]
    async fn deserialization_failure() {
        // TODO this can't be done easily yet
        // because we can't serialize something that
        // doesn't deserialize
    }

    #[tokio::test]
    async fn with_unique_parameters() {
        let uuid = Uuid::new_v4();
        let dna = fake_dna(&uuid.to_string());
        let properties = CustomProperties {
            the_answer: 42,
            secret: "types can sometimes hurt".to_string(),
        };

        let (fake_dna_path, _t) = fake_dna_file(dna.clone()).unwrap();
        let admin_api = setup().await;

        // Expecting
        let mut expecting = vec![dna.clone(), dna.clone()];
        expecting[0].properties = properties.clone().try_into().unwrap();

        // Install Dna 1
        let msg = AdminRequest::InstallDna(fake_dna_path.clone(), Some(properties.try_into().unwrap()));
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::DnaInstalled);
            async { Ok(()) }.boxed()
        };
        let respond = Box::new(respond);
        let msg = WebsocketMessage::Request(msg, respond);
        handle_incoming_message(msg, admin_api.clone()).await.unwrap();

        // Install Dna 2
        let properties = CustomProperties {
            the_answer: 4242,
            secret: "Sometimes they have you're back".to_string(),
        };
        expecting[1].properties = properties.clone().try_into().unwrap();
        let msg = AdminRequest::InstallDna(fake_dna_path, Some(properties.try_into().unwrap()));
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            assert_matches!(response, AdminResponse::DnaInstalled);
            async { Ok(()) }.boxed()
        };
        let respond = Box::new(respond);
        let msg = WebsocketMessage::Request(msg, respond);
        handle_incoming_message(msg, admin_api.clone()).await.unwrap();

        // List Dna
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = AdminRequest::ListDnas;
        let msg = msg.try_into().unwrap();
        let respond = |bytes: SerializedBytes| {
            let response: AdminResponse = bytes.try_into().unwrap();
            match response {
                AdminResponse::ListDnas(dnas) => tx.send(dnas).unwrap(),
                _ => panic!("Bad response {:?}", response),
            }
            async { Ok(()) }.boxed()
        };
        let respond = Box::new(respond);
        let msg = WebsocketMessage::Request(msg, respond);
        handle_incoming_message(msg, admin_api).await.unwrap();

        let expecting: HashSet<Address> = expecting.into_iter().map(|dna| dna.address()).collect();
        let result: HashSet<Address> = rx.await.unwrap().into_iter().collect();
        assert_eq!(expecting, result);
    }
}
