//! Holochain utilities for websocket serving and connecting

#![deny(missing_docs)]

use futures::future::{BoxFuture, FutureExt};
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use std::{
    convert::TryInto,
    io::{Error, ErrorKind, Result},
    net::SocketAddr,
    sync::Arc,
};
use url2::prelude::*;

/// See holochain_serialized_bytes::holochain_serial! macro.
/// This is similar, but makes use of std::io::Error for the error type.
#[macro_export]
macro_rules! try_from_serialized_bytes {
    ($s:ident) => {
        impl std::convert::TryFrom<$s> for SerializedBytes {
            type Error = Error;

            fn try_from(t: $s) -> Result<SerializedBytes> {
                ::holochain_serialized_bytes::to_vec_named(&t)
                    .map_err(|e| Error::new(ErrorKind::Other, e))
                    .map(|bytes| SerializedBytes::from(UnsafeBytes::from(bytes)))
            }
        }

        impl std::convert::TryFrom<SerializedBytes> for $s {
            type Error = Error;

            fn try_from(t: SerializedBytes) -> Result<$s> {
                ::holochain_serialized_bytes::from_read_ref(t.bytes())
                    .map_err(|e| Error::new(ErrorKind::Other, e))
            }
        }
    };
}

mod util;
use util::*;

mod websocket_config;
pub use websocket_config::*;

pub(crate) mod task_dispatch_incoming;
pub(crate) mod task_socket_sink;
pub(crate) mod task_socket_stream;

mod websocket_sender;
pub use websocket_sender::*;

mod websocket_receiver;
pub use websocket_receiver::*;

mod websocket_listener;
pub use websocket_listener::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn init_tracing() {
        sx_types::observability::test_run().unwrap();
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct TestMessage(pub String);
    try_from_serialized_bytes!(TestMessage);

    #[tokio::test]
    async fn sanity_test() {
        init_tracing();

        use tokio::stream::StreamExt;

        let mut server = websocket_bind(
            url2!("ws://127.0.0.1:0"),
            Arc::new(WebsocketConfig::default()),
        )
        .await
        .unwrap();
        let binding = server.local_addr().clone();
        tracing::info!(
            test = "got bound addr",
            %binding,
        );

        tokio::task::spawn(async move {
            while let Some(maybe_con) = server.next().await {
                tokio::task::spawn(async move {
                    let (_send, mut recv) = maybe_con.await.unwrap();
                    tracing::info!(
                        test = "incoming connection",
                        remote_addr = %recv.remote_addr(),
                    );

                    tokio::task::spawn(async move {
                        loop {
                            match recv.next().await {
                                Some(msg) => match msg {
                                    WebsocketMessage::Close(close) => {
                                        tracing::error!(error = ?close);
                                        break;
                                    }
                                    WebsocketMessage::Signal(data) => {
                                        let msg: TestMessage = data.try_into().unwrap();
                                        tracing::info!(
                                            test = "incoming signal",
                                            data = %msg.0,
                                        );
                                    }
                                    WebsocketMessage::Request(data, respond) => {
                                        let msg: TestMessage = data.try_into().unwrap();
                                        tracing::info!(
                                            test = "incoming message",
                                            data = %msg.0,
                                        );
                                        let msg = TestMessage(format!("echo: {}", msg.0));
                                        respond(msg.try_into().unwrap()).await.unwrap();
                                    }
                                },
                                None => break,
                            }
                        }
                        tracing::info!(test = "exit srv con loop");
                    });
                });
            }
            tracing::info!(test = "exit srv listen loop");
        });

        let (mut send, mut recv) = websocket_connect(binding, Arc::new(WebsocketConfig::default()))
            .await
            .unwrap();
        tracing::info!(
            test = "connection success",
            remote_addr = %recv.remote_addr(),
        );

        tokio::task::spawn(async move {
            // we need to process the recv side as well to make the socket work
            loop {
                match recv.next().await {
                    Some(msg) => {
                        if let WebsocketMessage::Close(close) = msg {
                            tracing::error!(error = ?close);
                            break;
                        }
                    }
                    None => break,
                }
            }
            tracing::info!(test = "exit cli con loop");
        });

        let msg = TestMessage("test-signal".to_string());
        send.signal(msg).await.unwrap();

        let msg = TestMessage("test-signal2".to_string());
        send.signal(msg).await.unwrap();

        let msg = TestMessage("test".to_string());
        let rsp: TestMessage = send.request(msg).await.unwrap();

        tracing::info!(
            test = "got response",
            data = %rsp.0,
        );

        send.close(1000, "test".to_string()).await.unwrap();

        tokio::time::delay_for(std::time::Duration::from_millis(20)).await;
    }
}
