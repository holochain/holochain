use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::graph::{DepGraph, GraphNode};
use crate::{dep::*, Fact};

#[derive(Debug, derive_more::From)]
pub struct TraversalError<'c, F: Fact> {
    pub inner: TraversalInnerError<F>,
    pub graph: DepGraph<'c, F>,
}

#[derive(Debug, derive_more::From)]
pub enum TraversalInnerError<F: Fact> {
    Dep(DepError<F>),
    // TODO: eventually allow errors in checks
    // Check(CheckError<F>),
}

#[derive(Debug, derive_more::From)]
pub struct Traversal<'c, T: Fact> {
    pub(crate) root_check_passed: bool,
    pub(crate) graph: DepGraph<'c, T>,
    pub(crate) terminals: HashSet<Dep<T>>,
    pub(crate) ctx: &'c T::Context,
}

impl<'c, T: Fact> Traversal<'c, T> {}

pub type TraversalResult<'c, F> = Result<Traversal<'c, F>, TraversalError<'c, F>>;

/// Different modes of traversing the graph
#[derive(Debug, Clone, Copy)]
pub enum TraversalMode {
    /// The default mode, which terminates traversal along a branch whenever a true fact is encountered.
    TraverseFails,
    /// Traverses the entire graph, expecting the entire traversal to consist of true facts.
    /// Useful for self-checking your model by running it against scenarios which are known to succeed.
    TraversePasses,
}

impl Default for TraversalMode {
    fn default() -> Self {
        Self::TraverseFails
    }
}

