use std::sync::Arc;

use crate::*;

pub trait Fact: std::fmt::Debug {
    fn cause(&self, ctx: &Context) -> ACause;
    fn check(&self, ctx: &Context) -> bool;
    fn explain(&self) -> String;
}

#[derive(Debug, derive_more::Deref)]
pub struct AFact(Arc<dyn Fact>);

impl Clone for AFact {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl AFact {
    pub fn new(f: impl Fact + 'static) -> Self {
        Self(Arc::new(f))
    }
}

impl<F: Fact> Cause for F {
    fn backtrack(&self, ctx: &Context) -> Report {
        let pass = self.check(ctx);
        if pass {
            // Terminate backtracking as soon as a passing check is reached
            Report::from(vec![])
        } else {
            // Add this fact to the path
            let mut report = self.cause(ctx).backtrack(ctx);
            report.push(ReportItem::Line(self.explain()));
            report
        }
    }
}

impl Fact for () {
    fn cause(&self, ctx: &Context) -> ACause {
        ().into()
    }

    fn check(&self, _: &Context) -> bool {
        true
    }

    fn explain(&self) -> String {
        unreachable!()
    }
}
