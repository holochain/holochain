//! This task manages the incoming messages and events from the websocket stream.

use crate::*;
use task_dispatch_incoming::{ToDispatchIncoming, ToDispatchIncomingSender};
use task_socket_sink::ToSocketSinkSender;

/// See module-level documentation for this internal task
pub(crate) fn build(
    remote_addr: Url2,
    mut send_sink: ToSocketSinkSender,
    mut send_dispatch: ToDispatchIncomingSender,
    mut stream: RawStream,
) {
    tokio::task::spawn(async move {
        tracing::trace!(
            message = "starting socket stream task",
            %remote_addr,
        );
        loop {
            use tokio::stream::StreamExt;
            match stream.next().await {
                Some(Ok(incoming)) => {
                    match process_incoming_message(
                        &remote_addr,
                        &mut send_sink,
                        &mut send_dispatch,
                        incoming,
                    )
                    .await
                    {
                        Ok(should_continue) => {
                            if !should_continue {
                                // end this task
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = ?e);
                            let msg = ToDispatchIncoming::Close(WebsocketClosed {
                                code: 0,
                                reason: format!("Internal Error: {:?}", e),
                            });
                            let _ = send_dispatch.send(msg).await;
                            // end this task
                            break;
                        }
                    }
                }
                Some(Err(e)) => {
                    tracing::error!(error = ?e);
                    let msg = ToDispatchIncoming::Close(WebsocketClosed {
                        code: 0,
                        reason: format!("Internal Error: {:?}", e),
                    });
                    let _ = send_dispatch.send(msg).await;
                    // end this task
                    break;
                }
                None => {
                    // end this task
                    break;
                }
            }
        }
        tracing::info!(
            message = "socket stream task ended",
            %remote_addr,
        );
    });
}

#[allow(dead_code)]
async fn process_incoming_message(
    remote_addr: &Url2,
    send_sink: &mut ToSocketSinkSender,
    send_dispatch: &mut ToDispatchIncomingSender,
    incoming: tungstenite::Message,
) -> Result<bool> {
    match incoming {
        tungstenite::Message::Close(close) => {
            let (code, reason) = match close {
                Some(frame) => (frame.code.into(), frame.reason.into()),
                None => (0_u16, "".to_string()),
            };

            tracing::info!(
                message = "closing websocket",
                %remote_addr,
                code = %code,
                reason = %reason,
            );

            send_dispatch
                .send(ToDispatchIncoming::Close(WebsocketClosed { code, reason }))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;

            // end this task
            return Ok(false);
        }
        tungstenite::Message::Ping(data) => {
            let (send, recv) = tokio::sync::oneshot::channel();
            send_sink
                .send((tungstenite::Message::Pong(data), send))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
            recv.await.map_err(|e| Error::new(ErrorKind::Other, e))?;
        }
        tungstenite::Message::Pong(_) => (), // no-op
        incoming => {
            let bytes = incoming.into_data();
            let bytes: SerializedBytes = UnsafeBytes::from(bytes).into();
            send_dispatch
                .send(ToDispatchIncoming::IncomingBytes(bytes))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
        }
    }

    // task can continue
    Ok(true)
}
