use crate::*;
use crate::tx2::*;
use futures::io::IoSlice;
use futures::io::IoSliceMut;
use futures::io::AsyncWriteExt;
use futures::future::TryFutureExt;

pub struct VectoredBuf(Box<[u8]>, usize);

impl VectoredBuf {
    pub fn new() -> Self {
        Self(Box::new([0; 4096]), 0)
    }

    pub fn for_read(&mut self) -> &mut I
}

pub struct VectoredWriter {
    sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
    buf: Option<Box<[u8]>>,
}

impl VectoredWriter {
    pub fn new(
        sub: Box<dyn futures::io::AsyncWrite + 'static + Send + Unpin>,
    ) -> Self {
        Self {
            sub,
            buf: None,
        }
    }

    pub async fn write(
        &mut self,
        to_write: &mut [IoSlice<'_>],
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        let mut buf = self.buf.take().unwrap_or_else(|| Box::new([0; 4096]));

        // TODO - when write_vectored is stablized / used by our deps
        //        make use of it!
        //        Until then, lets be smart about re-using buffers
        let mut idx = 0;
        while idx < to_write.len() {
            // if the next vectors are too large, just write the next one
            if to_write[idx].len() + to_write[idx].len() > buf.len() {
                println!("mono write {} bytes", to_write[idx].len());

                // TODO - Destroy this writer on timeouts!

                // timeout write the next buffer
                timeout.mix(
                    self.sub.write_all(&to_write[idx]).map_err(KitsuneError::other)
                ).await?;
                idx += 1;
                continue;
            }

            // join small buffers together for more efficient write
            let mut offset = 0;
            while offset + to_write[idx].len() < buf.len() {
                buf[offset..offset + to_write[idx].len()].copy_from_slice(&to_write[idx]);
                offset += to_write[idx].len();
                idx += 1;
                if idx >= to_write.len() {
                    break;
                }
            }

            println!("multi write {} bytes", buf[..offset].len());

            // TODO - Destroy this writer on timeouts!

            // timeout write the mixed buffers
            timeout.mix(
                self.sub.write_all(&buf[..offset]).map_err(KitsuneError::other)
            ).await?;
        }

        self.buf = Some(buf);

        Ok(())
    }
}

pub struct VectoredReader {
    sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
}

impl VectoredReader {
    pub fn new(
        sub: Box<dyn futures::io::AsyncRead + 'static + Send + Unpin>,
    ) -> Self {
        Self {
            sub,
        }
    }

    pub async read(
        &mut self,
        vector_pool: Arc<AsyncOwnedResourceBucket<Box<[u8]>>>,
        timeout: KitsuneTimeout,
    ) -> Box<[u8]> {
        let buf = vector_pool.acquire_or_else(|| Box::new([0; 4096]));


    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn play() {
        let (send, recv) = util::bound_async_mem_channel(4096);
        let mut send = VectoredWriter::new(send);
        let _recv = VectoredReader::new(recv);

        let wt = tokio::task::spawn(async move {
            let b512 = [0xd0; 512];
            let b8000 = [0xd1; 8000];
            send.write(
                &mut [
                    IoSlice::new(&b512),
                    IoSlice::new(&b512),
                    IoSlice::new(&b512),
                    IoSlice::new(&b8000),
                ],
                KitsuneTimeout::from_millis(1000 * 30),
            ).await.unwrap();
        });

        wt.await.unwrap();
    }
}
