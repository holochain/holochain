use crate::*;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use ghost_actor::dependencies::tracing;
use kitsune_p2p_types::dependencies::spawn_pressure;
use rustls::Session;
use std::io::Read;
use std::io::Write;

const MAX_SERVERS: usize = 1000;

pub(crate) async fn spawn_tls_server(
    short: String,
    incoming_base_url: url2::Url2,
    tls_server_config: Arc<rustls::ServerConfig>,
    evt_send: TransportEventSender,
    write: futures::channel::mpsc::Sender<ProxyWire>,
    read: futures::channel::mpsc::Receiver<ProxyWire>,
) {
    metric_task(
        spawn_pressure::spawn_limit!(MAX_SERVERS),
        tls_server(
            short,
            incoming_base_url,
            tls_server_config,
            evt_send,
            write,
            read,
        ),
    )
    .await;
}

async fn tls_server(
    short: String,
    incoming_base_url: url2::Url2,
    tls_server_config: Arc<rustls::ServerConfig>,
    mut evt_send: TransportEventSender,
    mut write: futures::channel::mpsc::Sender<ProxyWire>,
    read: futures::channel::mpsc::Receiver<ProxyWire>,
) -> TransportResult<()> {
    let res: TransportResult<()> = async {
        let mut srv = rustls::ServerSession::new(&tls_server_config);
        let mut buf = [0_u8; 4096];
        let mut in_pre = std::io::Cursor::new(Vec::new());

        let ((mut send1, recv1), (send2, recv2)) = create_transport_channel_pair();
        let mut send2 = Some(send2);
        let mut recv2 = Some(recv2);

        let mut merge = kitsune_p2p_types::auto_stream_select(recv1, read);
        use kitsune_p2p_types::AutoStreamSelect::*;

        let mut wants_write_close = false;
        let mut did_post_handshake_work = false;
        loop {
            if !did_post_handshake_work && !srv.is_handshaking() {
                did_post_handshake_work = true;

                let cert_digest = blake2b_32(
                    srv.get_peer_certificates()
                        .ok_or_else(|| TransportError::from("tls_srv: No peer tls"))?
                        .get(0)
                        .ok_or_else(|| TransportError::from("tls_srv: No peer tls"))?
                        .as_ref(),
                );

                let remote_proxy_url =
                    ProxyUrl::new(incoming_base_url.as_str(), cert_digest.into())?;
                tracing::info!("{}: SRV: INCOMING TLS: {}", short, remote_proxy_url);

                evt_send
                    .send(TransportEvent::IncomingChannel(
                        remote_proxy_url.into(),
                        send2.take().unwrap(),
                        recv2.take().unwrap(),
                    ))
                    .await
                    .map_err(TransportError::other)?;
            }

            if srv.wants_write() {
                let mut data = Vec::new();
                srv.write_tls(&mut data).map_err(TransportError::other)?;
                tracing::trace!("{}: SRV tls wants write {} bytes", short, data.len());
                write
                    .send(ProxyWire::chan_send(data.into()))
                    .await
                    .map_err(TransportError::other)?;
            }

            if wants_write_close && !srv.is_handshaking() {
                tracing::trace!("{}: SRV closing outgoing", short);
                write.close().await.map_err(TransportError::other)?;
            }

            match merge.next().await {
                Some(Left(Some(data))) => {
                    tracing::trace!("{}: SRV outgoing {} bytes", short, data.len());
                    srv.write_all(&data).map_err(TransportError::other)?;
                }
                Some(Left(None)) => {
                    tracing::trace!("{}: SRV wants close outgoing", short);
                    wants_write_close = true;
                }
                Some(Right(Some(wire))) => match wire {
                    ProxyWire::ChanSend(data) => {
                        tracing::trace!(
                            "{}: SRV incoming encrypted {} bytes",
                            short,
                            data.channel_data.len()
                        );
                        in_pre.get_mut().clear();
                        in_pre.set_position(0);
                        in_pre.get_mut().extend_from_slice(&data.channel_data);
                        let in_buf_len = in_pre.get_ref().len();
                        loop {
                            if in_pre.position() >= in_buf_len as u64 {
                                break;
                            }

                            srv.read_tls(&mut in_pre).map_err(TransportError::other)?;
                            srv.process_new_packets().map_err(TransportError::other)?;
                            while let Ok(size) = srv.read(&mut buf) {
                                tracing::trace!("{}: SRV incoming decrypted {} bytes", short, size);
                                if size == 0 {
                                    break;
                                }
                                send1.send(buf[..size].to_vec()).await?;
                            }
                        }
                    }
                    _ => return Err(format!("invalid wire: {:?}", wire).into()),
                },
                Some(Right(None)) => {
                    send1.close().await?;
                }
                None => return Ok(()),
            }
        }
    }
    .await;

    if let Err(e) = res {
        tracing::error!("{} SRV: ERROR: {:?}", short, e);
        let _ = write
            .send(ProxyWire::failure(format!("{:?}", e)))
            .await
            .map_err(TransportError::other);
    }

    Ok(())
}
