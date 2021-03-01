use crate::tx2::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::io::AsyncReadExt;
use futures::io::AsyncWriteExt;

const R_MASK: u64 = 1 << 63;
const R_FILT: u64 = !R_MASK;

/// 64 bit MsgId - the top bit identifies if Request or Response.
#[derive(Clone, Copy)]
pub struct MsgId(u64);

impl From<u64> for MsgId {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl From<MsgId> for u64 {
    fn from(m: MsgId) -> Self {
        m.0
    }
}

impl MsgId {
    /// Create a new MsgId from a raw u64.
    pub fn new(v: u64) -> Self {
        Self(v)
    }

    /// Create a new notify-type MsgId.
    pub fn new_notify() -> Self {
        Self(0)
    }

    /// Get the inner raw value.
    pub fn inner(&self) -> u64 {
        self.0
    }

    /// Get the ID-portion ignoring the req/res bit.
    pub fn as_id(&self) -> u64 {
        self.0 & R_FILT
    }

    /// Get this Id as a request-type MsgId.
    /// (will panic if `as_id() == 0`).
    pub fn as_req(&self) -> Self {
        if self.as_id() == 0 {
            panic!("MsgId::as_id() == 0 cannot be a request-type");
        }
        Self(self.0 & R_FILT)
    }

    /// Get this Id as a response-type MsgId.
    /// (will panic if `as_id() == 0`).
    pub fn as_res(&self) -> Self {
        if self.as_id() == 0 {
            panic!("MsgId::as_id() == 0 cannot be a response-type");
        }
        Self(self.0 | R_MASK)
    }

    /// Is this MsgId a notify-type?
    pub fn is_notify(&self) -> bool {
        self.0 == 0
    }

    /// Is this MsgId a request-type?
    pub fn is_req(&self) -> bool {
        self.0 != 0 && self.0 & R_MASK == 0
    }

    /// Is this MsgId a response-type?
    pub fn is_res(&self) -> bool {
        self.0 != 0 && self.0 & R_MASK > 0
    }
}

impl std::fmt::Debug for MsgId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_notify = self.is_notify();
        let is_req = self.is_req();
        let is_res = self.is_res();
        let id = self.as_id();
        f.debug_struct("MsgId")
            .field("is_notify", &is_notify)
            .field("is_req", &is_req)
            .field("is_res", &is_res)
            .field("id", &id)
            .finish()
    }
}

/// Efficiently read framed data.
pub trait AsFramedReader: 'static + Send + Unpin {
    /// Read a frame of data from this AsFramedReader instance.
    /// This returns a Vec in case the first read contains multiple small items.
    fn read(&mut self, timeout: KitsuneTimeout) -> BoxFuture<'_, KitsuneResult<RR>>;
}

struct FramedReaderInner {
    sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
    local_buf: [u8; POOL_BUF_MAX_CAPACITY],
}

/// Efficiently read framed data from a sub AsyncRead instance.
pub struct FramedReader(Option<FramedReaderInner>);

fn read_size(b: &[u8]) -> usize {
    let mut bytes = [0_u8; 4];
    bytes.copy_from_slice(&b[..4]);
    u32::from_le_bytes(bytes) as usize
}

fn read_msg_id(b: &[u8]) -> MsgId {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&b[..8]);
    u64::from_le_bytes(bytes).into()
}

type RR = Vec<(MsgId, PoolBuf)>;

impl FramedReader {
    /// Create a new FramedReader instance.
    pub fn new(sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>) -> Self {
        Self(Some(FramedReaderInner {
            sub,
            local_buf: [0; POOL_BUF_MAX_CAPACITY],
        }))
    }
}

impl AsFramedReader for FramedReader {
    fn read(&mut self, timeout: KitsuneTimeout) -> BoxFuture<'_, KitsuneResult<RR>> {
        async move {
            let mut inner = match self.0.take() {
                None => return Err(KitsuneError::Closed),
                Some(inner) => inner,
            };

            let out = match timeout
                .mix(async {
                    let mut read = 0;

                    while read < 4 + 8 {
                        read += inner
                            .sub
                            .read(&mut inner.local_buf[read..4 + 8])
                            .await
                            .map_err(KitsuneError::other)?;
                    }

                    let want_size = read_size(&inner.local_buf[..4]) - 4 - 8;
                    let msg_id = read_msg_id(&inner.local_buf[4..4 + 8]);

                    let mut buf = PoolBuf::new();
                    buf.reserve(want_size);

                    while buf.len() < want_size {
                        let to_read = std::cmp::min(inner.local_buf.len(), want_size - buf.len());
                        read = match inner
                            .sub
                            .read(&mut inner.local_buf[..to_read])
                            .await
                            .map_err(KitsuneError::other)
                        {
                            Err(e) => return Err(e),
                            Ok(read) => read,
                        };
                        buf.extend_from_slice(&inner.local_buf[..read]);
                    }

                    Ok(vec![(msg_id, buf)])
                })
                .await
            {
                Err(e) => {
                    return Err(e);
                }
                Ok(out) => out,
            };

            self.0 = Some(inner);
            Ok(out)
        }
        .boxed()
    }
}

