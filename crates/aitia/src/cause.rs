pub trait Fact: Sized + Clone + Eq + std::fmt::Display + std::fmt::Debug + std::hash::Hash {
    type Context;

    fn explain(&self, ctx: &Self::Context) -> String;
    fn cause(&self, ctx: &Self::Context) -> Option<Cause<Self>>;
    fn check(&self, ctx: &Self::Context) -> bool;
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
    // Every(Vec<Cause<T>>),
}

impl<T: Fact> Cause<T> {
    pub fn graph(&self, ctx: &T::Context) -> petgraph::graph::DiGraph<Self, ()> {
        use crate::graph::*;

        let t = traverse(self, ctx);
        produce_graph(&t, self)
    }
}

impl<T: Fact> std::fmt::Debug for Cause<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cause::Fact(fact) => f.write_str(&fact.to_string()),
            Cause::Any(cs) => f.write_fmt(format_args!("Any({:#?})", cs)),
            // cs.into_iter()
            //     .map(|c| format!("{:?}", c))
            //     .collect::<Vec<_>>()
            //     .join(", ")

            // Cause::Every(cs) => f.write_fmt(format_args!(
            //     "Every({})",
            //     cs.into_iter()
            //         .map(|c| format!("{:?}", c))
            //         .collect::<Vec<_>>()
            //         .join(", ")
            // )),
        }
    }
}
