use crate::*;
use futures::future::FutureExt;

/// internal send commands to connection task
enum ListenCommand {
    Custom(BoxAny),
    Shutdown,
    GetRemoteUrl,
    Request(Vec<u8>),
}

/// internal receive responses from connection task
enum ListenResponse {
    Custom(FutureResult<BoxAny>),
    Shutdown(FutureResult<()>),
    GetRemoteUrl(FutureResult<String>),
    Request(FutureResult<Vec<u8>>),
}

/// A handle to a connection task. Use this to control the connection / send requests.
#[derive(Clone)]
pub struct ListenerSender {
    sender: rpc_channel::RpcChannelSender<ListenCommand, ListenResponse>,
}

impl ListenerSender {
    /// Send a custom command to the connection task.
    /// See the documentation for the specific connection type you are messaging.
    pub async fn custom(&mut self, any: BoxAny) -> Result<BoxAny> {
        let res = self.sender.request(ListenCommand::Custom(any)).await?;
        if let ListenResponse::Custom(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Shutdown the connection. Expect that the next message will result in
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

    /// Get the remote url that this connection is pointing to.
    pub async fn get_remote_url(&mut self) -> Result<String> {
        let res = self.sender.request(ListenCommand::GetRemoteUrl).await?;
        if let ListenResponse::GetRemoteUrl(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }

    /// Make a request of the remote endpoint, allowing awaiting the response.
    pub async fn request(&mut self, data: Vec<u8>) -> Result<Vec<u8>> {
        let res = self.sender.request(ListenCommand::Request(data)).await?;
        if let ListenResponse::Request(res) = res {
            Ok(res.await?)
        } else {
            Err(TransportError::Other("invalid response type".into()))
        }
    }
}

/// Implement this to provide a type of Connection task.
pub trait ListenerHandler: 'static + Send {
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
    fn handle_request(&mut self, data: Vec<u8>) -> FutureResult<Vec<u8>>;
}

/// The handler constructor to be invoked from `spawn_listener`.
/// Will be supplied with a RpcChannelSender for this same task,
/// incase you need to set up custom messages, such as a timer tick, etc.
pub type SpawnListener<H> = Box<dyn FnOnce(ListenerSender) -> FutureResult<H> + 'static + Send>;

/// Create an actual connection task, returning the Sender reference that allows
/// controlling this task.
/// Note, as a user you probably don't want this function.
/// You probably want a spawn function for a specific type of connection.
pub async fn spawn_listener<H: ListenerHandler>(
    channel_size: usize,
    constructor: SpawnListener<H>,
) -> Result<ListenerSender> {
    let (sender, mut receiver) = rpc_channel::rpc_channel::<ListenCommand, ListenResponse>(channel_size);

    let sender = ListenerSender { sender };

    let mut handler = constructor(sender.clone()).await?;

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
                ListenCommand::GetRemoteUrl => {
                    let res = handler.handle_get_remote_url();
                    let _ = respond(Ok(ListenResponse::GetRemoteUrl(res)));
                }
                ListenCommand::Request(data) => {
                    let res = handler.handle_request(data);
                    let _ = respond(Ok(ListenResponse::Request(res)));
                }
            }
        }
    });

    Ok(sender)
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

            fn handle_get_remote_url(&mut self) -> FutureResult<String> {
                async move { Ok("test".to_string()) }.boxed()
            }

            fn handle_request(&mut self, data: Vec<u8>) -> FutureResult<Vec<u8>> {
                async move { Ok(data) }.boxed()
            }
        }
        let test_constructor: SpawnListener<Bob> = Box::new(|_| async move { Ok(Bob) }.boxed());
        let mut r = spawn_listener(10, test_constructor).await.unwrap();
        assert_eq!("test", r.get_remote_url().await.unwrap());
        assert_eq!(b"123".to_vec(), r.request(b"123".to_vec()).await.unwrap());
        r.custom(Box::new(()))
            .await
            .unwrap()
            .downcast::<()>()
            .unwrap();
        r.shutdown().await.unwrap();
    }
}
