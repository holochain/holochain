use crate::{dep::DepResult, Dep};

pub trait FactTraits: Clone + Eq + std::fmt::Debug + std::hash::Hash {}
impl<T> FactTraits for T where T: Clone + Eq + std::fmt::Debug + std::hash::Hash {}

pub trait Fact: FactTraits {
    type Context;

    fn check(&self, ctx: &Self::Context) -> bool;

    fn dep(&self, ctx: &Self::Context) -> DepResult<Self>;

    fn explain(&self, _ctx: &Self::Context) -> String {
        format!("{:?}", self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraversalStep<T: Fact> {
    /// This node terminates the traversal due to its check() status.
    Terminate,
    /// The traversal should continue with the following nodes.
    Continue(Vec<Dep<T>>),
}

impl<T: Fact> TraversalStep<T> {
    // pub fn deps(&self) -> &[Dep<T>] {
    //     match self {
    //         Check::Pass => &[],
    //         Check::Fail(cs) => cs.as_slice(),
    //     }
    // }

    // pub fn into_deps(self) -> Vec<Dep<T>> {
    //     match self {
    //         Check::Pass => vec![],
    //         Check::Fail(cs) => cs,
    //     }
    // }

    pub fn is_pass(&self) -> bool {
        matches!(self, TraversalStep::Terminate)
    }
}

#[derive(Debug)]
pub struct CheckError<F: Fact>(pub TraversalStep<F>);
