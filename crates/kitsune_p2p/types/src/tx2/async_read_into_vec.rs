use crate::*;

/// Type that AsyncReads data into a pre-existing Vec<u8> (growing the vec).
pub trait AsyncReadIntoVec: 'static + Send + Unpin {
    /// low-level poll for reading into a vec.
    fn poll_read_into_vec(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        vec: &mut Vec<u8>,
        byte_count: usize,
    ) -> std::task::Poll<KitsuneResult<Option<usize>>>;
}

/// Extension trait providing higher-level access API.
pub trait AsyncReadIntoVecExt: AsyncReadIntoVec {
    /// high-level async read data into a vec.
    fn read_into_vec<'a>(
        &'a mut self,
        vec: &'a mut Vec<u8>,
        byte_count: usize,
        timeout: KitsuneTimeout,
    ) -> AsyncReadIntoVecFut<'a, Self> {
        let this = std::pin::Pin::new(&mut *self);
        AsyncReadIntoVecFut(Some(AsyncReadIntoVecFutInner {
            sub: this,
            vec,
            byte_count,
            timeout,
        }))
    }
}

impl<A: AsyncReadIntoVec> AsyncReadIntoVecExt for A {}

struct AsyncReadIntoVecFutInner<'a, P>
where
    P: ?Sized + AsyncReadIntoVec,
{
    sub: std::pin::Pin<&'a mut P>,
    vec: &'a mut Vec<u8>,
    byte_count: usize,
    timeout: KitsuneTimeout,
}

/// Future returned from `AsyncReadIntoVec::read_into_vec()`.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct AsyncReadIntoVecFut<'a, P>(Option<AsyncReadIntoVecFutInner<'a, P>>)
where
    P: ?Sized + AsyncReadIntoVec;

impl<'a, P> std::future::Future for AsyncReadIntoVecFut<'a, P>
where
    P: ?Sized + AsyncReadIntoVec,
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
        let mut read = 0;
        let mut closed = false;

        while !inner.timeout.is_expired() {
            let rdr: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut inner.sub);
            match AsyncReadIntoVec::poll_read_into_vec(rdr, cx, inner.vec, inner.byte_count - read)
            {
                std::task::Poll::Pending => {
                    got_pending = true;
                    break;
                }
                std::task::Poll::Ready(Ok(None)) => {
                    closed = true;
                    break;
                }
                std::task::Poll::Ready(Ok(Some(size))) => {
                    if size == 0 {
                        unreachable!();
                    }
                    read += size;
                    if read >= inner.byte_count {
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

        if read > 0 {
            std::task::Poll::Ready(Ok(Some(read)))
        } else if got_pending {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(Ok(None))
        }
    }
}

type AR = Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>;

/// A filter allowing AsyncReadIntoVec.
pub struct AsyncReadIntoVecFilter(Option<AR>);

impl AsyncReadIntoVecFilter {
    /// Create a new AsyncReadIntoVecFilter instance.
    pub fn new(i: AR) -> Self {
        Self(Some(i))
    }
}

impl AsyncReadIntoVec for AsyncReadIntoVecFilter {
    fn poll_read_into_vec(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        vec: &mut Vec<u8>,
        byte_count: usize,
    ) -> std::task::Poll<KitsuneResult<Option<usize>>> {
        let mut inner = match self.0.take() {
            None => return std::task::Poll::Ready(Ok(None)),
            Some(inner) => inner,
        };

        let mut got_pending = false;
        let mut error = None;

        // allocate enough space for byte_count
        let orig_len = vec.len();
        vec.resize(orig_len + byte_count, 0);

        let read = {
            let vec = &mut vec[orig_len..orig_len + byte_count];

            let sub = &mut inner;
            tokio::pin!(sub);

            // poll our sub future
            match futures::io::AsyncRead::poll_read(sub, cx, vec) {
                std::task::Poll::Pending => {
                    got_pending = true;
                    0
                }
                std::task::Poll::Ready(Err(e)) => {
                    error = Some(KitsuneError::other(e));
                    0
                }
                std::task::Poll::Ready(Ok(size)) => size,
            }
        };

        // shrink our vec to the size we actually read
        vec.resize(orig_len + read, 0);

        if let Some(e) = error {
            std::task::Poll::Ready(Err(e))
        } else if read == 0 && got_pending {
            self.0 = Some(inner);
            std::task::Poll::Pending
        } else if read == 0 {
            std::task::Poll::Ready(Ok(None))
        } else {
            self.0 = Some(inner);
            std::task::Poll::Ready(Ok(Some(read)))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tx2::*;
    use crate::*;

    struct FakeRead;

    impl futures::io::AsyncRead for FakeRead {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> std::task::Poll<Result<usize, futures::io::Error>> {
            util::fill_with_latency_info(buf);
            std::task::Poll::Ready(Ok(buf.len()))
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_read_into_vec_filter() {
        let timeout = KitsuneTimeout::from_millis(1000);
        let mut r = AsyncReadIntoVecFilter::new(Box::new(FakeRead));
        let mut v = Vec::new();
        r.read_into_vec(&mut v, 32, timeout).await.unwrap();
        assert!(util::parse_latency_info(v.as_slice()).is_ok());
        r.read_into_vec(&mut v, 10000, timeout).await.unwrap();
        assert_eq!(10032, v.len());
    }
}
