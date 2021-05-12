use crate::tx2::tx2_utils::*;
use crate::*;
use futures::io::{Error, ErrorKind};

/// Construct a bound async read/write memory channel
pub fn bound_async_mem_channel(
    max_bytes: usize,
    maybe_active: Option<&Active>,
) -> (
    Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
    Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
) {
    let buf = Vec::with_capacity(max_bytes);

    let inner = Arc::new(Share::new(MemInner {
        buf,
        max_bytes,
        closed: false,
        want_read_waker: None,
        want_write_waker: None,
    }));

    if let Some(active) = maybe_active {
        let k_inner = inner.clone();
        active.register_kill_cb(move || {
            let _ = k_inner.share_mut(|i, c| {
                *c = true;
                if let Some(waker) = i.want_read_waker.take() {
                    waker.wake();
                }
                if let Some(waker) = i.want_write_waker.take() {
                    waker.wake();
                }
                Ok(())
            });
        });
    }

    (Box::new(MemWrite(inner.clone())), Box::new(MemRead(inner)))
}

struct MemInner {
    buf: Vec<u8>,
    max_bytes: usize,
    closed: bool,
    want_read_waker: Option<std::task::Waker>,
    want_write_waker: Option<std::task::Waker>,
}

/// close this channel from the writer side
fn write_close(inner: &Arc<Share<MemInner>>) {
    let _ = inner.share_mut(|i, _| {
        i.closed = true;
        if let Some(waker) = i.want_read_waker.take() {
            waker.wake();
        }
        Ok(())
    });
}

/// close this channel from the reader side
fn read_close(inner: &Arc<Share<MemInner>>) {
    let _ = inner.share_mut(|i, c| {
        *c = true;
        if let Some(waker) = i.want_write_waker.take() {
            waker.wake();
        }
        Ok(())
    });
}

/// the writer side of the channel
struct MemWrite(Arc<Share<MemInner>>);

impl Drop for MemWrite {
    fn drop(&mut self) {
        write_close(&self.0);
    }
}

impl futures::io::AsyncWrite for MemWrite {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        // we cannot handle zero buffers
        if buf.is_empty() {
            return std::task::Poll::Ready(Err(Error::new(
                ErrorKind::InvalidInput,
                "AmbiguousZeroBuffer",
            )));
        }

        match self.0.share_mut(|i, _| {
            // exit early if we are already write-side closed
            if i.closed {
                return Ok(std::task::Poll::Ready(Err(Error::new(
                    ErrorKind::Other,
                    "PreviouslyClosed",
                ))));
            }

            // how much can we write to the buffer?
            let amount = std::cmp::min(
                4096, //
                std::cmp::min(
                    buf.len(),                 //
                    i.max_bytes - i.buf.len(), //
                ),
            );

            // if we cannot write, schedule a waker / return pending
            if amount == 0 {
                i.want_write_waker = Some(cx.waker().clone());
                return Ok(std::task::Poll::Pending);
            }

            // write the amout we decided
            i.buf.extend_from_slice(&buf[..amount]);

            // wake the reader side if pending
            if let Some(waker) = i.want_read_waker.take() {
                waker.wake();
            }

            Ok(std::task::Poll::Ready(Ok(amount)))
        }) {
            Err(_) => std::task::Poll::Ready(Err(Error::new(ErrorKind::Other, "PreviouslyClosed"))),
            Ok(p) => p,
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>> {
        write_close(&self.0);
        std::task::Poll::Ready(Ok(()))
    }
}

/// the reader side of the channel
struct MemRead(Arc<Share<MemInner>>);

impl Drop for MemRead {
    fn drop(&mut self) {
        read_close(&self.0);
    }
}

impl futures::io::AsyncRead for MemRead {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        // we cannot handle zero buffers
        if buf.is_empty() {
            return std::task::Poll::Ready(Err(Error::new(
                ErrorKind::InvalidInput,
                "AmbiguousZeroBuffer",
            )));
        }

        match self.0.share_mut(|i, c| {
            // if the read buffer is empty...
            if i.buf.is_empty() {
                if i.closed {
                    // if we are writer-side closed, close reader side too
                    *c = true;
                    if let Some(waker) = i.want_write_waker.take() {
                        waker.wake();
                    }
                    return Ok(std::task::Poll::Ready(Ok(0)));
                } else {
                    // otherwise record waker / return pending
                    i.want_read_waker = Some(cx.waker().clone());
                    return Ok(std::task::Poll::Pending);
                }
            }

            // determine how much we can read
            let amount = std::cmp::min(
                4096, //
                std::cmp::min(
                    buf.len(),   //
                    i.buf.len(), //
                ),
            );

            // read that amount
            buf[..amount].copy_from_slice(&i.buf[..amount]);

            if i.buf.len() > amount {
                // if there is more that "could" be read...
                // move that data to the front of our buf / truncate
                i.buf.copy_within(amount.., 0);
                let new_len = i.buf.len() - amount;
                i.buf.truncate(new_len);
            } else {
                // otherwise we can more cheaply clear the buf
                i.buf.clear()
            }

            // notify the writer that maybe more can be written
            if let Some(waker) = i.want_write_waker.take() {
                waker.wake();
            }

            Ok(std::task::Poll::Ready(Ok(amount)))
        }) {
            Err(_) => std::task::Poll::Ready(Err(Error::new(ErrorKind::Other, "PreviouslyClosed"))),
            Ok(p) => p,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn _inner_test_async_bound_mem_channel(bind_size: usize, buf_size: usize) {
        let (mut send, mut recv) = bound_async_mem_channel(bind_size, None);

        let rt = metric_task(async move {
            let mut read_buf = vec![0_u8; buf_size];
            use futures::io::AsyncReadExt;
            recv.read_exact(&mut read_buf).await.unwrap();
            println!(
                "mem_chan(bind-{},buf-{}) in: {} us",
                bind_size,
                buf_size,
                parse_latency_info(&read_buf).unwrap().as_micros()
            );
            KitsuneResult::Ok(())
        });

        use futures::io::AsyncWriteExt;
        let mut write_buf = vec![0_u8; buf_size];
        fill_with_latency_info(&mut write_buf);
        send.write_all(&write_buf).await.unwrap();

        rt.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_async_bound_mem_channel_sm_buf() {
        _inner_test_async_bound_mem_channel(15, 4096).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_async_bound_mem_channel_lg_buf() {
        _inner_test_async_bound_mem_channel(4096 * 3, 4096 * 4).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_async_bound_mem_channel_disparity() {
        _inner_test_async_bound_mem_channel(4096, 1024 * 1024 * 8).await;
    }
}
