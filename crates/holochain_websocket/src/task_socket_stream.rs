//! This task manages the incoming messages and events from the websocket stream.

use super::task_socket_sink::ToSocketSinkSender;
use crate::*;
use task_dispatch_incoming::ToDispatchIncoming;
use task_dispatch_incoming::ToDispatchIncomingSender;

/// See module-level documentation for this internal task
pub(crate) fn build<S>(
    remote_addr: Url2,
    mut send_sink: ToSocketSinkSender,
    mut send_dispatch: ToDispatchIncomingSender,
    mut stream: S,
) where
    S: 'static
        + std::marker::Unpin
        + tokio::stream::Stream<Item = std::result::Result<tungstenite::Message, tungstenite::Error>>
        + Send,
{
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

/// internal process an individual incoming websocket message
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
        tungstenite::Message::Pong(_) => {} // no-op
        incoming => {
            let bytes = incoming.into_data();
            let bytes: SerializedBytes = UnsafeBytes::from(bytes).into();
            tracing::trace!(message = "incoming data", ?bytes,);
            send_dispatch
                .send(ToDispatchIncoming::IncomingBytes(bytes))
                .await
                .map_err(|e| Error::new(ErrorKind::Other, e))?;
        }
    }

    // task can continue
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::stream::StreamExt;

    struct Prep {
        recv_sink:
            tokio::sync::mpsc::Receiver<(tungstenite::Message, tokio::sync::oneshot::Sender<()>)>,
        recv_dispatch: tokio::sync::mpsc::Receiver<task_dispatch_incoming::ToDispatchIncoming>,
        send_stream: tokio::sync::mpsc::Sender<
            std::result::Result<tungstenite::Message, tungstenite::Error>,
        >,
    }

    fn prep_test() -> Prep {
        let (send_sink, recv_sink) = tokio::sync::mpsc::channel(1);
        let (send_dispatch, recv_dispatch) = tokio::sync::mpsc::channel(1);
        let (send_stream, recv_stream) = tokio::sync::mpsc::channel(1);

        build(url2!("test://"), send_sink, send_dispatch, recv_stream);

        Prep {
            recv_sink,
            recv_dispatch,
            send_stream,
        }
    }

    #[tokio::test]
    async fn test_task_socket_stream() {
        init_tracing();

        let Prep {
            mut recv_sink,
            mut recv_dispatch,
            mut send_stream,
        } = prep_test();

        send_stream
            .send(Ok(tungstenite::Message::Ping(b"test".to_vec())))
            .await
            .unwrap();

        let (msg, complete) = recv_sink.next().await.unwrap();
        complete.send(()).unwrap();
        assert_eq!("Pong([116, 101, 115, 116])", &format!("{:?}", msg));

        #[derive(serde::Serialize, serde::Deserialize, Debug)]
        struct Bob(String);
        try_from_serialized_bytes!(Bob);
        let msg = Bob("test".to_string());
        let msg: SerializedBytes = msg.try_into().unwrap();
        let msg: Vec<u8> = UnsafeBytes::from(msg).into();

        send_stream
            .send(Ok(tungstenite::Message::Binary(msg)))
            .await
            .unwrap();

        assert_eq!(
            "IncomingBytes(\"test\")",
            &format!("{:?}", recv_dispatch.next().await.unwrap()),
        );

        send_stream
            .send(Ok(tungstenite::Message::Close(Some(
                tungstenite::protocol::CloseFrame {
                    code: 42.into(),
                    reason: "test".to_string().into(),
                },
            ))))
            .await
            .unwrap();

        assert_eq!(
            "Close(WebsocketClosed { code: 42, reason: \"test\" })",
            &format!("{:?}", recv_dispatch.next().await.unwrap()),
        );

        assert_eq!("None", &format!("{:?}", recv_dispatch.next().await));

        assert_eq!("None", &format!("{:?}", recv_sink.next().await));
    }
}
