use holochain_serialized_bytes::prelude::*;
use holochain_websocket::*;
use std::convert::TryInto;
use tokio_stream::StreamExt;
use url2::prelude::*;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct BroadcastMessage(pub String);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
struct ResponseMessage(pub String);

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let (mut send_socket, mut recv_socket) = connect(
        url2!("ws://127.0.0.1:12345"),
        std::sync::Arc::new(WebsocketConfig::default()),
    )
    .await
    .unwrap();

    let recv_handle = recv_socket.take_handle().unwrap();

    let rl = std::sync::Arc::new(linefeed::Interface::new("echo_client").unwrap());
    rl.set_report_signal(linefeed::terminal::Signal::Interrupt, true);
    rl.set_prompt("echo_client> ").unwrap();

    let rl_t = rl.clone();
    tokio::task::spawn(async move {
        while let Some((msg, resp)) = recv_socket.next().await {
            let msg: BroadcastMessage = msg.try_into().unwrap();
            writeln!(rl_t, "Received: {}", msg.0).unwrap();
            if resp.is_request() {
                writeln!(rl_t, "This client doesn't take requests: {:?}", msg).unwrap();
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
                    recv_handle.close();
                    eprintln!("\nShutting down...");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    break;
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
