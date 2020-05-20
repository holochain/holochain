#[cfg(test)]
mod tests {
    use crate::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_message() {
        let (mut listener1, _events1) =
            spawn_transport_listener_quic(url2!("kitsune-quic://127.0.0.1:0"))
                .await
                .unwrap();

        let bound1 = listener1.bound_url().await.unwrap();
        println!("listener1 bound to: {}", bound1);

        let (mut listener2, _events2) =
            spawn_transport_listener_quic(url2!("kitsune-quic://127.0.0.1:0"))
                .await
                .unwrap();

        let bound2 = listener2.bound_url().await.unwrap();
        println!("listener2 bound to: {}", bound2);

        let (mut con1, _evt_con_1) = listener1.connect(bound2).await.unwrap();

        println!(
            "listener1 opened connection to 2 - remote_url: {}",
            con1.remote_url().await.unwrap()
        );
    }
}
