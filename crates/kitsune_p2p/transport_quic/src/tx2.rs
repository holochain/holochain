#![allow(clippy::new_ret_no_self)]
//! kitsune tx2 quic transport backend

use futures::future::{BoxFuture, FutureExt};
use futures::stream::{BoxStream, StreamExt};
use kitsune_p2p_types::config::*;
use kitsune_p2p_types::dependencies::{ghost_actor::dependencies::tracing, serde_json};
use kitsune_p2p_types::tls::*;
use kitsune_p2p_types::tx2::tx2_adapter::*;
use kitsune_p2p_types::tx2::tx2_utils::*;
use kitsune_p2p_types::tx2::*;
use kitsune_p2p_types::*;
use std::sync::Arc;

/// Configuration for QuicBackendAdapt
#[non_exhaustive]
pub struct QuicConfig {
    /// Tls config
    /// Default: None = ephemeral.
    pub tls: Option<TlsConfig>,

    /// Tuning Params
    /// Default: None = default.
    pub tuning_params: Option<KitsuneP2pTuningParams>,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            tls: None,
            tuning_params: None,
        }
    }
}

impl QuicConfig {
    /// into inner contents with default application
    pub async fn split(self) -> KitsuneResult<(TlsConfig, KitsuneP2pTuningParams)> {
        let QuicConfig { tls, tuning_params } = self;

        let tls = match tls {
            None => TlsConfig::new_ephemeral().await?,
            Some(tls) => tls,
        };

        let tuning_params = tuning_params.unwrap_or_else(KitsuneP2pTuningParams::default);

        Ok((tls, tuning_params))
    }
}

/// Quic endpoint bind adapter for kitsune tx2
pub async fn tx2_quic_adapter(config: QuicConfig) -> KitsuneResult<AdapterFactory> {
    QuicBackendAdapt::new(config).await
}

// -- private -- //

/// Tls ALPN identifier for kitsune quic handshaking
const ALPN_KITSUNE_QUIC_0: &[u8] = b"kitsune-quic/0";

struct QuicInChanRecvAdapt(BoxStream<'static, InChanFut>);

impl QuicInChanRecvAdapt {
    pub fn new(recv: quinn::IncomingUniStreams) -> Self {
        Self(
            futures::stream::unfold(recv, move |mut recv| async move {
                match recv.next().await {
                    None => None,
                    Some(in_) => Some((
                        async move {
                            let in_ = in_.map_err(KitsuneError::other)?;
                            let in_: InChan = Box::new(FramedReader::new(Box::new(in_)));
                            Ok(in_)
                        }
                        .boxed(),
                        recv,
                    )),
                }
            })
            .boxed(),
        )
    }
}

impl futures::stream::Stream for QuicInChanRecvAdapt {
    type Item = InChanFut;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.0;
        tokio::pin!(inner);
        futures::stream::Stream::poll_next(inner, cx)
    }
}

impl InChanRecvAdapt for QuicInChanRecvAdapt {}

struct QuicConAdaptInner {
    peer_cert: Tx2Cert,
    con: quinn::Connection,
}

struct QuicConAdapt(Share<QuicConAdaptInner>, Uniq, Tx2Cert);

pub(crate) fn blake2b_32(data: &[u8]) -> Vec<u8> {
    blake2b_simd::Params::new()
        .hash_length(32)
        .to_state()
        .update(data)
        .finalize()
        .as_bytes()
        .to_vec()
}

impl QuicConAdapt {
    pub fn new(con: quinn::Connection) -> KitsuneResult<Self> {
        let peer_cert: Tx2Cert = match con.peer_identity() {
            None => return Err("invalid peer certificate".into()),
            Some(chain) => match chain.iter().next() {
                None => return Err("invalid peer certificate".into()),
                Some(cert) => blake2b_32(cert.as_ref()).into(),
            },
        };
        Ok(Self(
            Share::new(QuicConAdaptInner {
                peer_cert: peer_cert.clone(),
                con,
            }),
            Uniq::default(),
            peer_cert,
        ))
    }
}

