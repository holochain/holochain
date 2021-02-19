use crate::tx2::*;
use crate::*;
use futures::io::AsyncReadExt;
use futures::io::AsyncWriteExt;

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
    pub fn new(sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>) -> Self {
        Self {
            sub,
            local_buf: [0; POOL_BUF_MAX_CAPACITY],
        }
    }

    /// Read a frame of data from this FramedReader instance.
    pub async fn read(&mut self, timeout: KitsuneTimeout) -> KitsuneResult<RR> {
        timeout
            .mix(async {
                // TODO - starting with a naive impl here, see if it performs

                let mut read = 0;

                while read < 4 + 8 {
                    read += self
                        .sub
                        .read(&mut self.local_buf[read..4 + 8])
                        .await
                        .map_err(KitsuneError::other)?;
                }

                let want_size = read_size(&self.local_buf[..4]) - 4 - 8;
                let msg_id = read_msg_id(&self.local_buf[4..4 + 8]);

                let mut buf = BUF_POOL.acquire().await;
                buf.reserve(want_size);

                while buf.len() < want_size {
                    let to_read = std::cmp::min(self.local_buf.len(), want_size - buf.len());
                    read = self
                        .sub
                        .read(&mut self.local_buf[..to_read])
                        .await
                        .map_err(KitsuneError::other)?;
                    buf.extend_from_slice(&self.local_buf[..read]);
                }

                Ok(vec![(msg_id, buf)])
            })
            .await
    }
}

/// Efficiently write framed data to a sub AsyncWrite instance.
pub struct FramedWriter {
    sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
}

impl FramedWriter {
    /// Create a new FramedWriter instance.
    pub fn new(sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>) -> Self {
        Self { sub }
    }

    /// Write a frame of data to this FramedWriter instance.
    pub async fn write(
        &mut self,
        msg_id: MsgId,
        data: &[u8],
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        timeout
            .mix(async {
                let total: u32 = data.len() as u32 + 4 /* len */ + 8 /* msg_id */;

                // if the size of data to be written is small,
                // it'll be more efficient to combine it into one buffer first
                // TODO - use a different value than POOL_BUF_MAX_CAPACITY?
                let combine = (total as usize) < POOL_BUF_MAX_CAPACITY;

                if combine {
                    let mut buf = BUF_POOL.acquire().await;
                    buf.reserve(total as usize);
                    buf.extend_from_slice(&total.to_le_bytes()[..]);
                    buf.extend_from_slice(&msg_id.inner().to_le_bytes()[..]);
                    buf.extend_from_slice(data);
                    self.sub
                        .write_all(&buf)
                        .await
                        .map_err(KitsuneError::other)?;
                    BUF_POOL.release(buf).await;
                } else {
                    let mut buf = [0_u8; 4 + 8];
                    buf[..4].copy_from_slice(&total.to_le_bytes());
                    buf[4..].copy_from_slice(&msg_id.inner().to_le_bytes());
                    self.sub
                        .write_all(&buf)
                        .await
                        .map_err(KitsuneError::other)?;
                    self.sub
                        .write_all(&data)
                        .await
                        .map_err(KitsuneError::other)?;
                }

                Ok(())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn play() {
        let (send, recv) = util::bound_async_mem_channel(4096).await;
        let mut send = FramedWriter::new(send);
        let mut recv = FramedReader::new(recv);

        let wt = tokio::task::spawn(async move {
            send.write(
                1.into(),
                &[0xd0; 512],
                KitsuneTimeout::from_millis(1000 * 30),
            )
            .await
            .unwrap();
            send.write(
                2.into(),
                &[0xd1; 8000],
                KitsuneTimeout::from_millis(1000 * 30),
            )
            .await
            .unwrap();
        });

        for _ in 0..2 {
            for (msg_id, data) in recv
                .read(KitsuneTimeout::from_millis(1000 * 30))
                .await
                .unwrap()
            {
                println!("got {} - {} bytes", msg_id.as_id(), data.len());
            }
        }

        wt.await.unwrap();
    }
}
