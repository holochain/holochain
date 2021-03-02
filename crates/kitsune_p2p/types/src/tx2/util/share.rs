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

/// Synchronized droppable share-lock around internal state date.
pub struct ShareSync<T: 'static + Send + Sync>(Arc<parking_lot::RwLock<Option<T>>>);

impl<T: 'static + Send + Sync> Clone for ShareSync<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: 'static + Send + Sync> PartialEq for ShareSync<T> {
    fn eq(&self, oth: &Self) -> bool {
        Arc::ptr_eq(&self.0, &oth.0)
    }
}

impl<T: 'static + Send + Sync> Eq for ShareSync<T> {}

impl<T: 'static + Send + Sync> std::hash::Hash for ShareSync<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl<T: 'static + Send + Sync> ShareSync<T> {
    /// Create a new share lock.
    pub fn new(t: T) -> Self {
        Self(Arc::new(parking_lot::RwLock::new(Some(t))))
    }

    /// Execute code with ref access to the internal state.
    pub fn share_ref<R, F>(&self, f: F) -> KitsuneResult<R>
    where
        F: FnOnce(&T) -> KitsuneResult<R>,
    {
        let t = self.0.read();
        if t.is_none() {
            return Err(KitsuneErrorKind::Closed.into());
        }
        f(t.as_ref().unwrap())
    }

    /// Execute code with mut access to the internal state.
    /// The second param, if set to true, will drop the shared state,
    /// any further access will `Err(KitsuneError::Closed)`.
    /// E.g. `share.share_mut(|_state, close| *close = true).unwrap();`
    pub fn share_mut<R, F>(&self, f: F) -> KitsuneResult<R>
    where
        F: FnOnce(&mut T, &mut bool) -> KitsuneResult<R>,
    {
        let mut t = self.0.write();
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
        self.0.read().is_none()
    }

    /// Explicity drop the internal state.
    pub fn close(&self) {
        *(self.0.write()) = None;
    }
}
