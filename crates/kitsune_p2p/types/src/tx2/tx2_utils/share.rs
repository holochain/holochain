use crate::*;

/// Synchronized droppable share-lock around internal state date.
pub struct Share<T: 'static + Send>(Arc<parking_lot::Mutex<Option<T>>>);

impl<T: 'static + Send> Clone for Share<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
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

impl<T: 'static + Send> Share<T> {
    /// Create a new share lock.
    pub fn new(t: T) -> Self {
        Self(Arc::new(parking_lot::Mutex::new(Some(t))))
    }

    /// Create a new closed share lock.
    pub fn new_closed() -> Self {
        Self(Arc::new(parking_lot::Mutex::new(None)))
    }

    /// Execute code with immutable access to the internal state.
    pub fn share_ref<R, F>(&self, f: F) -> KitsuneResult<R>
    where
        F: FnOnce(&T) -> KitsuneResult<R>,
    {
        let t = self.0.lock();
        if t.is_none() {
            return Err(KitsuneErrorKind::Closed.into());
        }
        f(t.as_ref().unwrap())
    }

    /// Attempt to unwrap the inner value, assuming this is the only instance.
    pub fn try_unwrap(self) -> Result<Option<T>, Self> {
        match Arc::try_unwrap(self.0) {
            Ok(inner) => Ok(inner.into_inner()),
            Err(inner) => Err(Self(inner)),
        }
    }

    /// Execute code with mut access to the internal state.
    /// The second param, if set to true, will drop the shared state,
    /// any further access will `Err(KitsuneError::Closed)`.
    /// E.g. `share.share_mut(|_state, close| *close = true).unwrap();`
    pub fn share_mut<R, F>(&self, f: F) -> KitsuneResult<R>
    where
        F: FnOnce(&mut T, &mut bool) -> KitsuneResult<R>,
    {
        let mut t = self.0.lock();
        if t.is_none() {
            return Err(KitsuneErrorKind::Closed.into());
        }
        let mut close = false;
        let r = f(t.as_mut().unwrap(), &mut close);
        if close {
            *t = None;
        }
        r
    }

    /// Returns true if the internal state has been dropped.
    pub fn is_closed(&self) -> bool {
        self.0.lock().is_none()
    }

    /// Explicity drop the internal state.
    pub fn close(&self) {
        *(self.0.lock()) = None;
    }
}

/// A version of Share which can never be closed, and thus every
/// share is infallible (no Err possible).
pub struct ShareOpen<T: 'static + Send>(Share<T>);

impl<T: 'static + Send> ShareOpen<T> {
    /// Create a new share lock.
    pub fn new(t: T) -> Self {
        Self(Share::new(t))
    }

    /// Execute code with immutable access to the internal state.
    pub fn share_ref<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.0
            .share_ref(|s| Ok(f(s)))
            .expect("ShareOpen state is never dropped")
    }

    /// Execute code with mutable access to the internal state.
    pub fn share_mut<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        self.0
            .share_mut(|s, _| Ok(f(s)))
            .expect("ShareOpen state is never dropped")
    }
}
