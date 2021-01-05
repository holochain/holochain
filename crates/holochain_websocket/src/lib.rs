#![allow(clippy::needless_doctest_main)]
//! Holochain utilities for websocket serving and connecting.
//!
//! To establish an outgoing connection, use [websocket_connect](fn.websocket_connect.html)
//! which will return a tuple (
//! [WebsocketSender](struct.WebsocketSender.html),
//! [WebsocketReceiver](struct.WebsocketReceiver.html)
//! ).
//!
//! To open a listening socket, use [websocket_bind](fn.websocket_bind.html)
//! which will give you a [WebsocketListener](struct.WebsocketListener.html)
//! which is an async Stream whose items resolve to that same tuple (
//! [WebsocketSender](struct.WebsocketSender.html),
//! [WebsocketReceiver](struct.WebsocketReceiver.html)
//! ).
//!
//! # Example
//!
//! ```
//! # async fn doc_test() {
//! #
//! use holochain_websocket::*;
//!
//! use url2::prelude::*;
//! use tokio::stream::StreamExt;
//! use std::convert::TryInto;
//!
//! #[derive(serde::Serialize, serde::Deserialize, Debug)]
//! struct TestMessage(pub String);
//! holochain_websocket::try_from_serialized_bytes!(TestMessage);
//!
//! let mut server = websocket_bind(
//!     url2!("ws://127.0.0.1:0"),
//!     std::sync::Arc::new(WebsocketConfig::default()),
//! )
//! .await
//! .unwrap();
//!
//! let binding = server.local_addr().clone();
//!
//! tokio::task::spawn(async move {
//!     while let Some(maybe_con) = server.next().await {
//!         let (_send, mut recv) = maybe_con.unwrap();
//!
//!         tokio::task::spawn(async move {
//!             if let Some(msg) = recv.next().await {
//!                 if let WebsocketMessage::Request(data, respond) = msg {
//!                     let msg: TestMessage = data.try_into().unwrap();
//!                     let msg = TestMessage(
//!                         format!("echo: {}", msg.0),
//!                     );
//!                     respond(msg.try_into().unwrap()).await.unwrap();
//!                 }
//!             }
//!         });
//!     }
//! });
//!
//! let (mut send, _recv) = websocket_connect(
//!     binding,
//!     std::sync::Arc::new(WebsocketConfig::default()),
//! )
//! .await
//! .unwrap();
//!
//! let msg = TestMessage("test".to_string());
//! let rsp: TestMessage = send.request(msg).await.unwrap();
//!
//! assert_eq!(
//!     "echo: test",
//!     &rsp.0,
//! );
//! #
//! # }
//! #
//! # fn main() {
//! #     tokio::runtime::Builder::new()
//! #         .threaded_scheduler()
//! #         .enable_all()
//! #         .build()
//! #         .unwrap()
//! #         .block_on(doc_test());
//! # }
//! ```

#![deny(missing_docs)]

use futures::future::BoxFuture;
use futures::future::FutureExt;
use holochain_serialized_bytes::SerializedBytes;
use holochain_serialized_bytes::UnsafeBytes;
use std::convert::TryInto;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::net::SocketAddr;
use std::sync::Arc;
use url2::prelude::*;

// helper utilities and macros
mod util;
use util::*;

// config for new listeners and connections
mod websocket_config;
pub use websocket_config::*;

// handles dispatching messages between the sender/receiver/sink/stream
pub(crate) mod task_dispatch_incoming;

// handles sending outgoing messages
pub(crate) mod task_socket_sink;

// handles receiving incoming messages
pub(crate) mod task_socket_stream;

// the sender/write half of the split websocket
mod websocket_sender;
pub use websocket_sender::*;

// the receiver/read half of the split websocket
mod websocket_receiver;
pub use websocket_receiver::*;

// the listener / server socket
mod websocket_listener;
pub use websocket_listener::*;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::stream::StreamExt;

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct TestMessage(pub String);
    crate::try_from_serialized_bytes!(TestMessage);

    #[tokio::test]
    async fn sanity_test() {
        observability::test_run().ok();
        let mut server = websocket_bind(
            url2!("ws://127.0.0.1:0"),
            Arc::new(WebsocketConfig::default()),
        )
        .await
        .unwrap();

        let binding = server.local_addr().clone();

        tokio::task::spawn(async move {
            while let Some(maybe_con) = server.next().await {
                let (_send, mut recv) = maybe_con.unwrap();

                tokio::task::spawn(async move {
                    if let Some(msg) = recv.next().await {
                        if let WebsocketMessage::Request(data, respond) = msg {
                            let msg: TestMessage = data.try_into().unwrap();
                            let msg = TestMessage(format!("echo: {}", msg.0));
                            respond(msg.try_into().unwrap()).await.unwrap();
                        }
                    }
                });
            }
        });

        let (mut send, _recv) = websocket_connect(binding, Arc::new(WebsocketConfig::default()))
            .await
            .unwrap();

        let msg = TestMessage("test".to_string());
        let rsp: TestMessage = send.request(msg).await.unwrap();

        assert_eq!("echo: test", &rsp.0,);
    }
}
