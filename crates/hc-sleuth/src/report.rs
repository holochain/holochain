use aitia::{simple_report, Fact};

use super::*;

pub fn report(event: Event, ctx: &Context) {
    simple_report(&event.traverse(ctx))
}
