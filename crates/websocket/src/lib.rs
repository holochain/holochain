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

mod util;
use util::*;

mod websocket_config;
pub use websocket_config::*;

mod websocket_sender;
pub use websocket_sender::*;

mod websocket_receiver;
pub use websocket_receiver::*;

mod websocket_listener;
pub use websocket_listener::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct TestMessage(pub String);

    impl std::convert::TryFrom<TestMessage> for SerializedBytes {
        type Error = Error;

        fn try_from(t: TestMessage) -> Result<SerializedBytes> {
            holochain_serialized_bytes::to_vec_named(&t)
                .map_err(|e| Error::new(ErrorKind::Other, e))
                .map(|bytes| SerializedBytes::from(UnsafeBytes::from(bytes)))
        }
    }

    impl std::convert::TryFrom<SerializedBytes> for TestMessage {
        type Error = Error;

        fn try_from(t: SerializedBytes) -> Result<TestMessage> {
            holochain_serialized_bytes::from_read_ref(t.bytes())
                .map_err(|e| Error::new(ErrorKind::Other, e))
        }
    }

    #[tokio::test]
    async fn sanity_test() {
        use tokio::stream::StreamExt;

        let mut server = websocket_bind(
            url2!("ws://127.0.0.1:0"),
            Arc::new(WebsocketConfig::default()),
        )
        .await
        .unwrap();
        let binding = server.local_addr().clone();
        println!("got bound addr: {}", binding);

        tokio::task::spawn(async move {
            while let Some(maybe_con) = server.next().await {
                tokio::task::spawn(async move {
                    let (_send, mut recv) = maybe_con.await.unwrap();
                    println!("got incoming connection: {}", recv.remote_addr());

                    tokio::task::spawn(async move {
                        while let Some(Ok(msg)) = recv.next().await {
                            match msg {
                                WebsocketMessage::Signal(data) => {
                                    let msg: TestMessage = data.try_into().unwrap();
                                    println!("got signal: {}", msg.0);
                                }
                                WebsocketMessage::Request(data, respond) => {
                                    let msg: TestMessage = data.try_into().unwrap();
                                    println!("got incoming message: {}", msg.0);
                                    let msg = TestMessage(format!("echo: {}", msg.0));
                                    respond(msg.try_into().unwrap()).await.unwrap();
                                }
                            }
                        }
                        println!("exit srv con loop");
                    });
                });
            }
            println!("exit listen loop");
        });

        let (mut send, mut recv) = websocket_connect(binding, Arc::new(WebsocketConfig::default()))
            .await
            .unwrap();
        println!("got remote addr: {}", recv.remote_addr());

        tokio::task::spawn(async move {
            // we need to process the recv side as well to make the socket work
            while let Some(_) = recv.next().await {}
            println!("exit cli con loop");
        });

        let msg = TestMessage("test-signal".to_string());
        send.signal(msg).await.unwrap();

        let msg = TestMessage("test-signal2".to_string());
        send.signal(msg).await.unwrap();

        let msg = TestMessage("test".to_string());
        let rsp: TestMessage = send.request(msg).await.unwrap();

        println!("got response: {:?}", rsp.0);
    }
}
