// TODO: remove
#![allow(warnings)]

use std::{collections::HashMap, fmt::Display, hash::Hash};

use petgraph::{
    data::{Build, FromElements},
    prelude::{DiGraph, DiGraphMap},
};

// #[macro_use]
// mod cause;
// mod fact;
// pub use cause::*;
// pub use fact::*;

// #[macro_use]
// pub(crate) mod report;
// pub use report::*;

// #[cfg(test)]
// pub mod test_fact;

// #[derive(Default)]
// pub struct Context;

pub type Tree<T> = petgraph::graph::DiGraph<Cause<T>, ()>;
pub type Table<T> = HashMap<Cause<T>, Vec<Cause<T>>>;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Cause<T> {
    Any(Vec<Cause<T>>),
    Every(Vec<Cause<T>>),
    Fact(T),
}

impl<F: Fact> Cause<F> {
    pub fn table(&self, ctx: &F::Context) -> Table<F> {
        let mut table = Table::new();
        traverse(self, ctx, &mut table);
        table
    }
}

impl<T: Fact> std::fmt::Debug for Cause<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cause::Any(cs) => f.write_fmt(format_args!(
                "Any({})",
                cs.into_iter()
                    .map(|c| format!("{:?}", c))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Cause::Every(cs) => f.write_fmt(format_args!(
                "Every({})",
                cs.into_iter()
                    .map(|c| format!("{:?}", c))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Cause::Fact(fact) => f.write_str(&fact.explain()),
        }
    }
}

fn traverse<F: Fact>(
    current: &Cause<F>,
    ctx: &F::Context,
    table: &mut Table<F>,
) -> Option<Traversal> {
    dbg!(current);
    if table.contains_key(&current) {
        // Prevent loops
        return Some(Traversal::Loop);
    } else {
        table.insert(current.clone(), vec![]);
    }
    dbg!(current);

    match &current {
        Cause::Fact(f) => {
            if f.check(ctx) {
                dbg!(current);
                Some(Traversal::Pass)
            } else {
                // If the check fails and there is no cause,
                // then completely throw away this whole branch.
                dbg!(current);
                let cause = f.cause(ctx)?;
                let t = traverse(&cause, ctx, table)?;
                let old = table.insert(current.clone(), vec![cause]);
                assert_eq!(old, Some(vec![]));
                dbg!(current);
                Some(Traversal::Fail)
            }
        }
        Cause::Any(cs) => {
            dbg!(current);
            // XXX: the traversal could be short-circuited if any pass
            let ts: Vec<_> = cs.iter().filter_map(|c| traverse(c, ctx, table)).collect();
            if ts.is_empty() {
                dbg!(current);
                None
            } else if ts.iter().any(|t| *t == Traversal::Pass) {
                dbg!(current);
                Some(Traversal::Pass)
            } else {
                table.insert(current.clone(), cs.to_vec());
                dbg!(current);
                Some(Traversal::Fail)
            }
        }
        Cause::Every(cs) => {
            unimplemented!();
            // XXX: the traversal could be short-circuited if any pass
            let ts: Vec<_> = cs
                .iter()
                .filter_map(|c| Some((c, traverse(c, ctx, table)?)))
                .collect();
            if ts.is_empty() {
                None
            } else if ts.iter().all(|(_, t)| *t == Traversal::Pass) {
                Some(Traversal::Pass)
            } else {
                let cs = ts
                    .into_iter()
                    .filter_map(|(c, t)| (t == Traversal::Fail).then_some(c.clone()))
                    .collect();
                table.insert(current.clone(), cs);
                Some(Traversal::Fail)
            }
        }
    }
}

pub fn graph<T: Eq + Hash>(table: &Table<T>) -> DiGraph<&Cause<T>, ()> {
    let mut g = DiGraph::new();
    let rows: Vec<_> = table.iter().collect();
    let mut nodemap = HashMap::new();
    for (i, (k, _)) in rows.iter().enumerate() {
        let id = g.add_node(*k);
        nodemap.insert(*k, id);
        assert_eq!(id.index(), i);
    }

    for (k, v) in rows.iter() {
        for c in v.iter() {
            g.add_edge(nodemap[k], nodemap[c], ());
        }
    }

    g
}

#[derive(PartialEq, Eq)]
enum Traversal {
    Pass,
    Fail,
    Loop,
}

pub trait Fact: Sized + Clone + Eq + std::fmt::Debug + std::hash::Hash {
    type Context;

    fn cause(&self, ctx: &Self::Context) -> Option<Cause<Self>>;
    fn check(&self, ctx: &Self::Context) -> bool;
    fn explain(&self) -> String;
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
    struct Step {
        which: bool,
        stage: Stage,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Stage {
        Create,
        Fetch,
        ReceiveA,
        ReceiveB,
        Store,
        SendA,
        SendB,
    }

    pub type StepValues = Box<dyn Fn(&Step) -> bool>;

    impl Fact for Step {
        type Context = StepValues;

        fn cause(&self, ctx: &Self::Context) -> Option<Cause<Self>> {
            use Stage::*;
            match self.stage {
                Create => None,
                Fetch => Some(Cause::Any(vec![self.mine(ReceiveA), self.mine(ReceiveB)])),
                ReceiveA => Some(self.theirs(SendA)),
                ReceiveB => Some(self.theirs(SendB)),
                Store => Some(Cause::Any(vec![self.mine(Create), self.mine(Fetch)])),
                SendA => Some(self.mine(Store)),
                SendB => Some(self.mine(Store)),
            }
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            (ctx)(self)
        }

        fn explain(&self) -> String {
            let who = match self.which {
                false => "Fatma",
                true => "Trudy",
            };
            format!("{} {:?}", who, self.stage)
        }
    }

    impl Step {
        pub fn mine(&self, stage: Stage) -> Cause<Self> {
            Cause::Fact(Self {
                which: self.which,
                stage,
            })
        }

        pub fn theirs(&self, stage: Stage) -> Cause<Self> {
            Cause::Fact(Self {
                which: !self.which,
                stage,
            })
        }

        pub fn any(cs: Vec<Arc<Cause<Self>>>) -> Arc<Cause<Self>> {
            Arc::new(Cause::Any(cs.into_iter().map(|c| (*c).clone()).collect()))
        }
    }

    #[test]
    fn one() {
        let fatma_store = Cause::Fact(Step {
            which: false,
            stage: Stage::Store,
        });

        let checks: StepValues = Box::new(|step: &Step| match (step.which, step.stage) {
            (true, Stage::Create) => true,
            _ => false,
            // (true, Stage::Fetch) => todo!(),
            // (true, Stage::ReceiveA) => todo!(),
            // (true, Stage::ReceiveB) => todo!(),
            // (true, Stage::Store) => todo!(),
            // (true, Stage::SendA) => todo!(),
            // (true, Stage::SendB) => todo!(),
            // (false, Stage::Create) => todo!(),
            // (false, Stage::Fetch) => todo!(),
            // (false, Stage::ReceiveA) => todo!(),
            // (false, Stage::ReceiveB) => todo!(),
            // (false, Stage::Store) => todo!(),
            // (false, Stage::SendA) => todo!(),
            // (false, Stage::SendB) => todo!(),
        });

        let t = fatma_store.table(&checks);
        let g = graph(&t);

        println!(
            "{:?}",
            petgraph::dot::Dot::with_config(&g, &[petgraph::dot::Config::EdgeNoLabel])
        )
    }
}
