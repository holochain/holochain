//! Combinator for buffering actions and effects of a State

use std::collections::VecDeque;

use crate::State;

/// The actions possible for a Buffer
#[derive(Debug, PartialEq, Eq, derive_more::From)]
pub enum BufferAction<A> {
    /// Queue up an action to be processed
    Push(A),
    /// Process one incoming action and queue the effect
    Process,
    /// Pop one effect
    Pop,
    // /// Bypass the buffer and do a transformation immediately
    // Bypass(S::Action),
}

#[derive(Debug, PartialEq, Eq)]
pub enum BufferError<S: State> {
    ActionDropped(S::Action),
    ProcessingBlocked,
}

pub type BufferResult<S> = Result<Option<<S as State>::Effect>, BufferError<S>>;

pub struct Buffer<S: State> {
    state: S,
    actions: VecDeque<S::Action>,
    effects: VecDeque<S::Effect>,
    capacity: usize,
}

impl<S: State + 'static> Buffer<S> {
    pub fn new(state: S, capacity: usize) -> Self {
        Self {
            state,
            actions: Default::default(),
            effects: Default::default(),
            capacity,
        }
    }
}

// #[stef::state(Action = BufferAction<S::Action>)]
impl<S: State + 'static> State for Buffer<S> {
    type Action = BufferAction<S::Action>;
    type Effect = BufferResult<S>;

    fn transition(&mut self, action: Self::Action) -> Self::Effect {
        match action {
            BufferAction::Push(action) => {
                if self.actions.len() >= self.capacity {
                    Err(BufferError::ActionDropped(action))
                } else {
                    self.actions.push_back(action);
                    Ok(None)
                }
            }
            BufferAction::Process => {
                if !self.actions.is_empty() && self.effects.len() >= self.capacity {
                    Err(BufferError::ProcessingBlocked)
                } else {
                    if let Some(action) = self.actions.pop_front() {
                        let fx = self.state.transition(action);
                        self.effects.push_back(fx);
                    }
                    Ok(None)
                }
            }
            BufferAction::Pop => Ok(self.effects.pop_front()),
            // BufferAction::Bypass(action) => Ok(Some(self.state.transition(action))),
        }
    }
}

#[test]
fn test_buffer() {
    #[derive(Debug, PartialEq, Eq)]
    struct S(i32);

    impl State<'static> for S {
        type Action = i32;
        type Effect = String;

        fn transition(&mut self, action: i32) -> Self::Effect {
            self.0 += action;
            if self.0 >= 0 {
                ".".repeat(self.0 as usize)
            } else {
                "negative".into()
            }
        }
    }

    let mut buffer = Buffer::new(S(0), 3);

    assert_eq!(buffer.transition(BufferAction::Process), Ok(None));
    assert_eq!(buffer.transition(BufferAction::Pop), Ok(None));

    assert_eq!(buffer.transition(BufferAction::Push(1)), Ok(None));
    assert_eq!(buffer.transition(BufferAction::Push(2)), Ok(None));
    assert_eq!(buffer.transition(BufferAction::Push(3)), Ok(None));
    assert_eq!(
        buffer.transition(BufferAction::Push(4)),
        Err(BufferError::ActionDropped(4))
    );

    assert_eq!(buffer.transition(BufferAction::Process), Ok(None));
    assert_eq!(buffer.transition(BufferAction::Process), Ok(None));
    assert_eq!(buffer.transition(BufferAction::Process), Ok(None));

    assert_eq!(buffer.transition(BufferAction::Push(4)), Ok(None));

    assert_eq!(
        buffer.transition(BufferAction::Process),
        Err(BufferError::ProcessingBlocked)
    );

    assert_eq!(
        buffer.transition(BufferAction::Pop),
        Ok(Some(".".repeat(1)))
    );
    assert_eq!(
        buffer.transition(BufferAction::Pop),
        Ok(Some(".".repeat(3)))
    );
    assert_eq!(
        buffer.transition(BufferAction::Pop),
        Ok(Some(".".repeat(6)))
    );

    assert_eq!(buffer.transition(BufferAction::Process), Ok(None));

    assert_eq!(
        buffer.transition(BufferAction::Pop),
        Ok(Some(".".repeat(10)))
    );
    assert_eq!(buffer.transition(BufferAction::Pop), Ok(None));
}
