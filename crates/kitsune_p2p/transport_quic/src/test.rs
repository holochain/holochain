#[cfg(test)]
mod tests {
    use crate::*;
    use futures::{future::FutureExt, stream::StreamExt};
    use kitsune_p2p_types::{transport::transport_connection::*, transport::transport_listener::*};

    #[tokio::test(threaded_scheduler)]
    async fn test_message() {
        let (listener1, _events1) =
            spawn_transport_listener_quic(url2!("kitsune-quic://127.0.0.1:0"))
                .await
                .unwrap();

        let bound1 = listener1.bound_url().await.unwrap();
        println!("listener1 bound to: {}", bound1);

        let (listener2, mut events2) =
            spawn_transport_listener_quic(url2!("kitsune-quic://127.0.0.1:0"))
                .await
                .unwrap();

        tokio::task::spawn(async move {
            while let Some(evt) = events2.next().await {
                match evt {
                    TransportListenerEvent::IncomingConnection {
                        respond,
                        sender: con,
                        receiver: mut evt,
                        ..
                    } => {
                        respond.respond(Ok(async move { Ok(()) }.boxed().into()));
                        println!(
                            "events2 incoming connection: {}",
                            con.remote_url().await.unwrap(),
                        );
                        while let Some(evt) = evt.next().await {
                            match evt {
                                TransportConnectionEvent::IncomingRequest {
                                    respond,
                                    url,
                                    data,
                                    ..
                                } => {
                                    println!(
                                        "message from {} : {}",
                                        url,
                                        String::from_utf8_lossy(&data),
                                    );
                                    let out = format!("echo: {}", String::from_utf8_lossy(&data),)
                                        .into_bytes();
                                    respond.respond(Ok(async move { Ok(out) }.boxed().into()));
                                }
                            }
                        }
                    }
                }
            }
        });

        let bound2 = listener2.bound_url().await.unwrap();
        println!("listener2 bound to: {}", bound2);

        let (con1, _evt_con_1) = listener1.connect(bound2).await.unwrap();

        println!(
            "listener1 opened connection to 2 - remote_url: {}",
            con1.remote_url().await.unwrap()
        );

        let resp = con1.request(b"hello".to_vec()).await.unwrap();

        println!("got resp: {}", String::from_utf8_lossy(&resp));

        assert_eq!("echo: hello", &String::from_utf8_lossy(&resp));
    }
}
