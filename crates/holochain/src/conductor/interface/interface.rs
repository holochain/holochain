use crate::conductor::api::ExternalConductorApi;
use async_trait::async_trait;

use futures::{
    channel::mpsc::{channel, Receiver, Sender},
    future::{BoxFuture, FutureExt},
    sink::{Sink, SinkExt},
    stream::{Stream, StreamExt},
};
use holochain_serialized_bytes::SerializedBytes;
use std::convert::{TryFrom, TryInto};

/// Allows the conductor or cell to forward signals to connected clients
pub struct ConductorSideSignalSender<Sig>
where
    Sig: 'static + Send + TryInto<SerializedBytes>,
{
    sender: Sender<Result<SerializedBytes, ()>>,
    phantom: std::marker::PhantomData<Sig>,
}

impl<Sig> ConductorSideSignalSender<Sig>
where
    Sig: 'static + Send + TryInto<SerializedBytes>,
{
    /// send the signal to the connected client
    pub async fn send(&mut self, data: Sig) -> Result<(), ()> {
        self.sender
            .send(data.try_into().map_err(|_| ()))
            .await
            .map_err(|_| ())
    }

    // -- private -- //

    /// internal constructor
    fn priv_new(sender: Sender<Result<SerializedBytes, ()>>) -> Self {
        Self {
            sender,
            phantom: std::marker::PhantomData,
        }
    }
}

/// callback type to handle incoming requests from a connected client
pub type ConductorSideResponder<Res> =
    Box<dyn FnOnce(Res) -> BoxFuture<'static, Result<(), ()>> + 'static + Send>;

/// receive a stream of incoming requests from a connected client
pub type ConductorSideRequestReceiver<Req, Res> =
    Receiver<Result<(Req, ConductorSideResponder<Res>), ()>>;

/// the external side callback type to use when implementing a client interface
pub type ExternalSideResponder =
    Box<dyn FnOnce(SerializedBytes) -> BoxFuture<'static, Result<(), ()>> + 'static + Send>;

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
    Res: 'static + Send + TryInto<SerializedBytes>,
    XSig: 'static + Send + Sink<SerializedBytes, Error = ()>,
    XReq: 'static + Send + Stream<Item = (SerializedBytes, ExternalSideResponder)>,
{
    let (sig_send, sig_recv) = channel(10);
    tokio::task::spawn(sig_recv.forward(x_sig));

    let (req_send, req_recv) = channel(10);
    tokio::task::spawn(
        x_req
            .map(|(data, respond)| {
                let respond: ConductorSideResponder<Res> = Box::new(move |res| {
                    async move {
                        let res: SerializedBytes = res.try_into().map_err(|_| ())?;
                        respond(res).await?;
                        Ok(())
                    }
                    .boxed()
                });
                Ok(match Req::try_from(data) {
                    Ok(data) => Ok((data, respond)),
                    Err(_) => Err(()),
                })
            })
            .forward(req_send),
    );

    (ConductorSideSignalSender::priv_new(sig_send), req_recv)
}

#[async_trait]
pub trait Interface {
    async fn spawn(self, api: ExternalConductorApi);
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn interface_sanity_test() {
        println!("yo");
    }
}
