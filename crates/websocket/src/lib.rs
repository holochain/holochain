//! Holochain utilities for websocket serving and connecting

#![deny(missing_docs)]

use futures::{sink::Sink, stream::Stream};
use std::{
    io::{Error, ErrorKind, Result},
    net::SocketAddr,
};
use tokio::net::ToSocketAddrs;
use url2::prelude::*;

mod util;
use util::*;

mod websocket_sender;
pub use websocket_sender::*;

mod websocket_receiver;
pub use websocket_receiver::*;

mod websocket_listener;
pub use websocket_listener::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sanity_test() {
        use tokio::stream::StreamExt;

        let mut server = websocket_bind("127.0.0.1:0").await.unwrap();
        let binding = server.local_addr().clone();
        println!("got bound addr: {}", binding);

        tokio::task::spawn(async move {
            while let Some(maybe_con) = server.next().await {
                tokio::task::spawn(async move {
                    let (mut send, mut recv) = maybe_con.await.unwrap();
                    println!("got incoming connection: {}", recv.remote_addr());

                    tokio::task::spawn(async move {
                        while let Some(Ok(msg)) = recv.next().await {
                            let msg = msg.into_text().unwrap();
                            println!("got incoming message: {}", msg);
                            let msg = tungstenite::Message::Text(format!("echo: {}", msg));
                            send.send(msg).await.unwrap();
                        }
                    });
                });
            }
        });

        let (mut send, mut recv) = websocket_connect(binding).await.unwrap();
        println!("got remote addr: {}", recv.remote_addr());

        send.send(tungstenite::Message::Text("test".to_string()))
            .await
            .unwrap();

        let response = recv.next().await;
        println!("got response: {:?}", response);
    }
}
