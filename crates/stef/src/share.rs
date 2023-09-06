use super::*;

/// Wrap a State in a mutex for shared access.
#[derive(Default)]
pub struct Share<S>(std::sync::Arc<parking_lot::RwLock<S>>);

impl<S> Clone for Share<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S: State> Share<S> {
    /// Constructor
    pub fn new(s: S) -> Self {
        Self(std::sync::Arc::new(parking_lot::RwLock::new(s)))
    }

    /// Acquire read-only access to the shared state.
    pub fn read<R>(&self, f: impl FnOnce(&S) -> R) -> R {
        let g = self.0.read();
        f(&g)
    }

    /// Acquire write access to the shared state.
    /// This isn't really ideal since it doesn't hide mutability,
    /// so you have to be careful. Better to use a macro (TODO).
    pub fn write(&self, f: impl FnOnce(&mut S)) {
        f(&mut self.0.write())
    }

    /// Acquire write access to the shared state to perform a mutation.
    pub fn transition(&self, t: S::Action) -> S::Effect {
        self.transition_with(t, |_| ()).1
    }

    /// Acquire write access to the shared state to perform a mutation,
    /// and do a read on the modified state within the same atomic mutex acquisition.
    pub fn transition_with<R>(&self, t: S::Action, f: impl FnOnce(&S) -> R) -> (R, S::Effect) {
        let mut g = self.0.write();
        let eff = g.transition(t);
        (f(&g), eff)
    }
}

impl<S: State + Clone> Share<S> {
    /// Return a cloned copy of the shared state
    pub fn get(&self) -> S {
        let g = self.0.read();
        g.clone()
    }
}

impl<S: State> State for Share<S> {
    type Action = S::Action;
    type Effect = S::Effect;

    fn transition(&mut self, t: Self::Action) -> Self::Effect {
        Share::transition(self, t)
    }
}

impl<T: 'static + State + std::fmt::Debug> std::fmt::Debug for Share<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.read(|s| f.debug_tuple("Share").field(s).finish())
    }
}
