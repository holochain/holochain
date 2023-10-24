use aitia::graph::graph_easy;

use super::*;

pub fn report(step: Step, ctx: &Context) {
    // TODO: differentiate between passing and failing
    let graph = aitia::Cause::from(step).graph(ctx);
    let dot = format!(
        "{:?}",
        petgraph::dot::Dot::with_config(&graph, &[petgraph::dot::Config::EdgeNoLabel],)
    );
    if let Ok(graph) = graph_easy(&dot) {
        println!("`graph-easy` output:\n{}", graph);
    } else {
        println!("`graph-easy` not installed. Original dot output: {}", dot);
    }
}
