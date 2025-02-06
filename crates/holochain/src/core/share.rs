use std::sync::Arc;

pub type ShareResult<T> = Result<T, ShareError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShareError {
    /// This object is closed, calls on it are invalid.
    #[error("This object is closed, calls on it are invalid.")]
    Closed,
    // /// An error ocurred in a closure that accessing the Share state
    // #[error(transparent)]
    // ClosureFailed(Box<dyn std::error::Error + Send + Sync>),
}

/// Synchronized droppable share-lock around internal state date.
pub struct Share<T: 'static + Send>(Arc<parking_lot::Mutex<Option<T>>>);

impl<T: 'static + Send> Clone for Share<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: 'static + Send + std::fmt::Debug> std::fmt::Debug for Share<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.share_ref(|s| Ok(f.debug_tuple("Share").field(s).finish()))
            .unwrap()
    }
}

impl<T: 'static + Send> PartialEq for Share<T> {
    fn eq(&self, oth: &Self) -> bool {
        Arc::ptr_eq(&self.0, &oth.0)
    }
}

impl<T: 'static + Send> Eq for Share<T> {}

impl<T: 'static + Send> std::hash::Hash for Share<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl<T: 'static + Send + Default> Default for Share<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: 'static + Send> Share<T> {
    /// Create a new share lock.
    pub fn new(t: T) -> Self {
        Self(Arc::new(parking_lot::Mutex::new(Some(t))))
    }

    /// Execute code with immutable access to the internal state.
    pub fn share_ref<R, F>(&self, f: F) -> ShareResult<R>
    where
        F: FnOnce(&T) -> ShareResult<R>,
    {
        let t = self.0.lock();
        if t.is_none() {
            return Err(ShareError::Closed.into());
        }
        f(t.as_ref().unwrap())
    }

    /// Execute code with mut access to the internal state.
    /// The second param, if set to true, will drop the shared state,
    /// any further access will `Err(ShareError::Closed)`.
    /// E.g. `share.share_mut(|_state, close| *close = true).unwrap();`
    pub fn share_mut<R, F>(&self, f: F) -> ShareResult<R>
    where
        F: FnOnce(&mut T, &mut bool) -> ShareResult<R>,
    {
        let mut t = self.0.lock();
        if t.is_none() {
            return Err(ShareError::Closed.into());
        }
        let mut close = false;
        let r = f(t.as_mut().unwrap(), &mut close);
        if close {
            *t = None;
        }
        r
    }
}
