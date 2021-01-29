use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::codec::Codec;

/// Wrap a TransportChannelWrite in a sender that takes/encodes ProxyWire items.
pub(crate) fn wrap_wire_write(
    mut write: TransportChannelWrite,
) -> futures::channel::mpsc::Sender<ProxyWire> {
    let (send, mut recv) = futures::channel::mpsc::channel::<ProxyWire>(10);

    metric_task(async move {
        while let Some(wire) = recv.next().await {
            tracing::trace!("proxy write {:?}", wire);
            let wire = wire.encode_vec().map_err(TransportError::other)?;
            write.send(wire).await?;
        }
        write.close().await?;
        TransportResult::Ok(())
    });

    send
}
