//! This task actually sending messages on the SocketSink.

use crate::*;

/// internal message type for forwarding messages to the actual socket sink.
pub(crate) type ToSocketSink = (tungstenite::Message, tokio::sync::oneshot::Sender<()>);

/// internal ToSocketSink Sender
pub(crate) type ToSocketSinkSender = tokio::sync::mpsc::Sender<ToSocketSink>;

// /// internal ToSocketSink Receiver
// pub(crate) type ToSocketSinkReceiver = tokio::sync::mpsc::Receiver<ToSocketSink>;

/// See module-level documentation for this internal task
pub(crate) fn build(
    config: Arc<WebsocketConfig>,
    remote_addr: Url2,
    mut sink: RawSink,
) -> ToSocketSinkSender {
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
