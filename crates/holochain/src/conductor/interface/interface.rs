use crate::conductor::api::*;

use futures::{
    channel::mpsc::{channel, Receiver, Sender},
    future::{BoxFuture, FutureExt},
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
};
use holochain_serialized_bytes::SerializedBytes;
use std::convert::{TryFrom, TryInto};

/// Interface Error Type
#[derive(Debug, thiserror::Error)]
pub enum InterfaceError {
    SerializedBytesConvert,
    SendError,
    Other(String),
}

impl std::fmt::Display for InterfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<String> for InterfaceError {
    fn from(o: String) -> Self {
        InterfaceError::Other(o)
    }
}

impl From<futures::channel::mpsc::SendError> for InterfaceError {
    fn from(_: futures::channel::mpsc::SendError) -> Self {
        InterfaceError::SendError
    }
}

/// Interface Result Type
pub type InterfaceResult<T> = Result<T, InterfaceError>;

/// Allows the conductor or cell to forward signals to connected clients
pub struct ConductorSideSignalSender<Sig>
where
    Sig: 'static + Send + TryInto<SerializedBytes>,
{
    sender: Sender<SerializedBytes>,
    phantom: std::marker::PhantomData<Sig>,
}

impl<Sig> ConductorSideSignalSender<Sig>
where
    Sig: 'static + Send + TryInto<SerializedBytes>,
{
    /// send the signal to the connected client
    pub async fn send(&mut self, data: Sig) -> InterfaceResult<()> {
        let data: SerializedBytes = data
            .try_into()
            .map_err(|_| InterfaceError::SerializedBytesConvert)?;
        self.sender.send(data).await?;
        Ok(())
    }

    // -- private -- //

    /// internal constructor
    fn priv_new(sender: Sender<SerializedBytes>) -> Self {
        Self {
            sender,
            phantom: std::marker::PhantomData,
        }
    }
}

/// callback type to handle incoming requests from a connected client
#[must_use]
pub type ConductorSideResponder<Res> =
    Box<dyn FnOnce(Res) -> BoxFuture<'static, InterfaceResult<()>> + 'static + Send>;

/// receive a stream of incoming requests from a connected client
pub type ConductorSideRequestReceiver<Req, Res> = Receiver<(Req, ConductorSideResponder<Res>)>;

/// the external side callback type to use when implementing a client interface
#[must_use]
pub type ExternalSideResponder =
    Box<dyn FnOnce(SerializedBytes) -> BoxFuture<'static, InterfaceResult<()>> + 'static + Send>;

/// construct a new api interface to allow clients to connect to conductor or cell
/// supply this function with:
/// - a signal sender(sink)
/// - a request(and response callback) stream
pub fn create_interface_channel<Sig, Req, Res, XSig, XReq>(
    x_sig: XSig,
    x_req: XReq,
) -> (
    ConductorSideSignalSender<Sig>,
    ConductorSideRequestReceiver<Req, Res>,
)
where
    Sig: 'static + Send + TryInto<SerializedBytes>,
    Req: 'static + Send + TryFrom<SerializedBytes>,
    <Req as TryFrom<SerializedBytes>>::Error: std::fmt::Debug + Send,
    Res: 'static + Send + TryInto<SerializedBytes>,
    XSig: 'static + Send + Sink<SerializedBytes>,
    <XSig as Sink<SerializedBytes>>::Error: std::fmt::Debug + Send,
    XReq: 'static + Send + Stream<Item = (SerializedBytes, ExternalSideResponder)>,
{
    // pretty straight forward to forward the signal sender : )
    let (sig_send, sig_recv) = channel(10);
    tokio::task::spawn(sig_recv.map(|x| Ok(x)).forward(x_sig));

    // we need to do some translations on the request/response flow
    let (req_send, req_recv) = channel(10);
    tokio::task::spawn(
        x_req
            .map(|(data, respond)| {
                // translate the response from concrete type to SerializedBytes
                let respond: ConductorSideResponder<Res> = Box::new(move |res| {
                    async move {
                        let res: SerializedBytes = res
                            .try_into()
                            .map_err(|_| InterfaceError::SerializedBytesConvert)?;
                        respond(res).await?;
                        Ok(())
                    }
                    .boxed()
                });

                // we cannot procede if the data is not serializable
                // let's fail fast here for now
                let data = Req::try_from(data).expect("deserialize failed");

                Ok((data, respond))
            })
            .forward(req_send),
    );

    // return the sender and the request/response stream
    (ConductorSideSignalSender::priv_new(sig_send), req_recv)
}

/// bind a conductor-side request receiver to a particular conductor api
pub fn attach_external_conductor_api(
    api: ExternalConductorApi,
    mut recv: ConductorSideRequestReceiver<ConductorRequest, ConductorResponse>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        while let Some((request, respond)) = recv.next().await {
            if let Err(e) = respond(api.handle_request(request).await).await {
                tracing::error!(error = ?e);
            }
        }
    })
}

#[cfg(test)]
mod tests {
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
            ConductorSideSignalSender<TestMsg>,
            ConductorSideRequestReceiver<TestMsg, TestMsg>,
        ) = create_interface_channel(send_sig, recv_req);

        chan_sig_send.send("test_sig_1".into()).await.unwrap();

        assert_eq!(
            "test_sig_1",
            &TestMsg::try_from(recv_sig.next().await.unwrap()).unwrap().0,
        );

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

        let (req, respond) = chan_req_recv.next().await.unwrap();

        assert_eq!("test_req_1", &req.0,);

        respond("test_res_1".into()).await.unwrap();

        assert_eq!(
            "test_res_1",
            &TestMsg::try_from(res_recv.await.unwrap()).unwrap().0,
        );
    }
}
