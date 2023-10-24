use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Read, Write};

use crate::cause::*;

pub type Tree<T> = petgraph::graph::DiGraph<Cause<T>, ()>;

#[derive(derive_more::Deref, derive_more::DerefMut, derive_more::From, derive_more::Into)]
pub struct Traversal<T: Fact>(TraversalMap<T>);

pub type TraversalMap<T> = HashMap<Cause<T>, Option<Check<T>>>;

impl<T: Fact> Default for Traversal<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Fact> std::fmt::Debug for Traversal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Traversal");
        for (k, v) in self.iter() {
            s.field(&format!("{:?}", k), v);
        }
        s.finish()
    }
}

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
pub fn traverse<F: Fact>(cause: &Cause<F>, ctx: &F::Context) -> Traversal<F> {
    let mut table = Traversal::default();
    traverse_inner(cause, ctx, &mut table);
    table
}

fn traverse_inner<F: Fact>(
    cause: &Cause<F>,
    ctx: &F::Context,
    table: &mut Traversal<F>,
) -> Option<Check<F>> {
    tracing::trace!("enter");
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
            return None;
        }
        Some(Some(check)) => {
            tracing::trace!("return cached: {:?}", check);
            return Some(check.clone());
        }
    }

    let check = match cause {
        Cause::Fact(f) => {
            if f.check(ctx) {
                tracing::trace!("fact pass");
                Check::Pass
            } else {
                if let Some(cause) = f.cause(ctx) {
                    tracing::trace!("fact fail with cause, traversing");
                    let check = traverse_inner(&cause, ctx, table)?;
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
                .filter_map(|c| Some((c.clone(), traverse_inner(c, ctx, table)?)))
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
    table.insert(cause.clone(), Some(check.clone()));
    tracing::trace!("exit. check: {:?}", check);
    Some(check)
}

/// Prune away any extraneous nodes or edges from a Traversal.
/// After pruning, the graph contains all edges starting with the specified cause
/// and ending with a true cause.
pub fn prune_traversal<'a, 'b: 'a, T: Fact + Eq + Hash>(
    table: &'a TraversalMap<T>,
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

pub fn produce_graph<'a, 'b: 'a, T: Fact + Eq + Hash>(
    table: &'a Traversal<T>,
    start: &'b Cause<T>,
) -> Tree<T> {
    let mut g = Tree::new();

    let sub = prune_traversal(&**table, start);

    let rows: Vec<_> = sub.into_iter().collect();
    let mut nodemap = HashMap::new();
    for (i, (k, _)) in rows.iter().enumerate() {
        let id = g.add_node((*k).to_owned());
        nodemap.insert(k, id);
        assert_eq!(id.index(), i);
    }

    for (k, v) in rows.iter() {
        for c in v.iter() {
            g.add_edge(nodemap[k], nodemap[&c], ());
        }
    }

    g
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
