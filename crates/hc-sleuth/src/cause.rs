use std::sync::Arc;

use crate::*;

#[derive(Clone, Debug, derive_more::Deref)]
pub struct ACause(Arc<dyn Cause>);

impl ACause {
    pub fn new(c: impl Cause + 'static) -> Self {
        Self(Arc::new(c))
    }
}

impl<T: Fact + 'static> From<T> for ACause {
    fn from(f: T) -> Self {
        ACause(Arc::new(f))
    }
}

pub trait Cause: std::fmt::Debug {
    fn backtrack(&self) -> Report;
}

pub type Report = Vec<String>;

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct Any(Vec<AFact>);

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct Every(Vec<AFact>);

impl Cause for Any {
    fn backtrack(&self) -> Report {
        todo!()
    }
}

impl Cause for Every {
    fn backtrack(&self) -> Report {
        todo!()
    }
}

macro_rules! facts {
    ( $($c:expr),+ ) => {
        vec![$(AFact::new($c)),+]
    };
}

macro_rules! every {
    ( $($c:expr),+ ) => {
        ACause::new(Every::new(facts![$(($c)),+]))
    };
}

macro_rules! any {
    ( $($c:expr),+ ) => {
        ACause::new(Any::new(facts![$(($c)),+]))
    };
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU8;

    use crate::{ACause, Cause, Fact};

    #[derive(Clone, PartialEq, Eq)]
    struct F<C>(u8, bool, C);

    static ID: AtomicU8 = AtomicU8::new(0);

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
        fn cause(&self) -> ACause {
            ACause::new(self.2.clone())
        }

        fn explain(&self) -> String {
            format!("F({})", self.id())
        }

        fn check(&self) -> bool {
            self.1
        }
    }

    #[test]
    fn complex() {
        let a = F::new(1, true, ());
        let b = F::new(2, true, a);
        let c = F::new(3, false, b);
        let d = F::new(4, false, c);

        dbg!(d.backtrack());
    }
}
