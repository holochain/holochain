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
pub enum Check<T: Fact> {
    Pass,
    Fail(Vec<Dep<T>>),
}

impl<T: Fact> Check<T> {
    pub fn is_pass(&self) -> bool {
        matches!(self, Check::Pass)
    }
}

#[derive(Debug)]
pub struct CheckError<F: Fact>(pub Check<F>);
