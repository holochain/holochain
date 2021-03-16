use std::cell::RefCell;

// TODO - expirement with these values for efficiency.

/// The max capacity of in-pool stored PoolBufs per thread.
pub(crate) const POOL_MAX_CAPACITY: usize = 1024;

/// Returned PoolBufs will be shrunk to this capacity when returned.
pub(crate) const POOL_BUF_SHRINK_TO_CAPACITY: usize = 4096;

/// PoolBufs will be allocated/reset with this byte count BEFORE
/// the readable buffer to make prepending frame info more efficient.
pub(crate) const POOL_BUF_PRE_WRITE_SPACE: usize = 128;

/// A buffer that will return to a pool after use.
///
/// When working with network code, we try to avoid two slow things:
/// - Allocation
/// - Initialization
///
/// We avoid allocation by returning used buffers to a pool for later re-use.
///
/// We avoid initialization by using `extend_from_slice()`.
#[derive(Clone, PartialEq, Eq)]
pub struct PoolBuf(Option<(usize, Vec<u8>)>);

impl std::fmt::Debug for PoolBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.0.as_ref().unwrap();
        let byte_count = inner.1.len() - inner.0;
        f.debug_struct("PoolBuf")
            .field("byte_count", &byte_count)
            .finish()
    }
}

impl std::io::Write for PoolBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.0.as_mut().unwrap().1, buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.0.as_mut().unwrap().1)
    }
}

impl serde::Serialize for PoolBuf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.as_ref())
    }
}

struct VisitBytes;

impl<'de> serde::de::Visitor<'de> for VisitBytes {
    type Value = PoolBuf;

    fn expecting(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "raw bytes")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut out = PoolBuf::new();
        out.extend_from_slice(v);
        Ok(out)
    }
}

impl<'de> serde::Deserialize<'de> for PoolBuf {
    fn deserialize<D>(deserializer: D) -> Result<PoolBuf, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // we might be tempted to deserialize_byte_buf here...
        // but that may cause decoders to clone when there was no need.
        deserializer.deserialize_bytes(VisitBytes)
    }
}

thread_local! {
    pub(crate) static BUF_POOL: RefCell<Vec<Vec<u8>>> = RefCell::new(Vec::with_capacity(POOL_MAX_CAPACITY));
}

impl Drop for PoolBuf {
    fn drop(&mut self) {
        if let Some((_, mut inner)) = self.0.take() {
            BUF_POOL.with(|p| {
                let mut p = p.borrow_mut();
                if p.len() < POOL_MAX_CAPACITY {
                    reset(&mut inner, true);
                    p.push(inner);
                }
            });
        }
    }
}

/// reset used both for requeuing into thread local, and for clear()
fn reset(v: &mut Vec<u8>, do_truncate: bool) {
    if do_truncate && v.capacity() > POOL_BUF_MAX_CAPACITY {
        v.truncate(POOL_BUF_MAX_CAPACITY);
        v.shrink_to_fit();
    }
    v.resize(POOL_BUF_PRE_WRITE_SPACE, 0);
}

impl PoolBuf {
    /// Create a new PoolBuf.
    pub fn new() -> Self {
        let inner = BUF_POOL.with(|p| {
            let mut p = p.borrow_mut();
            if p.is_empty() {
                vec![0; POOL_BUF_PRE_WRITE_SPACE]
            } else {
                p.remove(0)
            }
        });
        Self(Some((POOL_BUF_PRE_WRITE_SPACE, inner)))
    }

    /// Reset this buffer
    pub fn clear(&mut self) {
        let inner = self.0.as_mut().unwrap();
        reset(&mut inner.1, false);
        inner.0 = inner.1.len();
    }

    /// Like `drain(..len)` but without the iterator trappings.
    /// Note, this actually copies the memory, leaving start the same.
    /// perhaps you want `cheap_move_start()`?
    pub fn shift_data_forward(&mut self, len: usize) {
        if len == 0 {
            return;
        }

        let inner = self.0.as_mut().unwrap();

        let start = inner.0;
        let data_len = inner.1.len() - start;

        if len >= data_len {
            reset(&mut inner.1, false);
            inner.0 = POOL_BUF_PRE_WRITE_SPACE;
            return;
        }

        let r = len + start..inner.1.len();
        inner.1.copy_within(r, start);
        inner.1.truncate(inner.1.len() - len);
    }