/// Efficiently write framed data.
pub trait AsFramedWriter: 'static + Send + Unpin {
    /// Write a frame of data to this FramedWriter instance.
    /// If timeout is exceeded, a timeout error is returned,
    /// and the stream is closed.
    fn write(
        &mut self,
        msg_id: MsgId,
        data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'_, KitsuneResult<()>>;
}

struct FramedWriterInner {
    sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
}

/// Efficiently write framed data to a sub AsyncWrite instance.
pub struct FramedWriter(Option<FramedWriterInner>);

impl FramedWriter {
    /// Create a new FramedWriter instance.
    pub fn new(sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>) -> Self {
        Self(Some(FramedWriterInner { sub }))
    }
}

impl AsFramedWriter for FramedWriter {
    /// Write a frame of data to this FramedWriter instance.
    /// If timeout is exceeded, a timeout error is returned,
    /// and the stream is closed.
    fn write(
        &mut self,
        msg_id: MsgId,
        mut data: PoolBuf,
        timeout: KitsuneTimeout,
    ) -> BoxFuture<'_, KitsuneResult<()>> {
        async move {
            let mut inner = match self.0.take() {
                None => return Err(KitsuneError::Closed),
                Some(inner) => inner,
            };

            if let Err(e) = timeout
                .mix(async {
                    let total = data.len() as u32 + 4 /* len */ + 8 /* msg_id */;

                    data.reserve_front(4 + 8);
                    data.prepend_from_slice(&msg_id.inner().to_le_bytes()[..]);
                    data.prepend_from_slice(&total.to_le_bytes()[..]);

                    inner
                        .sub
                        .write_all(&data)
                        .await
                        .map_err(KitsuneError::other)?;

                    Ok(())
                })
                .await
            {
                let _ = inner.sub.close().await;
                return Err(e);
            }

            self.0 = Some(inner);
            Ok(())
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msgid() {
        let req = MsgId::new(1);
        println!("{:?}", req);

        // make sure it starts out as a req
        assert!(!req.is_notify());
        assert!(req.is_req());
        assert!(!req.is_res());
        assert_eq!(1, req.as_id());

        // make sure as_req doesn't toggle
        let req = req.as_req();
        assert!(!req.is_notify());
        assert!(req.is_req());
        assert!(!req.is_res());
        assert_eq!(1, req.as_id());

        // make sure as_res works
        let res = req.as_res();
        println!("{:?}", res);

        assert!(!res.is_notify());
        assert!(res.is_res());
        assert!(!res.is_req());
        assert_eq!(1, res.as_id());

        // make sure as_res doesn't toggle
        let res = res.as_res();
        assert!(!res.is_notify());
        assert!(res.is_res());
        assert!(!res.is_req());
        assert_eq!(1, res.as_id());

        // make sure as_req works
        let req = res.as_req();
        assert!(!req.is_notify());
        assert!(req.is_req());
        assert!(!req.is_res());
        assert_eq!(1, req.as_id());

        // make sure new_notify works
        let not = MsgId::new_notify();
        println!("{:?}", not);
        assert!(not.is_notify());
        assert!(!not.is_req());
        assert!(!not.is_res());
        assert_eq!(0, not.as_id());
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_framed() {
        let (send, recv) = util::bound_async_mem_channel(4096);
        let mut send = FramedWriter::new(send);
        let mut recv = FramedReader::new(recv);

        let wt = tokio::task::spawn(async move {
            let mut buf = PoolBuf::new();
            buf.extend_from_slice(&[0xd0; 512]);
            send.write(1.into(), buf, KitsuneTimeout::from_millis(1000 * 30))
                .await
                .unwrap();
            let mut buf = PoolBuf::new();
            buf.extend_from_slice(&[0xd1; 8000]);
            send.write(2.into(), buf, KitsuneTimeout::from_millis(1000 * 30))
                .await
                .unwrap();
        });

        for _ in 0..2 {
            for (msg_id, data) in recv
                .read(KitsuneTimeout::from_millis(1000 * 30))
                .await
                .unwrap()
            {
                println!("got {} - {} bytes", msg_id.as_id(), data.len());
            }
        }

        wt.await.unwrap();
    }
}
