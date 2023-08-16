use crate::*;

/// A model of an effectful state machine.
///
/// The state can only be mutated through a Action value, and each Action
/// may generate an Effect. The actual mutation of state is specified by implementing
/// the [`transition`] method of this trait in terms of an incoming Action.
///
/// The Effect returned represents some action to be taken. The action will not be
/// performed immediately -- it must be interpreted by some outside function.
pub trait State {
    /// The type which represents a change to the state
    type Action: Action;

    /// The type which represents a change to the outside world
    type Effect: Effect;

    /// The definition of how an incoming Action modifies the State, and what Effect it produces.
    fn transition(&mut self, action: Self::Action) -> Self::Effect;
}

impl<S> State for S
where
    S: ParamState,
{
    type Action = S::Action;
    type Effect = S::Effect;

    fn transition(&mut self, action: Self::Action) -> Self::Effect {
        let (state, data) = self.partition();
        Self::update(state, data, action)
    }
}

/// Parameterized State. An alternate definition for state machines
/// where each state has some immutable data associated with it.
pub trait ParamState {
    /// The part which represents the actual state
    type State;

    /// The immutable data
    type Params;

    /// The type which represents a change to the state
    type Action: Action;

    /// The type which represents a change to the outside world
    type Effect: Effect;

    /// Constructor to provide the initial state
    fn initial(params: Self::Params) -> Self;

    /// Distinguish the state from the non-state
    fn partition(&mut self) -> (&mut Self::State, &Self::Params);

    /// Perform the state transition using mutable state and immutable params.
    /// This is the whole reason for this trait existing, to be able to define
    /// the state transition in terms of the partitioned data, rather than giving
    /// mutable access to the entire datastructure
    fn update(state: &mut Self::State, params: &Self::Params, action: Self::Action)
        -> Self::Effect;
}

/// Extensions to make it easier to apply the built-in combinators to States
pub trait StateExt: State + Sized {
    /// Wrap in [`Share`]
    fn shared(self) -> Share<Self> {
        Share::new(self)
    }

    /// Wrap in [`StoreEffects`]
    fn store_effects(self, capacity: usize) -> StoreEffects<Self> {
        StoreEffects::new(self, capacity)
    }

    /// Wrap in [`RunEffects`]
    fn run_effects<Ret, Runner>(self, runner: Runner) -> RunEffects<Self, Ret, Runner>
    where
        Runner: Fn(Self::Effect) -> Ret,
    {
        RunEffects::new(self, runner)
    }
}

impl<S> StateExt for S where S: State + Sized {}

/// Convenience for updating state by returning an optional owned value
pub fn maybe_update<S, E>(s: &mut S, f: impl FnOnce(&S) -> (Option<S>, E)) -> E
where
    S: Sized,
{
    let (next, fx) = f(s);
    if let Some(next) = next {
        *s = next;
    }
    fx
}

/// Convenience for updating state by returning an owned value
pub fn update_replace<S, E>(s: &mut S, f: impl FnOnce(&S) -> (S, E)) -> E
where
    S: Sized + Clone,
{
    let (next, fx) = f(s);
    *s = next;
    fx
}

/// Convenience for updating state by returning an owned value
pub fn update_copy<S, E>(s: &mut S, f: impl FnOnce(S) -> (S, E)) -> E
where
    S: Sized + Copy,
{
    let (next, fx) = f(*s);
    *s = next;
    fx
}
