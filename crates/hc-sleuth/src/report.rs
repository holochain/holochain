use aitia::graph::{graph_easy, Traversal};

use super::*;

pub fn report(step: Step, ctx: &Context) {
    match aitia::Cause::from(step).traverse(ctx) {
        Traversal::Pass => println!("PASS"),
        Traversal::Groundless => println!("GROUNDLESS"),
        Traversal::Fail(graph) => {
            let dot = format!(
                "{:#?}",
                petgraph::dot::Dot::with_config(&*graph, &[petgraph::dot::Config::EdgeNoLabel],)
            );
            if let Ok(graph) = graph_easy(&dot) {
                println!("`graph-easy` output:\n{}", graph);
            } else {
                println!("`graph-easy` not installed. Original dot output: {}", dot);
            }
        }
    }
}
