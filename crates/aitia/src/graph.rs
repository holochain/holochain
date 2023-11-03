use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::io::{Read, Write};

use crate::cause::*;

#[derive(Debug, derive_more::From, derive_more::Deref, derive_more::DerefMut)]
pub struct CauseTree<'c, T: Fact>(petgraph::graph::DiGraph<TreeNode<'c, T>, ()>);

#[derive(PartialEq, Eq, Hash)]
pub struct TreeNode<'c, T: Fact> {
    pub cause: Cause<T>,
    pub ctx: &'c T::Context,
}

impl<'c, T: Fact> Clone for TreeNode<'c, T> {
    fn clone(&self) -> Self {
        Self {
            cause: self.cause.clone(),
            ctx: self.ctx,
        }
    }
}

impl<'c, T: Fact> CauseTree<'c, T> {
    pub fn causes(&self) -> HashSet<Cause<T>> {
        self.node_weights()
            .map(|n| n.cause.clone())
            .collect::<HashSet<_>>()
    }

    pub fn print(&self) {
        let dot = format!(
            "{:?}",
            petgraph::dot::Dot::with_attr_getters(
                &**self,
                &[petgraph::dot::Config::EdgeNoLabel],
                &|_g, _e| "".to_string(),
                &|_g, _n| { "nojustify=true".to_string() },
            )
        );

        if let Ok(graph) = graph_easy(&dot) {
            println!("Original dot output:\n\n{}", dot);
            println!("`graph-easy` output:\n{}", graph);
        } else {
            println!(
                "`graph-easy` not installed. Original dot output:\n\n{}",
                dot
            );
        }
    }
}

impl<'c, T: Fact> Debug for TreeNode<'c, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.cause.explain(self.ctx))
    }
}

impl<'c, T: Fact> Default for CauseTree<'c, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(Debug, derive_more::Constructor)]
pub struct CauseError<F: Fact> {
    pub info: String,
    pub fact: Option<F>,
}

#[derive(Debug)]
pub struct CheckError<F: Fact>(pub Check<F>);

#[derive(Debug, derive_more::From)]
pub enum TraversalError<F: Fact> {
    Cause(CauseError<F>),
    Check(CheckError<F>),
}

#[derive(Debug, derive_more::From)]
pub enum Traversal<'c, T: Fact> {
    /// The target fact is true; nothing more needs to be said
    Pass,
    /// The target fact is false, and all paths which lead to true facts
    /// are present in this graph
    Fail {
        tree: CauseTree<'c, T>,
        passes: Vec<Cause<T>>,
        ctx: &'c T::Context,
    },
    /// A cause or check call returned an error during traversal
    TraversalError {
        error: CauseError<T>,
        tree: CauseTree<'c, T>,
    },
}

impl<'c, T: Fact> Traversal<'c, T> {
    pub fn fail(self) -> Option<(CauseTree<'c, T>, Vec<Cause<T>>)> {
        match self {
            Traversal::Fail { tree, passes, .. } => Some((tree, passes)),
            _ => None,
        }
    }
}

pub type TraversalMap<T> = HashMap<Cause<T>, Option<Check<T>>>;

/// Traverse the causal graph implied by the specified Cause.
///
/// The Traversal is recorded as a sparse adjacency matrix.
/// Each cause which is visited in the traversal gets added as a node in the graph,
/// initially with no edges.
/// For each cause with a failing "check", we recursively visit its cause(s).
/// Any time we encounter a cause with a passing "check", we backtrack and add edges
/// to add this path to the graph.
/// If a path ends in a failing check, or if it forms a loop without encountering
/// a passing check, we don't add that path to the graph.
#[tracing::instrument(skip(ctx))]
pub fn traverse<'c, F: Fact>(cause: &Cause<F>, ctx: &'c F::Context) -> Traversal<'c, F> {
    let mut table = TraversalMap::default();
    match traverse_inner(cause, ctx, &mut table) {
        Ok(maybe_check) => {
            if let Some(Check::Pass) = maybe_check {
                Traversal::Pass
            } else {
                let (tree, passes) = produce_graph(&table, cause, ctx);
                Traversal::Fail { tree, passes, ctx }
            }
        }
        Err(error) => {
            let (tree, _) = produce_graph(&table, cause, ctx);
            Traversal::TraversalError { tree, error }
        }
    }
}

