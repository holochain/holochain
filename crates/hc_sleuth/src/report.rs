use aitia::{simple_report, Fact};

use super::*;

pub fn report(event: Event, ctx: &Context) {
    if let Some(report) = simple_report(&event.traverse(ctx)) {
        println!("hc_sleuth simple report:\n{report}");
    }
}
