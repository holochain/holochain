use crate::*;
use futures::future::FutureExt;
use std::sync::Arc;

/// internal send commands to transport pool task
enum TPoolCommand {
    Custom(BoxAny),
    Shutdown,
    RegisterListener(ListenerSender),
    OutgoingRequest(ConnectionUrl, Vec<u8>),
}

/// internal receive responses from transport pool task
enum TPoolResponse {
    Custom(FutureResult<BoxAny>),
    Shutdown(FutureResult<()>),
    RegisterListener(FutureResult<()>),
    OutgoingRequest(FutureResult<Vec<u8>>),
}

/// A handle to a transport pool task. Use this to send requests
/// utilizing a pool of listeners / connections.
#[derive(Clone)]
pub struct TransportPoolSender {
    sender: rpc_channel::RpcChannelSender<TPoolCommand, TPoolResponse>,
}

impl TransportPoolSender {
    /// Send a custom command to the transport pool task.
    /// See the documentation for the specific pool type you are messaging.
    pub async fn custom(&mut self, any: BoxAny) -> Result<BoxAny> {
        let res = self.sender.request(TPoolCommand::Custom(any)).await?;
        if let TPoolResponse::Custom(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Shutdown the bound endpoint. Expect that the next message will result in
    /// a disconnected channel error.
    pub async fn shutdown(&mut self) -> Result<()> {
        let res = self.sender.request(TPoolCommand::Shutdown).await?;
        if let TPoolResponse::Shutdown(res) = res {
            res.await?;
            Ok(())
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Register a new low-level transport listener to this connection pool.
    pub async fn register_lisener(&mut self, listener: ListenerSender) -> Result<()> {
        let res = self
            .sender
            .request(TPoolCommand::RegisterListener(listener))
            .await?;
        if let TPoolResponse::RegisterListener(res) = res {
            res.await?;
            Ok(())
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Make a request of the remote endpoint, allowing awaiting the response.
    pub async fn outgoing_request(&mut self, url: ConnectionUrl, data: Vec<u8>) -> Result<Vec<u8>> {
        let res = self
            .sender
            .request(TPoolCommand::OutgoingRequest(url, data))
            .await?;
        if let TPoolResponse::OutgoingRequest(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }
}

/// Implement this to provide a type of TransportPool task.
pub trait TransportPoolHandler: 'static + Send {
    /// Re-implement this if you want to handle custom messages,
    /// otherwise, you can leave this provided no-op.
    #[must_use]
    fn handle_custom(&mut self, _any: BoxAny) -> FutureResult<BoxAny> {
        let out: BoxAny = Box::new(());
        async move { Ok(out) }.boxed()
    }

    /// Shut down this transport pool task. Note, the future you return here
    /// will be driven to completion, but no other handlers will be invoked.
    #[must_use]
    fn handle_shutdown(&mut self) -> FutureResult<()>;

    /// Handle a request to register a new low-level transport listener.
    #[must_use]
    fn handle_register_listener(&mut self, listener: ListenerSender) -> FutureResult<()>;

    /// Forward the request data to the remote end, and await a response.
    #[must_use]
    fn handle_outgoing_request(
        &mut self,
        url: ConnectionUrl,
        data: Vec<u8>,
    ) -> FutureResult<Vec<u8>>;
}

/// TransportPool tracks connections by the remote url + unique id.
/// The unique id allows multiple connections to the same remote.
pub type ConnectionUrl = Arc<Url2>;

/// Events / Notifications / and Incoming Requests from the TransportPool.
pub enum TransportPoolEvent {
    /// A remote peer has established a connection with us.
    IncomingConnectionOpened { url: ConnectionUrl },

    /// A connection has closed.
    ConnectionClosed { url: ConnectionUrl },

    /// Incoming request from a remote peer.
    IncomingRequest {
        url: ConnectionUrl,
        data: Vec<u8>,
        respond: IncomingRequestResponder,
    },
}

/// Listeners can accept incoming connections. Your SpawnTransportPool callback
/// will be supplied with the sender portion of this channel.
pub type TransportPoolEventSender = tokio::sync::mpsc::Sender<TransportPoolEvent>;

/// Listeners can accept incoming connections. spawn_transport_pool will return
/// the receive portion of this channel.
pub type TransportPoolEventReceiver = tokio::sync::mpsc::Receiver<TransportPoolEvent>;

/// Create an actual transport pool task, returning the Sender reference that allows
/// controlling this task.
/// Note, as a user you probably don't want this function.
/// You probably want a spawn function for a specific type of connection.
pub async fn spawn_transport_pool<H, F>(
    channel_size: usize,
    constructor: F,
) -> Result<(TransportPoolSender, TransportPoolEventReceiver)>
where
    H: TransportPoolHandler,
    F: FnOnce(TransportPoolSender, TransportPoolEventSender) -> FutureResult<H> + 'static + Send,
{
    let (event_sender, event_receiver) = tokio::sync::mpsc::channel(channel_size);
    let (sender, mut receiver) =
        rpc_channel::rpc_channel::<TPoolCommand, TPoolResponse>(channel_size);

    let sender = TransportPoolSender { sender };

    let mut handler = constructor(sender.clone(), event_sender).await?;

    tokio::task::spawn(async move {
        while let Ok((data, respond, span)) = receiver.recv().await {
            let _g = span.enter();
            match data {
                TPoolCommand::Custom(any) => {
                    let res = handler.handle_custom(any);
                    let _ = respond(Ok(TPoolResponse::Custom(res)));
                }
                TPoolCommand::Shutdown => {
                    let res = handler.handle_shutdown();
                    let _ = respond(Ok(TPoolResponse::Shutdown(res)));

                    // don't process any further messages
                    return;
                }
                TPoolCommand::RegisterListener(listener) => {
                    let res = handler.handle_register_listener(listener);
                    let _ = respond(Ok(TPoolResponse::RegisterListener(res)));
                }
                TPoolCommand::OutgoingRequest(url, data) => {
                    let res = handler.handle_outgoing_request(url, data);
                    let _ = respond(Ok(TPoolResponse::OutgoingRequest(res)));
                }
            }
        }
    });

    Ok((sender, event_receiver))
}

#[cfg(test)]
mod tests {
    //use super::*;

    #[tokio::test]
    async fn test_transport_pool_api() {}
}
