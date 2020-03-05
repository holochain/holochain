//! Tokio mpsc channel supporting responding to requests.
//!
//! # Example
//!
//! ```rust
//! # use rpc_channel::*;
//! #
//! # pub async fn async_main() {
//! #
//! let (mut send, mut recv) = rpc_channel::<String, String>(10);
//!
//! tokio::task::spawn(async move {
//!     let (data, respond, span) = recv.recv().await.unwrap();
//!     let _g = span.enter();
//!     let _ = respond(Ok(format!("{} world", data)));
//! });
//!
//! let res = send.request("hello".to_string()).await.unwrap();
//! assert_eq!("hello world", &res);
//! #
//! # }
//! #
//! # pub fn main () {
//! #     tokio::runtime::Runtime::new().unwrap().block_on(async_main());
//! # }
//! ```

use thiserror::Error;

/// RpcChannel error type.
#[derive(Error, Debug)]
pub enum RpcChannelError {
    /// The other end of this channel has been dropped.
    /// No more communication will be possible.
    #[error("channel closed")]
    ChannelClosed,

    /// The handler end dropped the response channel,
    /// you will not receive a response to this request.
    #[error("response channel closed")]
    ResponseChannelClosed,

    /// An unspecified internal error occurred.
    #[error("{0}")]
    Other(String),
}

impl<R: AsRef<str>> From<R> for RpcChannelError {
    fn from(r: R) -> Self {
        RpcChannelError::Other(r.as_ref().to_string())
    }
}

/// RpcChannel result type.
pub type Result<T> = ::std::result::Result<T, RpcChannelError>;

/// Internal rpc channel type
type RpcChannelType<I, O> = (
    I,
    tokio::sync::oneshot::Sender<(Result<O>, tracing::Span)>,
    tracing::Span,
);

/// The "sender" side of an rpc_channel.
pub struct RpcChannelSender<I: 'static + Send, O: 'static + Send> {
    sender: tokio::sync::mpsc::Sender<RpcChannelType<I, O>>,
}

// not sure why derive(Clone) doesn't work here
impl<I: 'static + Send, O: 'static + Send> Clone for RpcChannelSender<I, O> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<I: 'static + Send, O: 'static + Send> RpcChannelSender<I, O> {
    /// The request function on an RpcChannelSender.
    pub async fn request(&mut self, data: I) -> Result<O> {
        let req_span = tracing::trace_span!("request");
        let (one_send, one_recv) = tokio::sync::oneshot::channel();
        self.sender
            .send((data, one_send, req_span))
            .await
            .map_err(|_| RpcChannelError::ChannelClosed)?;
        let (resp, recv_span) = one_recv
            .await
            .map_err(|_| RpcChannelError::ResponseChannelClosed)?;
        let _g = recv_span.enter();
        tracing::trace!("respond complete");
        resp
    }
}

/// Handler callback for servicing RpcChannelReceiver "recv" operations.
pub type RpcChannelResponder<O> = Box<dyn FnOnce(Result<O>) -> Result<()> + 'static + Send>;

/// The "receiver" side of an rpc_channel.
pub struct RpcChannelReceiver<I: 'static + Send, O: 'static + Send> {
    receiver: tokio::sync::mpsc::Receiver<RpcChannelType<I, O>>,
}

impl<I: 'static + Send, O: 'static + Send> RpcChannelReceiver<I, O> {
    /// Handle any incoming messages by invoking the "RpcChannelResponder" callback.
    /// Will return an error if the channel is broken.
    pub async fn recv(&mut self) -> Result<(I, RpcChannelResponder<O>, tracing::Span)> {
        let (data, respond, recv_span) = match self.receiver.recv().await {
            None => Err(RpcChannelError::ChannelClosed),
            Some(r) => Ok(r),
        }?;
        let out: RpcChannelResponder<O> = Box::new(|o| {
            let span = tracing::trace_span!("respond");
            respond
                .send((o, span))
                .map_err(|_| RpcChannelError::ResponseChannelClosed)?;
            Ok(())
        });
        Ok((data, out, recv_span))
    }
}

/// Create a new rpc_channel with given backlog buffer size.
///
/// See the module-level documentation for example usage.
pub fn rpc_channel<I: 'static + Send, O: 'static + Send>(
    channel_size: usize,
) -> (RpcChannelSender<I, O>, RpcChannelReceiver<I, O>) {
    let (sender, receiver) = tokio::sync::mpsc::channel(channel_size);
    (RpcChannelSender { sender }, RpcChannelReceiver { receiver })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rpc_channel_send_recv() {
        #[derive(Debug)]
        enum Msg {
            Request(String),
            Response(String),
        };

        use ::futures::future::{BoxFuture, FutureExt};

        trait MsgRpcSendExt {
            fn msg_request<'a>(&'a mut self, msg: &'a str) -> BoxFuture<'a, Result<String>>;
        }

        impl MsgRpcSendExt for RpcChannelSender<Msg, Msg> {
            fn msg_request<'a>(&'a mut self, msg: &'a str) -> BoxFuture<'a, Result<String>> {
                async move {
                    let res = self.request(Msg::Request(msg.to_string())).await?;
                    if let Msg::Response(res) = res {
                        Ok(res)
                    } else {
                        Err("invalid response type".into())
                    }
                }
                .boxed()
            }
        }

        trait MsgRpcRecvHandler: 'static + Send {
            fn handle_msg_request(&mut self, msg: &str) -> Result<String>;
        }

        fn spawn_msg_handler_task<H: MsgRpcRecvHandler>(
            mut receiver: RpcChannelReceiver<Msg, Msg>,
            mut handler: H,
        ) {
            tokio::task::spawn(async move {
                while let Ok((data, respond, span)) = receiver.recv().await {
                    let _g = span.enter();
                    match data {
                        Msg::Request(data) => {
                            let res = handler.handle_msg_request(&data);
                            let _ = match res {
                                Ok(s) => respond(Ok(Msg::Response(s))),
                                Err(e) => respond(Err(e)),
                            };
                        }
                        m @ _ => panic!("invalid data: {:?}", m),
                    }
                }
            });
        }

        let (mut send, recv) = rpc_channel::<Msg, Msg>(10);

        struct TestHandler;
        impl MsgRpcRecvHandler for TestHandler {
            fn handle_msg_request(&mut self, msg: &str) -> Result<String> {
                Ok(format!("{} world", msg))
            }
        }
        spawn_msg_handler_task(recv, TestHandler);

        let res = send.msg_request("hello").await.unwrap();
        assert_eq!("hello world", &res);
    }
}
