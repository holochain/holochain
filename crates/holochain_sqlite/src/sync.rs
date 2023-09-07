#[cfg(not(loom))]
pub use parking_lot::Mutex;

#[cfg(not(loom))]
pub use std::sync::Arc;

#[cfg(not(loom))]
pub mod atomic {
    pub use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
}

#[cfg(not(loom))]
pub use lazy_static::lazy_static;

#[cfg(loom)]
pub struct Mutex<T> {
    inner: loom::sync::Mutex<T>,
}

// Handles a loom API difference https://docs.rs/loom/latest/loom/#handling-loom-api-differences
#[cfg(loom)]
impl<T> Mutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: loom::sync::Mutex::new(value),
        }
    }

    pub fn lock(&self) -> loom::sync::MutexGuard<T> {
        self.inner.lock().unwrap()
    }
}

#[cfg(loom)]
pub use loom::sync::Arc;

#[cfg(loom)]
pub mod atomic {
    pub use loom::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
}

#[cfg(loom)]
pub use loom::lazy_static;
