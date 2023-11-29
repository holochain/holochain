use crate::graph::{traverse, CauseError, Traversal};

pub trait FactTraits: Clone + Eq + std::fmt::Debug + std::hash::Hash {}
impl<T> FactTraits for T where T: Clone + Eq + std::fmt::Debug + std::hash::Hash {}

pub trait Fact: FactTraits {
    type Context;

    fn explain(&self, _ctx: &Self::Context) -> String {
        format!("{:?}", self)
    }
    fn cause(&self, ctx: &Self::Context) -> CauseResult<Self>;
    fn check(&self, ctx: &Self::Context) -> bool;
}

pub type CauseResult<F> = Result<Option<Cause<F>>, CauseError<F>>;

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
    Any(Option<String>, Vec<Cause<T>>),
    Every(Option<String>, Vec<Cause<T>>),
}

impl<T: Fact> Cause<T> {
    pub fn any(causes: Vec<Cause<T>>) -> Self {
        Self::Any(None, causes)
    }

    pub fn any_named(name: impl ToString, causes: Vec<Cause<T>>) -> Self {
        Self::Any(Some(name.to_string()), causes)
    }

    pub fn every(causes: Vec<Cause<T>>) -> Self {
        Self::Every(None, causes)
    }

    pub fn every_named(name: impl ToString, causes: Vec<Cause<T>>) -> Self {
        Self::Every(Some(name.to_string()), causes)
    }

    pub fn traverse<'c>(&self, ctx: &'c T::Context) -> Traversal<'c, T> {
        traverse(self, ctx)
    }

    pub fn explain(&self, ctx: &T::Context) -> String {
        match &self {
            Cause::Fact(fact) => fact.explain(ctx),
            Cause::Any(name, cs) => {
                let cs = cs.iter().map(|c| c.explain(ctx)).collect::<Vec<_>>();
                if let Some(name) = name {
                    format!("ANY({:#?})", (name, cs))
                } else {
                    format!("ANY({:#?})", cs)
                }
            }
            Cause::Every(name, cs) => {
                let cs = cs.iter().map(|c| c.explain(ctx)).collect::<Vec<_>>();
                if let Some(name) = name {
                    format!("EVERY({:#?})", (name, cs))
                } else {
                    format!("EVERY({:#?})", cs)
                }
            }
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Cause<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cause::Fact(fact) => f.write_fmt(format_args!("{:?}", fact))?,
            Cause::Any(name, cs) => {
                f.write_fmt(format_args!("ANY({:#?})", (name, cs)))?;
            }
            Cause::Every(name, cs) => {
                f.write_fmt(format_args!("EVERY({:#?})", (name, cs)))?;
            }
        }
        Ok(())
    }
}
