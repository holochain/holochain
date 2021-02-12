use crate::tx2::*;

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

type RR = Result<Vec<(MsgId, Box<[u8]>)>, futures::io::Error>;

/// Read Frames one at a time from an async source.
pub trait AsyncReadFramed: 'static + Send + Unpin {
    /// low-level poll for reading a frame.
    fn poll_read_framed(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<RR>;
}

/// Extension trait providing higher-level access API.
pub trait AsyncReadFramedExt: AsyncReadFramed {
    /// high-level async read frames fn.
    fn read_frame(&mut self) -> AsyncReadFramedFut<'_, Self> {
        let this = std::pin::Pin::new(&mut *self);
        AsyncReadFramedFut(this)
    }
}

impl<A: AsyncReadFramed> AsyncReadFramedExt for A {}

/// Future returned from `AsyncReadFramed::read_framed()`.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AsyncReadFramedFut<'a, P>(std::pin::Pin<&'a mut P>)
where
    P: ?Sized + AsyncReadFramed;

impl<'a, P> std::future::Future for AsyncReadFramedFut<'a, P>
where
    P: ?Sized + AsyncReadFramed,
{
    type Output = RR;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = &mut *self;
        let rdr: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut this.0);
        AsyncReadFramed::poll_read_framed(rdr, cx)
    }
}

/// A filter allowing AsyncReadFramed.
pub struct AsyncReadFramedFilter {
    sub: Box<dyn AsyncReadIntoVec>,
    buf: Option<Vec<u8>>,
}

impl AsyncReadFramedFilter {
    /// Create a new AsyncReadFramedFilter instance.
    pub fn new(sub: Box<dyn AsyncReadIntoVec>) -> Self {
        Self { sub, buf: None }
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
    ) -> std::task::Poll<RR> {
        let mut buf = self.buf.take().unwrap_or_else(|| Vec::with_capacity(4096));
        let mut out = Vec::new();

        // currently, technically, the below loop cannot break
        // without setting got_pending to true...
        // but keeping this check here incase refactors in the future
        // might break this
        #[allow(unused_assignments)]
        let mut got_pending = false;

        loop {
            let max_bytes = if buf.len() < 4 {
                4096
            } else {
                std::cmp::max(4096, read_size(&buf))
            };

            let sub: &mut dyn AsyncReadIntoVec = &mut *self.sub;
            let sub: std::pin::Pin<&mut dyn AsyncReadIntoVec> = std::pin::Pin::new(sub);

            match AsyncReadIntoVec::poll_read_into_vec(sub, cx, &mut buf, max_bytes) {
                std::task::Poll::Pending => {
                    got_pending = true;
                    break;
                }
                std::task::Poll::Ready(Err(e)) => {
                    return std::task::Poll::Ready(Err(e));
                }
                std::task::Poll::Ready(Ok(_size)) => (),
            }

            while buf.len() >= 4 + 8 {
                let want_size = read_size(&buf);
                if buf.len() < want_size {
                    break;
                }
                let msg_id = read_msg_id(&buf);
                let mut data = buf.drain(..want_size).collect::<Vec<_>>();
                data.drain(..12);
                out.push((msg_id, data.into_boxed_slice()));
            }
        }
        self.buf = Some(buf);
        if got_pending && out.is_empty() {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(Ok(out))
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
    fn push_frame(self: std::pin::Pin<&mut Self>, msg_id: MsgId, buf: &[u8]) -> bool;

    /// low-level poll for writing framed data.
    /// Call `push_frame` first to enqueue data for sending.
    /// `false` indicates there is still more data to write.
    /// `true` indicates all data has been sent, ready for new `push_frame()`.
    fn poll_write_framed(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<bool, futures::io::Error>>;

    /// delegates to the underlying stream `poll_flush`.
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>>;

    /// delegates to the underlying stream `poll_close`.
    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>>;
}

/// Extension trait providing higher-level access API.
pub trait AsyncWriteFramedExt: AsyncWriteFramed {
    /// high-level async write frames fn.
    fn write_frame<'a>(
        &'a mut self,
        msg_id: MsgId,
        buf: &'a [u8],
    ) -> AsyncWriteFramedFut<'a, Self> {
        let this = std::pin::Pin::new(&mut *self);
        AsyncWriteFramedFut {
            stream: this,
            msg_id,
            buf,
            is_pre_push: true,
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
}

impl<'a, P> std::future::Future for AsyncWriteFramedFut<'a, P>
where
    P: ?Sized + AsyncWriteFramed,
{
    type Output = Result<(), futures::io::Error>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = &mut *self;

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
                AsyncWriteFramed::push_frame(stream, this.msg_id, this.buf);
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

/// A filter that will frame outgoing async writes.
pub struct AsyncWriteFramedFilter {
    sub: AW,
    to_send: Option<Vec<u8>>,
    did_err: bool,
}

impl AsyncWriteFramedFilter {
    /// Create a new AsyncWriteFramedFilter instance.
    pub fn new(sub: AW) -> Self {
        Self {
            sub,
            to_send: Some(Vec::with_capacity(4096)),
            did_err: false,
        }
    }
}

impl AsyncWriteFramed for AsyncWriteFramedFilter {
    fn push_frame(mut self: std::pin::Pin<&mut Self>, msg_id: MsgId, buf: &[u8]) -> bool {
        if !self.to_send.as_ref().unwrap().is_empty() {
            return false;
        }
        let size: u32 = buf.len() as u32 + 4 + 8;
        self.to_send
            .as_mut()
            .unwrap()
            .extend_from_slice(&size.to_le_bytes());
        self.to_send
            .as_mut()
            .unwrap()
            .extend_from_slice(&msg_id.inner().to_le_bytes());
        self.to_send.as_mut().unwrap().extend_from_slice(buf);
        true
    }

    fn poll_write_framed(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<bool, futures::io::Error>> {
        if self.did_err {
            // TODO - fix me
            panic!();
        }

        let mut to_send = self.to_send.take().unwrap();
        loop {
            let sub = &mut self.sub;
            tokio::pin!(sub);
            match futures::io::AsyncWrite::poll_write(sub, cx, &to_send) {
                std::task::Poll::Pending => break,
                std::task::Poll::Ready(Err(e)) => {
                    self.did_err = true;
                    return std::task::Poll::Ready(Err(e));
                }
                std::task::Poll::Ready(Ok(size)) => {
                    to_send.drain(..size);
                    if to_send.is_empty() {
                        break;
                    }
                }
            }
        }
        self.to_send = Some(to_send);
        if self.to_send.as_ref().unwrap().is_empty() {
            std::task::Poll::Ready(Ok(true))
        } else {
            std::task::Poll::Ready(Ok(false))
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>> {
        let sub = &mut self.sub;
        tokio::pin!(sub);
        futures::io::AsyncWrite::poll_flush(sub, cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>> {
        let sub = &mut self.sub;
        tokio::pin!(sub);
        futures::io::AsyncWrite::poll_close(sub, cx)
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
