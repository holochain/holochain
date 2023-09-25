use crate::tx2::tx2_utils::*;
use crate::*;
use futures::future::{BoxFuture, FutureExt};
use futures::io::AsyncReadExt;
use futures::io::AsyncWriteExt;

/// Request/Response bit mask.
const R_MASK: u64 = 1 << 63;

/// Request/Response bit filter.
const R_FILT: u64 = !R_MASK;

/// MsgSize Bytes
const MSG_SIZE_BYTES: usize = 4;

/// MsgId Bytes
const MSG_ID_BYTES: usize = 8;

/// MsgId type
#[derive(Debug)]
pub enum MsgIdType {
    /// Notify-type MsgId
    Notify,

    /// Req-type MsgId
    Req,

    /// Res-type MsgId
    Res,
}

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

    /// Get the MsgIdType of this MsgId.
    pub fn get_type(&self) -> MsgIdType {
        if self.is_notify() {
            MsgIdType::Notify
        } else if self.is_req() {
            MsgIdType::Req
        } else if self.is_res() {
            MsgIdType::Res
        } else {
            unreachable!()
        }
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
        let msg_id_type = self.get_type();
        let id = self.as_id();
        f.debug_struct("MsgId")
            .field("type", &msg_id_type)
            .field("id", &id)
            .finish()
    }
}

type RR = (MsgId, PoolBuf);

/// Efficiently read framed data.
#[cfg_attr(feature = "test_utils", mockall::automock)]
pub trait AsFramedReader: 'static + Send + Unpin {
    /// Read a frame of data from this AsFramedReader instance.
    /// This returns a Vec in case the first read contains multiple small items.
    fn read(&mut self, timeout: KitsuneTimeout) -> BoxFuture<'_, KitsuneResult<RR>>;
}

struct FramedReaderInner {
    sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
    local_buf: [u8; 4096],
}

/// Efficiently read framed data from a sub AsyncRead instance.
pub struct FramedReader(Option<FramedReaderInner>);

fn read_size(b: &[u8]) -> usize {
    let mut bytes = [0_u8; MSG_SIZE_BYTES];
    bytes.copy_from_slice(&b[..MSG_SIZE_BYTES]);
    u32::from_le_bytes(bytes) as usize
}

fn read_msg_id(b: &[u8]) -> MsgId {
    let mut bytes = [0_u8; MSG_ID_BYTES];
    bytes.copy_from_slice(&b[..MSG_ID_BYTES]);
    u64::from_le_bytes(bytes).into()
}

impl FramedReader {
    /// Create a new FramedReader instance.
    pub fn new(sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>) -> Self {
        Self(Some(FramedReaderInner {
            sub,
            local_buf: [0; 4096],
        }))
    }
}

impl AsFramedReader for FramedReader {
    fn read(&mut self, timeout: KitsuneTimeout) -> BoxFuture<'_, KitsuneResult<RR>> {
        async move {
            let mut inner = match self.0.take() {
                None => return Err(KitsuneErrorKind::Closed.into()),
                Some(inner) => inner,
            };

            let out = match timeout
                .mix("FramedReader::read", async {
                    let mut read = 0;
                    let want = MSG_SIZE_BYTES + MSG_ID_BYTES;
                    while read < want {
                        let sub_read = inner
                            .sub
                            .read(&mut inner.local_buf[read..MSG_SIZE_BYTES + MSG_ID_BYTES])
                            .await
                            .map_err(KitsuneError::other)?;
                        if sub_read == 0 {
                            return Err(KitsuneErrorKind::Closed.into());
                        }
                        read += sub_read;
                    }

                    let want_size = read_size(&inner.local_buf[..MSG_SIZE_BYTES])
                        - MSG_SIZE_BYTES
                        - MSG_ID_BYTES;
                    let msg_id = read_msg_id(
                        &inner.local_buf[MSG_SIZE_BYTES..MSG_SIZE_BYTES + MSG_ID_BYTES],
                    );

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
                        if read == 0 {
                            return Err(KitsuneErrorKind::Closed.into());
                        }
                        buf.extend_from_slice(&inner.local_buf[..read]);
                    }

                    Ok((msg_id, buf))
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
#[cfg_attr(feature = "test_utils", mockall::automock)]
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
                None => return Err(KitsuneErrorKind::Closed.into()),
                Some(inner) => inner,
            };

            if let Err(e) = timeout
                .mix("FramedWriter::write", async {
                    let total = (data.len() + MSG_SIZE_BYTES + MSG_ID_BYTES) as u32;

                    data.reserve_front(MSG_SIZE_BYTES + MSG_ID_BYTES);
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
                tracing::error!(?e, "writer closing due to error");
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

    #[tokio::test(flavor = "multi_thread")]
    async fn test_framed() {
        let t = KitsuneTimeout::from_millis(5000);

        let (send, recv) = bound_async_mem_channel(4096, None);
        let mut send = FramedWriter::new(send);
        let mut recv = FramedReader::new(recv);

        let wt = metric_task(async move {
            let mut buf = PoolBuf::new();
            buf.extend_from_slice(&[0xd0; 512]);
            send.write(1.into(), buf, t).await.unwrap();
            let mut buf = PoolBuf::new();
            buf.extend_from_slice(&[0xd1; 8000]);
            send.write(2.into(), buf, t).await.unwrap();
            KitsuneResult::Ok(())
        });

        for _ in 0..2 {
            let (msg_id, data) = recv.read(t).await.unwrap();
            println!("got {} - {} bytes", msg_id.as_id(), data.len());
        }

        wt.await.unwrap().unwrap();
    }

    #[tokio::test]
    #[cfg(feature = "test_utils")]
    async fn test_mock_framed() {
        let mut f = MockAsFramedReader::new();
        f.expect_read().returning(|_t| {
            async move {
                let mut buf = PoolBuf::new();
                buf.extend_from_slice(b"test");
                Ok((0.into(), buf))
            }
            .boxed()
        });
        let (_, buf) = f.read(KitsuneTimeout::from_millis(100)).await.unwrap();
        assert_eq!(b"test", buf.as_ref());

        let mut f = MockAsFramedWriter::new();
        f.expect_write().returning(|_, buf, _| {
            assert_eq!(b"test2", buf.as_ref());
            async move { Ok(()) }.boxed()
        });
        let mut buf = PoolBuf::new();
        buf.extend_from_slice(b"test2");
        f.write(0.into(), buf, KitsuneTimeout::from_millis(100))
            .await
            .unwrap();
    }
}
