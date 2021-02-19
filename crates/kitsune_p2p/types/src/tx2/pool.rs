use std::cell::RefCell;

// TODO - expirement with these values for efficiency.

/// The max capacity of in-pool stored PoolBufs per thread.
pub(crate) const POOL_MAX_CAPACITY: usize = 1024;

/// Returned PoolBufs will be shrunk to this capacity when returned.
pub(crate) const POOL_BUF_MAX_CAPACITY: usize = 4096;

/// A buffer that will return to a pool after use.
///
/// When working with network code, we try to avoid two slow things:
/// - Allocation
/// - Initialization
///
/// We avoid allocation by returning used buffers to a pool for later re-use.
///
/// We avoid initialization by using `extend_from_slice()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolBuf(Option<Vec<u8>>);

impl serde::Serialize for PoolBuf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.0.as_ref().unwrap())
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

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(PoolBuf(Some(v)))
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
        if let Some(mut inner) = self.0.take() {
            BUF_POOL.with(|p| {
                let mut p = p.borrow_mut();
                if p.len() < POOL_MAX_CAPACITY {
                    if inner.capacity() > POOL_BUF_MAX_CAPACITY {
                        inner.truncate(POOL_BUF_MAX_CAPACITY);
                        inner.shrink_to_fit();
                    }
                    inner.clear();
                    p.push(inner);
                }
            });
        }
    }
}

impl PoolBuf {
    /// Create a new PoolBuf.
    pub fn new() -> Self {
        let inner = BUF_POOL.with(|p| {
            let mut p = p.borrow_mut();
            if p.is_empty() {
                // I beleive Vec::new() actually starts with zero capacity,
                // but make it explicit just in case.
                Vec::with_capacity(0)
            } else {
                p.remove(0)
            }
        });
        Self(Some(inner))
    }

    /// Like `drain(..len)` but without the iterator trappings.
    pub fn truncate_front(&mut self, len: usize) {
        let this = self.0.as_mut().unwrap();
        if len == 0 {
            return;
        }

        if len >= this.len() {
            this.clear();
            return;
        }

        let r = len..this.len();
        this.copy_within(r, 0);
        this.truncate(this.len() - len);
    }
}

impl Default for PoolBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for PoolBuf {
    // really, we'd like `Target = [u8]`,
    // but we want access to Vec methods in `deref_mut()`.
    // At least we can differentiate `AsRef<[u8]>` and `AsMut<Vec<u8>>`.
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for PoolBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap()
    }
}

impl AsRef<[u8]> for PoolBuf {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref().unwrap()
    }
}

impl AsMut<Vec<u8>> for PoolBuf {
    fn as_mut(&mut self) -> &mut Vec<u8> {
        self.0.as_mut().unwrap()
    }
}
