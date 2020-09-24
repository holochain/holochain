use crate::{conductor::api::*, core::signal::Signal};
use error::{InterfaceError, InterfaceResult};
use futures::{
    channel::mpsc::{channel, Receiver, Sender},
    future::{BoxFuture, FutureExt},
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
};
use holochain_serialized_bytes::{SerializedBytes, SerializedBytesError};
use serde::{Deserialize, Serialize};
use std::convert::{TryFrom, TryInto};
use tokio::sync::broadcast;

pub mod error;
pub mod websocket;

#[derive(Clone, Debug)]
pub struct SignalMulticaster(Vec<broadcast::Sender<Signal>>);

impl SignalMulticaster {
    /// send the signal to the connected client
    pub async fn send(&mut self, sig: Signal) -> InterfaceResult<()> {
        self.0
            .iter_mut()
            .map(|tx| tx.send(sig.clone()))
            .collect::<Result<Vec<_>, broadcast::SendError<Signal>>>()
            .map_err(InterfaceError::SignalSend)?;
        Ok(())
    }

    /// internal constructor
    pub fn new(senders: Vec<broadcast::Sender<Signal>>) -> Self {
        Self(senders)
    }

    #[cfg(test)]
    /// A sender with nothing to send to. A placeholder for tests
    pub fn noop() -> Self {
        Self(Vec::new())
    }
}

/// Allows the conductor or cell to forward signals to connected clients
#[derive(Clone)]
pub struct ConductorSideSignalSenderXx(Sender<Signal>);

impl ConductorSideSignalSenderXx {
    /// send the signal to the connected client
    pub async fn send(&mut self, sig: Signal) -> InterfaceResult<()> {
        self.0.send(sig).await?;
        Ok(())
    }

    // -- private -- //

    /// internal constructor
    fn priv_new(sender: Sender<Signal>) -> Self {
        Self(sender)
    }
}

/// callback type to handle incoming requests from a connected client
pub type ConductorSideResponder<Res> =
    Box<dyn FnOnce(Res) -> BoxFuture<'static, InterfaceResult<()>> + 'static + Send>;

/// receive a stream of incoming requests from a connected client
pub type ConductorSideRequestReceiver<Req, Res> =
    Receiver<InterfaceResult<(Req, ConductorSideResponder<Res>)>>;

/// the external side callback type to use when implementing a client interface
pub type ExternalSideResponder =
    Box<dyn FnOnce(SerializedBytes) -> BoxFuture<'static, InterfaceResult<()>> + 'static + Send>;

/// construct a new api interface to allow clients to connect to conductor or cell
/// supply this function with:
/// - a signal sender(sink)
/// - a request(and response callback) stream
// FIXME: this is only used in a test. If it doesn't wind up getting used, remove.
pub fn create_interface_channel<Req, Res, ExternSig, ExternReq>(
    // the "external signal sink" - A sender that accepts already serialized
    // SerializedBytes.
    extern_sig: ExternSig,
    // the "external request stream" - A stream that provides serialized
    // SerializedBytes - as well as ExternalSideResponder callbacks.
    extern_req: ExternReq,
) -> (
    // creates a conductor side sender that accepts concrete signal types.
    ConductorSideSignalSenderXx,
    // creates a conductor side receiver that produces concrete request types.
    ConductorSideRequestReceiver<Req, Res>,
)
where
    Req: 'static + Send + TryFrom<SerializedBytes, Error = SerializedBytesError>,
    <Req as TryFrom<SerializedBytes>>::Error: std::fmt::Debug + Send,
    Res: 'static + Send + TryInto<SerializedBytes, Error = SerializedBytesError>,
    ExternSig: 'static + Send + Sink<Signal>,
    <ExternSig as Sink<Signal>>::Error: std::fmt::Debug + Send,
    ExternReq: 'static + Send + Stream<Item = (SerializedBytes, ExternalSideResponder)>,
{
    // pretty straight forward to forward the signal sender : )
    let (sig_send, sig_recv) = channel(10);

    // we can ignore this JoinHandle, because if conductor is dropped,
    // both sides of this forward will be dropped and the task will end.
    let _ = tokio::task::spawn(sig_recv.map(Ok).forward(extern_sig));

    // we need to do some translations on the request/response flow
    let (req_send, req_recv) = channel(10);

    // we can ignore this JoinHandle, because if conductor is dropped,
    // both sides of this forward will be dropped and the task will end.
    let _ = tokio::task::spawn(
        extern_req
            .map(|(data, respond)| {
                // translate the response from concrete type to SerializedBytes
                let respond: ConductorSideResponder<Res> = Box::new(move |res| {
                    async move {
                        let res: SerializedBytes = res.try_into()?;
                        respond(res).await?;
                        Ok(())
                    }
                    .boxed()
                });

                let data = match Req::try_from(data) {
                    Ok(data) => data,
                    Err(e) => return Ok(Err(e.into())),
                };

                Ok(Ok((data, respond)))
            })
            .forward(req_send),
    );

    // return the sender and the request/response stream
    (ConductorSideSignalSenderXx::priv_new(sig_send), req_recv)
}

