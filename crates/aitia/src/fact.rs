use crate::{graph::DepError, Dep};

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

pub type DepResult<F> = Result<Option<Dep<F>>, DepError<F>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check<T: Fact> {
    Pass,
    Fail(Vec<Dep<T>>),
}

impl<T: Fact> Check<T> {
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
        matches!(self, Check::Pass)
    }
}
