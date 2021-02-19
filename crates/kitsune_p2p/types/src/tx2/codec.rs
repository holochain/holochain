use crate::codec::*;
use crate::tx2::*;
use crate::*;

// NOTE - this is the termination point for PoolBuf usage...
//        we need to be extra careful in here to return them.

/// Result type returned from CodecReader::read
#[derive(Debug)]
pub enum CodecMessage<C: Codec> {
    /// Notify-type message.
    Notify(C),

    /// Request-type id & message.
    Request(u64, C),

    /// Response-type id & message.
    Response(u64, C),
}

/// Message type used for sending CodecWriter::write
#[derive(Debug)]
pub enum CodecMessageRef<'a, C: Codec> {
    /// Notify-type message.
    Notify(&'a C),

    /// Request-type id & message.
    Request(u64, &'a C),

    /// Response-type id & message.
    Response(u64, &'a C),
}

/// Efficiently read encoded data from a sub FramedReader.
/// Note, this is intentionally not a Stream - as TryStreams are hard to work
/// with, and we then would have no ability to pass individual timeout
/// values to read operations.
pub struct CodecReader<C: Codec>(Option<CodecReaderInner<C>>);

impl<C: Codec> CodecReader<C> {
    /// Construct a new CodecReader with a given FramedReader.
    pub fn new(sub: FramedReader) -> Self {
        Self(Some(CodecReaderInner {
            sub,
            _p: std::marker::PhantomData,
        }))
    }

    /// Read typed data from this CodecReader instance.
    pub async fn read(&mut self, timeout: KitsuneTimeout) -> KitsuneResult<Vec<CodecMessage<C>>> {
        let mut inner = match self.0.take() {
            None => return Err(KitsuneError::Closed),
            Some(inner) => inner,
        };

        let received = match inner.sub.read(timeout).await {
            Err(e) => return Err(e),
            Ok(r) => r,
        };

        let mut out = Vec::new();
        let mut err = None;
        for (msg_id, data) in received {
            if err.is_some() {
                BUF_POOL.release(data).await;
                continue;
            }
            let (_, dec) = match C::decode_ref(&data) {
                Err(e) => {
                    err = Some(e);
                    BUF_POOL.release(data).await;
                    continue;
                }
                Ok(dec) => dec,
            };
            BUF_POOL.release(data).await;
            if msg_id.is_notify() {
                out.push(CodecMessage::Notify(dec));
            } else if msg_id.is_req() {
                out.push(CodecMessage::Request(msg_id.as_id(), dec));
            } else {
                out.push(CodecMessage::Response(msg_id.as_id(), dec));
            }
        }

        if let Some(e) = err {
            return Err(KitsuneError::other(e));
        }

        self.0 = Some(inner);

        Ok(out)
    }
}

struct CodecReaderInner<C: Codec> {
    sub: FramedReader,
    _p: std::marker::PhantomData<C>,
}

/// Efficiently write encoded data to a sub FramedWriter.
pub struct CodecWriter<C: Codec>(Option<CodecWriterInner<C>>);

impl<C: Codec> CodecWriter<C> {
    /// Create a new CodecWriter instance.
    pub fn new(sub: FramedWriter) -> Self {
        Self(Some(CodecWriterInner {
            sub,
            _p: std::marker::PhantomData,
        }))
    }

    /// Write typed data to this CodecWriter instance.
    pub async fn write(
        &mut self,
        msg: &CodecMessageRef<'_, C>,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        let mut inner = match self.0.take() {
            None => return Err(KitsuneError::Closed),
            Some(inner) => inner,
        };

        let mut buf = BUF_POOL.acquire().await;

        let (msg_id, c) = match msg {
            CodecMessageRef::Notify(c) => (MsgId::new_notify(), c),
            CodecMessageRef::Request(id, c) => (MsgId::new(*id).as_req(), c),
            CodecMessageRef::Response(id, c) => (MsgId::new(*id).as_res(), c),
        };

        if let Err(e) = c.encode(&mut *buf) {
            BUF_POOL.release(buf).await;
            return Err(KitsuneError::other(e));
        }

        if let Err(e) = inner.sub.write(msg_id, &buf, timeout).await {
            BUF_POOL.release(buf).await;
            return Err(e);
        }

        BUF_POOL.release(buf).await;
        self.0 = Some(inner);
        Ok(())
    }

    /// Write typed notify data to this CodecWriter instance.
    pub async fn write_notify(&mut self, msg: &C, timeout: KitsuneTimeout) -> KitsuneResult<()> {
        self.write(&CodecMessageRef::Notify(msg), timeout).await
    }

    /// Write typed request data to this CodecWriter instance.
    pub async fn write_request(
        &mut self,
        id: u64,
        msg: &C,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        if id == 0 {
            return Err("id cannot be zero for request".into());
        }
        self.write(&CodecMessageRef::Request(id, msg), timeout)
            .await
    }

    /// Write typed response data to this CodecWriter instance.
    pub async fn write_response(
        &mut self,
        id: u64,
        msg: &C,
        timeout: KitsuneTimeout,
    ) -> KitsuneResult<()> {
        if id == 0 {
            return Err("id cannot be zero for response".into());
        }
        self.write(&CodecMessageRef::Response(id, msg), timeout)
            .await
    }
}

struct CodecWriterInner<C: Codec> {
    sub: FramedWriter,
    _p: std::marker::PhantomData<C>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    #[allow(dead_code)]
    async fn test_codec() {
        crate::write_codec_enum! {
            codec Test {
                One(0x01) {
                    data.0: usize,
                },
            }
        }

        let (send, recv) = util::bound_async_mem_channel(4096).await;
        let mut send = <CodecWriter<Test>>::new(FramedWriter::new(send));
        let mut recv = <CodecReader<Test>>::new(FramedReader::new(recv));

        let wt = tokio::task::spawn(async move {
            let timeout = KitsuneTimeout::from_millis(1000 * 30);
            send.write_notify(&Test::one(42), timeout).await.unwrap();
            send.write_request(42, &Test::one(42), timeout)
                .await
                .unwrap();
            send.write_response(42, &Test::one(42), timeout)
                .await
                .unwrap();
        });

        let timeout = KitsuneTimeout::from_millis(1000 * 30);
        let data = recv
            .read(timeout)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        println!("{:?}", data);
        assert!(matches!(
            data,
            CodecMessage::Notify(Test::One(One { data: 42 }))
        ));
        let data = recv
            .read(timeout)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        println!("{:?}", data);
        assert!(matches!(
            data,
            CodecMessage::Request(42, Test::One(One { data: 42 }))
        ));
        let data = recv
            .read(timeout)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        println!("{:?}", data);
        assert!(matches!(
            data,
            CodecMessage::Response(42, Test::One(One { data: 42 }))
        ));

        wt.await.unwrap();
    }
}
