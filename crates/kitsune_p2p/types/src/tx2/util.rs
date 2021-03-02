#![allow(clippy::never_loop)] // using for block breaking
//! Utilities to help with developing / testing tx2.

use crate::tx2::*;
use futures::io::{Error, ErrorKind};
use once_cell::sync::Lazy;

mod active;
pub use active::*;

mod async_map;
pub use async_map::*;

mod logic_chan;
pub use logic_chan::*;

mod tx_url;
pub use tx_url::*;

mod share;
pub use share::*;

static LOC_EPOCH: Lazy<std::time::Instant> = Lazy::new(std::time::Instant::now);
const LAT_TAG: &[u8; 8] = &[0xff, 0xff, 0xff, 0xfe, 0xfe, 0xff, 0xff, 0xff];

/// Fill a buffer with data that is readable as latency information.
/// Note, the minimum message size to get the timing data across is 16 bytes.
pub fn fill_with_latency_info(buf: &mut [u8]) {
    if buf.is_empty() {
        return;
    }

    let epoch = *LOC_EPOCH;

    let now = std::time::Instant::now();
    let now = now.duration_since(epoch).as_secs_f64();

    let mut pat = [0_u8; 16];
    pat[0..8].copy_from_slice(LAT_TAG);
    pat[8..16].copy_from_slice(&now.to_le_bytes());

    let mut offset = 0;
    while offset < buf.len() {
        let len = std::cmp::min(pat.len(), buf.len() - offset);
        buf[offset..offset + len].copy_from_slice(&pat[..len]);
        offset += len;
    }
}

/// Return the duration since the time encoded in a latency info buffer.
/// Returns a unit error if we could not parse the buffer into time data.
pub fn parse_latency_info(buf: &[u8]) -> Result<std::time::Duration, ()> {
    if buf.len() < 16 {
        return Err(());
    }
    for i in 0..buf.len() - 15 {
        if &buf[i..i + 8] == LAT_TAG {
            let mut time = [0; 8];
            time.copy_from_slice(&buf[i + 8..i + 16]);
            let time = f64::from_le_bytes(time);
            let now = std::time::Instant::now();
            let now = now.duration_since(*LOC_EPOCH).as_secs_f64();
            let time = std::time::Duration::from_secs_f64(now - time);
            return Ok(time);
        }
    }
    Err(())
}

/// Construct a bound async read/write memory channel
pub fn bound_async_mem_channel(
    max_bytes: usize,
) -> (
    Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
    Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
) {
    let mut buf = PoolBuf::new();
    buf.reserve(max_bytes);

    let inner = MemInner {
        buf,
        max_bytes,
        closed: false,
        want_read_waker: None,
        want_write_waker: None,
    };

    let (w_lock, r_lock) = futures::lock::BiLock::new(inner);

    (
        Box::new(MemWrite(Some(w_lock))),
        Box::new(MemRead(Some(r_lock))),
    )
}

struct MemInner {
    buf: PoolBuf,
    max_bytes: usize,
    closed: bool,
    want_read_waker: Option<std::task::Waker>,
    want_write_waker: Option<std::task::Waker>,
}

struct MemWrite(Option<futures::lock::BiLock<MemInner>>);

impl Drop for MemWrite {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            tokio::task::spawn(async move {
                let mut lock = inner.lock().await;
                lock.closed = true;
                if let Some(waker) = lock.want_read_waker.take() {
                    waker.wake();
                }
            });
        }
    }
}

impl futures::io::AsyncWrite for MemWrite {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        if buf.is_empty() {
            return std::task::Poll::Ready(Err(Error::new(
                ErrorKind::InvalidInput,
                "AmbiguousZeroBuffer",
            )));
        }
        let inner = match self.0.take() {
            None => {
                return std::task::Poll::Ready(Err(Error::new(
                    ErrorKind::Other,
                    "PreviouslyClosed",
                )))
            }
            Some(inner) => inner,
        };
        let res = 'res: loop {
            match inner.poll_lock(cx) {
                std::task::Poll::Pending => break 'res std::task::Poll::Pending,
                std::task::Poll::Ready(mut lock) => {
                    if lock.closed {
                        // reader side closed, shutdown
                        return std::task::Poll::Ready(Err(Error::new(
                            ErrorKind::ConnectionAborted,
                            "ReaderClosed",
                        )));
                    }

                    let amount = std::cmp::min(
                        4096, //
                        std::cmp::min(
                            buf.len(),                       //
                            lock.max_bytes - lock.buf.len(), //
                        ),
                    );

                    if amount == 0 {
                        lock.want_write_waker = Some(cx.waker().clone());
                        break 'res std::task::Poll::Pending;
                    }

                    lock.buf.extend_from_slice(&buf[..amount]);
                    if let Some(waker) = lock.want_read_waker.take() {
                        waker.wake();
                    }

                    break 'res std::task::Poll::Ready(Ok(amount));
                }
            }
        };
        self.0 = Some(inner);
        res
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>> {
        if self.0.is_none() {
            return std::task::Poll::Ready(Err(Error::new(ErrorKind::Other, "PreviouslyClosed")));
        }
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>> {
        let inner = match self.0.take() {
            None => {
                return std::task::Poll::Ready(Err(Error::new(
                    ErrorKind::Other,
                    "PreviouslyClosed",
                )))
            }
            Some(inner) => inner,
        };
        let mut closed = false;
        let res = 'res: loop {
            match inner.poll_lock(cx) {
                std::task::Poll::Pending => break 'res std::task::Poll::Pending,
                std::task::Poll::Ready(mut lock) => {
                    lock.closed = true;
                    closed = true;
                    if let Some(waker) = lock.want_read_waker.take() {
                        waker.wake();
                    }
                    break 'res std::task::Poll::Ready(Ok(()));
                }
            }
        };
        if !closed {
            self.0 = Some(inner);
        }
        res
    }
}

