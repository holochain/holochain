use std::io;
use std::sync::{Arc, Mutex, MutexGuard};

pub(crate) struct InMemoryWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl InMemoryWriter {
    pub(crate) fn new(buf: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { buf }
    }

    pub(crate) fn buf(&self) -> io::Result<MutexGuard<'_, Vec<u8>>> {
        self.buf
            .lock()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))
    }
}

impl io::Write for InMemoryWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf()?.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf()?.flush()
    }
}

impl io::Read for InMemoryWriter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let result = self.buf()?.as_slice().read(buf);

        if let Ok(count) = &result {
            self.buf()?.drain(0..*count);
        }

        result
    }
}
