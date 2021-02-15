use crate::tx2::*;
use crate::*;

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
    /// Create a new Request-Type MsgId.
    pub fn new(v: u64) -> Self {
        Self(v)
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
    pub fn as_req(&self) -> Self {
        Self(self.0 & R_FILT)
    }

    /// Get this Id as a response-type MsgId.
    pub fn as_res(&self) -> Self {
        Self(self.0 | R_MASK)
    }

    /// Is this MsgId a request-type?
    pub fn is_req(&self) -> bool {
        self.0 & R_MASK == 0
    }

    /// Is this MsgId a response-type?
    pub fn is_res(&self) -> bool {
        self.0 & R_MASK > 0
    }
}

impl std::fmt::Debug for MsgId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_req = self.is_req();
        let is_res = self.is_res();
        let id = self.as_id();
        f.debug_struct("MsgId")
            .field("is_req", &is_req)
            .field("is_res", &is_res)
            .field("id", &id)
            .finish()
    }
}

type FramedVec = Vec<(MsgId, Box<[u8]>)>;

/// Read Frames one at a time from an async source.
pub trait AsyncReadFramed: 'static + Send + Unpin {
    /// low-level poll for reading a frame.
    fn poll_read_framed(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        out: &mut Option<FramedVec>,
    ) -> std::task::Poll<KitsuneResult<Option<usize>>>;
}

/// Extension trait providing higher-level access API.
pub trait AsyncReadFramedExt: AsyncReadFramed {
    /// high-level async read frames fn.
    fn read_frame<'a>(
        &'a mut self,
        timeout: KitsuneTimeout,
        out: &'a mut Option<FramedVec>,
    ) -> AsyncReadFramedFut<'a, Self> {
        let this = std::pin::Pin::new(&mut *self);
        AsyncReadFramedFut(Some(AsyncReadFramedFutInner {
            sub: this,
            timeout,
            out,
        }))
    }
}

impl<A: AsyncReadFramed> AsyncReadFramedExt for A {}

struct AsyncReadFramedFutInner<'a, P>
where
    P: ?Sized + AsyncReadFramed,
{
    sub: std::pin::Pin<&'a mut P>,
    timeout: KitsuneTimeout,
    out: &'a mut Option<FramedVec>,
}

/// Future returned from `AsyncReadFramed::read_framed()`.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AsyncReadFramedFut<'a, P>(Option<AsyncReadFramedFutInner<'a, P>>)
where
    P: ?Sized + AsyncReadFramed;

impl<'a, P> std::future::Future for AsyncReadFramedFut<'a, P>
where
    P: ?Sized + AsyncReadFramed,
{
    type Output = KitsuneResult<Option<usize>>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut inner = match self.0.take() {
            None => return std::task::Poll::Ready(Ok(None)),
            Some(inner) => inner,
        };

        let mut got_pending = false;
        let mut frame_count = 0;
        let mut closed = false;

        while !inner.timeout.is_expired() {
            let rdr: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut inner.sub);
            match AsyncReadFramed::poll_read_framed(rdr, cx, inner.out) {
                std::task::Poll::Pending => {
                    got_pending = true;
                    break;
                }
                std::task::Poll::Ready(Ok(None)) => {
                    closed = true;
                    break;
                }
                std::task::Poll::Ready(Ok(Some(count))) => {
                    frame_count += count;
                    if frame_count > 0 {
                        break;
                    }
                }
                std::task::Poll::Ready(Err(e)) => {
                    // do not re-set our inner, we got an error
                    return std::task::Poll::Ready(Err(e));
                }
            }
        }

        if !closed {
            self.0 = Some(inner);
        }

        if frame_count > 0 {
            std::task::Poll::Ready(Ok(Some(frame_count)))
        } else if got_pending {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(Ok(None))
        }
    }
}

struct AsyncReadFramedFilterInner {
    sub: Box<dyn AsyncReadIntoVec>,
    buf: Option<Vec<u8>>,
}

/// A filter allowing AsyncReadFramed.
pub struct AsyncReadFramedFilter(Option<AsyncReadFramedFilterInner>);

impl AsyncReadFramedFilter {
    /// Create a new AsyncReadFramedFilter instance.
    pub fn new(sub: Box<dyn AsyncReadIntoVec>) -> Self {
        Self(Some(AsyncReadFramedFilterInner { sub, buf: None }))
    }
}

fn read_size(b: &[u8]) -> usize {
    let mut bytes = [0_u8; 4];
    bytes.copy_from_slice(&b[0..4]);
    u32::from_le_bytes(bytes) as usize
}

fn read_msg_id(b: &[u8]) -> MsgId {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&b[4..12]);
    u64::from_le_bytes(bytes).into()
}

