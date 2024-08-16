//! aitia, a crate for gaining insights from causal graphs
//!
//! ("aitia" is the root of "etiology", the study of causes, and also an easy-to-type palindrome)
//!
//! In complex systems, when something is going wrong, it can be difficult to
//! narrow down the dep to some subsystem without doing lots of forensic
//! poking around and exploratory logging. `aitia` aims to help with this process
//! of narrowing down the possible scope of a problem.
//!
//! You define a collection of [`Fact`]s about your system, each of which specifies
//! one or more [`Dep`]s (dependencies). The causal relationships between Depsimply a graph,
//! with each Fact connected to others. When testing your system, you can check whether a
//! particular Fact is true or not. If it's not true, `aitia` recursively follows the causal
//! relationships specified, building up a graph of deps, each of which is checked for
//! truth. The traversal stops only when either:
//! 1. a true fact is encountered, or
//! 2. a fact without any deps of its own is encountered (an "axiom" so to speak)
//! 3. a loop of unresolved deps is discovered, in which case that entire branch is discarded.
//!     Note that loops in causal relationships are allowed, as long as there is a Fact
//!     along the loop which passes, causing the loop to be broken
//!
//! The result is a directed acyclic graph (DAG) of all facts that are not true, with the original fact as the root.
//! The edges of the graph represent [logical conditionals or implications](https://en.wikipedia.org/wiki/Material_conditional):
//! i.e. an edge A -> B in the graph means
//! "If A is true, then B must be true", and also "if B is not true, then A cannot be true".
//! This means that the leaves of the DAG represent the possible root deps of the problem, i.e. the
//! "most upstream" known facts which could dep the root fact to not be true, and so the leaves
//! would represent the places in the system to look for the reason why your original fact was not true.
//!
//! `aitia` is as useful as the Facts you write. You can write very broad, vague facts, which can
//! help you hone in on broader parts of the system for further manual investigation, or you can
//! write very specific facts which can tell you at a glance what may be the problem. It lends
//! itself well to writing facts iteratively, broadly at first, and then adding more specificity
//! as you do the work of diagnosing the problems that it helped you find.
//!
//! `aitia` is meant to be an embodiment of the process of deducing the dep of a problem.
//! By encoding your search for a problem into an `aitia::Dep`, ideally you will never have to
//! hunt for that particular problem again.

// #![warn(missing_docs)]

mod dep;
mod fact;
mod graph;
pub(crate) mod traversal;

#[macro_use]
#[cfg(feature = "tracing")]
pub mod logging;

use std::collections::HashSet;

pub use dep::{Dep, DepError, DepResult};
pub use fact::{Fact, FactTraits};

#[cfg(test)]
mod tests;

use traversal::{Traversal, TraversalError, TraversalResult};

#[macro_export]
macro_rules! assert_fact {
    ($ctx: expr, $fact: expr) => {{
        use $crate::Fact;
        let tr = $fact.clone().traverse($ctx);
        if let Some(report) = $crate::simple_report(&tr) {
            panic!("{report}");
        }
    }};
}

pub enum TraversalOutcome<'c, F: Fact> {
    /// The fact was true and all dependencies were true
    Success,
    /// The fact was not true
    DependencyNotMet,
    /// The fact was true, but some dependencies were not, which indicates an incorrect model
    IncorrectModel(&'c HashSet<Dep<F>>),
}

impl<'c, F: Fact> TraversalOutcome<'c, F> {
    pub fn report(&self) -> Option<String> {
        match self {
            TraversalOutcome::Success => None,
            TraversalOutcome::DependencyNotMet => Some("aitia dependency not met given the context".to_string()) ,
            TraversalOutcome::IncorrectModel(deps) => Some(format!("Target fact was true, but some dependency checks failed. Your model may be incorrect. Failed checks: {deps:#?}")) ,
        }
    }

    pub fn from_traversal(tr: &'c Traversal<'c, F>) -> Self {
        let Traversal {
            root_check_passed,
            terminals,
            ..
        } = tr;
        if *root_check_passed {
            if terminals.is_empty() {
                // All is well
                TraversalOutcome::Success
            } else {
                TraversalOutcome::IncorrectModel(terminals)
            }
        } else {
            TraversalOutcome::DependencyNotMet
        }
    }
}

/// Helpful function for printing a report from a given Traversal
///
/// You're encouraged to write your own reports as best serve you, but this
/// is a good starting point.
#[must_use]
pub fn simple_report<T: Fact>(tr: &TraversalResult<T>) -> Option<String> {
    use std::fmt::Write;
    match tr {
        Ok(tr) => {
            let mut out = "".to_string();
            let outcome = TraversalOutcome::from_traversal(tr);
            if let Some(problem) = outcome.report() {
                writeln!(&mut out, "The targed fact FAILED: {problem}").unwrap();

                writeln!(&mut out, "{}", tr.graph.report().unwrap()).unwrap();
                let terminals: Vec<_> = tr.terminals.iter().map(|p| p.explain(tr.ctx)).collect();

                writeln!(&mut out, "Terminal nodes:").unwrap();
                for term in terminals {
                    writeln!(&mut out, "{term}").unwrap();
                }
                Some(out)
            } else {
                None
            }
        }
        Err(TraversalError { graph, inner }) => {
            let mut out = "".to_string();
            writeln!(&mut out, "{}", graph.report().unwrap()).unwrap();
            writeln!(&mut out, "Traversal error: {inner:?}").unwrap();
            Some(out)
        }
    }
}

pub fn print_simple_report<T: Fact>(tr: &TraversalResult<T>) {
    if let Some(report) = simple_report(tr) {
        println!("{report}");
    } else {
        println!("The target fact PASSED");
    }
}
