use aitia::{
    graph::{graph_easy, Traversal},
    simple_report, Fact, FactTraits,
};

use super::*;

pub fn report(step: Step, ctx: &Context) {
    simple_report(&aitia::Cause::from(step).traverse(ctx))
}
