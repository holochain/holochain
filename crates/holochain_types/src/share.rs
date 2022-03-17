//! A sync RwLock that uses closures to avoid deadlocks.
use std::sync::Arc;

/// A clonable thread safe read write lock designed to make it hard to create dead locks
/// or hold long long lived locks.
pub struct RwShare<T>(Arc<parking_lot::RwLock<T>>);

impl<T> Clone for RwShare<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for RwShare<T>
where
    T: Default,
{
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> RwShare<T> {
    /// Create a new shareable lock
    pub fn new(value: T) -> Self {
        Self(Arc::new(parking_lot::RwLock::new(value)))
    }

    /// Get a shared reference to the value. This will not block other readers
    /// but will block writers.
    /// This should never be used recursively or held over awaits
    /// or held for a long time.
    pub fn share_ref<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let t = self
            .0
            // First we try to get a fair reader that won't starve writers.
            .try_read_for(std::time::Duration::from_millis(100))
            // The lock is taking a little longer then we'd like so print an info
            // and try for a further 30 seconds.
            .or_else(|| {
                tracing::info!(
                    "Took over 100ms to get a RwShare reader. Conductor might be over utilized"
                );
                self.0.try_read_for(std::time::Duration::from_secs(30))
            })
            // However if that fails we may be in a recursive reader dead lock so we will try for
            // a recursive reader that may starve writers.
            .or_else(|| {
                tracing::warn!("Failed to get fair reader, trying for recursive reader");
                self.0
                    .try_read_recursive_for(std::time::Duration::from_secs(60))
            })
            // Now we are probably at a deadlock or a really long held lock so print an error.
            .or_else(|| {
                tracing::error!(
                    "Failed to get a RwShare read lock for over 120s this could be a dead lock"
                );
                self.0
                    .try_read_recursive_for(std::time::Duration::from_secs(180))
            })
            .expect("Failed to take a read lock for over 5 minutes this must be a deadlock");
        f(&t)
    }

    /// Get a mutable reference to the value.
    /// This should never be used recursively or held over awaits
    /// or held for a long time.
    pub fn share_mut<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        // First try to get the write lock in under 100 ms.
        let mut t = self
            .0
            .try_write_for(std::time::Duration::from_millis(100))
            // If that fails try print an info and try for a further 120 seconds.
            .or_else(|| {
                tracing::info!(
                    "Took over 100ms to get a RwShare writer. Conductor might be over utilized"
                );
                self.0.try_write_for(std::time::Duration::from_secs(120))
            })
            // Now we are probably at a deadlock or a really long held lock so print an error.
            .or_else(|| {
                tracing::error!(
                    "Failed to get a RwShare write lock for over 120s this could be a dead lock"
                );
                self.0.try_write_for(std::time::Duration::from_secs(180))
            })
            .expect("Failed to take a write lock for over 5 minutes this must be a deadlock");
        f(&mut t)
    }

    /// Try to unwrap the inner type if there are no outstanding references.
    pub fn try_unwrap(self) -> Result<T, Self> {
        Arc::try_unwrap(self.0)
            .map(|lock| lock.into_inner())
            .map_err(|t| Self(t))
    }
}
