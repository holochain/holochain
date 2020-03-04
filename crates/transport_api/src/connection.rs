use crate::*;
use futures::future::FutureExt;

/// internal send commands to connection task
enum ConCommand {
    Custom(BoxAny),
    Shutdown,
    GetRemoteUrl,
    OutgoingRequest(Vec<u8>),
}

/// internal receive responses from connection task
enum ConResponse {
    Custom(FutureResult<BoxAny>),
    Shutdown(FutureResult<()>),
    GetRemoteUrl(FutureResult<String>),
    OutgoingRequest(FutureResult<Vec<u8>>),
}

/// A handle to a connection task. Use this to control the connection / send requests.
#[derive(Clone)]
pub struct ConnectionSender {
    sender: rpc_channel::RpcChannelSender<ConCommand, ConResponse>,
}

impl ConnectionSender {
    /// Send a custom command to the connection task.
    /// See the documentation for the specific connection type you are messaging.
    pub async fn custom(&mut self, any: BoxAny) -> Result<BoxAny> {
        let res = self.sender.request(ConCommand::Custom(any)).await?;
        if let ConResponse::Custom(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Shutdown the connection. Expect that the next message will result in
    /// a disconnected channel error.
    pub async fn shutdown(&mut self) -> Result<()> {
        let res = self.sender.request(ConCommand::Shutdown).await?;
        if let ConResponse::Shutdown(res) = res {
            res.await?;
            Ok(())
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Get the remote url that this connection is pointing to.
    pub async fn get_remote_url(&mut self) -> Result<String> {
        let res = self.sender.request(ConCommand::GetRemoteUrl).await?;
        if let ConResponse::GetRemoteUrl(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Make a request of the remote endpoint, allowing awaiting the response.
    pub async fn outgoing_request(&mut self, data: Vec<u8>) -> Result<Vec<u8>> {
        let res = self
            .sender
            .request(ConCommand::OutgoingRequest(data))
            .await?;
        if let ConResponse::OutgoingRequest(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }
}

/// Implement this to provide a type of Connection task.
pub trait ConnectionHandler: 'static + Send {
    /// Re-implement this if you want to handle custom messages,
    /// otherwise, you can leave this provided no-op.
    #[must_use]
    fn handle_custom(&mut self, _any: BoxAny) -> FutureResult<BoxAny> {
        let out: BoxAny = Box::new(());
        async move { Ok(out) }.boxed()
    }

    /// Shut down this connection task. Note, the future you return here
    /// will be driven to completion, but no other handlers will be invoked.
    #[must_use]
    fn handle_shutdown(&mut self) -> FutureResult<()>;

    /// Return the remote url that this connection is pointing to.
    #[must_use]
    fn handle_get_remote_url(&mut self) -> FutureResult<String>;

    /// Forward the request data to the remote end, and await a response.
    #[must_use]
    fn handle_outgoing_request(&mut self, data: Vec<u8>) -> FutureResult<Vec<u8>>;
}

/// Handler callback for responding to incoming requests.
pub type IncomingRequestResponder = Box<dyn FnOnce(Result<Vec<u8>>) -> Result<()> + 'static + Send>;

/// Connections can accept incoming requests. Your SpawnConnection callback
/// will be supplied with the sender portion of this channel.
pub type IncomingRequestSender = tokio::sync::mpsc::Sender<(Vec<u8>, IncomingRequestResponder)>;

/// Connections can accept incoming requests. spawn_connection will return
/// the receive portion of this channel.
pub type IncomingRequestReceiver = tokio::sync::mpsc::Receiver<(Vec<u8>, IncomingRequestResponder)>;

/// The handler constructor to be invoked from `spawn_connection`.
/// Will be supplied with a RpcChannelSender for this same task,
/// incase you need to set up custom messages, such as a timer tick, etc.
pub type SpawnConnection<H> =
    Box<dyn FnOnce(ConnectionSender, IncomingRequestSender) -> FutureResult<H> + 'static + Send>;

/// Create an actual connection task, returning the Sender reference that allows
/// controlling this task.
/// Note, as a user you probably don't want this function.
/// You probably want a spawn function for a specific type of connection.
pub async fn spawn_connection<H: ConnectionHandler>(
    channel_size: usize,
    constructor: SpawnConnection<H>,
) -> Result<(ConnectionSender, IncomingRequestReceiver)> {
    let (incoming_sender, incoming_receiver) = tokio::sync::mpsc::channel(channel_size);
    let (sender, mut receiver) = rpc_channel::rpc_channel::<ConCommand, ConResponse>(channel_size);

    let sender = ConnectionSender { sender };

    let mut handler = constructor(sender.clone(), incoming_sender).await?;

    tokio::task::spawn(async move {
        while let Ok((data, respond, span)) = receiver.recv().await {
            let _g = span.enter();
            match data {
                ConCommand::Custom(any) => {
                    let res = handler.handle_custom(any);
                    let _ = respond(Ok(ConResponse::Custom(res)));
                }
                ConCommand::Shutdown => {
                    let res = handler.handle_shutdown();
                    let _ = respond(Ok(ConResponse::Shutdown(res)));

                    // don't process any further messages
                    return;
                }
                ConCommand::GetRemoteUrl => {
                    let res = handler.handle_get_remote_url();
                    let _ = respond(Ok(ConResponse::GetRemoteUrl(res)));
                }
                ConCommand::OutgoingRequest(data) => {
                    let res = handler.handle_outgoing_request(data);
                    let _ = respond(Ok(ConResponse::OutgoingRequest(res)));
                }
            }
        }
    });

    Ok((sender, incoming_receiver))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_api() {
        struct Bob;
        impl ConnectionHandler for Bob {
            fn handle_shutdown(&mut self) -> FutureResult<()> {
                async move { Ok(()) }.boxed()
            }

            fn handle_get_remote_url(&mut self) -> FutureResult<String> {
                async move { Ok("test".to_string()) }.boxed()
            }

            fn handle_outgoing_request(&mut self, data: Vec<u8>) -> FutureResult<Vec<u8>> {
                async move { Ok(data) }.boxed()
            }
        }
        let test_constructor: SpawnConnection<Bob> =
            Box::new(|_, _| async move { Ok(Bob) }.boxed());
        let (mut r, _) = spawn_connection(10, test_constructor).await.unwrap();
        assert_eq!("test", r.get_remote_url().await.unwrap());
        assert_eq!(
            b"123".to_vec(),
            r.outgoing_request(b"123".to_vec()).await.unwrap()
        );
        r.custom(Box::new(()))
            .await
            .unwrap()
            .downcast::<()>()
            .unwrap();
        r.shutdown().await.unwrap();
    }
}