fn traverse_inner<F: Fact>(
    cause: &Cause<F>,
    ctx: &F::Context,
    table: &mut TraversalMap<F>,
) -> Result<Option<Check<F>>, CauseError<F>> {
    tracing::trace!("enter {:?}", cause);
    match table.get(cause) {
        None => {
            tracing::trace!("marked visited");
            // Mark this node as visited but undetermined in case the traversal leads to a loop
            table.insert(cause.clone(), None);
        }
        Some(None) => {
            tracing::trace!("loop encountered");
            // We're currently processing a traversal that started from this cause.
            // Not even sure if this is even valid, but in any case
            // we certainly can't say anything about this traversal.
            return Ok(None);
        }
        Some(Some(check)) => {
            tracing::trace!("return cached: {:?}", check);
            return Ok(Some(check.clone()));
        }
    }

    let mut recursive_checks =
        |cs: &[Cause<F>]| -> Result<Vec<(Cause<F>, Check<F>)>, CauseError<F>> {
            let mut checks = vec![];
            for c in cs {
                if let Some(check) = traverse_inner(c, ctx, table)? {
                    checks.push((c.clone(), check));
                }
            }
            Ok(checks)
        };

    let check = match cause {
        Cause::Fact(f) => {
            if f.check(ctx) {
                tracing::trace!("fact pass");
                Check::Pass
            } else {
                if let Some(sub_cause) = f.cause(ctx)? {
                    tracing::trace!("fact fail with cause, traversing");
                    let check = traverse_inner(&sub_cause, ctx, table).map_err(|err| {
                        // Continue constructing the tree while we bubble up errors
                        tracing::error!("traversal ending due to error: {err:?}");
                        table.insert(cause.clone(), Some(Check::Fail(vec![sub_cause.clone()])));
                        err
                    })?;
                    tracing::trace!("traversal done, check: {:?}", check);
                    Check::Fail(vec![sub_cause])
                } else {
                    tracing::trace!("fact fail with no cause, terminating");
                    Check::Fail(vec![])
                }
            }
        }
        Cause::Any(_, cs) => {
            let checks = recursive_checks(cs).map_err(|err| {
                // Continue constructing the tree while we bubble up errors
                tracing::error!("traversal ending due to error: {err:?}");
                table.insert(cause.clone(), Some(Check::Fail(cs.clone())));
                err
            })?;
            tracing::trace!("Any. checks: {:?}", checks);
            if checks.is_empty() {
                // All loops
                tracing::debug!("All loops");
                return Ok(None);
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
        Cause::Every(_, cs) => {
            let checks = recursive_checks(cs).map_err(|err| {
                // Continue constructing the tree while we bubble up errors
                tracing::error!("traversal ending due to error: {err:?}");
                table.insert(cause.clone(), Some(Check::Fail(cs.clone())));
                err
            })?;

            tracing::trace!("Every. checks: {:?}", checks);
            if checks.is_empty() {
                // All loops
                tracing::debug!("All loops");
                return Ok(None);
            }
            let fails = checks.iter().filter(|(_, check)| !check.is_pass()).count();
            let causes: Vec<_> = checks.into_iter().map(|(cause, _)| cause).collect();
            tracing::trace!("Every. num fails: {}", fails);
            if fails == 0 {
                Check::Pass
            } else {
                Check::Fail(causes)
            }
        }
    };
    table.insert(cause.clone(), Some(check.clone()));
    tracing::trace!("exit. check: {:?}", check);
    Ok(Some(check))
}

/// Prune away any extraneous nodes or edges from a Traversal.
/// After pruning, the graph contains all edges starting with the specified cause
/// and ending with a true cause.
/// Passing facts are returned separately.
pub fn prune_traversal<'a, 'b: 'a, T: Fact + Eq + Hash>(
    table: &'a TraversalMap<T>,
    start: &'b Cause<T>,
) -> (HashMap<&'a Cause<T>, &'a [Cause<T>]>, Vec<&'a Cause<T>>) {
    let mut sub = HashMap::<&Cause<T>, &[Cause<T>]>::new();
    let mut passes = vec![];
    let mut to_add = vec![start];

    while let Some(next) = to_add.pop() {
        match table[&next].as_ref() {
            Some(Check::Fail(causes)) => {
                let old = sub.insert(next, causes.as_slice());
                if let Some(old) = old {
                    assert_eq!(
                        old, causes,
                        "Looped back to same node, but with different children?"
                    );
                } else {
                    to_add.extend(causes.iter());
                }
            }
            Some(Check::Pass) => {
                passes.push(next);
            }
            None => {}
        }
    }
    (sub, passes)
}

pub fn produce_graph<'a, 'b: 'a, 'c, T: Fact + Eq + Hash>(
    table: &'a TraversalMap<T>,
    start: &'b Cause<T>,
    ctx: &'c T::Context,
) -> (CauseTree<'c, T>, Vec<Cause<T>>) {
    let mut g = CauseTree::default();

    let (sub, passes) = prune_traversal(table, start);

    let rows: Vec<_> = sub.into_iter().collect();
    let mut nodemap = HashMap::new();
    for (i, (k, _)) in rows.iter().enumerate() {
        let id = g.add_node(TreeNode {
            cause: (*k).to_owned(),
            ctx,
        });
        nodemap.insert(k, id);
        assert_eq!(id.index(), i);
    }

    for (k, v) in rows.iter() {
        for c in v.iter() {
            if let (Some(k), Some(c)) = (nodemap.get(k), nodemap.get(&c)) {
                g.add_edge(*k, *c, ());
            }
        }
    }

    (g, passes.into_iter().cloned().collect())
}

/// If a `graph-easy` binary is installed, render an ASCII graph from the
/// provided dot syntax.
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
