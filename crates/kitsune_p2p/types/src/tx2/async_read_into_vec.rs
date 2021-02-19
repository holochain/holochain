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
            bytes_read: 0,
            remaining_bytes_wanted: byte_count,
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
    bytes_read: usize,
    remaining_bytes_wanted: usize,
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
        let mut closed = false;
        let mut bytes_read = inner.bytes_read;
        let mut remaining_bytes_wanted = inner.remaining_bytes_wanted;

        while !inner.timeout.is_expired() {
            let rdr: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut inner.sub);
            match AsyncReadIntoVec::poll_read_into_vec(rdr, cx, inner.vec, remaining_bytes_wanted) {
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
                    bytes_read += size;
                    remaining_bytes_wanted -= size;
                    if remaining_bytes_wanted == 0 {
                        break;
                    }
                }
                std::task::Poll::Ready(Err(e)) => {
                    // do not re-set our inner, we got an error
                    return std::task::Poll::Ready(Err(e));
                }
            }
        }

        inner.bytes_read = bytes_read;
        inner.remaining_bytes_wanted = remaining_bytes_wanted;

        if !closed {
            self.0 = Some(inner);
        }

        if remaining_bytes_wanted == 0 || (closed && bytes_read > 0) {
            std::task::Poll::Ready(Ok(Some(bytes_read)))
        } else if got_pending {
            std::task::Poll::Pending
        } else if bytes_read > 0 {
            std::task::Poll::Ready(Ok(Some(bytes_read)))
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
        vec.reserve(orig_len + byte_count);

        // SAFETY:
        //   - above reserve is large enough
        //   - only initialized bytes are retained
        let read = unsafe {
            // grow our vec without slow initialization
            vec.set_len(orig_len + byte_count);

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
            vec.set_len(orig_len + read);

            read
        };

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

    async fn _inner_test(byte_count: usize) {
        let (mut send, recv) = util::bound_async_mem_channel(4096).await;
        let mut recv = AsyncReadIntoVecFilter::new(recv);

        let wt = tokio::task::spawn(async move {
            let mut data = vec![0_u8; byte_count];
            util::fill_with_latency_info(&mut data);
            use futures::io::AsyncWriteExt;
            send.write_all(&data).await.unwrap();
        });

        let mut read = Vec::new();
        recv.read_into_vec(
            &mut read,
            byte_count,
            KitsuneTimeout::from_millis(1000 * 30),
        )
        .await
        .unwrap();
        assert_eq!(read.len(), byte_count);
        println!(
            "into_vec({}) in: {} us",
            byte_count,
            util::parse_latency_info(&read).unwrap().as_micros()
        );

        wt.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_read_into_vec_filter_sm() {
        _inner_test(512).await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_read_into_vec_filter_lg() {
        _inner_test(1024 * 1024 * 8).await;
    }
}