impl ConAdapt for QuicConAdapt {
    fn uniq(&self) -> Uniq {
        self.1
    }

    fn peer_addr(&self) -> KitsuneResult<TxUrl> {
        let addr = self.0.share_mut(|i, _| Ok(i.con.remote_address()))?;

        use kitsune_p2p_types::dependencies::url2;
        let url = url2::url2!("{}://{}", crate::SCHEME, addr);

        Ok(url.into())
    }

    fn peer_cert(&self) -> Tx2Cert {
        self.2.clone()
    }

    fn out_chan(&self, timeout: KitsuneTimeout) -> OutChanFut {
        let maybe_out_fut = self.0.share_mut(|i, _| Ok(i.con.open_uni()));
        timeout
            .mix(async move {
                let out = maybe_out_fut?.await.map_err(KitsuneError::other)?;
                let out: OutChan = Box::new(FramedWriter::new(Box::new(out)));
                Ok(out)
            })
            .boxed()
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        let _ = self.0.share_mut(|i, c| {
            tracing::info!(
                peer_cert=?i.peer_cert,
                %code,
                %reason,
                "close connection (quic)",
            );
            *c = true;
            i.con.close(code.into(), reason.as_bytes());
            Ok(())
        });
        async move {}.boxed()
    }
}

fn connecting(con_fut: quinn::Connecting, local_cert: Tx2Cert, is_outgoing: bool) -> ConFut {
    async move {
        let quinn::NewConnection {
            connection,
            uni_streams,
            ..
        } = con_fut.await.map_err(KitsuneError::other)?;

        let con: Arc<dyn ConAdapt> = Arc::new(QuicConAdapt::new(connection)?);
        let chan_recv: Box<dyn InChanRecvAdapt> = Box::new(QuicInChanRecvAdapt::new(uni_streams));

        let peer_cert = con.peer_cert();
        let url = con.peer_addr()?;
        if is_outgoing {
            tracing::info!(?local_cert, ?peer_cert, %url, "established outgoing connection (quic)");
        } else {
            tracing::info!(?local_cert, ?peer_cert, %url, "established incoming connection (quic)");
        }

        Ok((con, chan_recv))
    }
    .boxed()
}

struct QuicConRecvAdapt(BoxStream<'static, ConFut>);

impl QuicConRecvAdapt {
    pub fn new(recv: quinn::Incoming, local_cert: Tx2Cert) -> Self {
        Self(
            futures::stream::unfold(
                (recv, local_cert),
                move |(mut recv, local_cert)| async move {
                    match recv.next().await {
                        None => None,
                        Some(con) => Some((
                            connecting(con, local_cert.clone(), false),
                            (recv, local_cert),
                        )),
                    }
                },
            )
            .boxed(),
        )
    }
}

impl futures::stream::Stream for QuicConRecvAdapt {
    type Item = ConFut;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let inner = &mut self.0;
        tokio::pin!(inner);
        futures::stream::Stream::poll_next(inner, cx)
    }
}

impl ConRecvAdapt for QuicConRecvAdapt {}

struct QuicEndpointAdaptInner {
    ep: quinn::Endpoint,
    local_cert: Tx2Cert,
}

struct QuicEndpointAdapt(Share<QuicEndpointAdaptInner>, Uniq, Tx2Cert);

impl QuicEndpointAdapt {
    pub fn new(ep: quinn::Endpoint, local_cert: Tx2Cert) -> Self {
        Self(
            Share::new(QuicEndpointAdaptInner {
                ep,
                local_cert: local_cert.clone(),
            }),
            Uniq::default(),
            local_cert,
        )
    }
}

