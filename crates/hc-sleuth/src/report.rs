use aitia::simple_report;

use super::*;

pub fn report(step: Step, ctx: &Context) {
    simple_report(&aitia::Dep::from(step).traverse(ctx))
}
