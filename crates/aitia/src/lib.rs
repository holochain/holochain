use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    io::{Read, Write},
};

use petgraph::prelude::DiGraph;

pub type Tree<T> = petgraph::graph::DiGraph<Cause<T>, ()>;

#[derive(derive_more::Deref, derive_more::DerefMut, derive_more::From, derive_more::Into)]
pub struct Table<T: Fact>(TableMap<T>);

pub type TableMap<T> = HashMap<Cause<T>, Option<Check<T>>>;
pub type TableMapRef<'a, T> = HashMap<&'a Cause<T>, Option<&'a Check<T>>>;

impl<T: Fact> Default for Table<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Fact> Debug for Table<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Table");
        for (k, v) in self.iter() {
            s.field(&format!("{:?}", k), v);
        }
        s.finish()
    }
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

    pub fn is_pass(&self) -> bool {
        matches!(self, Check::Pass)
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Cause<T> {
    Fact(T),
    Any(Vec<Cause<T>>),
    // Every(Vec<Cause<T>>),
}

impl<T: Fact> std::fmt::Debug for Cause<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cause::Fact(fact) => f.write_str(&fact.explain()),
            Cause::Any(cs) => f.write_fmt(format_args!(
                "Any({})",
                cs.into_iter()
                    .map(|c| format!("{:?}", c))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
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

impl<F: Fact> Cause<F> {
    #[tracing::instrument(skip(ctx, table))]
    pub fn traverse(&self, ctx: &F::Context, table: &mut Table<F>) -> Option<Check<F>> {
        tracing::trace!("enter");
        match table.get(self) {
            None => {
                tracing::trace!("marked visited");
                // Mark this node as visited but undetermined in case the traversal leads to a loop
                table.insert(self.clone(), None);
            }
            Some(None) => {
                tracing::trace!("loop encountered");
                // We're currently processing a traversal that started from this cause.
                // Not even sure if this is even valid, but in any case
                // we certainly can't say anything about this traversal.
                return None;
            }
            Some(Some(check)) => {
                tracing::trace!("return cached: {:?}", check);
                return Some(check.clone());
            }
        }

        let check = match self {
            Cause::Fact(f) => {
                if f.check(ctx) {
                    tracing::trace!("fact pass");
                    Check::Pass
                } else {
                    if let Some(cause) = f.cause(ctx) {
                        tracing::trace!("fact fail with cause, traversing");
                        let check = cause.traverse(ctx, table)?;
                        tracing::trace!("traversal done, check: {:?}", check);
                        Check::Fail(vec![cause])
                    } else {
                        tracing::trace!("fact fail with no cause, terminating");
                        return None;
                    }
                }
            }
            Cause::Any(cs) => {
                let checks: Vec<_> = cs
                    .iter()
                    .filter_map(|c| Some((c.clone(), c.traverse(ctx, table)?)))
                    .collect();
                tracing::trace!("Any. checks: {:?}", checks);
                if checks.is_empty() {
                    // All loops
                    tracing::debug!("All loops");
                    return None;
                }
                let num_checks = checks.len();
                let fails: Vec<_> = checks
                    .into_iter()
                    .filter_map(|(cause, check)| (!check.is_pass()).then_some(cause))
                    .collect();
                tracing::trace!("Any. fails: {:?}", fails);
                if fails.len() < num_checks {
                    Check::Pass
                } else {
                    Check::Fail(fails)
                }
            }
        };
        table.insert(self.clone(), Some(check.clone()));
        tracing::trace!("exit. check: {:?}", check);
        Some(check)
    }

    pub fn table(&self, ctx: &F::Context) -> Table<F> {
        let mut table = Table::default();
        self.traverse(ctx, &mut table);
        table
    }
}

pub fn prune<'a, 'b: 'a, T: Fact + Eq + Hash>(
    table: &'a TableMap<T>,
    start: &'b Cause<T>,
) -> HashMap<&'a Cause<T>, &'a [Cause<T>]> {
    let mut sub = HashMap::<&Cause<T>, &[Cause<T>]>::new();
    let mut to_add = vec![start];

    while let Some(next) = to_add.pop() {
        let causes = table[&next].as_ref().map(|c| c.causes()).unwrap_or(&[]);
        to_add.extend(causes.iter());
        sub.insert(next, causes);
    }
    sub
}

pub fn graph<'a, 'b: 'a, T: Fact + Eq + Hash>(
    table: &'a Table<T>,
    start: &'b Cause<T>,
) -> DiGraph<&'a Cause<T>, ()> {
    let mut g = DiGraph::new();

    let sub = prune(&**table, start);

    let rows: Vec<_> = sub.iter().collect();
    let mut nodemap = HashMap::new();
    for (i, (k, _)) in rows.iter().enumerate() {
        let id = g.add_node(**k);
        nodemap.insert(**k, id);
        assert_eq!(id.index(), i);
    }

    for (k, v) in rows.iter() {
        for c in v.iter() {
            g.add_edge(nodemap[**k], nodemap[c], ());
        }
    }

    g
}

pub trait Fact: Sized + Clone + Eq + std::fmt::Debug + std::hash::Hash {
    type Context;

    fn cause(&self, ctx: &Self::Context) -> Option<Cause<Self>>;
    fn check(&self, ctx: &Self::Context) -> bool;
    fn explain(&self) -> String;
}

pub fn graph_easy(dot: &str) -> anyhow::Result<String> {
    let process = std::process::Command::new("graph-easy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    process.stdin.unwrap().write_all(dot.as_bytes()).unwrap();
    let mut s = String::new();
    process.stdout.unwrap().read_to_string(&mut s).unwrap();

    Ok(s)
}

#[cfg(test)]
mod tests {

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

    type StepValues = Box<dyn Fn(&Step) -> bool>;

    impl Fact for Step {
        type Context = StepValues;

        fn cause(&self, _ctx: &Self::Context) -> Option<Cause<Self>> {
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
    }

    #[test]
    fn one() {
        holochain_trace::test_run().ok().unwrap();

        let fatma_store = Cause::Fact(Step {
            which: false,
            stage: Stage::Store,
        });

        let checks: StepValues = Box::new(|step: &Step| match (step.which, step.stage) {
            (true, Stage::Create) => true,
            // (false, Stage::Create) => true,

            // this leads to Trudy Create in the graph, which is wrong
            // TODO: revisit pruning branches that terminate with false, including loops
            //       basically we only want to create edges on the way back up from either
            //       a None, or a Loop detection
            // (true, Stage::ReceiveA) => true,

            // TODO: if all are false, then that doesn't work either
            _ => false,
            // (true, Stage::Fetch) => todo!(),
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

        let table = fatma_store.table(&checks);
        let sub = prune(&table, &fatma_store);

        println!("TABLE\n{:#?}", table);
        println!("SUBTABLE\n{:#?}", sub);

        let g = graph(&table, &fatma_store);

        let dot = format!(
            "{:?}",
            petgraph::dot::Dot::with_config(&g, &[petgraph::dot::Config::EdgeNoLabel],)
        );

        if let Ok(graph) = graph_easy(&dot) {
            println!("`graph-easy` output:\n{}", graph);
        } else {
            println!("`graph-easy` not installed. Original dot output: {}", dot);
        }
    }
}