impl AsyncReadFramed for AsyncReadFramedFilter {
    fn poll_read_framed(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        out: &mut Option<FramedVec>,
    ) -> std::task::Poll<KitsuneResult<Option<usize>>> {
        let mut inner = match self.0.take() {
            None => return std::task::Poll::Ready(Ok(None)),
            Some(inner) => inner,
        };

        let mut buf = inner.buf.unwrap_or_else(|| Vec::with_capacity(4096));

        let mut got_pending = false;
        let mut closed = false;

        let max_bytes = if buf.len() < 4 {
            4096
        } else {
            std::cmp::max(4096, read_size(&buf))
        };

        let sub: &mut dyn AsyncReadIntoVec = &mut *inner.sub;
        let sub: std::pin::Pin<&mut dyn AsyncReadIntoVec> = std::pin::Pin::new(sub);

        match AsyncReadIntoVec::poll_read_into_vec(sub, cx, &mut buf, max_bytes) {
            std::task::Poll::Pending => {
                got_pending = true;
            }
            std::task::Poll::Ready(Err(e)) => {
                return std::task::Poll::Ready(Err(e));
            }
            std::task::Poll::Ready(Ok(None)) => {
                closed = true;
            }
            std::task::Poll::Ready(Ok(Some(_size))) => (),
        }

        let mut frame_count = 0;
        while buf.len() >= 4 + 8 {
            let want_size = read_size(&buf);
            if buf.len() < want_size {
                break;
            }
            let msg_id = read_msg_id(&buf);
            let mut data = buf.drain(..want_size).collect::<Vec<_>>();
            data.drain(..12);
            if out.is_none() {
                *out = Some(Vec::new());
            }
            out.as_mut()
                .unwrap()
                .push((msg_id, data.into_boxed_slice()));
            frame_count += 1;
        }

        if closed && !buf.is_empty() {
            return std::task::Poll::Ready(Err(KitsuneError::other(futures::io::Error::new(
                futures::io::ErrorKind::UnexpectedEof,
                "remaining buffer after sub-reader closed",
            ))));
        }

        inner.buf = Some(buf);

        if !closed {
            self.0 = Some(inner);
        }

        if frame_count == 0 && got_pending {
            std::task::Poll::Pending
        } else if frame_count == 0 && closed {
            std::task::Poll::Ready(Ok(None))
        } else {
            std::task::Poll::Ready(Ok(Some(frame_count)))
        }
    }
}

/// Write framed data.
pub trait AsyncWriteFramed: 'static + Send + Unpin {
    /// Enqueue data for sending.
    /// If the underlying stream is closed, or
    /// there is already data queued for sending,
    /// will return false.
    /// You also need to call `poll_write_framed` to send the data.
    fn push_frame(self: std::pin::Pin<&mut Self>, msg_id: MsgId, buf: &[u8])
        -> KitsuneResult<bool>;

    /// low-level poll for writing framed data.
    /// Call `push_frame` first to enqueue data for sending.
    /// `false` indicates there is still more data to write.
    /// `true` indicates all data has been sent, ready for new `push_frame()`.
    fn poll_write_framed(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<KitsuneResult<bool>>;

    /// delegates to the underlying stream `poll_flush`.
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<KitsuneResult<()>>;

    /// delegates to the underlying stream `poll_close`.
    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<KitsuneResult<()>>;
}

/// Extension trait providing higher-level access API.
pub trait AsyncWriteFramedExt: AsyncWriteFramed {
    /// high-level async write frames fn.
    fn write_frame<'a>(
        &'a mut self,
        msg_id: MsgId,
        buf: &'a [u8],
        timeout: KitsuneTimeout,
    ) -> AsyncWriteFramedFut<'a, Self> {
        let this = std::pin::Pin::new(&mut *self);
        AsyncWriteFramedFut {
            stream: this,
            msg_id,
            buf,
            is_pre_push: true,
            timeout,
        }
    }
}

impl<A: AsyncWriteFramed> AsyncWriteFramedExt for A {}

/// Future returned from `AsyncWriteFramed::write_frame()`.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AsyncWriteFramedFut<'a, P>
where
    P: ?Sized + AsyncWriteFramed,
{
    stream: std::pin::Pin<&'a mut P>,
    msg_id: MsgId,
    buf: &'a [u8],
    is_pre_push: bool,
    timeout: KitsuneTimeout,
}

impl<'a, P> std::future::Future for AsyncWriteFramedFut<'a, P>
where
    P: ?Sized + AsyncWriteFramed,
{
    type Output = KitsuneResult<()>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = &mut *self;

        // TODO - actually use the timeout
        let _timeout = this.timeout;

        if this.is_pre_push {
            loop {
                let stream: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut this.stream);
                match AsyncWriteFramed::poll_write_framed(stream, cx) {
                    std::task::Poll::Ready(Ok(true)) => break,
                    std::task::Poll::Ready(Ok(false)) => continue,
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                    std::task::Poll::Ready(Err(e)) => return std::task::Poll::Ready(Err(e)),
                }
            }

            {
                let stream: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut this.stream);
                // TODO - fix this unwarp()
                AsyncWriteFramed::push_frame(stream, this.msg_id, this.buf).unwrap();
            }

            this.is_pre_push = false;
        }

        loop {
            let stream: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut this.stream);
            match AsyncWriteFramed::poll_write_framed(stream, cx) {
                std::task::Poll::Ready(Ok(true)) => break,
                std::task::Poll::Ready(Ok(false)) => continue,
                std::task::Poll::Pending => return std::task::Poll::Pending,
                std::task::Poll::Ready(Err(e)) => return std::task::Poll::Ready(Err(e)),
            }
        }

        std::task::Poll::Ready(Ok(()))
    }
}

