use crate::*;
use crate::tx2::*;
use once_cell::sync::Lazy;
use futures::io::AsyncReadExt;
use futures::io::AsyncWriteExt;
use bytes::BufMut;

// TODO - expirement with these values for efficiency.
const POOL_BUF_START_CAPACITY: usize = 512;
const POOL_BUF_MAX_CAPACITY: usize = 4096;

/// A shared buffer that should be returned to the SharedBufferPool after use.
pub struct PoolBuf(Vec<u8>);

impl PoolBuf {
    /// Create a new PoolBuf.
    pub fn new() -> Self {
        Self(Vec::with_capacity(POOL_BUF_START_CAPACITY))
    }

    /// Reset this PoolBuf for further usage (shrinking if it was grown too much).
    pub fn reset(&mut self) {
        if self.0.capacity() > POOL_BUF_MAX_CAPACITY {
            self.0.truncate(POOL_BUF_MAX_CAPACITY);
            self.0.shrink_to_fit();
        }
        self.0.clear();
    }

    /// Reserve the max space in this buffer that will not be removed
    /// when released back to the pool.
    pub fn reserve_max(&mut self) {
        self.0.reserve(POOL_BUF_MAX_CAPACITY);
    }
}

impl Default for PoolBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for PoolBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for PoolBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<[u8]> for PoolBuf {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<Vec<u8>> for PoolBuf {
    fn as_mut(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }
}

/// A global static SharedBufferPool.
pub static BUF_POOL: Lazy<SharedBufferPool> = Lazy::new(|| SharedBufferPool::new());

/// A pool of shared buffers to avoid constant re-allocation.
pub struct SharedBufferPool(AsyncOwnedResourceBucket<PoolBuf>);

impl Default for SharedBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedBufferPool {
    /// Construct a new SharedBufferPool
    /// ¿ but, maybe you want to use the global static `BUF_POOL` ?
    pub fn new() -> Self {
        Self(AsyncOwnedResourceBucket::new(None))
    }

    /// Release a previously acquired buf to the pool.
    pub fn release(&self, mut buf: PoolBuf) -> impl std::future::Future<Output = ()> + 'static + Send {
        buf.reset();
        self.0.release(buf)
    }

    /// Acquire a buf from the pool.
    pub fn acquire(&self) -> impl std::future::Future<Output = PoolBuf> + 'static + Send {
        self.0.acquire_or_else(|| PoolBuf::new())
    }
}

/// Efficiently read framed data from a sub AsyncRead instance.
pub struct FramedReader {
    sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
    local_buf: [u8; POOL_BUF_MAX_CAPACITY],
}

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
    pub fn new(
        sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
    ) -> Self {
        Self {
            sub,
            local_buf: [0; POOL_BUF_MAX_CAPACITY],
        }
    }

    /// Read a frame of data from this FramedReader instance.
    pub async fn read(
        &mut self,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<RR> {
        timeout.mix(async {
            // TODO - starting with a naive impl here, see if it performs

            let mut read = 0;

            while read < 4 + 8 {
                read += self
                    .sub
                    .read(&mut self.local_buf[read..4+8])
                    .await
                    .map_err(KitsuneError::other)?;
            }

            let want_size = read_size(&self.local_buf[..4]) - 4 - 8;
            let msg_id = read_msg_id(&self.local_buf[4..4+8]);

            let mut buf = BUF_POOL.acquire().await;
            buf.as_mut().reserve(want_size);

            while buf.len() < want_size {
                let to_read = std::cmp::min(
                    self.local_buf.len(),
                    want_size - buf.len(),
                );
                read = self
                    .sub
                    .read(&mut self.local_buf[..to_read])
                    .await
                    .map_err(KitsuneError::other)?;
                buf.as_mut().put(&self.local_buf[..read]);
            }

            Ok(vec![(msg_id, buf)])
        }).await
    }
}

/// Efficiently write framed data to a sub AsyncWrite instance.
pub struct FramedWriter {
    sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
}

impl FramedWriter {
    /// Create a new FramedWriter instance.
    pub fn new(
        sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
    ) -> Self {
        Self {
            sub,
        }
    }

    /// Write a frame of data to this FramedWriter instance.
    pub async fn write(
        &mut self,
        msg_id: MsgId,
        data: &[u8],
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        timeout.mix(async {
            let total: u32 = data.len() as u32 + 4 /* len */ + 8 /* msg_id */;

            // if the size of data to be written is small,
            // it'll be more efficient to combine it into one buffer first
            // TODO - use a different value than POOL_BUF_MAX_CAPACITY?
            let combine = (total as usize) < POOL_BUF_MAX_CAPACITY;

            if combine {
                let mut buf = BUF_POOL.acquire().await;
                buf.as_mut().reserve(total as usize);
                buf.as_mut().put(&total.to_le_bytes()[..]);
                buf.as_mut().put(&msg_id.inner().to_le_bytes()[..]);
                buf.as_mut().put(data);
                self
                    .sub
                    .write_all(&buf)
                    .await
                    .map_err(KitsuneError::other)?;
                BUF_POOL.release(buf).await;
            } else {
                let mut buf = [0_u8; 4 + 8];
                buf[..4].copy_from_slice(&total.to_le_bytes());
                buf[4..].copy_from_slice(&msg_id.inner().to_le_bytes());
                self
                    .sub
                    .write_all(&buf)
                    .await
                    .map_err(KitsuneError::other)?;
                self
                    .sub
                    .write_all(&data)
                    .await
                    .map_err(KitsuneError::other)?;
            }

            Ok(())
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn play() {
        let (send, recv) = util::bound_async_mem_channel(4096);
        let mut send = FramedWriter::new(send);
        let mut recv = FramedReader::new(recv);

        let wt = tokio::task::spawn(async move {
            send.write(
                1.into(),
                &[0xd0; 512],
                KitsuneTimeout::from_millis(1000 * 30),
            ).await.unwrap();
            send.write(
                2.into(),
                &[0xd1; 8000],
                KitsuneTimeout::from_millis(1000 * 30),
            ).await.unwrap();
        });

        for _ in 0..2 {
            for (msg_id, data) in recv.read(KitsuneTimeout::from_millis(1000 * 30)).await.unwrap() {
                println!("got {} - {} bytes", msg_id.as_id(), data.len());
            }
        }

        wt.await.unwrap();
    }
}
