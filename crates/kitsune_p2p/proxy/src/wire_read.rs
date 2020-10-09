use crate::*;
use futures::{sink::SinkExt, stream::StreamExt};

/// Wrap a TransportChannelRead in code that decodes ProxyWire items.
pub(crate) fn wrap_wire_read(
    mut read: TransportChannelRead,
) -> futures::channel::mpsc::Receiver<ProxyWire> {
    let (mut send, recv) = futures::channel::mpsc::channel(10);

    tokio::task::spawn(async move {
        let mut buf = Vec::new();
        while let Some(data) = read.next().await {
            buf.extend_from_slice(&data);
            if let Ok((read_size, wire)) = ProxyWire::decode(&buf) {
                buf.drain(..read_size);
                send.send(wire).await.map_err(TransportError::other)?;
            }
        }
        TransportResult::Ok(())
    });

    recv
}