type AW = Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>;

struct AsyncWriteFramedFilterInner {
    sub: AW,
    to_send: Vec<u8>,
}

/// A filter that will frame outgoing async writes.
pub struct AsyncWriteFramedFilter(Option<AsyncWriteFramedFilterInner>);

impl AsyncWriteFramedFilter {
    /// Create a new AsyncWriteFramedFilter instance.
    pub fn new(sub: AW) -> Self {
        Self(Some(AsyncWriteFramedFilterInner {
            sub,
            to_send: Vec::with_capacity(4096),
        }))
    }
}

impl AsyncWriteFramed for AsyncWriteFramedFilter {
    fn push_frame(
        mut self: std::pin::Pin<&mut Self>,
        msg_id: MsgId,
        buf: &[u8],
    ) -> KitsuneResult<bool> {
        let mut inner = match self.0.take() {
            None => return Err("PreviouslyClosed".into()),
            Some(inner) => inner,
        };

        if !inner.to_send.is_empty() {
            return Ok(false);
        }

        let size: u32 = buf.len() as u32 + 4 + 8;
        inner.to_send.extend_from_slice(&size.to_le_bytes());
        inner
            .to_send
            .extend_from_slice(&msg_id.inner().to_le_bytes());
        inner.to_send.extend_from_slice(buf);

        self.0 = Some(inner);

        Ok(true)
    }

    fn poll_write_framed(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<KitsuneResult<bool>> {
        let mut inner = match self.0.take() {
            None => return std::task::Poll::Ready(Err("PreviouslyClosed".into())),
            Some(inner) => inner,
        };

        let sub = &mut inner.sub;
        tokio::pin!(sub);

        match futures::io::AsyncWrite::poll_write(sub, cx, &inner.to_send) {
            std::task::Poll::Pending => (),
            std::task::Poll::Ready(Err(e)) => {
                return std::task::Poll::Ready(Err(KitsuneError::other(e)));
            }
            std::task::Poll::Ready(Ok(size)) => {
                inner.to_send.drain(..size);
            }
        }

        let res = if inner.to_send.is_empty() {
            std::task::Poll::Ready(Ok(true))
        } else {
            std::task::Poll::Ready(Ok(false))
        };

        self.0 = Some(inner);
        res
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<KitsuneResult<()>> {
        let mut inner = match self.0.take() {
            None => return std::task::Poll::Ready(Err("PreviouslyClosed".into())),
            Some(inner) => inner,
        };

        let sub = &mut inner.sub;
        tokio::pin!(sub);

        let res = match futures::io::AsyncWrite::poll_flush(sub, cx) {
            std::task::Poll::Ready(Ok(_)) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Ready(Err(e)) => {
                return std::task::Poll::Ready(Err(KitsuneError::other(e)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        };

        self.0 = Some(inner);
        res
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<KitsuneResult<()>> {
        let mut inner = match self.0.take() {
            None => return std::task::Poll::Ready(Err("PreviouslyClosed".into())),
            Some(inner) => inner,
        };

        let sub = &mut inner.sub;
        tokio::pin!(sub);

        let res = match futures::io::AsyncWrite::poll_close(sub, cx) {
            std::task::Poll::Ready(Ok(_)) => std::task::Poll::Ready(Ok(())),
            std::task::Poll::Ready(Err(e)) => {
                return std::task::Poll::Ready(Err(KitsuneError::other(e)))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        };

        self.0 = Some(inner);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msgid() {
        let req = MsgId::new(1);

        // make sure it starts out as a req
        assert!(req.is_req());
        assert!(!req.is_res());
        assert_eq!(1, req.as_id());

        // make sure as_req doesn't toggle
        let req = req.as_req();
        assert!(req.is_req());
        assert!(!req.is_res());
        assert_eq!(1, req.as_id());

        // make sure as_res works
        let res = req.as_res();
        assert!(res.is_res());
        assert!(!res.is_req());
        assert_eq!(1, res.as_id());

        // make sure as_res doesn't toggle
        let res = res.as_res();
        assert!(res.is_res());
        assert!(!res.is_req());
        assert_eq!(1, res.as_id());

        // make sure as_req works
        let req = res.as_req();
        assert!(req.is_req());
        assert!(!req.is_res());
        assert_eq!(1, req.as_id());
    }
}
