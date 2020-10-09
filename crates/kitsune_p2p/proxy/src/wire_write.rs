use crate::*;
use futures::{sink::SinkExt, stream::StreamExt};

/// Wrap a TransportChannelWrite in a sender that takes/encodes ProxyWire items.
pub(crate) fn wrap_wire_write(
    mut write: TransportChannelWrite,
) -> futures::channel::mpsc::Sender<ProxyWire> {
    let (send, mut recv) = futures::channel::mpsc::channel::<ProxyWire>(10);

    tokio::task::spawn(async move {
        while let Some(wire) = recv.next().await {
            let wire = wire.encode()?;
            write.send(wire).await?;
        }
        write.close().await?;
        TransportResult::Ok(())
    });

    send
}
