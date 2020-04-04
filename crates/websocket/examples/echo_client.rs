use holochain_websocket::*;
use std::convert::TryInto;
use tokio::stream::StreamExt;
use url2::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct BroadcastMessage(pub String);
try_from_serialized_bytes!(BroadcastMessage);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct ResponseMessage(pub String);
try_from_serialized_bytes!(ResponseMessage);

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
                    if s.starts_with("req ") {
                        let mut s = s.splitn(2, ' ');
                        let resp: ResponseMessage = send_socket
                            .request(BroadcastMessage(s.nth(1).unwrap().to_string()))
                            .await
                            .unwrap();
                        writeln!(rl, "Request response: {}", resp.0).unwrap();
                    } else {
                        send_socket.signal(BroadcastMessage(s)).await.unwrap();
                    }
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
