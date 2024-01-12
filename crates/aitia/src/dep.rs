use crate::Fact;

#[derive(Clone, PartialEq, Eq, Hash, derive_more::From)]
pub enum Dep<T> {
    #[from]
    Fact(T),
    Any(Option<String>, Vec<Dep<T>>),
    Every(Option<String>, Vec<Dep<T>>),
}

impl<T: Fact> Dep<T> {
    pub fn any(deps: Vec<Dep<T>>) -> Self {
        Self::Any(None, deps)
    }

    pub fn any_named(name: impl ToString, deps: Vec<Dep<T>>) -> Self {
        Self::Any(Some(name.to_string()), deps)
    }

    pub fn every(deps: Vec<Dep<T>>) -> Self {
        Self::Every(None, deps)
    }

    pub fn every_named(name: impl ToString, deps: Vec<Dep<T>>) -> Self {
        Self::Every(Some(name.to_string()), deps)
    }

    pub fn fact(&self) -> Option<&T> {
        match self {
            Dep::Fact(f) => Some(f),
            _ => None,
        }
    }

    pub fn into_fact(self) -> Option<T> {
        match self {
            Dep::Fact(f) => Some(f),
            _ => None,
        }
    }

    pub fn explain(&self, ctx: &T::Context) -> String {
        match &self {
            Dep::Fact(fact) => fact.explain(ctx),
            Dep::Any(name, cs) => {
                let cs = cs.iter().map(|c| c.explain(ctx)).collect::<Vec<_>>();
                if let Some(name) = name {
                    format!("ANY({:#?})", (name, cs))
                } else {
                    format!("ANY({:#?})", cs)
                }
            }
            Dep::Every(name, cs) => {
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

impl<T: std::fmt::Debug> std::fmt::Debug for Dep<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dep::Fact(fact) => f.write_fmt(format_args!("{:?}", fact))?,
            Dep::Any(name, cs) => {
                f.write_fmt(format_args!("ANY({:#?})", (name, cs)))?;
            }
            Dep::Every(name, cs) => {
                f.write_fmt(format_args!("EVERY({:#?})", (name, cs)))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct DepError<F: Fact> {
    pub info: String,
    pub fact: Option<F>,
}

pub type DepResult<F> = Result<Option<Dep<F>>, DepError<F>>;