impl TraversalMode {
    /// When traversing in this mode, when a Check comes back with this value, terminate that branch.
    pub fn terminal_check_value(&self) -> bool {
        match self {
            TraversalMode::TraverseFails => true,
            TraversalMode::TraversePasses => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraversalStep<T: Fact> {
    /// This node terminates the traversal due to its check() status.
    Terminate,
    /// The traversal should continue with the following nodes.
    Continue(Vec<Dep<T>>),
}

impl<T: Fact> TraversalStep<T> {
    pub fn is_pass(&self) -> bool {
        matches!(self, TraversalStep::Terminate)
    }
}

pub type TraversalMap<T> = HashMap<Dep<T>, Option<TraversalStep<T>>>;

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
pub fn traverse<'c, F: Fact>(fact: F, ctx: &'c F::Context) -> TraversalResult<'c, F> {
    let mut table = TraversalMap::default();

    let root_check_passed = fact.check(ctx);
    let mode = if root_check_passed {
        TraversalMode::TraversePasses
    } else {
        TraversalMode::TraverseFails
    };

    let res = traverse_fact(&fact, ctx, &mut table, mode);
    let dep = Dep::from(fact);

    match res {
        Ok(check) => {
            table.insert(dep.clone(), Some(check.clone()));
            let (graph, terminals) = produce_graph(&table, &dep, ctx);

            Ok(Traversal {
                root_check_passed,
                graph,
                terminals,
                ctx,
            })
        }
        Err(inner) => {
            table.insert(
                dep.clone(),
                Some(TraversalStep::Continue(vec![dep.clone()])),
            );
            let (graph, _) = produce_graph(&table, &dep, ctx);

            Err(TraversalError { graph, inner })
        }
    }
}

fn traverse_inner<F: Fact>(
    dep: &Dep<F>,
    ctx: &F::Context,
    table: &mut TraversalMap<F>,
    mode: TraversalMode,
) -> Result<Option<TraversalStep<F>>, TraversalInnerError<F>> {
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

    #[allow(clippy::type_complexity)]
    let mut recursive_checks =
        |cs: &[Dep<F>]| -> Result<Vec<(Dep<F>, TraversalStep<F>)>, TraversalInnerError<F>> {
            let mut checks = vec![];
            for c in cs {
                if let Some(check) = traverse_inner(c, ctx, table, mode)? {
                    checks.push((c.clone(), check));
                }
            }
            Ok(checks)
        };

    let check = match dep {
        Dep::Fact(f) => {
            let terminate = f.check(ctx) == mode.terminal_check_value();
            if terminate {
                tracing::trace!("fact terminate");
                TraversalStep::Terminate
            } else {
                traverse_fact(f, ctx, table, mode)?
            }
        }
        Dep::Any(_, cs) => {
            let checks = recursive_checks(cs).map_err(|err| {
                // Continue constructing the graph while we bubble up errors
                tracing::error!("traversal ending due to error: {err:?}");
                table.insert(dep.clone(), Some(TraversalStep::Continue(cs.clone())));
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
                TraversalStep::Terminate
            } else {
                TraversalStep::Continue(fails)
            }
        }
        Dep::Every(_, cs) => {
            let checks = recursive_checks(cs).map_err(|err| {
                // Continue constructing the graph while we bubble up errors
                tracing::error!("traversal ending due to error: {err:?}");
                table.insert(dep.clone(), Some(TraversalStep::Continue(cs.clone())));
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
                TraversalStep::Terminate
            } else {
                TraversalStep::Continue(deps)
            }
        }
    };
    table.insert(dep.clone(), Some(check.clone()));
    tracing::trace!("exit. check: {:?}", check);
    Ok(Some(check))
}

fn traverse_fact<F: Fact>(
    fact: &F,
    ctx: &F::Context,
    table: &mut TraversalMap<F>,
    mode: TraversalMode,
) -> Result<TraversalStep<F>, TraversalInnerError<F>> {
    if let Some(sub_dep) = fact.dep(ctx)? {
        tracing::trace!("traversing fact");

        let check = traverse_inner(&sub_dep, ctx, table, mode).map_err(|err| {
            // Continue constructing the graph while we bubble up errors
            table.insert(
                Dep::from(fact.clone()),
                Some(TraversalStep::Continue(vec![sub_dep.clone()])),
            );
            tracing::error!("traversal ending due to error: {err:?}");
            err
        })?;
        tracing::trace!("traversal done, check: {:?}", check);
        Ok(TraversalStep::Continue(vec![sub_dep]))
    } else {
        tracing::trace!("fact fail with no dep, terminating");
        Ok(TraversalStep::Continue(vec![]))
    }
}

/// Prune away any extraneous nodes or edges from a Traversal.
/// After pruning, the graph contains either all true edges or all false edges,
/// with paths terminating at a transition point.
///
/// Terminal facts are returned separately.
#[allow(clippy::type_complexity)]
fn prune_traversal<'a, 'b: 'a, T: Fact + Eq + Hash>(
    table: &'a TraversalMap<T>,
    start: &'b Dep<T>,
) -> (HashMap<&'a Dep<T>, &'a [Dep<T>]>, Vec<&'a Dep<T>>) {
    let mut sub = HashMap::<&Dep<T>, &[Dep<T>]>::new();
    let mut terminals = vec![];
    let mut to_add = vec![start];

    while let Some(next) = to_add.pop() {
        if let Some(step) = table.get(next) {
            match step.as_ref() {
                Some(TraversalStep::Continue(deps)) => {
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
                Some(TraversalStep::Terminate) => {
                    terminals.push(next);
                }
                None => {}
            }
        } else {
            // Still include this as an orphan node.
            // This should only ever apply to the starting node.
            sub.insert(next, &[]);
        }
    }
    (sub, terminals)
}

pub fn produce_graph<'a, 'b: 'a, 'c, T: Fact + Eq + Hash>(
    table: &'a TraversalMap<T>,
    start: &'b Dep<T>,
    ctx: &'c T::Context,
) -> (DepGraph<'c, T>, HashSet<Dep<T>>) {
    let mut g = DepGraph::default();

    let (sub, passes) = prune_traversal(table, start);

    let rows: HashSet<_> = sub.into_iter().collect();
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
