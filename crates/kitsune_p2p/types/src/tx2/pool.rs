use crate::tx2::*;
use once_cell::sync::Lazy;

// TODO - expirement with this value for efficiency.
pub(crate) const POOL_BUF_MAX_CAPACITY: usize = 4096;

/// A shared buffer that should be returned to the SharedBufferPool after use.
///
/// When working with network code, we try to avoid two slow things:
/// - Allocation
/// - Initialization
///
/// We avoid allocation by returning used buffers to a pool for later re-use.
///
/// We avoid initialization by using `extend_from_slice()`.
pub struct PoolBuf(Vec<u8>);

impl PoolBuf {
    /// Create a new PoolBuf.
    pub fn new() -> Self {
        // I beleive Vec::new() actually starts with zero capacity,
        // but make it explicit just in case.
        Self(Vec::with_capacity(0))
    }

    /// Reset this PoolBuf for further usage.
    /// (shrinking if it was grown too much).
    pub fn reset(&mut self) {
        if self.0.capacity() > POOL_BUF_MAX_CAPACITY {
            self.0.truncate(POOL_BUF_MAX_CAPACITY);
            self.0.shrink_to_fit();
        }
        self.0.clear();
    }

    /// Like `drain(..len)` but without the iterator trappings.
    pub fn truncate_front(&mut self, len: usize) {
        if len == 0 {
            return;
        }

        if len >= self.0.len() {
            self.0.clear();
            return;
        }

        let r = len..self.0.len();
        self.0.copy_within(r, 0);
        self.0.truncate(self.0.len() - len);
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
pub static BUF_POOL: Lazy<SharedBufferPool> = Lazy::new(SharedBufferPool::new);

/// A pool of shared buffers to avoid constant re-allocation.
pub struct SharedBufferPool(ResourceBucket<PoolBuf>);

impl Default for SharedBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedBufferPool {
    /// Construct a new SharedBufferPool
    /// ¿ but, maybe you want to use the global static `BUF_POOL` ?
    pub fn new() -> Self {
        Self(ResourceBucket::new(None))
    }

    /// Release a previously acquired buf to the pool.
    pub fn release(
        &self,
        mut buf: PoolBuf,
    ) -> impl std::future::Future<Output = ()> + 'static + Send {
        buf.reset();
        self.0.release(buf)
    }

    /// Acquire a buf from the pool.
    pub fn acquire(&self) -> impl std::future::Future<Output = PoolBuf> + 'static + Send {
        self.0.acquire_or_else(PoolBuf::new)
    }
}
