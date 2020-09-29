use crate::*;
use futures::{sink::SinkExt, stream::StreamExt};
use rustls::Session;
use std::io::{Read, Write};

pub(crate) fn spawn_tls_server(
    incoming_base_url: url2::Url2,
    tls_server_config: Arc<rustls::ServerConfig>,
    evt_send: TransportIncomingChannelSender,
    write: futures::channel::mpsc::Sender<ProxyWire>,
    read: futures::channel::mpsc::Receiver<ProxyWire>,
) {
    tokio::task::spawn(tls_server(
        incoming_base_url,
        tls_server_config,
        evt_send,
        write,
        read,
    ));
}

async fn tls_server(
    incoming_base_url: url2::Url2,
    tls_server_config: Arc<rustls::ServerConfig>,
    mut evt_send: TransportIncomingChannelSender,
    mut write: futures::channel::mpsc::Sender<ProxyWire>,
    mut read: futures::channel::mpsc::Receiver<ProxyWire>,
) {
    let res: TransportResult<()> = async {
        let mut srv = rustls::ServerSession::new(&tls_server_config);
        let mut buf = [0_u8; 4096];
        let mut in_pre = std::io::Cursor::new(Vec::new());

        let ((mut send1, mut recv1), (send2, recv2)) = create_transport_channel_pair();
        let mut send2 = Some(send2);
        let mut recv2 = Some(recv2);
        let mut outgoing_data_fut = recv1.next();
        let mut incoming_wire_fut = read.next();
        let mut did_post_handshake_work = false;
        loop {
            if !did_post_handshake_work && !srv.is_handshaking() {
                did_post_handshake_work = true;

                let cert_digest = blake2b_32(
                    srv.get_peer_certificates()
                        .unwrap()
                        .get(0)
                        .unwrap()
                        .as_ref(),
                );

                let remote_proxy_url =
                    ProxyUrl::new(incoming_base_url.as_str(), cert_digest.into())?;

                evt_send
                    .send((
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
                write
                    .send(ProxyWire::chan_send(data.into()))
                    .await
                    .map_err(TransportError::other)?;
            }

            if !srv.wants_read() {
                tokio::time::delay_for(std::time::Duration::from_millis(10)).await;
                continue;
            }

            use futures::future::Either;
            match futures::future::select(outgoing_data_fut, incoming_wire_fut).await {
                Either::Left((data, fut)) => {
                    match data {
                        Some(data) => {
                            srv.write_all(&data).map_err(TransportError::other)?;
                        }
                        None => return Err("write side shutdown".into()),
                    }
                    outgoing_data_fut = recv1.next();
                    incoming_wire_fut = fut;
                }
                Either::Right((wire, fut)) => {
                    match wire {
                        Some(ProxyWire::ChanSend(ChanSend(data))) => {
                            in_pre.get_mut().extend_from_slice(&data);
                            srv.read_tls(&mut in_pre).map_err(TransportError::other)?;
                            srv.process_new_packets().map_err(TransportError::other)?;
                            while let Ok(size) = srv.read(&mut buf) {
                                if size == 0 {
                                    // End of stream
                                    return Err("reached end of stream".into());
                                }
                                send1.send(buf[..size].to_vec()).await?;
                            }
                        }
                        _ => return Err(format!("invalid wire: {:?}", wire).into()),
                    }
                    outgoing_data_fut = fut;
                    incoming_wire_fut = read.next();
                }
            }
        }
    }
    .await;

    if let Err(e) = res {
        let _ = write
            .send(ProxyWire::failure(format!("{:?}", e)))
            .await
            .map_err(TransportError::other);
    }
}