struct MemRead(Option<futures::lock::BiLock<MemInner>>);

impl Drop for MemRead {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            tokio::task::spawn(async move {
                let mut lock = inner.lock().await;
                lock.closed = true;
                if let Some(waker) = lock.want_write_waker.take() {
                    waker.wake();
                }
            });
        }
    }
}

impl futures::io::AsyncRead for MemRead {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        if buf.is_empty() {
            return std::task::Poll::Ready(Err(Error::new(
                ErrorKind::InvalidInput,
                "AmbiguousZeroBuffer",
            )));
        }
        let inner = match self.0.take() {
            None => {
                return std::task::Poll::Ready(Err(Error::new(
                    ErrorKind::Other,
                    "PreviouslyClosed",
                )))
            }
            Some(inner) => inner,
        };
        let mut closed = false;
        let res = 'res: loop {
            match inner.poll_lock(cx) {
                std::task::Poll::Pending => break 'res std::task::Poll::Pending,
                std::task::Poll::Ready(mut lock) => {
                    if lock.buf.is_empty() {
                        if lock.closed {
                            closed = true;
                            break 'res std::task::Poll::Ready(Ok(0));
                        } else {
                            lock.want_read_waker = Some(cx.waker().clone());
                            break 'res std::task::Poll::Pending;
                        }
                    }

                    let amount = std::cmp::min(
                        4096, //
                        std::cmp::min(
                            buf.len(),      //
                            lock.buf.len(), //
                        ),
                    );

                    buf[..amount].copy_from_slice(&lock.buf[..amount]);
                    lock.buf.truncate_front(amount);
                    if let Some(waker) = lock.want_write_waker.take() {
                        waker.wake();
                    }

                    break 'res std::task::Poll::Ready(Ok(amount));
                }
            }
        };
        if !closed {
            self.0 = Some(inner);
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_latency_buffer_sizes() {
        for i in 0..16 {
            let mut buf = vec![0; i];
            fill_with_latency_info(&mut buf);
            assert!(parse_latency_info(&buf).is_err());
        }
    }

    #[test]
    fn test_bad_latency_buffer_data() {
        assert!(parse_latency_info(&[0; 64]).is_err());
    }

    #[test]
    fn test_good_latency_buffers() {
        for i in 16..64 {
            let mut buf = vec![0; i];
            fill_with_latency_info(&mut buf);
            let val = parse_latency_info(&buf).unwrap();
            assert!(val.as_micros() < 10_000);
        }
    }

    async fn _inner_test_async_bound_mem_channel(bind_size: usize, buf_size: usize) {
        let (mut send, mut recv) = bound_async_mem_channel(bind_size);

        let rt = tokio::task::spawn(async move {
            let mut read_buf = vec![0_u8; buf_size];
            use futures::io::AsyncReadExt;
            recv.read_exact(&mut read_buf).await.unwrap();
            println!(
                "mem_chan(bind-{},buf-{}) in: {} us",
                bind_size,
                buf_size,
                parse_latency_info(&read_buf).unwrap().as_micros()
            );
        });

        use futures::io::AsyncWriteExt;
        let mut write_buf = vec![0_u8; buf_size];
        fill_with_latency_info(&mut write_buf);
        send.write_all(&write_buf).await.unwrap();

        rt.await.unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_bound_mem_channel_sm_buf() {
        _inner_test_async_bound_mem_channel(15, 4096).await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_bound_mem_channel_lg_buf() {
        _inner_test_async_bound_mem_channel(4096 * 3, 4096 * 4).await;
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_async_bound_mem_channel_disparity() {
        _inner_test_async_bound_mem_channel(4096, 1024 * 1024 * 8).await;
    }
}
