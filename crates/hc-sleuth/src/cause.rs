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
        ACause(Arc::new(AFact::new(f)))
    }
}

pub trait Cause: std::fmt::Debug {
    fn backtrack(&self) -> (bool, Vec<AFact>);
}

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct Any(Vec<AFact>);

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct Every(Vec<AFact>);

impl Cause for () {
    fn backtrack(&self) -> (bool, Vec<AFact>) {
        unreachable!()
    }
}

impl Cause for Any {
    fn backtrack(&self) -> (bool, Vec<AFact>) {
        todo!()
    }
}

impl Cause for Every {
    fn backtrack(&self) -> (bool, Vec<AFact>) {
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

    use crate::{ACause, AFact, Cause, Fact};

    #[derive(Clone)]
    struct F(u8, bool, ACause);

    static ID: AtomicU8 = AtomicU8::new(0);

    impl std::fmt::Debug for F {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("F").field(&self.0).finish()
        }
    }

    impl F {
        pub fn new(check: bool, cause: impl Cause + 'static) -> AFact {
            let id = ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            AFact::new(Self(dbg!(id), dbg!(check), ACause::new(cause)))
        }

        pub fn id(&self) -> u8 {
            self.0
        }
    }

    impl Fact for F {
        fn cause(&self) -> ACause {
            self.2.clone()
        }

        fn check(&self) -> bool {
            self.1
        }
    }

    #[test]
    fn complex() {
        let a = F::new(true, ());
        let b = F::new(true, a);
        let c = F::new(false, b);
        let d = F::new(false, c);

        dbg!(d.backtrack());
    }
}
