use std::sync::Arc;

use crate::*;

pub trait Fact: std::fmt::Debug {
    fn cause(&self) -> ACause;
    fn check(&self) -> bool;
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
    fn backtrack(&self) -> Report {
        let pass = self.check();
        if pass {
            // Terminate backtracking as soon as a passing check is reached
            vec![]
        } else {
            // Add this fact to the path
            let mut report = self.cause().backtrack();
            report.push(ReportItem::Line(self.explain()));
            report
        }
    }
}

impl Fact for () {
    fn cause(&self) -> ACause {
        ().into()
    }

    fn check(&self) -> bool {
        true
    }

    fn explain(&self) -> String {
        unreachable!()
    }
}
