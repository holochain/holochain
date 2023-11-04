use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use crate::fact::{Check, CheckError};
use crate::graph::{DepGraph, GraphNode};
use crate::{dep::*, Fact};

#[derive(Debug, derive_more::From)]
pub enum TraversalError<F: Fact> {
    Dep(DepError<F>),
    Check(CheckError<F>),
}

#[derive(Debug, derive_more::From)]
pub enum Traversal<'c, T: Fact> {
    /// The target fact is true; nothing more needs to be said
    Pass,
    /// The target fact is false, and all paths which lead to true facts
    /// are present in this graph
    Fail {
        tree: DepGraph<'c, T>,
        passes: Vec<Dep<T>>,
        ctx: &'c T::Context,
    },
    /// A dep or check call returned an error during traversal
    TraversalError {
        error: DepError<T>,
        tree: DepGraph<'c, T>,
    },
}

impl<'c, T: Fact> Traversal<'c, T> {
    pub fn fail(self) -> Option<(DepGraph<'c, T>, Vec<Dep<T>>)> {
        match self {
            Traversal::Fail { tree, passes, .. } => Some((tree, passes)),
            _ => None,
        }
    }
}

pub type TraversalMap<T> = HashMap<Dep<T>, Option<Check<T>>>;

/// Traverse the causal graph implied by the specified Dep.
///
/// The Traversal is recorded as a sparse adjacency matrix.
/// Each dep which is visited in the traversal gets added as a node in the graph,
/// initially with no edges.
/// For each dep with a failing "check", we recursively visit its dep(s).
/// Any time we encounter a dep with a passing "check", we backtrack and add edges
/// to add this path to the graph.
/// If a path ends in a failing check, or if it forms a loop without encountering
/// a passing check, we don't add that path to the graph.
#[tracing::instrument(skip(ctx))]
pub fn traverse<'c, F: Fact>(dep: &Dep<F>, ctx: &'c F::Context) -> Traversal<'c, F> {
    let mut table = TraversalMap::default();
    match traverse_inner(dep, ctx, &mut table) {
        Ok(maybe_check) => {
            if let Some(Check::Pass) = maybe_check {
                Traversal::Pass
            } else {
                let (tree, passes) = produce_graph(&table, dep, ctx);
                Traversal::Fail { tree, passes, ctx }
            }
        }
        Err(error) => {
            let (tree, _) = produce_graph(&table, dep, ctx);
            Traversal::TraversalError { tree, error }
        }
    }
}

fn traverse_inner<F: Fact>(
    dep: &Dep<F>,
    ctx: &F::Context,
    table: &mut TraversalMap<F>,
) -> Result<Option<Check<F>>, DepError<F>> {
    tracing::trace!("enter {:?}", dep);
    match table.get(dep) {
        None => {
            tracing::trace!("marked visited");
            // Mark this node as visited but undetermined in case the traversal leads to a loop
            table.insert(dep.clone(), None);
        }
        Some(None) => {
            tracing::trace!("loop encountered");
            // We're currently processing a traversal that started from this dep.
            // Not even sure if this is even valid, but in any case
            // we certainly can't say anything about this traversal.
            return Ok(None);
        }
        Some(Some(check)) => {
            tracing::trace!("return cached: {:?}", check);
            return Ok(Some(check.clone()));
        }
    }

    let mut recursive_checks = |cs: &[Dep<F>]| -> Result<Vec<(Dep<F>, Check<F>)>, DepError<F>> {
        let mut checks = vec![];
        for c in cs {
            if let Some(check) = traverse_inner(c, ctx, table)? {
                checks.push((c.clone(), check));
            }
        }
        Ok(checks)
    };

    let check = match dep {
        Dep::Fact(f) => {
            if f.check(ctx) {
                tracing::trace!("fact pass");
                Check::Pass
            } else {
                if let Some(sub_dep) = f.dep(ctx)? {
                    tracing::trace!("fact fail with dep, traversing");
                    let check = traverse_inner(&sub_dep, ctx, table).map_err(|err| {
                        // Continue constructing the tree while we bubble up errors
                        tracing::error!("traversal ending due to error: {err:?}");
                        table.insert(dep.clone(), Some(Check::Fail(vec![sub_dep.clone()])));
                        err
                    })?;
                    tracing::trace!("traversal done, check: {:?}", check);
                    Check::Fail(vec![sub_dep])
                } else {
                    tracing::trace!("fact fail with no dep, terminating");
                    Check::Fail(vec![])
                }
            }
        }
        Dep::Any(_, cs) => {
            let checks = recursive_checks(cs).map_err(|err| {
                // Continue constructing the tree while we bubble up errors
                tracing::error!("traversal ending due to error: {err:?}");
                table.insert(dep.clone(), Some(Check::Fail(cs.clone())));
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
                .filter_map(|(dep, check)| (!check.is_pass()).then_some(dep))
                .collect();
            tracing::trace!("Any. fails: {:?}", fails);
            if fails.len() < num_checks {
                Check::Pass
            } else {
                Check::Fail(fails)
            }
        }
        Dep::Every(_, cs) => {
            let checks = recursive_checks(cs).map_err(|err| {
                // Continue constructing the tree while we bubble up errors
                tracing::error!("traversal ending due to error: {err:?}");
                table.insert(dep.clone(), Some(Check::Fail(cs.clone())));
                err
            })?;

            tracing::trace!("Every. checks: {:?}", checks);
            if checks.is_empty() {
                // All loops
                tracing::debug!("All loops");
                return Ok(None);
            }
            let fails = checks.iter().filter(|(_, check)| !check.is_pass()).count();
            let deps: Vec<_> = checks.into_iter().map(|(dep, _)| dep).collect();
            tracing::trace!("Every. num fails: {}", fails);
            if fails == 0 {
                Check::Pass
            } else {
                Check::Fail(deps)
            }
        }
    };
    table.insert(dep.clone(), Some(check.clone()));
    tracing::trace!("exit. check: {:?}", check);
    Ok(Some(check))
}

/// Prune away any extraneous nodes or edges from a Traversal.
/// After pruning, the graph contains all edges starting with the specified dep
/// and ending with a true dep.
/// Passing facts are returned separately.
pub fn prune_traversal<'a, 'b: 'a, T: Fact + Eq + Hash>(
    table: &'a TraversalMap<T>,
    start: &'b Dep<T>,
) -> (HashMap<&'a Dep<T>, &'a [Dep<T>]>, Vec<&'a Dep<T>>) {
    let mut sub = HashMap::<&Dep<T>, &[Dep<T>]>::new();
    let mut passes = vec![];
    let mut to_add = vec![start];

    while let Some(next) = to_add.pop() {
        match table[&next].as_ref() {
            Some(Check::Fail(deps)) => {
                let old = sub.insert(next, deps.as_slice());
                if let Some(old) = old {
                    assert_eq!(
                        old, deps,
                        "Looped back to same node, but with different children?"
                    );
                } else {
                    to_add.extend(deps.iter());
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
    start: &'b Dep<T>,
    ctx: &'c T::Context,
) -> (DepGraph<'c, T>, Vec<Dep<T>>) {
    let mut g = DepGraph::default();

    let (sub, passes) = prune_traversal(table, start);

    let rows: Vec<_> = sub.into_iter().collect();
    let mut nodemap = HashMap::new();
    for (i, (k, _)) in rows.iter().enumerate() {
        let id = g.add_node(GraphNode {
            dep: (*k).to_owned(),
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
