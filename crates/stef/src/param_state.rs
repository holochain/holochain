use crate::*;

impl<'a, S> State<'a> for S
where
    S: ParamState<'a> + 'a,
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
pub trait ParamState<'a> {
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
