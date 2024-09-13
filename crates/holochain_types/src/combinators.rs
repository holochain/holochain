//! Combinator functions, for more easeful functional programming

use futures::{Future, FutureExt};
use must_future::MustBoxFuture;

/// Return the first element of a 2-tuple
pub fn first<A, B>(tup: (A, B)) -> A {
    tup.0
}

/// Return the first element of a 2-tuple ref
pub fn first_ref<A, B>(tup: &(A, B)) -> &A {
    &tup.0
}

/// Return the second element of a 2-tuple
pub fn second<A, B>(tup: (A, B)) -> B {
    tup.1
}

/// Return the second element of a 2-tuple ref
pub fn second_ref<A, B>(tup: &(A, B)) -> &B {
    &tup.1
}

/// Swap the two items in 2-tuple
pub fn swap2<A, B>(tup: (A, B)) -> (B, A) {
    (tup.1, tup.0)
}

/// Map an iterator into a vec of a new type
pub fn mapvec<'a, T: 'a, U>(it: impl Iterator<Item = &'a T>, f: impl FnMut(&'a T) -> U) -> Vec<U> {
    it.map(f).collect()
}

/// Transpose an Option of a Future into a Future of an Option
pub trait OptionFuture<T>
where
    T: 'static + Send + Sync,
{
    /// Transpose an Option of a Future into a Future of an Option
    fn transpose(self) -> MustBoxFuture<'static, Option<T>>;
}

impl<T, F> OptionFuture<T> for Option<F>
where
    T: 'static + Send + Sync,
    F: 'static + Send + Sync + Future<Output = T>,
{
    fn transpose(self) -> MustBoxFuture<'static, Option<T>> {
        match self {
            Some(f) => f.map(Some).boxed().into(),
            None => futures::future::ready(None).boxed().into(),
        }
    }
}
