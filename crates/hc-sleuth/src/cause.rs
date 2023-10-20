use std::sync::Arc;

use crate::*;

#[derive(Clone, Debug, derive_more::Deref)]
pub struct ACause(Arc<dyn Cause>);

impl ACause {
    pub fn new(c: impl Cause + 'static) -> Self {
        Self(Arc::new(c))
    }
}

impl Cause for ACause {
    fn backtrack(&self, ctx: &Context) -> Report {
        self.0.backtrack(ctx)
    }
}

impl<T: Fact + 'static> From<T> for ACause {
    fn from(f: T) -> Self {
        ACause(Arc::new(f))
    }
}

pub trait Cause: std::fmt::Debug {
    fn backtrack(&self, ctx: &Context) -> Report;
}

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct Any(Vec<ACause>);

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct Every(Vec<ACause>);

impl Cause for Any {
    fn backtrack(&self, ctx: &Context) -> Report {
        let mut reports = vec![];
        for c in self.0.iter() {
            let report = c.backtrack(ctx);
            if report.is_empty() {
                return Report::from(vec![]);
            }
            reports.push(report.into())
        }
        Report::from(vec![ReportItem::Fork(reports)])
    }
}

impl Cause for Every {
    fn backtrack(&self, ctx: &Context) -> Report {
        todo!()
    }
}

macro_rules! causes {
    ( $($c:expr),+ ) => {
        vec![$($crate::ACause::new($c)),+]
    };
}

#[macro_export]
macro_rules! every {
    ( $($c:expr),+ ) => {
        $crate::ACause::new($crate::Every::new(causes![$(($c)),+]))
    };
}

#[macro_export]
macro_rules! any {
    ( $($c:expr),+ ) => {
        $crate::ACause::new($crate::Any::new(causes![$(($c)),+]))
    };
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;

    use crate::{item, report, test_fact::F, Cause, Context, Report};

    #[test]
    fn single_path() {
        let ctx = Context::default();
        let a = F::new(1, true, ());
        let b = F::new(2, true, a);
        let c = F::new(3, false, b);
        let d = F::new(4, false, c);
        let e = F::new(5, true, d);

        assert_eq!(a.backtrack(&ctx), report![]);
        assert_eq!(b.backtrack(&ctx), report![]);
        assert_eq!(c.backtrack(&ctx), report![c]);
        assert_eq!(d.backtrack(&ctx), report![d, e]);
        assert_eq!(e.backtrack(&ctx), report![]);
    }

    #[test]
    fn all_fail() {
        let ctx = Context::default();
        let a = F::new(1, false, ());
        let b = F::new(2, false, a);
        let c = F::new(3, false, b);
        assert_eq!(c.backtrack(&ctx), report![a, b, c]);
    }

    #[test]
    fn any() {
        let ctx = Context::default();
        let a0 = F::new(1, true, ());
        let a1 = F::new(2, true, a0);

        let b0 = F::new(3, true, ());
        let b1 = F::new(4, false, b0);

        let c0 = F::new(5, false, ());
        let c1 = F::new(6, false, c0);

        let d = F::new(7, false, any![a1, b1, c1]);
        let e = F::new(8, false, any![b1, c1]);

        // a1 passes, so d is the sole failure
        assert_eq!(d.backtrack(&ctx), report!(d));
        // a1 passes, so d is the sole failure
        assert_eq!(
            e.backtrack(&ctx),
            Report::from(vec![item!([b1], [c0, c1]), item!(e)])
        );
    }

    #[test]
    fn every() {
        let ctx = Context::default();
        let a0 = F::new(1, true, ());
        let a1 = F::new(2, true, a0);

        let b0 = F::new(3, true, ());
        let b1 = F::new(4, false, b0);

        let c0 = F::new(5, false, ());
        let c1 = F::new(6, false, c0);

        let d = F::new(7, false, every![a1, b0]);
        let e = F::new(8, false, every![a1, b1]);

        assert_eq!(d.backtrack(&ctx), report!(d));
        assert_eq!(
            e.backtrack(&ctx),
            Report::from(vec![item!([b1], [c0, c1]), item!(e)])
        );
    }
}
