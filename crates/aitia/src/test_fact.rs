use crate::{ACause, Cause, Context, Fact, Report, ReportItem};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct F<C>(u8, bool, C);

impl<C> std::fmt::Debug for F<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("F").field(&self.0).finish()
    }
}

impl<C: Cause> F<C> {
    pub fn new(id: u8, check: bool, cause: C) -> Self {
        // let id = ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self(id, check, cause)
    }

    pub fn id(&self) -> u8 {
        self.0
    }
}

impl<C: Cause + Clone + 'static> Fact for F<C> {
    fn cause(&self, ctx: &Context) -> ACause {
        ACause::new(self.2.clone())
    }

    fn explain(&self) -> String {
        self.id().to_string()
    }

    fn check(&self, ctx: &Context) -> bool {
        self.1
    }
}
