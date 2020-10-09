#[cfg(test)]
mod tests {
    use crate::*;
    use futures::stream::StreamExt;
    use kitsune_p2p_types::transport::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_message() {
        let (listener1, _events1) =
            spawn_transport_listener_quic(url2!("kitsune-quic://127.0.0.1:0"), None)
                .await
                .unwrap();

        let bound1 = listener1.bound_url().await.unwrap();
        println!("listener1 bound to: {}", bound1);

        let (listener2, mut events2) =
            spawn_transport_listener_quic(url2!("kitsune-quic://127.0.0.1:0"), None)
                .await
                .unwrap();

        tokio::task::spawn(async move {
            while let Some((url, mut write, read)) = events2.next().await {
                println!("events2 incoming connection: {}", url,);
                let data = read.read_to_end().await;
                println!("message from {} : {}", url, String::from_utf8_lossy(&data),);
                let data = format!("echo: {}", String::from_utf8_lossy(&data)).into_bytes();
                write.write_and_close(data).await?;
            }
            TransportResult::Ok(())
        });

        let bound2 = listener2.bound_url().await.unwrap();
        println!("listener2 bound to: {}", bound2);

        let resp = listener1.request(bound2, b"hello".to_vec()).await.unwrap();

        println!("got resp: {}", String::from_utf8_lossy(&resp));

        assert_eq!("echo: hello", &String::from_utf8_lossy(&resp));
    }
}
