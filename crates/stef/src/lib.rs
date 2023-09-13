//! Modular state management model based on Redux, Elm, Haskell, etc.
//!
//! The basic unit is the [`State`] trait, which defines a state module with a pure
//! transition function and a pure Effect type. Each state transition is defined in
//! terms of a declarative [`Action`][State::Action] type, and produces an
//! [`Effect`][State::Effect].
//!
//! Actions are declarative so that they may be recorded and played back. By wrapping
//! up all mutable state in a system into these State modules, the entire system can
//! be treated as a state machine. Actions are often enums with the various types of
//! mutation as variants.
//!
//! Effects are declarative so that transition logic can be tested in isolation. Effects
//! are often enums as well, with variants for each different type of effect which can be
//! produced. Effects should be pure, in that they don't actually "do" anything.
//! In a real-world environment, effects should usually be immediately "executed" via a
//! function which performs some useful operation based on the effect -- however in a test
//! environment, it can be useful to not execute the effects and instead write assertions
//! against them, to see what *would* have happened, without allowing anything to actually
//! happen. The pure effect system can also be useful for intentionally manipulating the
//! effects to occur out-of-order, with time delays, etc., to push the system into edge cases.
//!
//! ## Invariants
//!
//! There are two invariants you must uphold in your code.
//!
//! `stef` promises many cool things, like time travel, free snapshots, exhaustive state exploration
//! via fuzz testing, and automatic state diagram generation from code. However, these things
//! only work if you uphold these invariants:
//!
//! ### Invariant 1: Determinism
//!
//! The [`State::transition`] function must be completely deterministic, both in terms of
//! its mutation and its return value. For any given State, performing a transition with a
//! given Action must always result in the same state mutation, and must produce the same Effect.
//!
//! One way to look at this is that the transition function must not read any data outside of the
//! State itself. The transition function must be blind to any data which is modified by something
//! other than itself: no database reads (unless the database *is* the state under management), no
//! system time calls, no randomness, no file access -- you get the idea.
//!
//! State *may* read constant values that never change. If a constant value exists, unchanging, from
//! the moment the State was created, then that value can be considered part of the definition of
//! the state machine, rather than state. A provision is made for this case via the `ParamState`
//! trait, which helps separate out some constant "parameters" from the actual mutable "state".
//!
//! ### Invariant 2: Exclusive mutability
//!
//! A [`State`] must never be modified outside of its transition function. You must take care to not
//! expose any mutable access to a State that doesn't go through its transition function. This also
//! means that you cannot define two or more state machines that manage the same state. To do so would
//! be a violation of the first invariant, since the last proper transition of one state machine would
//! be "undone" by some other mutation.
//!
//! ## Tips
//!
//! Meeting the invariants can be cumbersome, but the benefits are worth it. Some tips:
//!
//! - Sometimes it can be helpful to think about constructing your Actions in two steps. If you have
//! an input from some other part of the system (perhaps some other Effect), and your state machine
//! requires some data from some other part of the system state, then read the data outside the
//! transition function and pass it in as part of your Action. The Action then becomes the combination
//! of the original input plus the data needed to make the transition.

mod combinators;
pub use combinators::*;

mod share;
pub use share::Share;

mod state;
pub use state::*;

mod param_state;
pub use param_state::*;

mod action;
pub use action::*;

mod effect;
pub use effect::*;

pub mod util;

#[cfg(feature = "derive")]
pub use stef_derive::{state, State};

#[cfg(feature = "diagramming")]
pub mod diagram;

#[cfg(feature = "invariants")]
pub mod invariants;
