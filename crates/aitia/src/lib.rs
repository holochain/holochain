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
        Traversal::Fail { tree, passes } => {
            tree.print();

            println!("Passing checks: {:#?}", passes);
        }
        Traversal::TraversalError { error, tree } => {
            tree.print();
            println!("Traversal error: {:?}", error)
        }
    }
}