/// bind a conductor-side request receiver to a particular conductor api
pub fn attach_external_conductor_api<A: AppInterfaceApi>(
    api: A,
    mut recv: ConductorSideRequestReceiver<AppRequest, AppResponse>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        while let Some(msg) = recv.next().await {
            match msg {
                Ok((request, respond)) => {
                    if let Err(e) = respond(api.handle_app_request(request).await).await {
                        tracing::error!(error = ?e);
                    }
                }
                Err(e) => {
                    tracing::error!(error = ?e);
                }
            }
        }
    })
}

/// Configuration for interfaces, specifying the means by which an interface
/// should be opened.
///
/// NB: This struct is used in both [ConductorConfig] and [ConductorState], so
/// it is important that the serialization technique is not altered.
//
// TODO: write test that ensures the serialization is unaltered
#[derive(Clone, Deserialize, Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InterfaceDriver {
    Websocket { port: u16 },
}

#[cfg(test)]
mod tests {

    use crate::core::signal::test_signal;

    use super::*;

    #[tokio::test]
    async fn test_create_interface_channel() {
        #[derive(Debug, serde::Serialize, serde::Deserialize)]
        struct TestMsg(pub String);
        holochain_serialized_bytes::holochain_serial!(TestMsg);

        impl From<&str> for TestMsg {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        let (send_sig, mut recv_sig) = channel(1);
        let (mut send_req, recv_req) = channel(1);

        let (mut chan_sig_send, mut chan_req_recv): (
            ConductorSideSignalSenderXx,
            ConductorSideRequestReceiver<TestMsg, TestMsg>,
        ) = create_interface_channel(send_sig, recv_req);

        chan_sig_send.send(test_signal("test_sig_1")).await.unwrap();
        let signal_bytes = SerializedBytes::try_from(recv_sig.next().await.unwrap()).unwrap();
        assert_eq!("test_sig_1", &TestMsg::try_from(signal_bytes).unwrap().0,);

        let (res_send, res_recv) = tokio::sync::oneshot::channel();
        let respond: ExternalSideResponder = Box::new(move |res| {
            async move {
                let _ = res_send.send(res);
                Ok(())
            }
            .boxed()
        });
        let msg: TestMsg = "test_req_1".into();
        send_req
            .send((msg.try_into().unwrap(), respond))
            .await
            .unwrap();

        let (req, respond) = chan_req_recv.next().await.unwrap().unwrap();

        assert_eq!("test_req_1", &req.0,);

        respond("test_res_1".into()).await.unwrap();

        assert_eq!(
            "test_res_1",
            &TestMsg::try_from(res_recv.await.unwrap()).unwrap().0,
        );
    }
}
