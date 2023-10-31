use aitia::{
    graph::{graph_easy, Traversal},
    Fact, FactTraits,
};

use super::*;

pub fn report(step: Step<OpAction>, ctx: &Context) {
    let step = ctx.expand(step);
    match aitia::Cause::from(step).traverse(ctx) {
        Traversal::Pass => println!("PASS"),
        Traversal::Groundless => println!("GROUNDLESS"),
        Traversal::Fail { tree, passes } => {
            let dot = format!(
                "{:#?}",
                petgraph::dot::Dot::with_config(&*tree, &[petgraph::dot::Config::EdgeNoLabel],)
            );
            if let Ok(tree) = graph_easy(&dot) {
                println!("`graph-easy` output:\n{}", tree);
            } else {
                println!("`graph-easy` not installed. Original dot output: {}", dot);
            }
            println!("Passing checks: {:#?}", passes);
        }
    }
}