impl EndpointAdapt for QuicEndpointAdapt {
    fn debug(&self) -> serde_json::Value {
        match self.local_addr() {
            Ok(addr) => serde_json::json!({
                "type": "tx2_quic",
                "state": "open",
                "addr": addr,
            }),
            Err(_) => serde_json::json!({
                "type": "tx2_quic",
                "state": "closed",
            }),
        }
    }

    fn uniq(&self) -> Uniq {
        self.1
    }

    fn local_addr(&self) -> KitsuneResult<TxUrl> {
        let addr = self
            .0
            .share_mut(|i, _| i.ep.local_addr().map_err(KitsuneError::other))?;

        use kitsune_p2p_types::dependencies::url2;
        let mut url = url2::url2!("{}://{}", crate::SCHEME, addr);

        // TODO - FIXME - not sure how slow `get_if_addrs` is
        //                might be better to do this once on bind
        //                and just cache the bound address
        if let Some(host) = url.host_str() {
            if host == "0.0.0.0" {
                for iface in if_addrs::get_if_addrs().map_err(KitsuneError::other)? {
                    // super naive - just picking the first v4 that is not 127.0.0.1
                    let addr = iface.addr.ip();
                    if let std::net::IpAddr::V4(addr) = addr {
                        if addr != std::net::Ipv4Addr::from([127, 0, 0, 1]) {
                            url.set_host(Some(&iface.addr.ip().to_string())).unwrap();
                            break;
                        }
                    }
                }
            }
        }

        Ok(url.into())
    }

    fn local_cert(&self) -> Tx2Cert {
        self.2.clone()
    }

    fn connect(&self, url: TxUrl, timeout: KitsuneTimeout) -> ConFut {
        let maybe_ep = self
            .0
            .share_mut(|i, _| Ok((i.ep.clone(), i.local_cert.clone())));
        timeout
            .mix(async move {
                let (ep, local_cert) = maybe_ep?;
                let addr = crate::url_to_addr(url.as_url2(), crate::SCHEME)
                    .await
                    .map_err(KitsuneError::other)?;
                let con = ep.connect(&addr, "stub.stub").map_err(KitsuneError::other);
                match connecting(con?, local_cert, true).await {
                    Ok(con) => Ok(con),
                    Err(err) => {
                        tracing::warn!(?err, "failed to establish outgoing connection (quic)");
                        Err(err)
                    }
                }
            })
            .boxed()
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn close(&self, code: u32, reason: &str) -> BoxFuture<'static, ()> {
        let _ = self.0.share_mut(|i, c| {
            tracing::warn!(
                local_cert=?i.local_cert,
                "CLOSING ENDPOINT"
            );
            *c = true;
            i.ep.close(code.into(), reason.as_bytes());
            Ok(())
        });
        async move {}.boxed()
    }
}

/// Quic endpoint backend bind adapter for kitsune tx2
pub struct QuicBackendAdapt {
    local_cert: Tx2Cert,
    quic_srv: quinn::ServerConfig,
    quic_cli: quinn::ClientConfig,
}

impl QuicBackendAdapt {
    /// Construct a new quic tx2 backend bind adapter
    pub async fn new(config: QuicConfig) -> KitsuneResult<AdapterFactory> {
        let (tls, tuning_params) = config.split().await?;

        let local_cert = tls.cert_digest.clone().into();

        let (tls_srv, tls_cli) = gen_tls_configs(ALPN_KITSUNE_QUIC_0, &tls, tuning_params.clone())?;

        let mut transport = quinn::TransportConfig::default();

        // We don't use bidi streams in kitsune - only uni streams
        transport
            .max_concurrent_bidi_streams(0)
            .map_err(KitsuneError::other)?;

        // We don't use "Application" datagrams in kitsune -
        // only bidi streams.
        transport.datagram_receive_buffer_size(None);

        // Disable spin bit - we'd like the extra privacy
        // any metrics we implement will be opt-in self reporting
        transport.allow_spin(false);

        // see also `keep_alive_interval`.
        // right now keep_alive_interval is None,
        transport
            .max_idle_timeout(Some(std::time::Duration::from_millis(
                tuning_params.tx2_quic_max_idle_timeout_ms as u64,
            )))
            .unwrap();

        let transport = Arc::new(transport);

        let mut quic_srv = quinn::ServerConfig::default();
        quic_srv.transport = transport.clone();
        quic_srv.crypto = tls_srv;

        let mut quic_cli = quinn::ClientConfig::default();
        quic_cli.transport = transport;
        quic_cli.crypto = tls_cli;

        tracing::debug!(?quic_srv, ?quic_cli, "build quinn configs");

        let out: AdapterFactory = Arc::new(Self {
            local_cert,
            quic_srv,
            quic_cli,
        });

        Ok(out)
    }
}

