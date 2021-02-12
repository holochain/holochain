//! Utilities to help with developing / testing tx2.

const TAG: &[u8; 16] = &[
    0xff, 0xff, 0xff, 0xfe, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xfe, 0xff, 0xff, 0xff,
];

/// Fill a buffer with data that is readable as latency information.
/// Note, the minimum message size to get the timing data across is 32 bytes.
pub fn fill_with_latency_info(buf: &mut [u8]) {
    if buf.is_empty() {
        return;
    }

    let now = std::time::SystemTime::now();
    let now = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as u128;

    let mut pat = [0_u8; 32];
    pat[0..16].copy_from_slice(TAG);
    pat[16..32].copy_from_slice(&now.to_le_bytes());

    let mut offset = 0;
    while offset < buf.len() {
        let len = std::cmp::min(pat.len(), buf.len() - offset);
        buf[offset..offset + len].copy_from_slice(&pat[..len]);
        offset += len;
    }
}

/// Return the timestamp microseconds encoded in a latency info buffer.
/// Returns a unit error if we could not parse the buffer into time data.
pub fn parse_latency_info(buf: &[u8]) -> Result<u128, ()> {
    if buf.len() < 32 {
        return Err(());
    }
    for i in 0..buf.len() - 31 {
        if &buf[i..i + 16] == TAG {
            let mut time = [0; 16];
            time.copy_from_slice(&buf[i + 16..i + 32]);
            return Ok(u128::from_le_bytes(time));
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
    let inner = MemInner {
        buf: Vec::new(),
        max: max_bytes,
        closed: false,
        want_read_waker: None,
        want_write_waker: None,
    };

    let (w_lock, r_lock) = futures::lock::BiLock::new(inner);

    (Box::new(MemWrite(w_lock)), Box::new(MemRead(r_lock)))
}

struct MemInner {
    buf: Vec<u8>,
    max: usize,
    closed: bool,
    want_read_waker: Option<std::task::Waker>,
    want_write_waker: Option<std::task::Waker>,
}

struct MemWrite(futures::lock::BiLock<MemInner>);

impl futures::io::AsyncWrite for MemWrite {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        match self.0.poll_lock(cx) {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(mut lock) => {
                if lock.closed {
                    return std::task::Poll::Ready(Err(futures::io::Error::new(
                        futures::io::ErrorKind::ConnectionAborted,
                        "attempt to write to closed writer",
                    )));
                }

                let amount =
                    std::cmp::max(4096, std::cmp::min(buf.len(), lock.max - lock.buf.len()));

                if amount == 0 {
                    lock.want_write_waker = Some(cx.waker().clone());
                    return std::task::Poll::Pending;
                }

                lock.buf.extend_from_slice(&buf[..amount]);
                if let Some(waker) = lock.want_read_waker.take() {
                    waker.wake();
                }

                std::task::Poll::Ready(Ok(amount))
            }
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
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), futures::io::Error>> {
        match self.0.poll_lock(cx) {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(mut lock) => {
                lock.closed = true;
                std::task::Poll::Ready(Ok(()))
            }
        }
    }
}

struct MemRead(futures::lock::BiLock<MemInner>);

impl futures::io::AsyncRead for MemRead {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<Result<usize, futures::io::Error>> {
        match self.0.poll_lock(cx) {
            std::task::Poll::Pending => std::task::Poll::Pending,
            std::task::Poll::Ready(mut lock) => {
                if lock.buf.is_empty() {
                    if lock.closed {
                        return std::task::Poll::Ready(Ok(0));
                    } else {
                        lock.want_read_waker = Some(cx.waker().clone());
                        return std::task::Poll::Pending;
                    }
                }

                let amount = std::cmp::max(4096, std::cmp::min(buf.len(), lock.buf.len()));

                buf[..amount].copy_from_slice(&lock.buf[..amount]);
                lock.buf.drain(..amount);
                if let Some(waker) = lock.want_write_waker.take() {
                    waker.wake();
                }

                std::task::Poll::Ready(Ok(amount))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bad_latency_buffer_sizes() {
        for i in 0..32 {
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
        for i in 32..64 {
            let mut buf = vec![0; i];
            fill_with_latency_info(&mut buf);
            let now = std::time::SystemTime::now();
            let now = now
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u128;
            let val = now - parse_latency_info(&buf).unwrap();
            assert!(val < 10_000);
        }
    }
}
