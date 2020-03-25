use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_websocket::*;
use std::{
    convert::TryInto,
    io::{Error, ErrorKind, Result},
};
use tokio::stream::StreamExt;
use url2::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct BroadcastMessage(pub String);

impl std::convert::TryFrom<BroadcastMessage> for SerializedBytes {
    type Error = Error;

    fn try_from(t: BroadcastMessage) -> Result<SerializedBytes> {
        holochain_serialized_bytes::to_vec_named(&t)
            .map_err(|e| Error::new(ErrorKind::Other, e))
            .map(|bytes| SerializedBytes::from(UnsafeBytes::from(bytes)))
    }
}

impl std::convert::TryFrom<SerializedBytes> for BroadcastMessage {
    type Error = Error;

    fn try_from(t: SerializedBytes) -> Result<BroadcastMessage> {
        holochain_serialized_bytes::from_read_ref(t.bytes())
            .map_err(|e| Error::new(ErrorKind::Other, e))
    }
}

#[tokio::main(threaded_scheduler)]
async fn main() {
    let (mut send_socket, mut recv_socket) = websocket_connect(
        url2!("ws://127.0.0.1:12345"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    let rl = std::sync::Arc::new(linefeed::Interface::new("echo_client").unwrap());
    rl.set_report_signal(linefeed::terminal::Signal::Interrupt, true);
    rl.set_prompt("echo_client> ").unwrap();

    let rl_t = rl.clone();
    tokio::task::spawn(async move {
        while let Some(msg) = recv_socket.next().await {
            match msg {
                WebsocketMessage::Signal(msg) => {
                    let msg: BroadcastMessage = msg.try_into().unwrap();
                    writeln!(rl_t, "Received: {}", msg.0).unwrap();
                }
                msg => {
                    writeln!(rl_t, "Error: {:?}", msg).unwrap();
                }
            }
        }
    });

    loop {
        let res = rl.read_line_step(Some(std::time::Duration::from_millis(100)));
        match res {
            Ok(Some(line)) => match line {
                linefeed::reader::ReadResult::Input(s) => {
                    send_socket.signal(BroadcastMessage(s)).await.unwrap();
                }
                linefeed::reader::ReadResult::Eof => {
                    eprintln!("\nEof");
                    break;
                }
                linefeed::reader::ReadResult::Signal(s) => {
                    eprintln!("\nSignal: {:?}", s);
                }
            },
            Err(e) => {
                eprintln!("{:?}", e);
                break;
            }
            Ok(None) => {}
        }
    }
}

/*


#[tokio::main(threaded_scheduler)]
async fn main() {
    let mut listener = websocket_bind(
        url2!("ws://127.0.0.1:0"),
        std::sync::Arc::new(WebsocketConfig::default()),
    ).await.unwrap();
    println!("LISTENING AT: {}", listener.local_addr());

    let (send_b, _) = tokio::sync::broadcast::channel(10);

    while let Some(maybe_con) = listener.next().await {
        let loc_send_b = send_b.clone();
        let mut loc_recv_b = send_b.subscribe();

        tokio::task::spawn(async move {
            let (mut send_socket, mut recv_socket) = maybe_con.await.unwrap();

            tokio::task::spawn(async move {
                while let Some(msg) = recv_socket.next().await {
                    match msg {
                        WebsocketMessage::Signal(msg) => {
                            let msg: BroadcastMessage = msg.try_into().unwrap();
                            loc_send_b.send(msg).unwrap();
                        }
                        msg => {
                            eprintln!("ERROR: {:?}", msg);
                            break;
                        }
                    }
                }
            });

            tokio::task::spawn(async move {
                while let Some(Ok(msg)) = loc_recv_b.next().await {
                    send_socket.signal(msg).await.unwrap();
                }
            });
        });
    }
}
*/