impl BindAdapt for QuicBackendAdapt {
    fn bind(&self, url: TxUrl, timeout: KitsuneTimeout) -> EndpointFut {
        let local_cert = self.local_cert.clone();
        let quic_srv = self.quic_srv.clone();
        let quic_cli = self.quic_cli.clone();
        timeout
            .mix(async move {
                let mut builder = quinn::Endpoint::builder();
                builder.listen(quic_srv);
                builder.default_client_config(quic_cli);

                let addr = crate::url_to_addr(url.as_url2(), crate::SCHEME)
                    .await
                    .map_err(KitsuneError::other)?;

                let (ep, inc) = builder.bind(&addr).map_err(KitsuneError::other)?;

                let ep: Arc<dyn EndpointAdapt> =
                    Arc::new(QuicEndpointAdapt::new(ep, local_cert.clone()));
                let con_recv: Box<dyn ConRecvAdapt> =
                    Box::new(QuicConRecvAdapt::new(inc, local_cert.clone()));

                let url = ep.local_addr()?;

                tracing::info!(?local_cert, %url, "bound local endpoint (quic)");

                Ok((ep, con_recv))
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_quic_tx2() {
        kitsune_p2p_types::dependencies::observability::test_run().ok();

        let t = KitsuneTimeout::from_millis(5000);

        let (s_done, r_done) = tokio::sync::oneshot::channel();

        let config = QuicConfig::default();
        let factory = QuicBackendAdapt::new(config).await.unwrap();
        let (ep1, _con_recv1) = factory
            .bind("kitsune-quic://0.0.0.0:0".into(), t)
            .await
            .unwrap();

        let config = QuicConfig::default();
        let factory = QuicBackendAdapt::new(config).await.unwrap();
        let (ep2, mut con_recv2) = factory
            .bind("kitsune-quic://0.0.0.0:0".into(), t)
            .await
            .unwrap();

        let addr2 = ep2.local_addr().unwrap();
        println!("addr2: {}", addr2);

        let rt = kitsune_p2p_types::metrics::metric_task(async move {
            if let Some(mc) = con_recv2.next().await {
                let (_con, mut recv) = mc.await.unwrap();
                if let Some(mc) = recv.next().await {
                    let mut c = mc.await.unwrap();
                    let t = KitsuneTimeout::from_millis(5000);
                    let (_, data) = c.read(t).await.unwrap();
                    println!("GOT: {:?}", data.as_ref());
                    s_done.send(()).unwrap();
                }
            }
            KitsuneResult::Ok(())
        });

        let (c, _recv) = ep1.connect(addr2, t).await.unwrap();
        let mut c = c.out_chan(t).await.unwrap();

        let mut data = PoolBuf::new();
        data.extend_from_slice(b"hello");
        c.write(0.into(), data, t).await.unwrap();

        let debug = ep1.debug();
        println!("{}", serde_json::to_string_pretty(&debug).unwrap());

        r_done.await.unwrap();

        ep1.close(0, "").await;
        ep2.close(0, "").await;

        rt.await.unwrap().unwrap();
    }
}
