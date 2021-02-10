/// Type that AsyncReads data into a pre-existing Vec<u8> (growing the vec).
pub trait AsyncReadIntoVec: 'static + Send + Unpin {
    /// low-level poll for reading into a vec.
    fn poll_read_into_vec(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        vec: &mut Vec<u8>,
        max_bytes: usize,
    ) -> std::task::Poll<Result<usize, futures::io::Error>>;

    /// high-level async read data into a vec.
    fn read_into_vec<'a>(
        &'a mut self,
        vec: &'a mut Vec<u8>,
        max_bytes: usize,
    ) -> futures::future::BoxFuture<'a, Result<usize, futures::io::Error>> {
        struct Read<'a, P>(std::pin::Pin<&'a mut P>, &'a mut Vec<u8>, usize)
        where
            P: ?Sized + AsyncReadIntoVec;

        impl<'a, P> std::future::Future for Read<'a, P>
        where
            P: ?Sized + AsyncReadIntoVec,
        {
            type Output = Result<usize, futures::io::Error>;

            fn poll(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Self::Output> {
                let this = &mut *self;
                let rdr: std::pin::Pin<&mut P> = std::pin::Pin::new(&mut this.0);
                AsyncReadIntoVec::poll_read_into_vec(rdr, cx, this.1, this.2)
            }
        }

        let this = std::pin::Pin::new(&mut *self);
        let read: Read<'a, Self> = Read(this, vec, max_bytes);
        futures::future::FutureExt::boxed(read)
    }
}

type AR = Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>;

/// A filter allowing AsyncReadIntoVec.
pub struct AsyncReadIntoVecFilter(AR, Option<[u8; 4096]>);

impl AsyncReadIntoVecFilter {
    /// Create a new AsyncReadIntoVecFilter instance.
    pub fn new(i: AR) -> Self {
        Self(i, None)
    }
}

impl AsyncReadIntoVec for AsyncReadIntoVecFilter {
    fn poll_read_into_vec(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        vec: &mut Vec<u8>,
        max_bytes: usize,
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        let mut got_pending = false;
        let mut read = 0;

        // grab our buffer
        let mut buf = self.1.take().unwrap_or_else(|| [0; 4096]);

        // loop to gather up to our max_bytes
        loop {
            // calculate how many more bytes to read
            let want_read = std::cmp::min(max_bytes - read, 4096);
            if want_read < 1 {
                // if we don't want to read anything, break out
                break;
            }

            // size our buffer appropriately
            let buf = &mut buf[0..want_read];

            let sub = &mut self.0;
            tokio::pin!(sub);

            // poll our sub future
            match futures::io::AsyncRead::poll_read(sub, cx, buf) {
                std::task::Poll::Pending => {
                    got_pending = true;
                    break;
                }
                std::task::Poll::Ready(Err(e)) => {
                    return std::task::Poll::Ready(Err(e));
                }
                std::task::Poll::Ready(Ok(size)) => {
                    read += size;
                    vec.extend_from_slice(&buf[0..size]);
                }
            }
        }

        // restore our buffer
        self.1 = Some(buf);

        if read == 0 && got_pending {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(Ok(read))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeRead;

    impl futures::io::AsyncRead for FakeRead {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> std::task::Poll<Result<usize, futures::io::Error>> {
            static DATA: &'static [u8; 4096] = &[0xdb; 4096];
            let mut offset = 0;
            while offset < buf.len() {
                let len = std::cmp::min(4096, buf.len() - offset);
                buf[offset..offset + len].copy_from_slice(&DATA[0..len]);
                offset += len;
            }
            std::task::Poll::Ready(Ok(buf.len()))
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_read_into_vec_filter() {
        let mut r = AsyncReadIntoVecFilter::new(Box::new(FakeRead));
        let mut v = Vec::new();
        r.read_into_vec(&mut v, 1).await.unwrap();
        assert_eq!(&[0xdb], v.as_slice());
        r.read_into_vec(&mut v, 10000).await.unwrap();
        for i in v.iter() {
            assert_eq!(0xdb, *i);
        }
    }
}
