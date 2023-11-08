use aitia::{simple_report, Fact};

use super::*;

pub fn report(step: Step, ctx: &Context) {
    simple_report(&step.traverse(ctx))
}
