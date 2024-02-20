use crate::{dep::DepResult, traversal::TraversalResult};

pub trait FactTraits: Clone + Eq + std::fmt::Debug + std::hash::Hash {}
impl<T> FactTraits for T where T: Clone + Eq + std::fmt::Debug + std::hash::Hash {}

pub trait Fact: FactTraits {
    type Context;

    fn check(&self, ctx: &Self::Context) -> bool;

    fn dep(&self, ctx: &Self::Context) -> DepResult<Self>;

    fn explain(&self, _ctx: &Self::Context) -> String {
        format!("{:?}", self)
    }

    fn traverse(self, ctx: &Self::Context) -> TraversalResult<'_, Self> {
        crate::traversal::traverse(self, ctx)
    }
}
