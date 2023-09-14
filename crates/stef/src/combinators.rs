//! Wrapper types to add functionality and behavior to a [`State`]

use std::collections::VecDeque;

use super::*;

/// Every time an Effect is generated from this State, just store it in a Vec.
/// The stored Effects can be accessed at any time later.
/// Converts the State's Effect type to `()`
#[derive(Debug, derive_more::Deref, derive_more::DerefMut)]
pub struct StoreEffects<S: State<'static>> {
    #[deref]
    #[deref_mut]
    state: S,
    effects: VecDeque<S::Effect>,
}

impl<S: State<'static>> State<'static> for StoreEffects<S> {
    type Action = S::Action;
    type Effect = ();

    fn transition(&mut self, t: Self::Action) -> Self::Effect {
        let eff = self.state.transition(t);
        self.effects.push_back(eff);
    }
}

impl<S: State<'static>> StoreEffects<S> {
    /// Constructor
    pub fn new(state: S) -> Self {
        Self {
            state,
            effects: VecDeque::new(),
        }
    }

    /// Accessor for the stored effects
    pub fn effects(&self) -> &VecDeque<S::Effect> {
        &self.effects
    }

    /// Drain and return all effects.
    /// Useful if you want to defer execution of some effects.
    pub fn drain_effects(&mut self) -> Vec<S::Effect> {
        std::mem::take(&mut self.effects).into_iter().collect()
    }
}
