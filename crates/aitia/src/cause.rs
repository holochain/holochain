use std::fmt::Display;

use crate::graph::{traverse, Traversal};

pub trait FactTraits: Clone + Eq + std::fmt::Display + std::fmt::Debug + std::hash::Hash {}
impl<T> FactTraits for T where T: Clone + Eq + std::fmt::Display + std::fmt::Debug + std::hash::Hash {}

pub trait Context<F: Fact> {
    fn check(&self, fact: &F) -> bool;
}

pub trait Fact: FactTraits {
    fn explain(&self, _ctx: &impl Context<Self>) -> String {
        self.to_string()
    }
    fn cause(&self, ctx: &impl Context<Self>) -> Option<Cause<Self>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check<T: Fact> {
    Pass,
    Fail(Vec<Cause<T>>),
}

impl<T: Fact> Check<T> {
    pub fn causes(&self) -> &[Cause<T>] {
        match self {
            Check::Pass => &[],
            Check::Fail(cs) => cs.as_slice(),
        }
    }

    pub fn into_causes(self) -> Vec<Cause<T>> {
        match self {
            Check::Pass => vec![],
            Check::Fail(cs) => cs,
        }
    }

    pub fn is_pass(&self) -> bool {
        matches!(self, Check::Pass)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, derive_more::From)]
pub enum Cause<T> {
    #[from]
    Fact(T),
    Any(Vec<Cause<T>>),
    Every(Vec<Cause<T>>),
}

impl<T: Fact> Cause<T> {
    pub fn traverse(&self, ctx: &impl Context<T>) -> Traversal<T> {
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
