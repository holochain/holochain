use crate::*;
use futures::future::FutureExt;

/// internal send commands to listener task
enum ListenCommand {
    Custom(BoxAny),
    Shutdown,
    GetBoundUrl,
    Connect(Url2),
}

/// internal receive responses from listener task
enum ListenResponse {
    Custom(FutureResult<BoxAny>),
    Shutdown(FutureResult<()>),
    GetBoundUrl(FutureResult<Url2>),
    Connect(FutureResult<(ConnectionSender, IncomingRequestReceiver)>),
}

/// A handle to a listener task. Use this to control the bound endpoint, and
/// receive or create connections.
#[derive(Clone)]
pub struct ListenerSender {
    sender: rpc_channel::RpcChannelSender<ListenCommand, ListenResponse>,

    // safe to clone as these should generally be small
    protocol: String,
}

impl ListenerSender {
    /// What protocol is this connection functioning over.
    /// i.e. "tcp", "wss", "holo-quic", ...
    pub fn get_protocol(&self) -> &str {
        &self.protocol
    }

    /// Send a custom command to the listener task.
    /// See the documentation for the specific listener type you are messaging.
    pub async fn custom(&mut self, any: BoxAny) -> Result<BoxAny> {
        let res = self.sender.request(ListenCommand::Custom(any)).await?;
        if let ListenResponse::Custom(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Shutdown the bound endpoint. Expect that the next message will result in
    /// a disconnected channel error.
    pub async fn shutdown(&mut self) -> Result<()> {
        let res = self.sender.request(ListenCommand::Shutdown).await?;
        if let ListenResponse::Shutdown(res) = res {
            res.await?;
            Ok(())
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Get the post-binding url this listener endpoint is attached to.
    pub async fn get_bound_url(&mut self) -> Result<Url2> {
        let res = self.sender.request(ListenCommand::GetBoundUrl).await?;
        if let ListenResponse::GetBoundUrl(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Attempt to establish an outgoing connection to a remote peer.
    pub async fn connect(
        &mut self,
        url: Url2,
    ) -> Result<(ConnectionSender, IncomingRequestReceiver)> {
        let res = self.sender.request(ListenCommand::Connect(url)).await?;
        if let ListenResponse::Connect(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }
}

/// Implement this to provide a type of Listener task.
pub trait ListenerHandler: 'static + Send {
    /// Re-implement this if you want to handle custom messages,
    /// otherwise, you can leave this provided no-op.
    #[must_use]
    fn handle_custom(&mut self, _any: BoxAny) -> FutureResult<BoxAny> {
        let out: BoxAny = Box::new(());
        async move { Ok(out) }.boxed()
    }

    /// Shut down this listener task. Note, the future you return here
    /// will be driven to completion, but no other handlers will be invoked.
    #[must_use]
    fn handle_shutdown(&mut self) -> FutureResult<()>;

    /// Return the url that this listener endpoint is bound to.
    #[must_use]
    fn handle_get_bound_url(&mut self) -> FutureResult<Url2>;

    /// Establish a new outgoing connection.
    #[must_use]
    fn handle_connect(
        &mut self,
        url: Url2,
    ) -> FutureResult<(ConnectionSender, IncomingRequestReceiver)>;
}

/// Listeners can accept incoming connections. Your SpawnListener callback
/// will be supplied with the sender portion of this channel.
pub type IncomingConnectionSender =
    tokio::sync::mpsc::Sender<(ConnectionSender, IncomingRequestReceiver)>;

/// Listeners can accept incoming connections. spawn_listener will return
/// the receive portion of this channel.
pub type IncomingConnectionReceiver =
    tokio::sync::mpsc::Receiver<(ConnectionSender, IncomingRequestReceiver)>;

/// The handler constructor to be invoked from `spawn_listener`.
/// Will be supplied with a RpcChannelSender for this same task,
/// incase you need to set up custom messages, such as a timer tick, etc.
pub type SpawnListener<H> =
    Box<dyn FnOnce(ListenerSender, IncomingConnectionSender) -> FutureResult<H> + 'static + Send>;

/// Create an actual listener task, returning the Sender reference that allows
/// controlling this task.
/// Note, as a user you probably don't want this function.
/// You probably want a spawn function for a specific type of connection.
pub async fn spawn_listener<H: ListenerHandler>(
    channel_size: usize,
    protocol: &str,
    constructor: SpawnListener<H>,
) -> Result<(ListenerSender, IncomingConnectionReceiver)> {
    let (incoming_sender, incoming_receiver) = tokio::sync::mpsc::channel(channel_size);
    let (sender, mut receiver) =
        rpc_channel::rpc_channel::<ListenCommand, ListenResponse>(channel_size);

    let sender = ListenerSender {
        sender,
        protocol: protocol.to_string(),
    };

    let mut handler = constructor(sender.clone(), incoming_sender).await?;

    tokio::task::spawn(async move {
        while let Ok((data, respond, span)) = receiver.recv().await {
            let _g = span.enter();
            match data {
                ListenCommand::Custom(any) => {
                    let res = handler.handle_custom(any);
                    let _ = respond(Ok(ListenResponse::Custom(res)));
                }
                ListenCommand::Shutdown => {
                    let res = handler.handle_shutdown();
                    let _ = respond(Ok(ListenResponse::Shutdown(res)));

                    // don't process any further messages
                    return;
                }
                ListenCommand::GetBoundUrl => {
                    let res = handler.handle_get_bound_url();
                    let _ = respond(Ok(ListenResponse::GetBoundUrl(res)));
                }
                ListenCommand::Connect(url) => {
                    let res = handler.handle_connect(url);
                    let _ = respond(Ok(ListenResponse::Connect(res)));
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
        impl ListenerHandler for Bob {
            fn handle_shutdown(&mut self) -> FutureResult<()> {
                async move { Ok(()) }.boxed()
            }

            fn handle_get_bound_url(&mut self) -> FutureResult<Url2> {
                async move { Ok(url2!("test://test/")) }.boxed()
            }

            fn handle_connect(
                &mut self,
                _url: Url2,
            ) -> FutureResult<(ConnectionSender, IncomingRequestReceiver)> {
                async move { Err(TransportError::Other("unimplemented".into())) }.boxed()
            }
        }
        let test_constructor: SpawnListener<Bob> = Box::new(|_, _| async move { Ok(Bob) }.boxed());
        let (mut r, _) = spawn_listener(10, "test", test_constructor).await.unwrap();
        assert_eq!("test", r.get_protocol());
        assert_eq!("test://test/", r.get_bound_url().await.unwrap().as_str());
        assert!(r.connect(url2!("test://test/")).await.is_err());
        r.custom(Box::new(()))
            .await
            .unwrap()
            .downcast::<()>()
            .unwrap();
        r.shutdown().await.unwrap();
    }
}
