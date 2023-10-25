//! Defines the types needed to create [`Fact`]s about a system and their [`Cause`]s.

use std::fmt::Display;

use crate::graph::{traverse, Traversal};

/// A Fact is a simple boolean check with other Facts specified as causes.
pub trait Fact: Sized + Clone + Eq + std::fmt::Display + std::fmt::Debug + std::hash::Hash {
    /// The system-specific context needed to check facts and construct causal relationships.
    /// For instance, this might include handles to databases or other forensic artifacts.
    type Context;

    /// A nice human-readable explanation of what this Fact means.
    fn explain(&self, _ctx: &Self::Context) -> String {
        self.to_string()
    }

    /// The logical consequent of this Fact, i.e. the return value of this function
    /// is the "B" in "A implies B"
    fn cause(&self, ctx: &Self::Context) -> Option<Cause<Self>>;

    /// Run this fact to see if it's true or not, given the context
    fn check(&self, ctx: &Self::Context) -> bool;
}

/// The result of checking a Fact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check<T: Fact> {
    /// The Fact is true.
    Pass,
    /// The Fact is false, and the following causes should be included in the causal graph
    Fail(Vec<Cause<T>>),
}

impl<T: Fact> Check<T> {
    /// Just the relevant causes related to this fact which should appear in the causal graph
    pub fn causes(&self) -> &[Cause<T>] {
        match self {
            Check::Pass => &[],
            Check::Fail(cs) => cs.as_slice(),
        }
    }

    /// Just the relevant causes related to this fact which should appear in the causal graph
    pub fn into_causes(self) -> Vec<Cause<T>> {
        match self {
            Check::Pass => vec![],
            Check::Fail(cs) => cs,
        }
    }

    /// Did the check pass?
    pub fn is_pass(&self) -> bool {
        matches!(self, Check::Pass)
    }
}

/// An implication of a Fact being true.
/// More precisely a [logical consequent](https://en.wikipedia.org/wiki/Consequent) of a Fact.
#[derive(Clone, PartialEq, Eq, Hash, derive_more::From)]
pub enum Cause<T> {
    /// Just a Fact which can be checked for truth
    #[from]
    Fact(T),
    /// A collection of Causes joined together by logical OR
    Any(Vec<Cause<T>>),
    /// A collection of Causes joined together by logical AND
    Every(Vec<Cause<T>>),
}

impl<T: Fact> Cause<T> {
    /// Determine if the fact is true or not, and if not, return the causal DAG
    /// which shows the root cause(s) of the fact being false
    pub fn traverse(&self, ctx: &T::Context) -> Traversal<T> {
        traverse(self, ctx)
    }
}

impl<T: Display> std::fmt::Debug for Cause<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cause::Fact(fact) => f.write_str(&fact.to_string())?,
            Cause::Any(cs) => {
                f.write_str("Any(")?;
                f.debug_list().entries(cs.iter()).finish()?;
                f.write_str(")")?;
            }
            Cause::Every(cs) => {
                f.write_str("Every(")?;
                f.debug_list().entries(cs.iter()).finish()?;
                f.write_str(")")?;
            }
        }
        Ok(())
    }
}