    /// Like `drain(..len)` but without the iterator trappings.
    /// Note, this just moves the start pointer forward,
    /// if you actually want to reclaim space,
    /// perhaps you want `shift_data_forward()`?
    pub fn cheap_move_start(&mut self, len: usize) {
        if len == 0 {
            return;
        }

        let inner = self.0.as_mut().unwrap();

        let start = inner.0;
        let data_len = inner.1.len() - start;

        if len >= data_len {
            reset(&mut inner.1, false);
            inner.0 = POOL_BUF_PRE_WRITE_SPACE;
            return;
        }

        inner.0 += len;
    }

    /// Reserve desired capacity. Prefer doing this once at the beginning
    /// of an operation to avoid the time cost of allocation.
    pub fn reserve(&mut self, want_size: usize) {
        let inner = self.0.as_mut().unwrap();
        inner.1.reserve(want_size + inner.0);
    }

    /// Extend this buffer with data from src.
    pub fn extend_from_slice(&mut self, src: &[u8]) {
        let inner = self.0.as_mut().unwrap();
        inner.1.extend_from_slice(src);
    }

    /// Ensure we have enough front space to prepend the given byte count.
    /// If not, shift all data over to the right, making more prepend space.
    pub fn reserve_front(&mut self, mut len: usize) {
        let inner = self.0.as_mut().unwrap();

        if len < inner.0 {
            // we already have enough space, return early
            return;
        }

        // we don't have enough space - allocate a little extra
        len += POOL_BUF_PRE_WRITE_SPACE;

        let prev_len = inner.1.len();
        let new_len = prev_len + len;

        inner.1.reserve(new_len);

        // any way to work around this unsafe without needlessly
        // initializing this data we're going to overwrite?
        unsafe {
            inner.1.set_len(new_len);
        }

        inner.1.copy_within(inner.0..prev_len, inner.0 + len);
        inner.0 += len;
    }

    /// Efficiently copy data *before* the current data.
    pub fn prepend_from_slice(&mut self, src: &[u8]) {
        self.reserve_front(src.len());

        let inner = self.0.as_mut().unwrap();
        inner.1[inner.0 - src.len()..inner.0].copy_from_slice(src);
        inner.0 -= src.len();
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
        self.as_ref()
    }
}

impl AsRef<[u8]> for PoolBuf {
    fn as_ref(&self) -> &[u8] {
        let inner = self.0.as_ref().unwrap();
        &inner.1[inner.0..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_buf_prepend() {
        let mut b = PoolBuf::new();
        b.extend_from_slice(b"World!");
        b.prepend_from_slice(b"Hello ");
        assert_eq!("Hello World!", String::from_utf8_lossy(&b));
    }

    #[test]
    fn pool_buf_prepend_large() {
        const D: [u8; 512] = [0xdb; 512];
        let mut b = PoolBuf::new();
        b.extend_from_slice(b"apple");
        b.prepend_from_slice(&D[..]);
        b.prepend_from_slice(b"banana");
        assert_eq!(b"banana", &b[0..6]);
        assert_eq!(b"apple", &b[518..523]);
        assert_eq!(&D[..], &b[6..518]);
        assert_eq!(523, b.len());
    }

    #[test]
    fn pool_buf_grow_shrink_reset_reuse() {
        let mut b = PoolBuf::new();
        b.extend_from_slice(b"bar");
        assert_eq!(b"bar", &*b);
        b.prepend_from_slice(b"foo");
        assert_eq!(b"foobar", &*b);
        b.cheap_move_start(3);
        assert_eq!(b"bar", &*b);
        b.clear();
        b.extend_from_slice(b"ab");
        assert_eq!(b"ab", &*b);
    }
}
