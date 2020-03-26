//! This task actually sending messages on the SocketSink.

use crate::*;

/// internal message type for forwarding messages to the actual socket sink.
pub(crate) type ToSocketSink = (tungstenite::Message, tokio::sync::oneshot::Sender<()>);

/// internal ToSocketSink Sender
pub(crate) type ToSocketSinkSender = tokio::sync::mpsc::Sender<ToSocketSink>;

/// See module-level documentation for this internal task
pub(crate) fn build<S>(
    config: Arc<WebsocketConfig>,
    remote_addr: Url2,
    mut sink: S,
) -> ToSocketSinkSender
where
    S: 'static + std::marker::Unpin + futures::sink::Sink<tungstenite::Message> + Send,
    <S as futures::sink::Sink<tungstenite::Message>>::Error: std::fmt::Debug,
{
    let (send_sink, mut recv_sink) =
        tokio::sync::mpsc::channel::<ToSocketSink>(config.max_send_queue);
    tokio::task::spawn(async move {
        tracing::trace!(
            message = "starting socket sink task",
            %remote_addr,
        );
        use tokio::stream::StreamExt;
        while let Some((msg, send_complete)) = recv_sink.next().await {
            use futures::sink::SinkExt;
            if let Err(e) = sink.send(msg).await {
                tracing::error!(error = ?e);
                // end task
                break;
            }
            let _ = send_complete.send(());
        }
        tracing::info!(
            message = "socket sink task ended",
            %remote_addr,
        );
    });
    send_sink
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prep_test() -> (
        ToSocketSinkSender,
        futures::channel::mpsc::Receiver<tungstenite::Message>,
    ) {
        let (sink, stream) = futures::channel::mpsc::channel(1);

        (
            build(Arc::new(WebsocketConfig::default()), url2!("test://"), sink),
            stream,
        )
    }

    #[tokio::test]
    async fn test_task_socket_sink() {
        init_tracing();

        use tokio::stream::StreamExt;

        let (mut send, mut recv) = prep_test();
        let (os, or) = tokio::sync::oneshot::channel();
        send.send((tungstenite::Message::Text("test1".to_string()), os))
            .await
            .unwrap();

        or.await.unwrap();

        assert_eq!("test1", &recv.next().await.unwrap().into_text().unwrap());

        drop(send);

        assert_eq!(None, recv.next().await);
    }
}
