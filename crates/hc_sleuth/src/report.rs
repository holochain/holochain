use aitia::{simple_report, Fact};

use super::*;

pub fn report(fact: crate::Fact, ctx: &Context) {
    if let Some(report) = simple_report(&fact.traverse(ctx)) {
        println!("hc_sleuth simple report:\n{report}");
    }
}
