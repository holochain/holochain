pub mod cause;
pub mod graph;

#[macro_use]
#[cfg(feature = "tracing")]
pub mod logging;

pub use cause::{Cause, Fact, FactTraits};

#[cfg(test)]
mod tests;

use graph::Traversal;

pub fn simple_report<T: Fact>(traversal: &Traversal<T>) {
    match traversal {
        Traversal::Pass => println!("PASS"),
        Traversal::Groundless => println!("GROUNDLESS"),
        Traversal::Fail { tree, passes, ctx } => {
            tree.print();
            let passes: Vec<_> = passes.into_iter().map(|p| p.explain(ctx)).collect();
            println!("Passing checks: {:#?}", passes);
        }
        Traversal::TraversalError { error, tree } => {
            tree.print();
            println!("Traversal error: {:?}", error)
        }
    }
}
