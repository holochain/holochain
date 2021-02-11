#![deny(missing_docs)]
//! Holochain utilities for websocket serving and connecting.
//!
//!  To establish an outgoing connection, use [`connect`]
//! which will return a tuple
//! ([`WebsocketSender`], [`WebsocketReceiver`])
//!
//! To open a listening socket, use [`WebsocketListener::bind`]
//! which will give you a [`WebsocketListener`]
//! which is an async Stream whose items resolve to that same tuple (
//! [`WebsocketSender`],
//! [`WebsocketReceiver`]
//! ).
//!
//! If you want to be able to shutdown the stream use [`WebsocketListener::bind_with_handle`]
//! which will give you a tuple ([`ListenerHandle`], [`ListenerStream`]).
//! You can use [`ListenerHandle::close`] to close immediately or
//! [`ListenerHandle::close_on`] to close on a future completing.
//!
//! # Example
//!
//! ```
//! use holochain_serialized_bytes::prelude::*;
//! use holochain_websocket::*;
//!
//! use std::convert::TryInto;
//! use tokio::stream::StreamExt;
//! use url2::prelude::*;
//!
//! #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
//! struct TestMessage(pub String);
//!
//! # async fn doc_test() {
//! // Create a new server listening for connections
//! let mut server = WebsocketListener::bind(
//!     url2!("ws://127.0.0.1:0"),
//!     std::sync::Arc::new(WebsocketConfig::default()),
//! )
//! .await
//! .unwrap();
//!
//! // Get the address of the server
//! let binding = server.local_addr().clone();
//!
//! tokio::task::spawn(async move {
//!     // Handle new connections
//!     while let Some(Ok((_send, mut recv))) = server.next().await {
//!         tokio::task::spawn(async move {
//!             // Receive a message and echo it back
//!             if let Some((msg, resp)) = recv.next().await {
//!                 // Deserialize the message
//!                 let msg: TestMessage = msg.try_into().unwrap();
//!                 // If this message is a request then we can respond
//!                 if resp.is_request() {
//!                     let msg = TestMessage(format!("echo: {}", msg.0));
//!                     resp.respond(msg.try_into().unwrap()).await.unwrap();
//!                 }
//!             }
//!         });
//!     }
//! });
//!
//! // Connect the client to the server
//! let (mut send, _recv) = connect(binding, std::sync::Arc::new(WebsocketConfig::default()))
//!     .await
//!     .unwrap();
//!
//! let msg = TestMessage("test".to_string());
//! // Make a request and get the echoed response
//! let rsp: TestMessage = send.request(msg).await.unwrap();
//!
//! assert_eq!("echo: test", &rsp.0,);
//! # }
//! # fn main() {
//! #     tokio::runtime::Builder::new()
//! #         .threaded_scheduler()
//! #         .enable_all()
//! #         .build()
//! #         .unwrap()
//! #         .block_on(doc_test());
//! # }
//! ```
//!

use std::io::Error;
use std::io::ErrorKind;
use std::sync::Arc;

use stream_cancel::Valve;
use tracing::instrument;
use url2::Url2;
use util::url_to_addr;
use websocket::Websocket;

mod websocket_config;
pub use websocket_config::*;

#[allow(missing_docs)]
mod error;
pub use error::*;

mod websocket_listener;
pub use websocket_listener::*;

mod websocket_sender;
pub use websocket_sender::*;

mod websocket_receiver;
pub use websocket_receiver::*;

#[instrument(skip(config))]
/// Create a new external websocket connection.
pub async fn connect(
    url: Url2,
    config: Arc<WebsocketConfig>,
) -> WebsocketResult<(WebsocketSender, WebsocketReceiver)> {
    let addr = url_to_addr(&url, config.scheme).await?;
    let socket = tokio::net::TcpStream::connect(addr).await?;
    socket.set_keepalive(Some(std::time::Duration::from_secs(
        config.tcp_keepalive_s as u64,
    )))?;
    let (socket, _) = tokio_tungstenite::client_async_with_config(
        url.as_str(),
        socket,
        Some(config.to_tungstenite()),
    )
    .await
    .map_err(|e| Error::new(ErrorKind::Other, e))?;
    tracing::debug!("Client connected");

    // Noop valve because we don't have a listener to shutdown the
    // ends when creating a client
    let (exit, valve) = Valve::new();
    exit.disable();
    Websocket::create_ends(config, socket, valve)
}

mod websocket;

mod util;
